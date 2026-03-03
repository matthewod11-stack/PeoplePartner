// HR Command Center - Gemini Provider
// Implements the Provider trait for Google's Gemini Generative Language API

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::provider::{Provider, ProviderConfig, ProviderMessage, ProviderResponse, StreamDelta};

// ============================================================================
// Constants
// ============================================================================

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
const MODEL: &str = "gemini-2.5-flash";
const MAX_TOKENS: u32 = 8192;

// ============================================================================
// Gemini Wire Types (Request)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<SystemInstruction>,
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Part {
    #[serde(default)]
    text: String,
}

#[derive(Debug, Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    max_output_tokens: u32,
}

// ============================================================================
// Gemini Wire Types (Response)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentResponse {
    candidates: Option<Vec<Candidate>>,
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: Option<CandidateContent>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CandidateContent {
    parts: Option<Vec<Part>>,
    #[allow(dead_code)]
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    prompt_token_count: u32,
    candidates_token_count: Option<u32>,
    #[allow(dead_code)]
    total_token_count: u32,
}

// ============================================================================
// Gemini Wire Types (Error)
// ============================================================================

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    #[allow(dead_code)]
    code: u32,
    message: String,
    status: String,
}

// ============================================================================
// GeminiProvider
// ============================================================================

pub struct GeminiProvider {
    config: ProviderConfig,
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                model: MODEL.to_string(),
                max_tokens: MAX_TOKENS,
                api_url: GEMINI_API_BASE.to_string(),
            },
        }
    }

    pub fn new_with_model(model_id: &str, max_tokens: u32) -> Self {
        Self {
            config: ProviderConfig {
                model: model_id.to_string(),
                max_tokens,
                api_url: GEMINI_API_BASE.to_string(),
            },
        }
    }

    fn generate_url(&self) -> String {
        format!(
            "{}/models/{}:generateContent",
            self.config.api_url, self.config.model
        )
    }

    fn stream_url(&self) -> String {
        format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.config.api_url, self.config.model
        )
    }

    /// Build a GenerateContentRequest body.
    /// System prompt goes into the separate systemInstruction field.
    /// Role "assistant" is mapped to "model" for the Gemini API.
    fn build_generate_request(
        &self,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
    ) -> GenerateContentRequest {
        let contents: Vec<GeminiContent> = messages
            .iter()
            .map(|m| GeminiContent {
                role: match m.role.as_str() {
                    "assistant" => "model".to_string(),
                    other => other.to_string(),
                },
                parts: vec![Part {
                    text: m.content.clone(),
                }],
            })
            .collect();

        let system_instruction = system_prompt.as_ref().map(|text| SystemInstruction {
            parts: vec![Part { text: text.clone() }],
        });

        GenerateContentRequest {
            contents,
            system_instruction,
            generation_config: GenerationConfig {
                max_output_tokens: self.config.max_tokens,
            },
        }
    }
}

impl Provider for GeminiProvider {
    fn id(&self) -> &str {
        "gemini"
    }

