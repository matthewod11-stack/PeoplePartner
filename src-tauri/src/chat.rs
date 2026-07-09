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

/// Scrub plausible API key substrings from upstream error text before it
/// reaches the UI / logs / support transcripts.
///
/// Provider error bodies occasionally echo a portion of the offending key
/// (e.g., a 401 message that quotes the key). Even a partial leak — to a
/// browser console, an exported audit log, or a screenshot in a support
/// email — is a key-rotation event. The helper covers the three live
/// providers (Anthropic, OpenAI, Google) plus the generic `sk-` prefix
/// shared by Anthropic + OpenAI legacy keys, and intentionally over-matches
/// (long-enough alphanumeric runs after a known prefix) to be robust to
/// minor prefix variations.
fn redact_api_keys(text: &str) -> String {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // Anthropic: sk-ant-... (covers sk-ant-api03-..., sk-ant-admin-..., etc.)
            regex::Regex::new(r"sk-ant-[A-Za-z0-9_\-]{10,}").unwrap(),
            // OpenAI: sk-proj-..., sk-svcacct-..., sk-...
            regex::Regex::new(r"sk-[A-Za-z0-9_\-]{20,}").unwrap(),
            // Google AI Studio / Gemini
            regex::Regex::new(r"AIzaSy[A-Za-z0-9_\-]{20,}").unwrap(),
        ]
    });

    let mut out = text.to_string();
    for re in patterns {
        out = re.replace_all(&out, "[API_KEY_REDACTED]").to_string();
    }
    out
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

/// The user's active provider + model, resolved from settings (#108).
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveProvider {
    pub provider_id: String,
    /// Explicit model choice for this provider, if the user set one.
    /// `None` defers to the provider's catalog default.
    pub model_id: Option<String>,
}

/// Resolve the active provider + its model from settings. The single source
/// of truth for every backend-initiated LLM call — interactive chat, memory
/// summarization, and review-highlight extraction all route through this so
/// no call site hardcodes a provider (#108: hardcoded "anthropic" made
/// memory/highlights silently dead for trial users and falsely erroring for
/// OpenAI/Gemini BYOK customers).
pub async fn resolve_active_provider(
    pool: &crate::db::DbPool,
) -> Result<ActiveProvider, crate::settings::SettingsError> {
    let provider_id = crate::settings::get_setting(pool, "active_provider")
        .await?
        .unwrap_or_else(|| "anthropic".to_string());
    let model_key = format!("active_model_{}", provider_id);
    let model_id = crate::settings::get_setting(pool, &model_key).await?;
    Ok(ActiveProvider {
        provider_id,
        model_id,
    })
}

/// Send a message to an AI provider and get a response (non-streaming).
/// Uses the provider's default temperature.
pub async fn send_message(
    pool: &crate::db::DbPool,
    audit: crate::audit::EgressAudit,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    provider_id: &str,
    model_id: Option<&str>,
) -> Result<ChatResponse, ChatError> {
    send_message_with_temperature(pool, audit, messages, system_prompt, provider_id, model_id, None)
        .await
}

