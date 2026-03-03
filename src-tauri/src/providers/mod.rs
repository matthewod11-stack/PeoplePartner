// HR Command Center - Provider Registry
// Creates and lists available AI providers

pub mod anthropic;
pub mod gemini;
pub mod openai;

use crate::models;
use crate::provider::{Provider, ProviderInfo};
use anthropic::AnthropicProvider;
use gemini::GeminiProvider;
use openai::OpenAIProvider;

/// Get a provider by its ID string, optionally with a specific model.
///
/// When `model_override` is `Some`, looks up the model in the catalog and
/// creates a provider configured for that model. When `None`, uses the
/// provider's default model.
pub fn get_provider(id: &str, model_override: Option<&str>) -> Option<Box<dyn Provider>> {
    if let Some(model_id) = model_override {
        if let Some(info) = models::get_model_info(id, model_id) {
            return match id {
                "anthropic" => Some(Box::new(AnthropicProvider::new_with_model(model_id, info.max_output_tokens))),
                "openai" => Some(Box::new(OpenAIProvider::new_with_model(model_id, info.max_output_tokens))),
                "gemini" => Some(Box::new(GeminiProvider::new_with_model(model_id, info.max_output_tokens))),
                _ => None,
            };
        }
    }

    match id {
        "anthropic" => Some(Box::new(AnthropicProvider::new())),
        "openai" => Some(Box::new(OpenAIProvider::new())),
        "gemini" => Some(Box::new(GeminiProvider::new())),
        _ => None,
    }
}

/// Get the default provider (Anthropic).
pub fn get_default_provider() -> Box<dyn Provider> {
    Box::new(AnthropicProvider::new())
}

/// List all available providers with their metadata.
pub fn available_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "anthropic".to_string(),
            display_name: "Anthropic".to_string(),
            key_prefix_hint: "sk-ant-...".to_string(),
        },
        ProviderInfo {
            id: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            key_prefix_hint: "sk-...".to_string(),
        },
        ProviderInfo {
            id: "gemini".to_string(),
            display_name: "Google Gemini".to_string(),
            key_prefix_hint: "AIzaSy...".to_string(),
        },
    ]
}
