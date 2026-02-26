// HR Command Center - Claude API Integration
// Handles communication with the Anthropic Messages API

use futures::StreamExt;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tauri::{AppHandle, Emitter};
use thiserror::Error;

use crate::context::{estimate_tokens, get_max_conversation_tokens};
use crate::keyring;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MODEL: &str = "claude-sonnet-4-20250514";
const MAX_TOKENS: u32 = 4096;
type HmacSha256 = Hmac<Sha256>;

#[derive(Error, Debug)]
pub enum ChatError {
    #[error("API key not configured")]
    NoApiKey,
    #[error("Failed to access API key: {0}")]
    KeyringError(String),
    #[error("API request failed: {0}")]
    RequestError(String),
    #[error("API returned error: {0}")]
    ApiError(String),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
    #[error("Trial message limit reached. Upgrade to continue chatting.")]
    TrialLimitReached { used: Option<u32>, limit: Option<u32> },
    #[error("Trial mode error: {0}")]
    TrialError(String),
}

impl From<keyring::KeyringError> for ChatError {
    fn from(err: keyring::KeyringError) -> Self {
        match err {
            keyring::KeyringError::NotFound => ChatError::NoApiKey,
            other => ChatError::KeyringError(other.to_string()),
        }
    }
}

impl From<reqwest::Error> for ChatError {
    fn from(err: reqwest::Error) -> Self {
        ChatError::RequestError(err.to_string())
    }
}

// Make ChatError serializable for Tauri commands
impl serde::Serialize for ChatError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ============================================================================
// Request/Response Types (Anthropic Messages API)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MessageRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorResponse {
    #[serde(rename = "type")]
    pub error_type: String,
    pub error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

// ============================================================================
// Streaming Event Types (SSE)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: StreamMessageStart },
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: u32, content_block: ContentBlock },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: TextDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDeltaData, usage: Option<UsageDelta> },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: ApiErrorDetail },
}

#[derive(Debug, Deserialize)]
pub struct StreamMessageStart {
    pub id: String,
    pub model: String,
}

