//! API key + provider + model selection commands.
//!
//! Includes the legacy single-key Anthropic flow (greet/check_db live in
//! `system`), the multi-provider key store, the provider catalog, and the
//! per-provider model selection.

use crate::db::Database;
use crate::keyring;
use crate::models;
use crate::provider;
use crate::providers;
use crate::settings;

// ---- Legacy API key (single Anthropic key) ----

/// Store the Anthropic API key in macOS Keychain
#[tauri::command]
pub(crate) fn store_api_key(api_key: String) -> Result<(), keyring::KeyringError> {
    keyring::store_api_key(&api_key)
}

/// Check if an API key exists in the Keychain
#[tauri::command]
pub(crate) fn has_api_key() -> bool {
    keyring::has_api_key()
}

/// Delete the API key from the Keychain
#[tauri::command]
pub(crate) fn delete_api_key() -> Result<(), keyring::KeyringError> {
    keyring::delete_api_key()
}

/// Validate an API key format (does not store it)
#[tauri::command]
pub(crate) fn validate_api_key_format(api_key: String) -> bool {
    api_key.starts_with("sk-ant-") && api_key.len() > 20
}

// ---- Multi-provider API key + provider catalog ----

/// Get the active AI provider (default: "anthropic")
#[tauri::command]
pub(crate) async fn get_active_provider(
    state: tauri::State<'_, Database>,
) -> Result<String, settings::SettingsError> {
    let value = settings::get_setting(&state.pool, "active_provider").await?;
    Ok(value.unwrap_or_else(|| "anthropic".to_string()))
}

/// Set the active AI provider (validates provider exists)
#[tauri::command]
pub(crate) async fn set_active_provider(
    state: tauri::State<'_, Database>,
    provider_id: String,
) -> Result<(), String> {
    if providers::get_provider(&provider_id, None).is_none() {
        return Err(format!("Unknown provider: {}", provider_id));
    }
    settings::set_setting(&state.pool, "active_provider", &provider_id)
        .await
        .map_err(|e| e.to_string())
}

/// List all available AI providers
#[tauri::command]
pub(crate) fn list_providers() -> Vec<provider::ProviderInfo> {
    providers::available_providers()
}

/// Validate an API key format for a specific provider
#[tauri::command]
pub(crate) fn validate_provider_api_key_format(provider_id: String, api_key: String) -> Result<bool, String> {
    let p = providers::get_provider(&provider_id, None)
        .ok_or_else(|| format!("Unknown provider: {}", provider_id))?;
    Ok(p.validate_key_format(&api_key))
}

/// Store an API key for a specific provider in Keychain
/// Validates that the provider exists and the key format is correct before storing.
#[tauri::command]
pub(crate) fn store_provider_api_key(
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let p = providers::get_provider(&provider_id, None)
        .ok_or_else(|| format!("Unknown provider: {}", provider_id))?;
    if !p.validate_key_format(&api_key) {
        return Err(format!("Invalid API key format for {}", p.display_name()));
    }
    keyring::store_provider_api_key(&provider_id, &api_key)
        .map_err(|e| e.to_string())
}

/// Check if an API key exists for a specific provider
#[tauri::command]
pub(crate) fn has_provider_api_key(provider_id: String) -> Result<bool, String> {
    providers::get_provider(&provider_id, None)
        .ok_or_else(|| format!("Unknown provider: {}", provider_id))?;
    Ok(keyring::has_provider_api_key(&provider_id))
}

/// Delete the API key for a specific provider from Keychain
#[tauri::command]
pub(crate) fn delete_provider_api_key(provider_id: String) -> Result<(), String> {
    providers::get_provider(&provider_id, None)
        .ok_or_else(|| format!("Unknown provider: {}", provider_id))?;
    keyring::delete_provider_api_key(&provider_id).map_err(|e| e.to_string())
}

/// Check if ANY provider has an API key stored (for onboarding completion)
#[tauri::command]
pub(crate) fn has_any_provider_api_key() -> bool {
    ["anthropic", "openai", "gemini"]
        .iter()
        .any(|id| keyring::has_provider_api_key(id))
}

// ---- Per-provider model selection ----

/// Get available models for a provider
#[tauri::command]
pub(crate) fn get_models_for_provider(provider_id: String) -> Vec<models::ModelInfo> {
    models::models_for_provider(&provider_id)
}

/// Get the active model for a provider (reads from settings)
#[tauri::command]
pub(crate) async fn get_active_model(
    state: tauri::State<'_, Database>,
    provider_id: String,
) -> Result<Option<String>, String> {
    let key = format!("active_model_{}", provider_id);
    settings::get_setting(&state.pool, &key)
        .await
        .map_err(|e| e.to_string())
}

/// Set the active model for a provider (validates against catalog)
#[tauri::command]
pub(crate) async fn set_active_model(
    state: tauri::State<'_, Database>,
    provider_id: String,
    model_id: String,
) -> Result<(), String> {
    // Validate that the model exists in the catalog
    if models::get_model_info(&provider_id, &model_id).is_none() {
        return Err(format!("Unknown model '{}' for provider '{}'", model_id, provider_id));
    }
    let key = format!("active_model_{}", provider_id);
    settings::set_setting(&state.pool, &key, &model_id)
        .await
        .map_err(|e| e.to_string())
}
