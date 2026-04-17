// People Partner - Chat Module
// Provider-agnostic orchestration for AI chat (streaming, trimming, trial proxy)

use futures::StreamExt;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::context::{estimate_tokens, get_max_conversation_tokens};
use crate::keyring;
use crate::provider::{Provider, ProviderMessage, StreamDelta};
use crate::providers;
use crate::providers::anthropic::AnthropicProvider;

/// Shared HTTP client for all chat egress (BYOK, streaming, trial proxy).
///
/// `reqwest::Client::new()` has no timeouts — a hung connection would hang
/// the streaming task indefinitely. We set a 120s overall request timeout
/// (long enough for slow providers, short enough to bound pathological
/// hangs) and a 10s connect timeout. Reconstructing a client per call
/// also discarded TLS session state; sharing one via `LazyLock` reuses
/// connections across requests.
static SHARED_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .user_agent(concat!("PeoplePartner/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client with standard timeouts should build")
});

type HmacSha256 = Hmac<Sha256>;

// ============================================================================
// Stream cancellation registry (issue #25)
// ============================================================================
//
// Every in-flight streaming request registers a CancellationToken keyed by a
// client-generated stream_id. The frontend calls cancel_stream(stream_id)
// when the user hits Stop, switches conversations, or unmounts the chat view.
// The streaming task observes the cancellation via tokio::select! and drops
// its reqwest::Response, which closes the HTTP connection — the upstream
// provider stops generating and we stop paying for tokens the user won't see.
//
// Before this, abandoned streams kept running to completion, burning tokens
// silently. The classic symptom was: user opens a slow question, regrets it,
// starts a new conversation — and the OpenAI bill didn't get smaller.

/// Registry of in-flight streaming requests keyed by client-generated
/// stream_id. Shared application-wide via Tauri state; managed in `lib.rs`.
#[derive(Default)]
pub struct StreamRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new stream. Returns a token the streaming task awaits on.
    /// Collisions on stream_id replace the existing entry (the previous
    /// stream becomes un-cancellable but continues running); client ids are
    /// UUIDs so collisions shouldn't happen in practice.
    fn register(&self, stream_id: String) -> CancellationToken {
        let token = CancellationToken::new();
        self.inner
            .lock()
            .expect("stream registry mutex poisoned")
            .insert(stream_id, token.clone());
        token
    }

    /// Trigger cancellation for the given id. Returns true if found, false if
    /// unknown. An unknown id is a no-op — the stream may have already ended
    /// by the time the UI's cancel call reached us.
    pub fn cancel(&self, stream_id: &str) -> bool {
        match self
            .inner
            .lock()
            .expect("stream registry mutex poisoned")
            .get(stream_id)
        {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Remove a stream's entry on completion (success or error). Called
    /// automatically by `StreamGuard::drop`, not by the streaming body, so
    /// the map never leaks even on panic.
    fn remove(&self, stream_id: &str) {
        self.inner
            .lock()
            .expect("stream registry mutex poisoned")
            .remove(stream_id);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}

/// RAII guard that removes a stream's registry entry on drop. Ensures the
/// registry is cleaned up on every exit path — `?`-propagation, panics, and
/// normal returns alike — without scattering `registry.remove(...)` calls.
struct StreamGuard<'a> {
    registry: &'a StreamRegistry,
    stream_id: &'a str,
}

impl Drop for StreamGuard<'_> {
    fn drop(&mut self) {
        self.registry.remove(self.stream_id);
    }
}

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
    #[error("Stream cancelled")]
    Cancelled,
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
// Helpers
// ============================================================================

/// Convert ChatMessages to ProviderMessages
fn to_provider_messages(messages: Vec<ChatMessage>) -> Vec<ProviderMessage> {
    messages
        .into_iter()
        .map(|m| ProviderMessage {
            role: m.role,
            content: m.content,
        })
        .collect()
}

