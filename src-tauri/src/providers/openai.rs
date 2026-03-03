// People Partner - OpenAI Provider
// Implements the Provider trait for the OpenAI Chat Completions API

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::provider::{Provider, ProviderConfig, ProviderMessage, ProviderResponse, StreamDelta};

// ============================================================================
// Constants
// ============================================================================

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const MODEL: &str = "gpt-4o";
const MAX_TOKENS: u32 = 4096;

// ============================================================================
// OpenAI Wire Types (Request)
// ============================================================================

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

// ============================================================================
// OpenAI Wire Types (Response)
// ============================================================================

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<Choice>,
    #[allow(dead_code)]
    model: String,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    #[allow(dead_code)]
    index: u32,
    message: ChoiceMessage,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    #[allow(dead_code)]
    role: String,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    #[allow(dead_code)]
    total_tokens: u32,
}

// ============================================================================
// OpenAI Wire Types (Error)
// ============================================================================

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    #[allow(dead_code)]
    code: Option<String>,
}

// ============================================================================
// OpenAI SSE Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct StreamChunkResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[allow(dead_code)]
    index: u32,
    delta: StreamDeltaContent,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDeltaContent {
    #[serde(default)]
    content: Option<String>,
}

// ============================================================================
// OpenAIProvider
// ============================================================================

pub struct OpenAIProvider {
    config: ProviderConfig,
}

impl OpenAIProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                model: MODEL.to_string(),
                max_tokens: MAX_TOKENS,
                api_url: OPENAI_API_URL.to_string(),
            },
        }
    }

    pub fn new_with_model(model_id: &str, max_tokens: u32) -> Self {
        Self {
            config: ProviderConfig {
                model: model_id.to_string(),
                max_tokens,
                api_url: OPENAI_API_URL.to_string(),
            },
        }
    }

    /// Build a ChatCompletionRequest body.
    /// System prompt is prepended as messages[0] with role "system".
    fn build_chat_request(
        &self,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        stream: bool,
    ) -> ChatCompletionRequest {
        let mut openai_messages: Vec<OpenAIMessage> = Vec::new();

        // Prepend system prompt as first message
        if let Some(system) = system_prompt {
            openai_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: system.clone(),
            });
        }

        // Convert ProviderMessages to OpenAIMessages
        for m in messages {
            openai_messages.push(OpenAIMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            });
        }

        ChatCompletionRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            messages: openai_messages,
            stream: if stream { Some(true) } else { None },
        }
    }
}

impl Provider for OpenAIProvider {
    fn id(&self) -> &str {
        "openai"
    }

