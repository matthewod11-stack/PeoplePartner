// People Partner - License Validation Cache
// Caches server-side license validation results to support offline grace periods.
// A license key must be validated at least once online before offline use is permitted.
// After successful validation, the app works offline for up to GRACE_PERIOD_DAYS.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::DbPool;

// ============================================================================
// Constants
// ============================================================================

/// Number of days a cached validation remains valid without re-checking the server.
pub const GRACE_PERIOD_DAYS: i64 = 30;

/// Server confirmed the license is valid.
pub const STATUS_VALID: &str = "VALID";

/// License was revoked server-side (refund/dispute).
pub const STATUS_REVOKED: &str = "REVOKED";

/// License was not found or is invalid.
pub const STATUS_INVALID: &str = "INVALID";

/// Seat limit exceeded.
pub const STATUS_SEAT_LIMIT: &str = "SEAT_LIMIT_EXCEEDED";

/// Legacy key found on upgrade — assumed valid until next server check.
pub const STATUS_LEGACY: &str = "LEGACY_ASSUMED_VALID";

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CachedValidation {
    pub license_key: String,
    pub device_id: String,
    pub validated_at: String,
    pub server_status: String,
}

// ============================================================================
// Grace Period Logic
// ============================================================================

/// Check if a validated_at timestamp (ISO 8601 / RFC 3339) is within the grace period.
pub fn is_within_grace_period(validated_at: &str) -> bool {
    let parsed = match DateTime::parse_from_rfc3339(validated_at) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => return false,
    };
    let age = Utc::now() - parsed;
    age.num_days() <= GRACE_PERIOD_DAYS
}

/// Calculate how many days remain in the grace period. Returns 0 if expired.
pub fn days_remaining(validated_at: &str) -> i64 {
    let parsed = match DateTime::parse_from_rfc3339(validated_at) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => return 0,
    };
    let age = Utc::now() - parsed;
    let remaining = GRACE_PERIOD_DAYS - age.num_days();
    remaining.max(0)
}

/// Whether a cached status should be treated as valid (eligible for grace period).
pub fn is_valid_status(status: &str) -> bool {
    status == STATUS_VALID || status == STATUS_LEGACY
}

// ============================================================================
// Database Operations
// ============================================================================

/// Cache a validation result (upsert).
pub async fn cache_validation(
    pool: &DbPool,
    license_key: &str,
    device_id: &str,
    status: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO license_validation_cache (license_key, device_id, validated_at, server_status, created_at, updated_at)
        VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))
        ON CONFLICT(license_key) DO UPDATE SET
            device_id = excluded.device_id,
            validated_at = excluded.validated_at,
            server_status = excluded.server_status,
            updated_at = datetime('now')
        "#,
    )
    .bind(license_key)
    .bind(device_id)
    .bind(&now)
    .bind(status)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get the cached validation for a license key.
pub async fn get_cached_validation(
    pool: &DbPool,
    license_key: &str,
) -> Result<Option<CachedValidation>, String> {
    let result: Option<CachedValidation> = sqlx::query_as(
        "SELECT license_key, device_id, validated_at, server_status FROM license_validation_cache WHERE license_key = ?",
    )
    .bind(license_key)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Clear the cached validation for a license key.
pub async fn clear_cache(pool: &DbPool, license_key: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM license_validation_cache WHERE license_key = ?")
        .bind(license_key)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn timestamp_days_ago(days: i64) -> String {
        (Utc::now() - chrono::Duration::days(days)).to_rfc3339()
    }

    #[test]
    fn test_grace_period_within_bounds() {
        let ts = timestamp_days_ago(29);
        assert!(is_within_grace_period(&ts));
    }

    #[test]
    fn test_grace_period_expired() {
        let ts = timestamp_days_ago(31);
        assert!(!is_within_grace_period(&ts));
    }

    #[test]
    fn test_grace_period_boundary_day_30() {
        // Day 30 should still be within the grace period (<=30).
        let ts = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        assert!(is_within_grace_period(&ts));
    }

    #[test]
    fn test_grace_period_fresh() {
        let ts = Utc::now().to_rfc3339();
        assert!(is_within_grace_period(&ts));
    }

    #[test]
    fn test_grace_period_invalid_timestamp() {
        assert!(!is_within_grace_period("not-a-date"));
        assert!(!is_within_grace_period(""));
    }

    #[test]
    fn test_days_remaining_fresh() {
        let ts = Utc::now().to_rfc3339();
        assert_eq!(days_remaining(&ts), GRACE_PERIOD_DAYS);
    }

    #[test]
    fn test_days_remaining_halfway() {
        let ts = timestamp_days_ago(15);
        assert_eq!(days_remaining(&ts), 15);
    }

    #[test]
    fn test_days_remaining_expired() {
        let ts = timestamp_days_ago(60);
        assert_eq!(days_remaining(&ts), 0);
    }

    #[test]
    fn test_valid_status_accepts_valid() {
        assert!(is_valid_status(STATUS_VALID));
    }

    #[test]
    fn test_valid_status_accepts_legacy() {
        assert!(is_valid_status(STATUS_LEGACY));
    }

    #[test]
    fn test_valid_status_rejects_revoked() {
        assert!(!is_valid_status(STATUS_REVOKED));
    }

    #[test]
    fn test_valid_status_rejects_invalid() {
        assert!(!is_valid_status(STATUS_INVALID));
    }

    #[test]
    fn test_valid_status_rejects_seat_limit() {
        assert!(!is_valid_status(STATUS_SEAT_LIMIT));
    }
}
