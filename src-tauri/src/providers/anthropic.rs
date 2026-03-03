// HR Command Center - Anthropic Provider
// Implements the Provider trait for the Anthropic Messages API

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::provider::{Provider, ProviderConfig, ProviderMessage, ProviderResponse, StreamDelta};

// ============================================================================
// Constants
// ============================================================================

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MODEL: &str = "claude-sonnet-4-6";
const MAX_TOKENS: u32 = 8192;

// ============================================================================
// Anthropic Wire Types (Request/Response)
// ============================================================================

/// Anthropic Messages API request body.
/// Public because the trial proxy path needs to serialize it.
#[derive(Debug, Serialize)]
pub struct MessageRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// A single message in the Anthropic format.
/// Public because MessageRequest contains it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct MessageResponse {
    #[allow(dead_code)]
    id: String,
    content: Vec<ContentBlock>,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    error_type: String,
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

// ============================================================================
// Anthropic SSE Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart {
        #[allow(dead_code)]
        message: StreamMessageStart,
    },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        #[allow(dead_code)]
        index: u32,
        #[allow(dead_code)]
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        #[allow(dead_code)]
        index: u32,
        delta: TextDelta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        #[allow(dead_code)]
        index: u32,
    },
    #[serde(rename = "message_delta")]
    MessageDelta {
        #[allow(dead_code)]
        delta: MessageDeltaData,
        #[allow(dead_code)]
        usage: Option<UsageDelta>,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: ApiErrorDetail },
}

#[derive(Debug, Deserialize)]
struct StreamMessageStart {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    model: String,
}

#[derive(Debug, Deserialize)]
struct TextDelta {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    delta_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaData {
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageDelta {
    #[allow(dead_code)]
    output_tokens: u32,
}

// ============================================================================
// AnthropicProvider
// ============================================================================

pub struct AnthropicProvider {
    config: ProviderConfig,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                model: MODEL.to_string(),
                max_tokens: MAX_TOKENS,
                api_url: ANTHROPIC_API_URL.to_string(),
            },
        }
    }

    pub fn new_with_model(model_id: &str, max_tokens: u32) -> Self {
        Self {
            config: ProviderConfig {
                model: model_id.to_string(),
                max_tokens,
                api_url: ANTHROPIC_API_URL.to_string(),
            },
        }
    }

    /// Build a serializable MessageRequest body.
    /// Public because the trial proxy path needs to serialize and forward it.
    pub fn build_message_request(
        &self,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        stream: bool,
    ) -> MessageRequest {
        MessageRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            messages: messages
                .iter()
                .map(|m| AnthropicMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect(),
            system: system_prompt.clone(),
            stream: if stream { Some(true) } else { None },
        }
    }
}

impl Provider for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    fn display_name(&self) -> &str {
        "Anthropic"
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn validate_key_format(&self, key: &str) -> bool {
        key.starts_with("sk-ant-") && key.len() > 20
    }

    fn key_prefix_hint(&self) -> &str {
        "sk-ant-..."
    }

