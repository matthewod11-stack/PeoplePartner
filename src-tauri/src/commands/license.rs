//! License key validation, storage, and revalidation commands.
//!
//! Includes the private `validate_license_key_remote` helper plus the
//! `cached_token_verifies` re-verification path used by both store and
//! revalidate flows.

use crate::db::Database;
use crate::license_cache;
use crate::license_signing;
use crate::trial;

/// Result of remote license key validation.
#[derive(Debug)]
enum LicenseValidationResult {
    /// `signed_token` is the JWT returned by the server for issue #22.
    /// `None` means the server response didn't include one (pre-signing
    /// deploys, transition mode, or LICENSE_SIGNING_PRIVATE_KEY unset).
    Valid { signed_token: Option<String> },
    Invalid,
    SeatLimitExceeded(String),
}

/// Validate a license key against the remote server.
/// Sends device_id for seat-limit enforcement.
/// Returns the validation result or Err if the server is unreachable (fail-open).
///
/// When the server returns a `signed_token` AND `license_signing::signing_enabled()`
/// is true, the token's signature + license_key / device_id claims are
/// verified before we trust the `valid: true` bit. A tampered or wrong-device
/// token downgrades the result to Invalid, closing the local-proxy forgery
/// vector. When the token is absent or signing is in transition mode, we
/// trust the unsigned `valid` flag for backward compatibility.
async fn validate_license_key_remote(
    license_key: &str,
    device_id: &str,
) -> Result<LicenseValidationResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post("https://peoplepartner.io/api/validate-license")
        .json(&serde_json::json!({
            "license_key": license_key,
            "device_id": device_id,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    #[derive(serde::Deserialize)]
    struct ValidationResponse {
        valid: bool,
        reason: Option<String>,
        message: Option<String>,
        signed_token: Option<String>,
    }

    let body: ValidationResponse = resp.json().await.map_err(|e| e.to_string())?;

    if body.valid {
        // If the server supplied a signed token AND verification is armed,
        // it must verify. A bad signature or claim mismatch means the
        // `valid: true` line can't be trusted — likely a local proxy.
        if let Some(token) = &body.signed_token {
            match license_signing::verify_signed_token(token, license_key, device_id) {
                Ok(Some(_claims)) => {
                    // Verified — treat as Valid and retain the token for caching.
                }
                Ok(None) => {
                    // Transition mode (public key not baked in yet). Accept
                    // but log so we can confirm the rollout is producing
                    // tokens before flipping strict mode on.
                    log::debug!(
                        "license server returned signed_token but verification is disabled (transition mode)"
                    );
                }
                Err(e) => {
                    log::warn!("license signed_token verification failed: {e}");
                    return Ok(LicenseValidationResult::Invalid);
                }
            }
        }
        Ok(LicenseValidationResult::Valid {
            signed_token: body.signed_token,
        })
    } else if body.reason.as_deref() == Some("SEAT_LIMIT_EXCEEDED") {
        let msg = body
            .message
            .unwrap_or_else(|| "This license key has been activated on too many devices.".to_string());
        Ok(LicenseValidationResult::SeatLimitExceeded(msg))
    } else {
        Ok(LicenseValidationResult::Invalid)
    }
}

/// Store a license key after local format validation and remote/cached validation.
/// First-time activation requires an internet connection.
/// After one successful server validation, offline use is allowed for up to 30 days.
#[tauri::command]
pub(crate) async fn store_license_key(
    state: tauri::State<'_, Database>,
    license_key: String,
) -> Result<(), String> {
    let normalized = license_key.trim().to_string();
    if !validate_license_key_format(normalized.clone()) {
        return Err(
            "Invalid license key format. Expected format: PP-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX".to_string(),
        );
    }

    // Fetch device_id for seat-limit enforcement (fall back to empty string on error)
    let device_id = trial::get_device_id(&state.pool)
        .await
        .unwrap_or_default();

    // Attempt remote validation; fall back to cache if server is unreachable
    match validate_license_key_remote(&normalized, &device_id).await {
        Ok(LicenseValidationResult::Valid { signed_token }) => {
            // Cache the successful server validation for offline grace period
            let _ = license_cache::cache_validation(
                &state.pool,
                &normalized,
                &device_id,
                license_cache::STATUS_VALID,
                signed_token.as_deref(),
            )
            .await;
        }
        Ok(LicenseValidationResult::Invalid) => {
            return Err(
                "This license key was not recognized. Please check the key and try again.".to_string(),
            );
        }
        Ok(LicenseValidationResult::SeatLimitExceeded(msg)) => {
            return Err(format!("Seat limit reached: {}", msg));
        }
        Err(_) => {
            // Server unreachable — check for a cached validation.
            // Grace-period acceptance re-verifies the cached signed_token
            // against the local device_id (if signing is armed), so a
            // cache row stolen from another machine fails here.
            match license_cache::get_cached_validation(&state.pool, &normalized).await {
                Ok(Some(cached))
                    if license_cache::is_valid_status(&cached.server_status)
                        && license_cache::is_within_grace_period(&cached.validated_at)
                        && cached_token_verifies(&cached, &normalized, &device_id) =>
                {
                    // Cached validation still fresh AND signature (if any) holds.
                }
                Ok(Some(_)) => {
                    return Err(
                        "License validation has expired. Please connect to the internet to re-verify your license.".to_string(),
                    );
                }
                _ => {
                    // No cache — first-time activation requires internet
                    return Err(
                        "Unable to verify your license key. Please check your internet connection and try again. An internet connection is required for first-time activation.".to_string(),
                    );
                }
            }
        }
    }

    trial::store_license_key(&state.pool, &normalized)
        .await
        .map_err(|e| e.to_string())?;

    // Purchased installs should not keep stale trial usage counts.
    trial::reset_trial_messages(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Result of license revalidation on app launch.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub(crate) enum RevalidationResult {
    /// License confirmed valid by server.
    Valid,
    /// License was revoked or invalid — removed from device.
    Revoked,
    /// Offline but within grace period.
    GracePeriod { days_remaining: i64 },
    /// Grace period expired — license removed.
    Expired,
    /// No license stored (trial mode).
    NoLicense,
}

/// Revalidate the stored license key against the server on app launch.
/// Catches revocations, refreshes the cache, and enforces grace period expiry.
#[tauri::command]
pub(crate) async fn revalidate_license(
    state: tauri::State<'_, Database>,
) -> Result<RevalidationResult, String> {
    // Check if a license key is stored
    let license_key = match trial::get_license_key(&state.pool).await {
        Ok(Some(key)) => key,
        _ => return Ok(RevalidationResult::NoLicense),
    };

    let device_id = trial::get_device_id(&state.pool)
        .await
        .unwrap_or_default();

    // Attempt server validation
    match validate_license_key_remote(&license_key, &device_id).await {
        Ok(LicenseValidationResult::Valid { signed_token }) => {
            // Refresh the cache timestamp
            let _ = license_cache::cache_validation(
                &state.pool,
                &license_key,
                &device_id,
                license_cache::STATUS_VALID,
                signed_token.as_deref(),
            )
            .await;
            Ok(RevalidationResult::Valid)
        }
        Ok(LicenseValidationResult::Invalid) | Ok(LicenseValidationResult::SeatLimitExceeded(_)) => {
            // License revoked or invalid — remove it
            let _ = trial::delete_license_key(&state.pool).await;
            let _ = license_cache::clear_cache(&state.pool, &license_key).await;
            Ok(RevalidationResult::Revoked)
        }
        Err(_) => {
            // Server unreachable — check cached validation. If a signed_token
            // is cached, re-verify it here; a row whose device_id claim
            // doesn't match the local device_id (stolen cache) fails.
            match license_cache::get_cached_validation(&state.pool, &license_key).await {
                Ok(Some(cached)) if license_cache::is_valid_status(&cached.server_status) => {
                    if !cached_token_verifies(&cached, &license_key, &device_id) {
                        let _ = trial::delete_license_key(&state.pool).await;
                        let _ = license_cache::clear_cache(&state.pool, &license_key).await;
                        return Ok(RevalidationResult::Revoked);
                    }
                    if license_cache::is_within_grace_period(&cached.validated_at) {
                        let remaining = license_cache::days_remaining(&cached.validated_at);
                        Ok(RevalidationResult::GracePeriod {
                            days_remaining: remaining,
                        })
                    } else {
                        // Grace period expired — remove license
                        let _ = trial::delete_license_key(&state.pool).await;
                        let _ = license_cache::clear_cache(&state.pool, &license_key).await;
                        Ok(RevalidationResult::Expired)
                    }
                }
                Ok(Some(_)) => {
                    // Cached status is not valid (e.g., revoked) — remove
                    let _ = trial::delete_license_key(&state.pool).await;
                    let _ = license_cache::clear_cache(&state.pool, &license_key).await;
                    Ok(RevalidationResult::Revoked)
                }
                _ => {
                    // No cache but key exists — legacy upgrade (pre-cache customer).
                    // Grant a grace period so they don't lose access while offline.
                    // No signed_token on legacy rows (they predate #22).
                    let _ = license_cache::cache_validation(
                        &state.pool,
                        &license_key,
                        &device_id,
                        license_cache::STATUS_LEGACY,
                        None,
                    )
                    .await;
                    Ok(RevalidationResult::GracePeriod {
                        days_remaining: license_cache::GRACE_PERIOD_DAYS,
                    })
                }
            }
        }
    }
}

/// Re-verify a cached signed_token against the local device_id. Returns true
/// if the cached row is trustworthy on the current machine:
///   - row has no signed_token (pre-#22 cache or transition-mode response), or
///   - signing is disabled (const key unset — transition mode), or
///   - the signed_token validates under the configured public key AND its
///     device_id claim matches `expected_device_id`.
///
/// Returns false only when signing is armed AND the token fails — which
/// covers the stolen-cache attack: a row copied from another machine has
/// the wrong device_id claim and fails verification here.
fn cached_token_verifies(
    cached: &license_cache::CachedValidation,
    expected_license_key: &str,
    expected_device_id: &str,
) -> bool {
    let Some(token) = cached.signed_token.as_deref() else {
        return true;
    };
    match license_signing::verify_signed_token(token, expected_license_key, expected_device_id) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("cached license signed_token failed re-verification: {e}");
            false
        }
    }
}

/// Remove stored license key and clear the validation cache.
#[tauri::command]
pub(crate) async fn delete_license_key(state: tauri::State<'_, Database>) -> Result<(), String> {
    // Clear cache before deleting the key (need the key value for cache lookup)
    if let Ok(Some(key)) = trial::get_license_key(&state.pool).await {
        let _ = license_cache::clear_cache(&state.pool, &key).await;
    }
    trial::delete_license_key(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Check whether a license key is present.
#[tauri::command]
pub(crate) async fn has_license_key(state: tauri::State<'_, Database>) -> Result<bool, String> {
    trial::has_license_key(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Validate license key format without storing it.
/// Expected format: PP-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX where X is hex (0-9, A-F).
#[tauri::command]
pub(crate) fn validate_license_key_format(license_key: String) -> bool {
    let trimmed = license_key.trim();
    // Expected: "PP-" + 6 groups of 4 hex chars separated by dashes = 32 chars
    if trimmed.len() != 32 {
        return false;
    }
    if !trimmed.starts_with("PP-") {
        return false;
    }
    let groups: Vec<&str> = trimmed[3..].split('-').collect();
    if groups.len() != 6 {
        return false;
    }
    groups.iter().all(|g| {
        g.len() == 4 && g.chars().all(|c| c.is_ascii_hexdigit())
    })
}