/// Send a message to an AI provider with an explicit generation temperature.
/// `None` defers to the provider's configured default.
///
/// #112: every attempt past the redaction point writes an audit row via
/// `audit` — success or failure. Only a missing API key exits row-less
/// (nothing was redacted, nothing left the machine).
pub async fn send_message_with_temperature(
    pool: &crate::db::DbPool,
    audit: crate::audit::EgressAudit,
    messages: Vec<ChatMessage>,
    system_prompt: Option<String>,
    provider_id: &str,
    model_id: Option<&str>,
    temperature: Option<f32>,
) -> Result<ChatResponse, ChatError> {
    let provider = resolve_provider(provider_id, model_id);
    let api_key = get_api_key_for_provider(provider_id)?;

    // Enforce PII redaction before anything leaves the machine. This covers
    // backend-initiated calls (memory summarization, highlight extraction)
    // that don't have an AppHandle to emit an event from — summary is dropped.
    let (messages, system_prompt, _pii_summary) = redact_chat_payload(messages, system_prompt);
    let request_redacted = last_user_message(&messages);

    let result = async {
        // Trim conversation to fit within token budget (silently drops oldest messages)
        let trimmed_messages = trim_conversation_to_budget(messages, &system_prompt);
        let provider_messages = to_provider_messages(trimmed_messages);

        // Build and send the request via the provider
        let client = SHARED_CLIENT.clone();
        let request_builder =
            provider.build_request(&client, &provider_messages, &system_prompt, &api_key, temperature);
        let response = request_builder.send().await?;

        // Check for HTTP errors
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            let parsed = provider.parse_error_response(&error_text);
            return Err(ChatError::ApiError(redact_api_keys(&format!(
                "HTTP {}: {}",
                status.as_u16(),
                parsed
            ))));
        }

        // Parse successful response via the provider
        let body_text = response
            .text()
            .await
            .map_err(|e| ChatError::ParseError(e.to_string()))?;

        let provider_response = provider
            .parse_response(&body_text)
            .map_err(ChatError::ParseError)?;

        Ok(ChatResponse {
            content: provider_response.content,
            input_tokens: provider_response.input_tokens,
            output_tokens: provider_response.output_tokens,
        })
    }
    .await;

    let outcome = stream_outcome(
        &result,
        result
            .as_ref()
            .map(|r: &ChatResponse| r.content.chars().count())
            .unwrap_or(0),
    );
    write_egress_audit(pool, &audit, &request_redacted, &outcome).await;

    result
}

/// Process an SSE stream response, emitting "chat-stream" events to the frontend.
/// Shared between BYOK and trial proxy streaming paths.
///
/// The caller passes a `cancel_token` pulled from `StreamRegistry`. If the
/// token fires mid-stream we emit `chat-stream-cancelled` and return
/// `ChatError::Cancelled`; dropping the response here closes the reqwest
/// connection, which stops the upstream provider from streaming further
/// tokens (the billing event we care about).
/// On success returns the full accumulated response text (the caller audits
/// its length). `chars_streamed` is updated as deltas arrive so that on error
/// or cancel the caller can record how much streamed before the interruption
/// (#112 partial audit rows).
async fn process_sse_stream<R: tauri::Runtime>(
    app: &AppHandle<R>,
    response: reqwest::Response,
    provider: &dyn Provider,
    aggregates: Option<crate::context::OrgAggregates>,
    query_type: Option<crate::context::QueryType>,
    cancel_token: CancellationToken,
    chars_streamed: &mut usize,
) -> Result<String, ChatError> {
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
        // (chunk errors above return with `chars_streamed` already reflecting
        // everything that arrived — the partial audit row stays truthful)
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
                                *chars_streamed = full_response.chars().count();

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
                                return Err(ChatError::ApiError(redact_api_keys(&msg)));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(full_response)
}

/// The last user-role message of a redacted payload — what an audit row
/// records as "what was asked". Uniform across interactive chat (the user's
/// latest message) and backend egress (the single constructed user prompt).
fn last_user_message(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default()
}

/// Write the egress audit row for one attempt (#112). Best-effort by design:
/// audit failures are logged, never allowed to fail the chat flow.
async fn write_egress_audit(
    pool: &crate::db::DbPool,
    audit: &crate::audit::EgressAudit,
    request_redacted: &str,
    outcome: &crate::audit::EgressOutcome,
) {
    if let Err(e) =
        crate::audit::record_llm_egress(pool, audit, request_redacted, outcome).await
    {
        log::warn!("egress audit write failed (source={}): {e}", audit.source.as_str());
    }
}

/// Map a streaming result to its audit outcome. `streamed` is how many chars
/// arrived before the stream ended (equals the full length on success).
fn stream_outcome<T>(result: &Result<T, ChatError>, streamed: usize) -> crate::audit::EgressOutcome {
    use crate::audit::EgressOutcome;
    match result {
        Ok(_) => EgressOutcome::Ok { response_chars: streamed },
        Err(ChatError::Cancelled) => EgressOutcome::Cancelled { partial_chars: streamed },
        Err(e) => EgressOutcome::Error { partial_chars: streamed, error: e.to_string() },
    }
}