    fn display_name(&self) -> &str {
        "Google Gemini"
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn validate_key_format(&self, key: &str) -> bool {
        key.starts_with("AIzaSy") && key.len() == 39
    }

    fn key_prefix_hint(&self) -> &str {
        "AIzaSy..."
    }

    fn build_request(
        &self,
        client: &Client,
        messages: &[ProviderMessage],
        system_prompt: &Option<String>,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        let body = self.build_generate_request(messages, system_prompt);
        client
            .post(self.generate_url())
            .header("x-goog-api-key", api_key)
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
        let body = self.build_generate_request(messages, system_prompt);
        client
            .post(self.stream_url())
            .header("x-goog-api-key", api_key)
            .header("content-type", "application/json")
            .json(&body)
    }

    fn parse_response(&self, body: &str) -> Result<ProviderResponse, String> {
        let response: GenerateContentResponse =
            serde_json::from_str(body).map_err(|e| format!("Failed to parse response: {}", e))?;

        let candidates = response
            .candidates
            .as_ref()
            .ok_or_else(|| "No candidates in response".to_string())?;

        let first = candidates
            .first()
            .ok_or_else(|| "Empty candidates array".to_string())?;

        let content = first
            .content
            .as_ref()
            .and_then(|c| c.parts.as_ref())
            .map(|parts| {
                parts
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let input_tokens = response
            .usage_metadata
            .as_ref()
            .map(|u| u.prompt_token_count)
            .unwrap_or(0);

        let output_tokens = response
            .usage_metadata
            .as_ref()
            .and_then(|u| u.candidates_token_count)
            .unwrap_or(0);

        Ok(ProviderResponse {
            content,
            input_tokens,
            output_tokens,
        })
    }

    fn parse_sse_event(&self, data: &str) -> Option<StreamDelta> {
        let response: GenerateContentResponse = serde_json::from_str(data).ok()?;

        let candidate = response.candidates.as_ref()?.first()?;

        // finishReason present → stream is done
        if candidate.finish_reason.is_some() {
            return Some(StreamDelta::Done);
        }

        // Extract text from content parts
        let text = candidate
            .content
            .as_ref()?
            .parts
            .as_ref()?
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            return None;
        }

        Some(StreamDelta::TextDelta(text))
    }

    fn parse_error_response(&self, body: &str) -> String {
        if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(body) {
            format!("{}: {}", api_error.error.status, api_error.error.message)
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
    fn test_gemini_provider_id() {
        let provider = GeminiProvider::new();
        assert_eq!(provider.id(), "gemini");
    }

    #[test]
    fn test_gemini_provider_display_name() {
        let provider = GeminiProvider::new();
        assert_eq!(provider.display_name(), "Google Gemini");
    }

    #[test]
    fn test_gemini_provider_config() {
        let provider = GeminiProvider::new();
        let config = provider.config();
        assert_eq!(config.model, "gemini-2.5-flash");
        assert_eq!(config.max_tokens, 8192);
        assert_eq!(
            config.api_url,
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn test_validate_key_format_valid() {
        let provider = GeminiProvider::new();
        // "AIzaSy" (6 chars) + 33 more chars = 39 total
        assert!(provider.validate_key_format("AIzaSyA23456789012345678901234567890abc"));
    }

    #[test]
    fn test_validate_key_format_invalid() {
        let provider = GeminiProvider::new();
        assert!(!provider.validate_key_format(""));
        assert!(!provider.validate_key_format("wrong-prefix-key-1234567890123456789"));
        assert!(!provider.validate_key_format("AIzaSyTooShort"));
        assert!(!provider.validate_key_format(
            "AIzaSyA234567890123456789012345678901234TooLong"
        ));
    }

    #[test]
    fn test_generate_url() {
        let provider = GeminiProvider::new();
        assert_eq!(
            provider.generate_url(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent"
        );
    }

    #[test]
    fn test_stream_url() {
        let provider = GeminiProvider::new();
        assert_eq!(
            provider.stream_url(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn test_parse_response_success() {
        let provider = GeminiProvider::new();
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello, how can I help?"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 8,
                "totalTokenCount": 18
            }
        }"#;

        let result = provider.parse_response(json).unwrap();
        assert_eq!(result.content, "Hello, how can I help?");
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 8);
    }

    #[test]
    fn test_parse_response_error() {
        let provider = GeminiProvider::new();
        let result = provider.parse_response("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_response_multiple_parts() {
        let provider = GeminiProvider::new();
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello, "}, {"text": "world!"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 5,
                "candidatesTokenCount": 4,
                "totalTokenCount": 9
            }
        }"#;

        let result = provider.parse_response(json).unwrap();
        assert_eq!(result.content, "Hello, world!");
    }

    #[test]
    fn test_parse_response_no_candidates() {
        let provider = GeminiProvider::new();
        let json = r#"{
            "usageMetadata": {
                "promptTokenCount": 5,
                "candidatesTokenCount": 0,
                "totalTokenCount": 5
            }
        }"#;

        let result = provider.parse_response(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No candidates"));
    }

    #[test]
    fn test_parse_sse_text_delta() {
        let provider = GeminiProvider::new();
        let data = r#"{"candidates":[{"content":{"parts":[{"text":"Hello"}],"role":"model"}}]}"#;

        let delta = provider.parse_sse_event(data).unwrap();
        match delta {
            StreamDelta::TextDelta(text) => assert_eq!(text, "Hello"),
            other => panic!("Expected TextDelta, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sse_finish_reason_stop() {
        let provider = GeminiProvider::new();
        let data = r#"{"candidates":[{"content":{"parts":[{"text":"!"}],"role":"model"},"finishReason":"STOP"}]}"#;

        let delta = provider.parse_sse_event(data).unwrap();
        match delta {
            StreamDelta::Done => {}
            other => panic!("Expected Done, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sse_empty_content() {
        let provider = GeminiProvider::new();
        let data = r#"{"candidates":[{"content":{"parts":[{"text":""}],"role":"model"}}]}"#;

        let result = provider.parse_sse_event(data);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_invalid_json() {
        let provider = GeminiProvider::new();
        let result = provider.parse_sse_event("not json at all");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_error_response_structured() {
        let provider = GeminiProvider::new();
        let json = r#"{"error": {"code": 400, "message": "Invalid API key", "status": "INVALID_ARGUMENT"}}"#;

        let result = provider.parse_error_response(json);
        assert_eq!(result, "INVALID_ARGUMENT: Invalid API key");
    }

    #[test]
    fn test_parse_error_response_fallback() {
        let provider = GeminiProvider::new();
        let raw = "Some unexpected error text";

        let result = provider.parse_error_response(raw);
        assert_eq!(result, "Some unexpected error text");
    }

    #[test]
    fn test_build_request_role_mapping() {
        let provider = GeminiProvider::new();
        let messages = vec![
            ProviderMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
            ProviderMessage {
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
            },
            ProviderMessage {
                role: "user".to_string(),
                content: "How are you?".to_string(),
            },
        ];
        let system = Some("You are helpful.".to_string());

        let req = provider.build_generate_request(&messages, &system);

        // "assistant" should be mapped to "model"
        assert_eq!(req.contents.len(), 3);
        assert_eq!(req.contents[0].role, "user");
        assert_eq!(req.contents[0].parts[0].text, "Hello");
        assert_eq!(req.contents[1].role, "model");
        assert_eq!(req.contents[1].parts[0].text, "Hi there!");
        assert_eq!(req.contents[2].role, "user");

        // System prompt in systemInstruction
        let si = req.system_instruction.unwrap();
        assert_eq!(si.parts[0].text, "You are helpful.");
    }

    #[test]
    fn test_build_request_no_system() {
        let provider = GeminiProvider::new();
        let messages = vec![ProviderMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];

        let req = provider.build_generate_request(&messages, &None);
        assert!(req.system_instruction.is_none());
        assert_eq!(req.contents.len(), 1);
    }
}