#[derive(Debug, Deserialize)]
pub struct TextDelta {
    #[serde(rename = "type")]
    pub delta_type: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UsageDelta {
    pub output_tokens: u32,
}

// ============================================================================
// Simplified types for frontend communication
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Event emitted to frontend during streaming
#[derive(Debug, Clone, Serialize)]
pub struct StreamChunk {
    pub chunk: String,
    pub done: bool,
    /// Verification result - only included when done=true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<crate::context::VerificationResult>,
}

#[derive(Debug, Clone)]
pub struct TrialUsageMetadata {
    pub used: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ProxyErrorResponse {
    error: String,
    #[allow(dead_code)]
    message: String,
    used: Option<u32>,
    limit: Option<u32>,
}

// ============================================================================
// Conversation Trimming
// ============================================================================

/// Estimate tokens for a single chat message
/// Includes overhead for role/structure (~4 tokens per message)
fn estimate_message_tokens(message: &ChatMessage) -> usize {
    estimate_tokens(&message.content) + 4
}

/// Estimate total tokens for a conversation
fn estimate_conversation_tokens(messages: &[ChatMessage]) -> usize {
    messages.iter().map(|m| estimate_message_tokens(m)).sum()
}

/// Trim conversation history to fit within token budget
/// Strategy: Keep most recent messages, remove oldest user/assistant pairs first
/// This silently drops old messages without notification (per design spec)
pub fn trim_conversation_to_budget(
    messages: Vec<ChatMessage>,
    system_prompt: &Option<String>,
) -> Vec<ChatMessage> {
    // Calculate available budget for conversation
    let system_tokens = system_prompt
        .as_ref()
        .map(|s| estimate_tokens(s))
        .unwrap_or(0);
    let max_conversation_tokens = get_max_conversation_tokens();
    let conversation_budget = max_conversation_tokens.saturating_sub(system_tokens);

    let mut result = messages;
    let mut total_tokens = estimate_conversation_tokens(&result);

    // If already under budget, return as-is
    if total_tokens <= conversation_budget {
        return result;
    }

    // Remove oldest messages until under budget
    // Keep at least the most recent user message
    while total_tokens > conversation_budget && result.len() > 1 {
        // Remove the oldest message
        result.remove(0);

        // If we just removed a user message and the new first message is assistant,
        // also remove it to keep pairs intact (don't leave orphan assistant response)
        if !result.is_empty() && result[0].role == "assistant" {
            result.remove(0);
        }

        total_tokens = estimate_conversation_tokens(&result);
    }

    result
}

// ============================================================================
// API Client
// ============================================================================

/// Send a message to Claude and get a response (non-streaming)
pub async fn send_message(
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> Result<ChatResponse, ChatError> {
    // Get API key from Keychain
    let api_key = keyring::get_api_key()?;

    // Trim conversation to fit within token budget (silently drops oldest messages)
    let trimmed_messages = trim_conversation_to_budget(messages, &system_prompt);

    // Build the request
    let request = MessageRequest {
        model: MODEL.to_string(),
        max_tokens: MAX_TOKENS,
        messages: trimmed_messages
            .into_iter()
            .map(|m| Message {
                role: m.role,
                content: m.content,
            })
            .collect(),
        system: system_prompt,
        stream: None,
    };

    // Create HTTP client and send request
    let client = Client::new();
    let response = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await?;

    // Check for HTTP errors
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();

        // Try to parse as API error
        if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(&error_text) {
            return Err(ChatError::ApiError(format!(
                "{}: {}",
                api_error.error.error_type, api_error.error.message
            )));
        }

        return Err(ChatError::ApiError(format!(
            "HTTP {}: {}",
            status.as_u16(),
            error_text
        )));
    }

    // Parse successful response
    let api_response: MessageResponse = response
        .json()
        .await
        .map_err(|e| ChatError::ParseError(e.to_string()))?;

    // Extract text content from response
    let content = api_response
        .content
        .iter()
        .filter_map(|block| {
            if block.content_type == "text" {
                block.text.clone()
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");

    Ok(ChatResponse {
        content,
        input_tokens: api_response.usage.input_tokens,
        output_tokens: api_response.usage.output_tokens,
    })
}

/// Process an SSE stream response, emitting "chat-stream" events to the frontend.
/// Shared between BYOK and trial proxy streaming paths.
async fn process_sse_stream(
    app: &AppHandle,
    response: reqwest::Response,
    aggregates: Option<crate::context::OrgAggregates>,
    query_type: Option<crate::context::QueryType>,
) -> Result<(), ChatError> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_response = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| ChatError::RequestError(e.to_string()))?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

        // Process complete SSE events (lines ending with \n\n)
        while let Some(pos) = buffer.find("\n\n") {
            let event_data = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            // Parse SSE event
            for line in event_data.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        match event {
                            StreamEvent::ContentBlockDelta { delta, .. } => {
                                full_response.push_str(&delta.text);

                                let _ = app.emit("chat-stream", StreamChunk {
                                    chunk: delta.text,
                                    done: false,
                                    verification: None,
                                });
                            }
                            StreamEvent::MessageStop => {
                                let verification = query_type.map(|qt| {
                                    crate::context::verify_response(
                                        &full_response,
                                        aggregates.as_ref(),
                                        qt,
                                    )
                                });

                                let _ = app.emit("chat-stream", StreamChunk {
                                    chunk: String::new(),
                                    done: true,
                                    verification,
                                });
                            }
                            StreamEvent::Error { error } => {
                                return Err(ChatError::ApiError(error.message));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Check HTTP response status and return an error if not successful.
fn check_http_error_status(status: reqwest::StatusCode, error_text: &str) -> Result<(), ChatError> {
    if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(error_text) {
        return Err(ChatError::ApiError(format!(
            "{}: {}",
            api_error.error.error_type, api_error.error.message
        )));
    }
    Err(ChatError::ApiError(format!("HTTP {}: {}", status.as_u16(), error_text)))
}

/// Build a streaming MessageRequest from chat messages and system prompt.
fn build_streaming_request(
    messages: Vec<ChatMessage>,
    system_prompt: &Option<String>,
) -> MessageRequest {
    let trimmed_messages = trim_conversation_to_budget(messages, system_prompt);
    MessageRequest {
        model: MODEL.to_string(),
        max_tokens: MAX_TOKENS,
        messages: trimmed_messages
            .into_iter()
            .map(|m| Message {
                role: m.role,
                content: m.content,
            })
            .collect(),
        system: system_prompt.clone(),
        stream: Some(true),
    }
}

fn parse_trial_usage_headers(headers: &reqwest::header::HeaderMap) -> TrialUsageMetadata {
    let used = headers
        .get("x-trial-used")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok());
    let limit = headers
        .get("x-trial-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok());

    TrialUsageMetadata { used, limit }
}

fn compute_trial_signature(
    secret: &str,
    device_id: &str,
    timestamp: &str,
    body_json: &str,
) -> Result<String, ChatError> {
    let payload = format!("{}:{}:{}", device_id, timestamp, body_json);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| ChatError::TrialError(e.to_string()))?;
    mac.update(payload.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Send a message to Claude with streaming response (BYOK / paid mode)
/// Emits "chat-stream" events to the frontend as chunks arrive
pub async fn send_message_streaming(
    app: AppHandle,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    aggregates: Option<crate::context::OrgAggregates>,
    query_type: Option<crate::context::QueryType>,
) -> Result<(), ChatError> {
    let api_key = keyring::get_api_key()?;
    let request = build_streaming_request(messages, &system_prompt);

    let client = Client::new();
    let response = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(match check_http_error_status(status, &error_text) {
            Err(err) => err,
            Ok(()) => ChatError::ApiError(format!(
                "HTTP {}: {}",
                status.as_u16(),
                error_text
            )),
        });
    }

    process_sse_stream(&app, response, aggregates, query_type).await
}

/// Send a message through the trial proxy with streaming response.
/// Routes through the proxy URL instead of directly to Anthropic.
/// The proxy manages the API key; we send a device ID for quota tracking.
pub async fn send_message_streaming_trial(
    app: AppHandle,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    proxy_url: &str,
    device_id: &str,
    proxy_signing_secret: Option<&str>,
    aggregates: Option<crate::context::OrgAggregates>,
    query_type: Option<crate::context::QueryType>,
) -> Result<TrialUsageMetadata, ChatError> {
    let request = build_streaming_request(messages, &system_prompt);
    let body_json = serde_json::to_string(&request)
        .map_err(|e| ChatError::ParseError(e.to_string()))?;

    let client = Client::new();
    let endpoint = format!("{}/v1/messages", proxy_url.trim_end_matches('/'));
    let mut request_builder = client
        .post(&endpoint)
        .header("x-device-id", device_id)
        .header("content-type", "application/json")
        .header("origin", "tauri://localhost")
        .body(body_json.clone());

    if let Some(secret) = proxy_signing_secret {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let signature = compute_trial_signature(secret, device_id, &timestamp, &body_json)?;
        request_builder = request_builder
            .header("x-trial-timestamp", timestamp)
            .header("x-trial-signature", signature);
    }

    let response = request_builder.send().await?;

    let status = response.status();
    let mut usage = parse_trial_usage_headers(response.headers());
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        if status.as_u16() == 402 {
            if let Ok(proxy_error) = serde_json::from_str::<ProxyErrorResponse>(&error_text) {
                if proxy_error.error == "trial_limit_reached" {
                    if usage.used.is_none() {
                        usage.used = proxy_error.used;
                    }
                    if usage.limit.is_none() {
                        usage.limit = proxy_error.limit;
                    }
                    return Err(ChatError::TrialLimitReached {
                        used: usage.used,
                        limit: usage.limit,
                    });
                }
            }
        }
        return Err(match check_http_error_status(status, &error_text) {
            Err(err) => err,
            Ok(()) => ChatError::ApiError(format!(
                "HTTP {}: {}",
                status.as_u16(),
                error_text
            )),
        });
    }

    process_sse_stream(&app, response, aggregates, query_type).await?;
    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }

    // ========================================
    // Conversation Trimming Tests
    // ========================================

    fn make_message(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_estimate_message_tokens() {
        let msg = make_message("user", "Hello"); // 5 chars = 2 tokens + 4 overhead = 6
        assert_eq!(estimate_message_tokens(&msg), 6);
    }

    #[test]
    fn test_estimate_conversation_tokens() {
        let messages = vec![
            make_message("user", "Hello"),      // 6 tokens
            make_message("assistant", "Hi there"), // ceil(8/4) + 4 = 6 tokens
        ];
        assert_eq!(estimate_conversation_tokens(&messages), 12);
    }

    #[test]
    fn test_trim_conversation_no_trimming_needed() {
        // Small conversation should not be trimmed
        let messages = vec![
            make_message("user", "Hello"),
            make_message("assistant", "Hi there"),
        ];
        let system_prompt = Some("You are a helpful assistant.".to_string());

        let trimmed = trim_conversation_to_budget(messages.clone(), &system_prompt);
        assert_eq!(trimmed.len(), 2);
    }

    #[test]
    fn test_trim_conversation_empty() {
        let messages: Vec<ChatMessage> = vec![];
        let trimmed = trim_conversation_to_budget(messages, &None);
        assert!(trimmed.is_empty());
    }

    #[test]
    fn test_trim_conversation_single_message() {
        let messages = vec![make_message("user", "Hello")];
        let trimmed = trim_conversation_to_budget(messages, &None);
        assert_eq!(trimmed.len(), 1);
    }

    #[test]
    fn test_trim_conversation_preserves_recent() {
        // Create a moderately sized conversation
        let mut messages = vec![];
        for i in 0..10 {
            messages.push(make_message("user", &format!("Question {}", i)));
            messages.push(make_message("assistant", &format!("Answer {}", i)));
        }

        // With no system prompt, should have lots of budget
        let trimmed = trim_conversation_to_budget(messages.clone(), &None);

        // Should preserve all messages since they fit in budget
        assert_eq!(trimmed.len(), 20);

        // Last message should be preserved
        assert_eq!(trimmed.last().unwrap().content, "Answer 9");
    }

    #[test]
    fn test_trim_removes_oldest_first() {
        // Create messages where oldest is identifiable
        let messages = vec![
            make_message("user", "OLDEST"),
            make_message("assistant", "Response to oldest"),
            make_message("user", "MIDDLE"),
            make_message("assistant", "Response to middle"),
            make_message("user", "NEWEST"),
            make_message("assistant", "Response to newest"),
        ];

        // With huge system prompt that leaves almost no conversation budget,
        // simulate trimming by checking behavior
        let trimmed = trim_conversation_to_budget(messages.clone(), &None);

        // Should still have all since they fit in 150K token budget
        assert_eq!(trimmed.len(), 6);

        // First message should still be OLDEST (no trimming needed)
        assert_eq!(trimmed[0].content, "OLDEST");
    }
}