    fn build_request(
        &self,
        client: &Client,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        let body = self.build_message_request(messages, system_prompt, false);
        client
            .post(&self.config.api_url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
    }

    fn build_streaming_request(
        &self,
        client: &Client,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        let body = self.build_message_request(messages, system_prompt, true);
        client
            .post(&self.config.api_url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
    }

    fn parse_response(&self, body: &str) -> Result<ProviderResponse, String> {
        let api_response: MessageResponse =
            serde_json::from_str(body).map_err(|e| format!("Failed to parse response: {}", e))?;

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

        Ok(ProviderResponse {
            content,
            input_tokens: api_response.usage.input_tokens,
            output_tokens: api_response.usage.output_tokens,
        })
    }

    fn parse_sse_event(&self, data: &str) -> Option<StreamDelta> {
        let event: StreamEvent = serde_json::from_str(data).ok()?;
        match event {
            StreamEvent::ContentBlockDelta { delta, .. } => {
                Some(StreamDelta::TextDelta(delta.text))
            }
            StreamEvent::MessageStop => Some(StreamDelta::Done),
            StreamEvent::Error { error } => Some(StreamDelta::Error(error.message)),
            _ => None,
        }
    }

    fn parse_error_response(&self, body: &str) -> String {
        if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(body) {
            format!("{}: {}", api_error.error.error_type, api_error.error.message)
        } else {
            body.to_string()
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_id() {
        let provider = AnthropicProvider::new();
        assert_eq!(provider.id(), "anthropic");
    }

    #[test]
    fn test_anthropic_provider_display_name() {
        let provider = AnthropicProvider::new();
        assert_eq!(provider.display_name(), "Anthropic");
    }

    #[test]
    fn test_anthropic_provider_config() {
        let provider = AnthropicProvider::new();
        let config = provider.config();
        assert_eq!(config.model, "claude-sonnet-4-6");
        assert_eq!(config.max_tokens, 8192);
        assert_eq!(config.api_url, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_validate_key_format_valid() {
        let provider = AnthropicProvider::new();
        assert!(provider.validate_key_format("sk-ant-api03-abcdefghijk"));
        assert!(provider.validate_key_format("sk-ant-XXXXXXXXXXXXXXXXXXXXXXXXXXXX"));
    }

    #[test]
    fn test_validate_key_format_invalid() {
        let provider = AnthropicProvider::new();
        assert!(!provider.validate_key_format("")); // empty
        assert!(!provider.validate_key_format("sk-proj-abc")); // wrong prefix
        assert!(!provider.validate_key_format("sk-ant-short")); // too short (<=20)
        assert!(!provider.validate_key_format("openai-key-12345678901234567890")); // wrong prefix
    }

    #[test]
    fn test_parse_response_success() {
        let provider = AnthropicProvider::new();
        let json = r#"{
            "id": "msg_123",
            "content": [{"type": "text", "text": "Hello, how can I help?"}],
            "model": "claude-sonnet-4-6",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 8}
        }"#;

        let result = provider.parse_response(json).unwrap();
        assert_eq!(result.content, "Hello, how can I help?");
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 8);
    }

    #[test]
    fn test_parse_response_error() {
        let provider = AnthropicProvider::new();
        let result = provider.parse_response("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_sse_content_delta() {
        let provider = AnthropicProvider::new();
        let data = r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#;

        let delta = provider.parse_sse_event(data).unwrap();
        match delta {
            StreamDelta::TextDelta(text) => assert_eq!(text, "Hello"),
            other => panic!("Expected TextDelta, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sse_message_stop() {
        let provider = AnthropicProvider::new();
        let data = r#"{"type": "message_stop"}"#;

        let delta = provider.parse_sse_event(data).unwrap();
        match delta {
            StreamDelta::Done => {}
            other => panic!("Expected Done, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sse_error() {
        let provider = AnthropicProvider::new();
        let data = r#"{"type": "error", "error": {"type": "overloaded_error", "message": "API is overloaded"}}"#;

        let delta = provider.parse_sse_event(data).unwrap();
        match delta {
            StreamDelta::Error(msg) => assert_eq!(msg, "API is overloaded"),
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_error_response() {
        let provider = AnthropicProvider::new();
        let json = r#"{"type": "error", "error": {"type": "authentication_error", "message": "Invalid API key"}}"#;

        let result = provider.parse_error_response(json);
        assert_eq!(result, "authentication_error: Invalid API key");
    }

    #[test]
    fn test_parse_error_response_fallback() {
        let provider = AnthropicProvider::new();
        let raw = "Some unexpected error text";

        let result = provider.parse_error_response(raw);
        assert_eq!(result, "Some unexpected error text");
    }

    #[test]
    fn test_build_message_request() {
        let provider = AnthropicProvider::new();
        let messages = vec![
            ProviderMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
        ];
        let system = Some("You are helpful.".to_string());

        let req = provider.build_message_request(&messages, &system, true);
        assert_eq!(req.model, "claude-sonnet-4-6");
        assert_eq!(req.max_tokens, 8192);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[0].content, "Hello");
        assert_eq!(req.system, Some("You are helpful.".to_string()));
        assert_eq!(req.stream, Some(true));
    }

    #[test]
    fn test_build_message_request_no_stream() {
        let provider = AnthropicProvider::new();
        let messages = vec![
            ProviderMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            },
        ];

        let req = provider.build_message_request(&messages, &None, false);
        assert!(req.stream.is_none());
        assert!(req.system.is_none());
    }

    #[test]
    fn test_parse_response_multiple_content_blocks() {
        let provider = AnthropicProvider::new();
        let json = r#"{
            "id": "msg_456",
            "content": [
                {"type": "text", "text": "Part one. "},
                {"type": "text", "text": "Part two."}
            ],
            "model": "claude-sonnet-4-6",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 5, "output_tokens": 12}
        }"#;

        let result = provider.parse_response(json).unwrap();
        assert_eq!(result.content, "Part one. Part two.");
    }

    #[test]
    fn test_parse_sse_ping_returns_none() {
        let provider = AnthropicProvider::new();
        let data = r#"{"type": "ping"}"#;

        let result = provider.parse_sse_event(data);
        assert!(result.is_none());
    }
}
