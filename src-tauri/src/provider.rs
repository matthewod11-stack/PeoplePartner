// People Partner - Provider Trait
// Abstracts AI provider communication (Anthropic, OpenAI, Gemini)

use serde::Serialize;

/// Provider-agnostic streaming delta
#[derive(Debug, Clone)]
pub enum StreamDelta {
    TextDelta(String),
    Done,
    Error(String),
}

/// Provider-agnostic response from a non-streaming request
#[derive(Debug)]
pub struct ProviderResponse {
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Provider-agnostic message input
#[derive(Debug, Clone)]
pub struct ProviderMessage {
    pub role: String,
    pub content: String,
}

/// Provider configuration constants
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub model: String,
    pub max_tokens: u32,
    pub api_url: String,
}

/// Provider info for listing available providers (serializable for frontend)
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub key_prefix_hint: String,
}

/// Trait for AI providers (Anthropic, OpenAI, Gemini)
/// All methods are synchronous — HTTP send is done by chat.rs
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn config(&self) -> &ProviderConfig;
    fn validate_key_format(&self, key: &str) -> bool;
    fn key_prefix_hint(&self) -> &str;

    /// Build an HTTP request for a non-streaming message
    fn build_request(
        &self,
        client: &reqwest::Client,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        api_key: &str,
    ) -> reqwest::RequestBuilder;

    /// Build an HTTP request for a streaming message
    fn build_streaming_request(
        &self,
        client: &reqwest::Client,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        api_key: &str,
    ) -> reqwest::RequestBuilder;

    /// Parse a successful non-streaming response body
    fn parse_response(&self, body: &str) -> Result<ProviderResponse, String>;

    /// Parse a single SSE event data line
    fn parse_sse_event(&self, data: &str) -> Option<StreamDelta>;

    /// Parse an error response body into a human-readable message
    fn parse_error_response(&self, body: &str) -> String;
}
