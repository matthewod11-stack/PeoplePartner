//! Tauri command bridge for the `recruiting` module.

use crate::db::Database;
use crate::keyring::{self, KeyringError};
use crate::recruiting::adapters::exa::{self, ExaError, ExaSearchResponse};
use crate::recruiting::{self, RecruitingSearch, EXA_PROVIDER_ID};
use serde::Serialize;

/// Create a new recruiting search. Returns the generated row ID.
#[tauri::command]
pub(crate) async fn recruiting_create_search(
    state: tauri::State<'_, Database>,
    query: String,
    seed_employee_id: Option<String>,
) -> Result<String, String> {
    recruiting::create_search(&state.pool, &query, seed_employee_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// List all recruiting searches, newest first.
#[tauri::command]
pub(crate) async fn recruiting_list_searches(
    state: tauri::State<'_, Database>,
) -> Result<Vec<RecruitingSearch>, String> {
    recruiting::list_searches(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Discriminated error type for `recruiting_search_exa`. The frontend
/// pattern-matches on `kind`:
///   - `MissingKey`  → render the "Recruiting needs your Exa API key" banner.
///   - `InvalidKey`  → same banner; suffix that Exa rejected the stored key.
///   - `RateLimit`   → soft toast with the Exa-supplied message.
///   - `Network`     → soft toast ("couldn't reach Exa").
///   - `ExaApi`      → surface status + body inline (debugging-friendly).
///   - `Internal`    → unexpected path: Keychain read failure, response
///                     parse failure, etc.
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum RecruitingSearchError {
    MissingKey,
    InvalidKey,
    RateLimit { message: String },
    Network { message: String },
    ExaApi { status: u16, body: String },
    Internal { message: String },
}

impl From<ExaError> for RecruitingSearchError {
    fn from(err: ExaError) -> Self {
        match err {
            ExaError::Network(e) => RecruitingSearchError::Network {
                message: e.to_string(),
            },
            ExaError::InvalidKey => RecruitingSearchError::InvalidKey,
            ExaError::RateLimit { message } => RecruitingSearchError::RateLimit { message },
            ExaError::Api { status, body } => RecruitingSearchError::ExaApi { status, body },
            ExaError::InvalidResponse(message) => RecruitingSearchError::Internal { message },
        }
    }
}

// ============================================================================
// Exa API key management — Recruiting-namespaced wrappers
// ============================================================================
//
// These deliberately bypass `commands::api_keys::*_provider_api_key` because
// those commands gate on the LLM-provider registry (`providers::get_provider`)
// — Exa is a data source, not an LLM, so it isn't (and shouldn't be) registered
// there. The gate is a real safety net for the LLM-key path (catches typos
// like "anthropic-key"), so punching a hole in it would be wrong; better to
// keep the LLM and data-source key surfaces independent.
//
// Storage is shared with the LLM keys (same Keychain service, account
// `exa_api_key` per `keychain_account_for_provider("exa")`), so a key written
// here is readable by `recruiting_search_exa` and vice-versa.

/// Check whether an Exa API key is stored in the Keychain.
#[tauri::command]
pub(crate) fn recruiting_has_exa_key() -> Result<bool, String> {
    Ok(keyring::has_provider_api_key(EXA_PROVIDER_ID))
}

/// Store the Exa API key in the Keychain. Format validation lives client-side
/// (UUID regex in `ExaKeyInput.tsx`); the Rust side is intentionally permissive
/// to keep the recruiting command surface small. If Exa changes its key format,
/// nothing here needs to change.
#[tauri::command]
pub(crate) fn recruiting_store_exa_key(api_key: String) -> Result<(), String> {
    keyring::store_provider_api_key(EXA_PROVIDER_ID, &api_key).map_err(|e| e.to_string())
}

/// Delete the Exa API key from the Keychain. Idempotent — returns Ok even
/// when no key was stored (matches `keyring::delete_provider_api_key`).
#[tauri::command]
pub(crate) fn recruiting_delete_exa_key() -> Result<(), String> {
    keyring::delete_provider_api_key(EXA_PROVIDER_ID).map_err(|e| e.to_string())
}

/// Execute an Exa search using the user's BYOK key from Keychain.
///
/// Returns `MissingKey` when no key is stored — the frontend uses this as
/// the signal to render the "add your Exa key in Settings" banner instead
/// of treating it as an error.
#[tauri::command]
pub(crate) async fn recruiting_search_exa(
    query: String,
) -> Result<ExaSearchResponse, RecruitingSearchError> {
    let api_key = match keyring::get_provider_api_key(EXA_PROVIDER_ID) {
        Ok(key) => key,
        Err(KeyringError::NotFound) => return Err(RecruitingSearchError::MissingKey),
        Err(other) => {
            return Err(RecruitingSearchError::Internal {
                message: other.to_string(),
            })
        }
    };

    exa::search(&query, &api_key).await.map_err(Into::into)
}