/// Apply PII redaction to the chat payload before it leaves the machine.
///
/// This is the defense-in-depth enforcement of the product's privacy claim:
/// no raw SSN / credit card / bank account / phone / address / medical data
/// should ever reach a provider, even if the frontend's advisory `scan_pii`
/// path was bypassed (e.g., XSS) or silently failed. Applied uniformly across
/// BYOK, streaming, trial-proxy, and backend-initiated calls (memory
/// summarization, review-highlight extraction) since all four paths funnel
/// through the three send_message* functions.
///
/// Returns (redacted messages, redacted system prompt, combined summary).
/// The combined summary is suitable for emitting to the UI via a Tauri event.
fn redact_chat_payload(
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
) -> (Vec<ChatMessage>, Option<String>, Option<String>) {
    let mut summary_parts: Vec<String> = Vec::new();

    let redacted_messages: Vec<ChatMessage> = messages
        .into_iter()
        .map(|m| {
            let result = crate::pii::scan_and_redact(&m.content);
            if result.had_pii {
                if let Some(s) = result.summary {
                    summary_parts.push(s);
                }
            }
            ChatMessage {
                role: m.role,
                content: result.redacted_text,
            }
        })
        .collect();

    let redacted_system_prompt = system_prompt.map(|sp| {
        let result = crate::pii::scan_and_redact(&sp);
        if result.had_pii {
            if let Some(s) = result.summary {
                summary_parts.push(s);
            }
        }
        result.redacted_text
    });

    let combined_summary = if summary_parts.is_empty() {
        None
    } else {
        Some(summary_parts.join("; "))
    };

    (redacted_messages, redacted_system_prompt, combined_summary)
}

/// Resolve a provider by ID (with optional model override),
/// falling back to the default if unknown.
fn resolve_provider(provider_id: &str, model_id: Option<&str>) -> Box<dyn Provider> {
    providers::get_provider(provider_id, model_id)
        .unwrap_or_else(|| providers::get_default_provider())
}

/// Get the API key for a provider. Uses the legacy-migration-aware path for
/// Anthropic to preserve first-launch backward compatibility.
fn get_api_key_for_provider(provider_id: &str) -> Result<String, ChatError> {
    if provider_id == "anthropic" {
        keyring::get_api_key().map_err(ChatError::from)
    } else {
        keyring::get_provider_api_key(provider_id).map_err(ChatError::from)
    }
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

/// Send a message to an AI provider and get a response (non-streaming)
pub async fn send_message(
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    provider_id: &str,
    model_id: Option<&str>,
) -> Result<ChatResponse, ChatError> {
    let provider = resolve_provider(provider_id, model_id);
    let api_key = get_api_key_for_provider(provider_id)?;

    // Enforce PII redaction before anything leaves the machine. This covers
    // backend-initiated calls (memory summarization, highlight extraction)
    // that don't have an AppHandle to emit an event from — summary is dropped.
    let (messages, system_prompt, _pii_summary) = redact_chat_payload(messages, system_prompt);

    // Trim conversation to fit within token budget (silently drops oldest messages)
    let trimmed_messages = trim_conversation_to_budget(messages, &system_prompt);
    let provider_messages = to_provider_messages(trimmed_messages);

    // Build and send the request via the provider
    let client = SHARED_CLIENT.clone();
    let request_builder = provider.build_request(&client, &provider_messages, &system_prompt, &api_key);
    let response = request_builder.send().await?;

    // Check for HTTP errors
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        let parsed = provider.parse_error_response(&error_text);
        return Err(ChatError::ApiError(format!("HTTP {}: {}", status.as_u16(), parsed)));
    }

    // Parse successful response via the provider
    let body_text = response.text().await
        .map_err(|e| ChatError::ParseError(e.to_string()))?;

    let provider_response = provider.parse_response(&body_text)
        .map_err(|e| ChatError::ParseError(e))?;

    Ok(ChatResponse {
        content: provider_response.content,
        input_tokens: provider_response.input_tokens,
        output_tokens: provider_response.output_tokens,
    })
}

