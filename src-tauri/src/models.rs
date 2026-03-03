// HR Command Center - Model Catalog
// Hardcoded catalog of available models per provider
//
// Key responsibilities:
// 1. List available models for each provider
// 2. Provide default model per provider
// 3. Look up model info (context window, max output, tier)

use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

/// Tier label for display in the UI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelTier {
    Recommended,
    Premium,
    Fast,
}

/// Information about a single model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub context_window: usize,
    pub max_output_tokens: u32,
    pub tier: ModelTier,
    pub is_default: bool,
}

// ============================================================================
// Catalog
// ============================================================================

/// Get all available models for a given provider.
pub fn models_for_provider(provider_id: &str) -> Vec<ModelInfo> {
    match provider_id {
        "anthropic" => vec![
            ModelInfo {
                id: "claude-sonnet-4-6".into(),
                display_name: "Claude Sonnet 4.6".into(),
                context_window: 200_000,
                max_output_tokens: 8192,
                tier: ModelTier::Recommended,
                is_default: true,
            },
            ModelInfo {
                id: "claude-opus-4-6".into(),
                display_name: "Claude Opus 4.6".into(),
                context_window: 200_000,
                max_output_tokens: 8192,
                tier: ModelTier::Premium,
                is_default: false,
            },
            ModelInfo {
                id: "claude-haiku-4-5-20251001".into(),
                display_name: "Claude Haiku 4.5".into(),
                context_window: 200_000,
                max_output_tokens: 8192,
                tier: ModelTier::Fast,
                is_default: false,
            },
        ],
        "openai" => vec![
            ModelInfo {
                id: "gpt-4o".into(),
                display_name: "GPT-4o".into(),
                context_window: 128_000,
                max_output_tokens: 4096,
                tier: ModelTier::Recommended,
                is_default: true,
            },
            ModelInfo {
                id: "gpt-4o-mini".into(),
                display_name: "GPT-4o Mini".into(),
                context_window: 128_000,
                max_output_tokens: 4096,
                tier: ModelTier::Fast,
                is_default: false,
            },
        ],
        "gemini" => vec![
            ModelInfo {
                id: "gemini-2.5-flash".into(),
                display_name: "Gemini 2.5 Flash".into(),
                context_window: 1_000_000,
                max_output_tokens: 8192,
                tier: ModelTier::Recommended,
                is_default: true,
            },
            ModelInfo {
                id: "gemini-2.5-pro".into(),
                display_name: "Gemini 2.5 Pro".into(),
                context_window: 1_000_000,
                max_output_tokens: 8192,
                tier: ModelTier::Premium,
                is_default: false,
            },
        ],
        _ => vec![],
    }
}

/// Get the default model ID for a provider.
pub fn default_model_for_provider(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "anthropic" => Some("claude-sonnet-4-6"),
        "openai" => Some("gpt-4o"),
        "gemini" => Some("gemini-2.5-flash"),
        _ => None,
    }
}

/// Look up a specific model's info.
pub fn get_model_info(provider_id: &str, model_id: &str) -> Option<ModelInfo> {
    models_for_provider(provider_id)
        .into_iter()
        .find(|m| m.id == model_id)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_models() {
        let models = models_for_provider("anthropic");
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].id, "claude-sonnet-4-6");
        assert!(models[0].is_default);
        assert_eq!(models[1].id, "claude-opus-4-6");
        assert!(!models[1].is_default);
    }

    #[test]
    fn test_openai_models() {
        let models = models_for_provider("openai");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4o");
        assert!(models[0].is_default);
    }

    #[test]
    fn test_gemini_models() {
        let models = models_for_provider("gemini");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gemini-2.5-flash");
        assert!(models[0].is_default);
    }

    #[test]
    fn test_unknown_provider() {
        let models = models_for_provider("unknown");
        assert!(models.is_empty());
    }

    #[test]
    fn test_default_model_for_provider() {
        assert_eq!(default_model_for_provider("anthropic"), Some("claude-sonnet-4-6"));
        assert_eq!(default_model_for_provider("openai"), Some("gpt-4o"));
        assert_eq!(default_model_for_provider("gemini"), Some("gemini-2.5-flash"));
        assert_eq!(default_model_for_provider("unknown"), None);
    }

    #[test]
    fn test_get_model_info_found() {
        let info = get_model_info("anthropic", "claude-opus-4-6").unwrap();
        assert_eq!(info.display_name, "Claude Opus 4.6");
        assert_eq!(info.context_window, 200_000);
        assert_eq!(info.max_output_tokens, 8192);
        assert_eq!(info.tier, ModelTier::Premium);
    }

    #[test]
    fn test_get_model_info_not_found() {
        assert!(get_model_info("anthropic", "nonexistent").is_none());
        assert!(get_model_info("unknown", "gpt-4o").is_none());
    }

    #[test]
    fn test_each_provider_has_exactly_one_default() {
        for provider in &["anthropic", "openai", "gemini"] {
            let models = models_for_provider(provider);
            let defaults: Vec<_> = models.iter().filter(|m| m.is_default).collect();
            assert_eq!(defaults.len(), 1, "{} should have exactly one default model", provider);
        }
    }

    #[test]
    fn test_context_windows_are_reasonable() {
        for provider in &["anthropic", "openai", "gemini"] {
            for model in models_for_provider(provider) {
                assert!(model.context_window >= 100_000, "{} context window too small", model.id);
                assert!(model.max_output_tokens >= 4096, "{} max_output too small", model.id);
            }
        }
    }
}
