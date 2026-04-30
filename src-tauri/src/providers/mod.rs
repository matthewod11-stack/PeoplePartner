// People Partner - Provider Registry
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

// ============================================================================
// Tests (issue #39)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_provider_resolves_known_ids() {
        // The settings UI passes provider id strings unmodified — a wrong
        // factory mapping silently routes to the wrong provider.
        let anthropic = get_provider("anthropic", None).expect("anthropic resolves");
        assert_eq!(anthropic.id(), "anthropic");

        let openai = get_provider("openai", None).expect("openai resolves");
        assert_eq!(openai.id(), "openai");

        let gemini = get_provider("gemini", None).expect("gemini resolves");
        assert_eq!(gemini.id(), "gemini");
    }

    #[test]
    fn get_provider_returns_none_for_unknown_id() {
        assert!(get_provider("not-a-real-provider", None).is_none());
        assert!(get_provider("", None).is_none());
    }

    #[test]
    fn get_default_provider_is_anthropic() {
        // Default-provider identity is documented on the trial proxy and the
        // first-launch onboarding copy. Lock it so a future contributor
        // can't quietly switch the cold-start experience.
        let default = get_default_provider();
        assert_eq!(default.id(), "anthropic");
    }

    #[test]
    fn available_providers_lists_all_three_in_expected_shape() {
        let providers = available_providers();
        assert_eq!(providers.len(), 3);

        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"anthropic"));
        assert!(ids.contains(&"openai"));
        assert!(ids.contains(&"gemini"));

        // Every entry must carry a non-empty display name + key prefix hint —
        // these drive the UI selector and the empty-API-key prompts.
        for info in &providers {
            assert!(!info.display_name.is_empty(), "missing display_name for {}", info.id);
            assert!(!info.key_prefix_hint.is_empty(), "missing key prefix hint for {}", info.id);
        }
    }

    #[test]
    fn unknown_model_override_falls_through_to_default_for_known_provider() {
        // Subtle behavior: passing an unknown model id for a *known* provider
        // returns the provider's default configuration (not None). Documents
        // the fallback so a future refactor doesn't change to fail-closed
        // without intent.
        let provider = get_provider("anthropic", Some("nonexistent-model-xyz"))
            .expect("known provider should resolve even with unknown model");
        assert_eq!(provider.id(), "anthropic");
    }
}