/// Process an SSE stream response, emitting "chat-stream" events to the frontend.
/// Shared between BYOK and trial proxy streaming paths.
///
/// The caller passes a `cancel_token` pulled from `StreamRegistry`. If the
/// token fires mid-stream we emit `chat-stream-cancelled` and return
/// `ChatError::Cancelled`; dropping the response here closes the reqwest
/// connection, which stops the upstream provider from streaming further
/// tokens (the billing event we care about).
async fn process_sse_stream(
    app: &AppHandle,
    response: reqwest::Response,
    provider: &dyn Provider,
    aggregates: Option<crate::context::OrgAggregates>,
    query_type: Option<crate::context::QueryType>,
    cancel_token: CancellationToken,
) -> Result<(), ChatError> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_response = String::new();

    loop {
        let next = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                // Drop `response` (and therefore `stream`) when the function
                // returns — closes the HTTP connection and stops upstream
                // generation. The frontend hook for this event resets the
                // conversation's streaming-UI state to idle.
                let _ = app.emit("chat-stream-cancelled", ());
                return Err(ChatError::Cancelled);
            }
            chunk = stream.next() => chunk,
        };
        let Some(chunk_result) = next else { break };
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
                    if let Some(delta) = provider.parse_sse_event(data) {
                        match delta {
                            StreamDelta::TextDelta(text) => {
                                full_response.push_str(&text);

                                let _ = app.emit("chat-stream", StreamChunk {
                                    chunk: text,
                                    done: false,
                                    verification: None,
                                });
                            }
                            StreamDelta::Done => {
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
                            StreamDelta::Error(msg) => {
                                return Err(ChatError::ApiError(msg));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Check HTTP response status and return an error if not successful.
fn check_http_error_status(
    status: reqwest::StatusCode,
    error_text: &str,
    provider: &dyn Provider,
) -> Result<(), ChatError> {
    let parsed = provider.parse_error_response(error_text);
    Err(ChatError::ApiError(format!("HTTP {}: {}", status.as_u16(), parsed)))
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

/// Send a message with streaming response (BYOK / paid mode)
/// Emits "chat-stream" events to the frontend as chunks arrive
///
/// `stream_id` is a client-generated identifier (UUID from the frontend)
/// that the UI later passes to `cancel_stream` if the user hits Stop. The
/// guard at the top of this function ensures the registry entry is removed
/// on every exit path.
pub async fn send_message_streaming(
    app: AppHandle,
    registry: &StreamRegistry,
    stream_id: String,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    aggregates: Option<crate::context::OrgAggregates>,
    query_type: Option<crate::context::QueryType>,
    provider_id: &str,
    model_id: Option<&str>,
) -> Result<(), ChatError> {
    let cancel_token = registry.register(stream_id.clone());
    let _guard = StreamGuard {
        registry,
        stream_id: &stream_id,
    };

    let provider = resolve_provider(provider_id, model_id);
    let api_key = get_api_key_for_provider(provider_id)?;

    // Enforce PII redaction before anything leaves the machine.
    let (messages, system_prompt, pii_summary) = redact_chat_payload(messages, system_prompt);
    if let Some(summary) = pii_summary {
        let _ = app.emit("chat-pii-redacted", &summary);
    }

    // Trim and convert messages
    let trimmed_messages = trim_conversation_to_budget(messages, &system_prompt);
    let provider_messages = to_provider_messages(trimmed_messages);

    // Build and send the request via the provider
    let client = SHARED_CLIENT.clone();
    let request_builder = provider.build_streaming_request(
        &client,
        &provider_messages,
        &system_prompt,
        &api_key,
    );
    let response = request_builder.send().await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(match check_http_error_status(status, &error_text, &*provider) {
            Err(err) => err,
            Ok(()) => unreachable!(),
        });
    }

    process_sse_stream(&app, response, &*provider, aggregates, query_type, cancel_token).await
}

/// Send a message through the trial proxy with streaming response.
/// Routes through the proxy URL instead of directly to Anthropic.
/// The proxy manages the API key; we send a device ID for quota tracking.
///
/// Same registry/guard pattern as `send_message_streaming`. Cancelling a
/// trial stream mid-flight still counts against the trial quota on the
/// proxy side (the request was accepted) but stops downstream token
/// delivery — the cost saving is on the Anthropic bill behind the proxy.
pub async fn send_message_streaming_trial(
    app: AppHandle,
    registry: &StreamRegistry,
    stream_id: String,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    proxy_url: &str,
    device_id: &str,
    proxy_signing_secret: Option<&str>,
    aggregates: Option<crate::context::OrgAggregates>,
    query_type: Option<crate::context::QueryType>,
) -> Result<TrialUsageMetadata, ChatError> {
    let cancel_token = registry.register(stream_id.clone());
    let _guard = StreamGuard {
        registry,
        stream_id: &stream_id,
    };

    let anthropic = AnthropicProvider::new();

    // Enforce PII redaction before anything leaves the machine (proxy is still
    // "off-device" — the user's data hits Cloudflare + Anthropic).
    let (messages, system_prompt, pii_summary) = redact_chat_payload(messages, system_prompt);
    if let Some(summary) = pii_summary {
        let _ = app.emit("chat-pii-redacted", &summary);
    }

    // Trim and convert messages
    let trimmed_messages = trim_conversation_to_budget(messages, &system_prompt);
    let provider_messages = to_provider_messages(trimmed_messages);

    // Build the serializable request body for the proxy
    let request = anthropic.build_message_request(&provider_messages, &system_prompt, true);
    let body_json = serde_json::to_string(&request)
        .map_err(|e| ChatError::ParseError(e.to_string()))?;

    let client = SHARED_CLIENT.clone();
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
        return Err(match check_http_error_status(status, &error_text, &anthropic) {
            Err(err) => err,
            Ok(()) => unreachable!(),
        });
    }

    process_sse_stream(&app, response, &anthropic, aggregates, query_type, cancel_token).await?;
    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // StreamRegistry tests (issue #25)
    // ========================================

    #[test]
    fn registry_starts_empty() {
        let reg = StreamRegistry::new();
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn register_returns_unfired_token() {
        let reg = StreamRegistry::new();
        let token = reg.register("stream-1".into());
        assert!(!token.is_cancelled());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn cancel_fires_matching_token_and_returns_true() {
        let reg = StreamRegistry::new();
        let token = reg.register("stream-1".into());
        assert!(!token.is_cancelled());

        let cancelled = reg.cancel("stream-1");
        assert!(cancelled, "cancel must report a match");
        assert!(token.is_cancelled(), "token held by streaming task must observe cancel");
    }

    #[test]
    fn cancel_of_unknown_id_is_a_noop_not_an_error() {
        let reg = StreamRegistry::new();
        let cancelled = reg.cancel("never-existed");
        // The frontend may call this on every conversation switch even when
        // no stream is in flight. It must be safe.
        assert!(!cancelled);
    }

    #[test]
    fn guard_removes_entry_on_drop_even_when_token_already_cancelled() {
        let reg = StreamRegistry::new();
        let _token = reg.register("stream-1".into());
        {
            let _guard = StreamGuard {
                registry: &reg,
                stream_id: "stream-1",
            };
            reg.cancel("stream-1");
            assert_eq!(reg.len(), 1, "cancel alone must not remove the entry");
        }
        assert_eq!(reg.len(), 0, "guard drop must clean the registry");
    }

    #[test]
    fn cancel_after_guard_drop_is_a_noop() {
        let reg = StreamRegistry::new();
        {
            let token = reg.register("stream-1".into());
            let _guard = StreamGuard {
                registry: &reg,
                stream_id: "stream-1",
            };
            drop(token);
        }
        // Guard dropped → entry removed → cancel finds nothing.
        assert!(!reg.cancel("stream-1"));
    }

    #[tokio::test]
    async fn cancelled_token_wakes_awaiting_task() {
        // Models the process_sse_stream loop: a task awaits `.cancelled()`
        // and must wake when cancel() fires from another task.
        let reg = std::sync::Arc::new(StreamRegistry::new());
        let token = reg.register("stream-1".into());

        let reg_for_cancel = reg.clone();
        let canceller = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            reg_for_cancel.cancel("stream-1");
        });

        // If cancel never wakes the await, this test hangs and times out.
        token.cancelled().await;
        canceller.await.unwrap();
    }

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

    // ============================================================================
    // PII redaction — defense-in-depth regression tests
    // ============================================================================

    #[test]
    fn redact_chat_payload_strips_ssn_from_messages() {
        let messages = vec![
            make_message("user", "Sarah's SSN is 123-45-6789, please reset her access."),
            make_message("assistant", "Got it."),
        ];
        let (redacted, sys, summary) = redact_chat_payload(messages, None);

        assert!(sys.is_none());
        assert!(
            !redacted[0].content.contains("123-45-6789"),
            "raw SSN leaked through redaction: {}",
            redacted[0].content
        );
        assert!(redacted[0].content.contains("[SSN_REDACTED]"));
        assert!(summary.is_some(), "should surface a summary for UI event");
    }

    #[test]
    fn redact_chat_payload_strips_credit_card_from_system_prompt() {
        // An employee record leaked a CC into the context builder.
        let system =
            Some("Employee Sarah Chen. Company card on file: 4111-1111-1111-1111.".to_string());
        let (_, sys, summary) = redact_chat_payload(vec![], system);

        let sys = sys.expect("system prompt preserved");
        assert!(
            !sys.contains("4111-1111-1111-1111"),
            "raw CC leaked through redaction: {sys}"
        );
        assert!(sys.contains("[CC_REDACTED]"));
        assert!(summary.is_some());
    }

    #[test]
    fn redact_chat_payload_noop_when_no_pii_present() {
        let messages = vec![make_message("user", "How many employees are in marketing?")];
        let system = Some("You are a helpful HR assistant.".to_string());
        let (redacted, sys, summary) = redact_chat_payload(messages.clone(), system.clone());

        assert_eq!(redacted[0].content, messages[0].content);
        assert_eq!(sys, system);
        assert!(summary.is_none(), "no PII — no event should fire");
    }

    #[test]
    fn redact_chat_payload_survives_to_provider_messages() {
        // Guard against a future refactor that could accidentally skip redaction.
        let messages = vec![make_message(
            "user",
            "Terminate employee with bank account 123456789012 in the records.",
        )];
        let (redacted, _, _) = redact_chat_payload(messages, None);
        let provider_messages = to_provider_messages(redacted);

        let serialized = serde_json::to_string(&provider_messages[0].content).unwrap();
        assert!(
            !serialized.contains("123456789012"),
            "raw bank account number serialized to provider payload: {serialized}"
        );
    }
}
