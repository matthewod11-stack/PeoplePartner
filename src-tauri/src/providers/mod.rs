// HR Command Center - Provider Registry
// Creates and lists available AI providers

pub mod anthropic;

use crate::provider::{Provider, ProviderInfo};
use anthropic::AnthropicProvider;

/// Get a provider by its ID string.
pub fn get_provider(id: &str) -> Option<Box<dyn Provider>> {
    match id {
        "anthropic" => Some(Box::new(AnthropicProvider::new())),
        _ => None,
    }
}

/// Get the default provider (Anthropic).
pub fn get_default_provider() -> Box<dyn Provider> {
    Box::new(AnthropicProvider::new())
}

/// List all available providers with their metadata.
pub fn available_providers() -> Vec<ProviderInfo> {
    vec![ProviderInfo {
        id: "anthropic".to_string(),
        display_name: "Anthropic".to_string(),
        key_prefix_hint: "sk-ant-...".to_string(),
    }]
}