    fn display_name(&self) -> &str {
        "OpenAI"
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn validate_key_format(&self, key: &str) -> bool {
        key.starts_with("sk-") && key.len() > 20
    }

    fn key_prefix_hint(&self) -> &str {
        "sk-..."
    }

    fn build_request(
        &self,
        client: &Client,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        let body = self.build_chat_request(messages, system_prompt, false);
        client
            .post(&self.config.api_url)
            .header("Authorization", format!("Bearer {}", api_key))
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
        let body = self.build_chat_request(messages, system_prompt, true);
        client
            .post(&self.config.api_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("content-type", "application/json")
            .json(&body)
    }

    fn parse_response(&self, body: &str) -> Result<ProviderResponse, String> {
        let response: ChatCompletionResponse =
            serde_json::from_str(body).map_err(|e| format!("Failed to parse response: {}", e))?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(ProviderResponse {
            content,
            input_tokens: response.usage.prompt_tokens,
            output_tokens: response.usage.completion_tokens,
        })
    }

    fn parse_sse_event(&self, data: &str) -> Option<StreamDelta> {
        // [DONE] is not valid JSON — check before parsing
        if data.trim() == "[DONE]" {
            return Some(StreamDelta::Done);
        }

        let chunk: StreamChunkResponse = serde_json::from_str(data).ok()?;
        let choice = chunk.choices.first()?;

        // finish_reason: "stop" → ignore (avoid double-Done with [DONE])
        if choice.finish_reason.as_deref() == Some("stop") {
            return None;
        }

        // Text content delta
        if let Some(ref text) = choice.delta.content {
            return Some(StreamDelta::TextDelta(text.clone()));
        }

        // Empty delta (e.g. role-only chunk at start) → ignore
        None
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
    fn test_openai_provider_id() {
        let provider = OpenAIProvider::new();
        assert_eq!(provider.id(), "openai");
    }

    #[test]
    fn test_openai_provider_display_name() {
        let provider = OpenAIProvider::new();
        assert_eq!(provider.display_name(), "OpenAI");
    }

    #[test]
    fn test_openai_provider_config() {
        let provider = OpenAIProvider::new();
        let config = provider.config();
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.api_url, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn test_validate_key_format_valid() {
        let provider = OpenAIProvider::new();
        assert!(provider.validate_key_format("sk-proj-abcdefghijklmnopqrst"));
        assert!(provider.validate_key_format("sk-abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn test_validate_key_format_invalid() {
        let provider = OpenAIProvider::new();
        assert!(!provider.validate_key_format(""));
        assert!(!provider.validate_key_format("sk-short"));
        assert!(!provider.validate_key_format("openai-key-12345678901234567890"));
        assert!(!provider.validate_key_format("pk-abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn test_parse_response_success() {
        let provider = OpenAIProvider::new();
        let json = r#"{
            "id": "chatcmpl-123",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello, how can I help?"},
                "finish_reason": "stop"
            }],
            "model": "gpt-4o",
            "usage": {"prompt_tokens": 10, "completion_tokens": 8, "total_tokens": 18}
        }"#;

        let result = provider.parse_response(json).unwrap();
        assert_eq!(result.content, "Hello, how can I help?");
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 8);
    }

    #[test]
    fn test_parse_response_error() {
        let provider = OpenAIProvider::new();
        let result = provider.parse_response("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_response_null_content() {
        let provider = OpenAIProvider::new();
        let json = r#"{
            "id": "chatcmpl-456",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": null},
                "finish_reason": "stop"
            }],
            "model": "gpt-4o",
            "usage": {"prompt_tokens": 5, "completion_tokens": 0, "total_tokens": 5}
        }"#;

        let result = provider.parse_response(json).unwrap();
        assert_eq!(result.content, "");
    }

    #[test]
    fn test_parse_sse_text_delta() {
        let provider = OpenAIProvider::new();
        let data = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;

        let delta = provider.parse_sse_event(data).unwrap();
        match delta {
            StreamDelta::TextDelta(text) => assert_eq!(text, "Hello"),
            other => panic!("Expected TextDelta, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sse_done() {
        let provider = OpenAIProvider::new();
        let delta = provider.parse_sse_event("[DONE]").unwrap();
        match delta {
            StreamDelta::Done => {}
            other => panic!("Expected Done, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sse_finish_reason_stop() {
        let provider = OpenAIProvider::new();
        let data = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;

        let result = provider.parse_sse_event(data);
        assert!(result.is_none(), "finish_reason=stop should return None to avoid double-Done");
    }

    #[test]
    fn test_parse_sse_empty_delta() {
        let provider = OpenAIProvider::new();
        let data = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{},"finish_reason":null}]}"#;

        let result = provider.parse_sse_event(data);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_invalid_json() {
        let provider = OpenAIProvider::new();
        let result = provider.parse_sse_event("not json at all");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_error_response_structured() {
        let provider = OpenAIProvider::new();
        let json = r#"{"error": {"message": "Invalid API key", "type": "invalid_request_error", "code": "invalid_api_key"}}"#;

        let result = provider.parse_error_response(json);
        assert_eq!(result, "invalid_request_error: Invalid API key");
    }

    #[test]
    fn test_parse_error_response_fallback() {
        let provider = OpenAIProvider::new();
        let raw = "Some unexpected error text";

        let result = provider.parse_error_response(raw);
        assert_eq!(result, "Some unexpected error text");
    }

    #[test]
    fn test_build_chat_request() {
        let provider = OpenAIProvider::new();
        let messages = vec![
            ProviderMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
        ];
        let system = Some("You are helpful.".to_string());

        let req = provider.build_chat_request(&messages, &system, true);
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.max_tokens, 4096);
        // System prompt becomes messages[0]
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, "system");
        assert_eq!(req.messages[0].content, "You are helpful.");
        assert_eq!(req.messages[1].role, "user");
        assert_eq!(req.messages[1].content, "Hello");
        assert_eq!(req.stream, Some(true));
    }
}