/// Check HTTP response status and return an error if not successful.
fn check_http_error_status(
    status: reqwest::StatusCode,
    error_text: &str,
    provider: &dyn Provider,
) -> Result<(), ChatError> {
    let parsed = provider.parse_error_response(error_text);
    Err(ChatError::ApiError(redact_api_keys(&format!(
        "HTTP {}: {}",
        status.as_u16(),
        parsed
    ))))
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
pub async fn send_message_streaming<R: tauri::Runtime>(
    app: AppHandle<R>,
    registry: &StreamRegistry,
    stream_id: String,
    pool: &crate::db::DbPool,
    audit: crate::audit::EgressAudit,
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
    let request_redacted = last_user_message(&messages);

    let mut streamed = 0usize;
    let result = async {
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

        process_sse_stream(
            &app,
            response,
            &*provider,
            aggregates,
            query_type,
            cancel_token,
            &mut streamed,
        )
        .await
    }
    .await;

    let final_streamed = result.as_ref().map(|full| full.chars().count()).unwrap_or(streamed);
    let outcome = stream_outcome(&result, final_streamed);
    write_egress_audit(pool, &audit, &request_redacted, &outcome).await;

    result.map(|_| ())
}

/// Send a message through the trial proxy with streaming response.
/// Routes through the proxy URL instead of directly to Anthropic.
/// The proxy manages the API key; we send a device ID for quota tracking.
///
/// Same registry/guard pattern as `send_message_streaming`. Cancelling a
/// trial stream mid-flight still counts against the trial quota on the
/// proxy side (the request was accepted) but stops downstream token
/// delivery — the cost saving is on the Anthropic bill behind the proxy.
pub async fn send_message_streaming_trial<R: tauri::Runtime>(
    app: AppHandle<R>,
    registry: &StreamRegistry,
    stream_id: String,
    pool: &crate::db::DbPool,
    audit: crate::audit::EgressAudit,
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
    let request_redacted = last_user_message(&messages);

    let mut streamed = 0usize;
    let result = async {
        // Trim and convert messages
        let trimmed_messages = trim_conversation_to_budget(messages, &system_prompt);
        let provider_messages = to_provider_messages(trimmed_messages);

        // Build the serializable request body for the proxy (trial always uses default temperature)
        let request = anthropic.build_message_request(&provider_messages, &system_prompt, true, None);
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

        let full = process_sse_stream(
            &app,
            response,
            &anthropic,
            aggregates,
            query_type,
            cancel_token,
            &mut streamed,
        )
        .await?;
        Ok((usage, full))
    }
    .await;

    let final_streamed = result
        .as_ref()
        .map(|(_, full): &(TrialUsageMetadata, String)| full.chars().count())
        .unwrap_or(streamed);
    let outcome = stream_outcome(&result, final_streamed);
    write_egress_audit(pool, &audit, &request_redacted, &outcome).await;

    result.map(|(usage, _)| usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // Active-provider resolution (#108)
    // ========================================

    async fn test_pool() -> crate::db::DbPool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::time::Duration;
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect :memory: pool");
        crate::db::run_migrations_for_tests(&pool)
            .await
            .expect("run migrations");
        pool
    }

    #[tokio::test]
    async fn resolve_active_provider_defaults_to_anthropic() {
        let pool = test_pool().await;
        let active = resolve_active_provider(&pool).await.unwrap();
        assert_eq!(active.provider_id, "anthropic");
        assert_eq!(active.model_id, None);
    }

    #[tokio::test]
    async fn resolve_active_provider_reads_settings() {
        let pool = test_pool().await;
        crate::settings::set_setting(&pool, "active_provider", "openai")
            .await
            .unwrap();
        crate::settings::set_setting(&pool, "active_model_openai", "gpt-4o-mini")
            .await
            .unwrap();
        let active = resolve_active_provider(&pool).await.unwrap();
        assert_eq!(active.provider_id, "openai");
        assert_eq!(active.model_id.as_deref(), Some("gpt-4o-mini"));
    }

    #[tokio::test]
    async fn resolve_active_provider_ignores_other_providers_model() {
        // The model setting is provider-scoped: a leftover anthropic model
        // must not leak into an openai session.
        let pool = test_pool().await;
        crate::settings::set_setting(&pool, "active_provider", "openai")
            .await
            .unwrap();
        crate::settings::set_setting(&pool, "active_model_anthropic", "claude-sonnet-4-6")
            .await
            .unwrap();
        let active = resolve_active_provider(&pool).await.unwrap();
        assert_eq!(active.provider_id, "openai");
        assert_eq!(active.model_id, None);
    }

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

    // ========================================
    // API key redaction in error text (issue #36)
    // ========================================

    #[test]
    fn redact_api_keys_strips_anthropic_key() {
        let msg = "HTTP 401: authentication_error: Invalid API key sk-ant-api03-AbCdEf1234567890XYZ provided";
        let out = redact_api_keys(msg);
        assert!(
            !out.contains("sk-ant-api03-AbCdEf1234567890XYZ"),
            "anthropic key leaked: {out}"
        );
        assert!(out.contains("[API_KEY_REDACTED]"));
    }

    #[test]
    fn redact_api_keys_strips_openai_project_key() {
        let msg = "HTTP 401: invalid_api_key: sk-proj-abcdefghijklmnopqrstuvwxyz1234567890 is not valid";
        let out = redact_api_keys(msg);
        assert!(
            !out.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"),
            "openai key leaked: {out}"
        );
        assert!(out.contains("[API_KEY_REDACTED]"));
    }

    #[test]
    fn redact_api_keys_strips_gemini_key() {
        let msg = "API key not valid. Please pass a valid API key. (key=AIzaSyAbcdEfgh1234567890IjklMnopQr)";
        let out = redact_api_keys(msg);
        assert!(
            !out.contains("AIzaSyAbcdEfgh1234567890IjklMnopQr"),
            "gemini key leaked: {out}"
        );
        assert!(out.contains("[API_KEY_REDACTED]"));
    }

    #[test]
    fn redact_api_keys_strips_multiple_keys_in_one_message() {
        let msg = "Compared keys sk-ant-abcdef1234567890 and sk-proj-zzzzzzzzzzzzzzzzzzzzzzzzz; both invalid.";
        let out = redact_api_keys(msg);
        assert!(!out.contains("sk-ant-abcdef1234567890"));
        assert!(!out.contains("sk-proj-zzzzzzzzzzzzzzzzzzzzzzzzz"));
        // Two redactions for two keys.
        assert_eq!(out.matches("[API_KEY_REDACTED]").count(), 2);
    }

    #[test]
    fn redact_api_keys_passthrough_when_no_key_present() {
        let msg = "HTTP 503: service_unavailable: Anthropic API is temporarily overloaded. Try again.";
        assert_eq!(redact_api_keys(msg), msg);
    }

    #[test]
    fn redact_api_keys_does_not_match_short_sk_strings() {
        // Avoid false positives on short product SKUs like sk-100 or sk-pro.
        let msg = "Item code sk-shortidx and SKU sk-100 unaffected.";
        let out = redact_api_keys(msg);
        assert_eq!(out, msg, "short sk- substrings must not match (false positive)");
    }

    // ============================================================================
    // #112 — egress audit at the streaming seam.
    //
    // These drive the REAL trial streaming path end-to-end against a local
    // one-shot HTTP server (the `proxy_url` param is the injection seam) with
    // a mock AppHandle, and assert the audit row the seam writes: ok on
    // completion, error partial row on provider failure, cancelled partial
    // row on user cancel. The BYOK path shares process_sse_stream and the
    // same outcome→row mapping.
    // ============================================================================

    fn egress_ctx() -> crate::audit::EgressAudit {
        crate::audit::EgressAudit {
            source: crate::audit::EgressSource::Interactive,
            conversation_id: Some("conv-1".into()),
            employee_ids: vec![],
            query_category: None,
        }
    }

    /// One-shot HTTP server: accepts a single connection, reads the request,
    /// writes `response` verbatim, then closes (EOF-terminated body).
    async fn spawn_one_shot_server(response: String) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 8192];
            let _ = sock.read(&mut buf).await;
            let _ = sock.write_all(response.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
        format!("http://{addr}")
    }

    async fn fetch_single_audit_row(
        pool: &crate::db::DbPool,
    ) -> (Option<String>, Option<String>, String, String) {
        sqlx::query_as(
            "SELECT source, status, request_redacted, response_text FROM audit_log",
        )
        .fetch_one(pool)
        .await
        .expect("exactly one audit row must exist")
    }

    #[tokio::test]
    async fn trial_stream_success_writes_ok_audit_row_with_redacted_request() {
        let pool = test_pool().await;
        let sse = concat!(
            "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"Hello\"}}\n\n",
            "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \" world\"}}\n\n",
            "data: {\"type\": \"message_stop\"}\n\n",
        );
        let url = spawn_one_shot_server(format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{sse}"
        ))
        .await;

        let app = tauri::test::mock_app();
        let registry = StreamRegistry::new();
        let messages = vec![make_message(
            "user",
            "Sarah's SSN is 123-45-6789, summarize her file.",
        )];

        let result = send_message_streaming_trial(
            app.handle().clone(),
            &registry,
            "stream-ok".to_string(),
            &pool,
            egress_ctx(),
            messages,
            None,
            &url,
            "device-1",
            None,
            None,
            None,
        )
        .await;
        assert!(result.is_ok(), "stream should complete: {result:?}");

        let (source, status, request, response_meta) = fetch_single_audit_row(&pool).await;
        assert_eq!(source.as_deref(), Some("interactive"));
        assert_eq!(status.as_deref(), Some("ok"));
        assert!(
            !request.contains("123-45-6789"),
            "raw SSN must never reach the audit log: {request}"
        );
        assert!(
            request.contains("[SSN_REDACTED]"),
            "audit row must record the redacted request: {request}"
        );
        // "Hello world" = 11 chars
        assert_eq!(response_meta, "[REDACTED_RESPONSE length=11 chars]");
    }

    #[tokio::test]
    async fn trial_stream_http_error_writes_error_audit_row() {
        let pool = test_pool().await;
        let body = r#"{"type":"error","error":{"type":"api_error","message":"boom"}}"#;
        let url = spawn_one_shot_server(format!(
            "HTTP/1.1 500 Internal Server Error\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n{body}"
        ))
        .await;

        let app = tauri::test::mock_app();
        let registry = StreamRegistry::new();
        let messages = vec![make_message("user", "hello")];

        let result = send_message_streaming_trial(
            app.handle().clone(),
            &registry,
            "stream-err".to_string(),
            &pool,
            egress_ctx(),
            messages,
            None,
            &url,
            "device-1",
            None,
            None,
            None,
        )
        .await;
        assert!(result.is_err(), "HTTP 500 must surface as an error");

        let (_, status, _, response_meta) = fetch_single_audit_row(&pool).await;
        assert_eq!(status.as_deref(), Some("error"));
        assert!(
            response_meta.contains("[STREAM_ERROR after 0 chars"),
            "error row must record zero streamed chars: {response_meta}"
        );
        assert!(
            response_meta.contains("500"),
            "error row must carry the failure class: {response_meta}"
        );
    }

    #[tokio::test]
    async fn trial_stream_cancel_writes_cancelled_audit_row() {
        let pool = test_pool().await;
        // Server sends headers + one delta, then holds the connection open —
        // the stream can only end via cancellation.
        let url = {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\ndata: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"Hel\"}}\n\n";
                let _ = sock.write_all(head.as_bytes()).await;
                // Hold open far longer than the test will run.
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            });
            format!("http://{addr}")
        };

        let app = tauri::test::mock_app();
        let registry = std::sync::Arc::new(StreamRegistry::new());
        let pool_for_task = pool.clone();
        let registry_for_task = registry.clone();
        let handle = app.handle().clone();

        let send_task = tokio::spawn(async move {
            send_message_streaming_trial(
                handle,
                &registry_for_task,
                "stream-cancel".to_string(),
                &pool_for_task,
                egress_ctx(),
                vec![make_message("user", "hello")],
                None,
                &url,
                "device-1",
                None,
                None,
                None,
            )
            .await
        });

        // The stream registers its id synchronously at entry; poll until the
        // cancel lands, then the seam must observe it and write the row.
        loop {
            if registry.cancel("stream-cancel") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        let result = send_task.await.unwrap();
        assert!(
            matches!(result, Err(ChatError::Cancelled)),
            "cancel must surface as ChatError::Cancelled: {result:?}"
        );

        let (_, status, _, response_meta) = fetch_single_audit_row(&pool).await;
        assert_eq!(status.as_deref(), Some("cancelled"));
        assert!(
            response_meta.starts_with("[STREAM_CANCELLED after"),
            "cancelled row must record partial progress: {response_meta}"
        );
    }
}
