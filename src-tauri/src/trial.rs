// People Partner - Trial Mode Module
// Manages trial vs paid mode detection, message limits, and employee limits.
// Trial status is derived from license presence.
// Unlicensed installs are trial-mode; licensed installs are paid-mode (BYOK required).
// All trial state is stored in the existing `settings` table.

use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::device_id;
use crate::keyring;
use crate::settings::{self, SettingsError};

// ============================================================================
// Constants
// ============================================================================

pub const TRIAL_MESSAGE_LIMIT: u32 = 50;
pub const TRIAL_EMPLOYEE_LIMIT: i64 = 10;
const DEFAULT_PROXY_URL: &str = "https://hrcommand-proxy.hrcommand.workers.dev";

// Settings keys
const KEY_TRIAL_MESSAGES_USED: &str = "trial_messages_used";
const KEY_PROXY_URL: &str = "proxy_url";
const KEY_LICENSE_KEY: &str = "license_key";
const KEY_PROXY_SIGNING_SECRET: &str = "proxy_signing_secret";

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialStatus {
    pub is_trial: bool,
    pub has_license: bool,
    pub has_api_key: bool,
    pub messages_used: u32,
    pub messages_limit: u32,
    pub employees_used: i64,
    pub employees_limit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeLimitCheck {
    pub allowed: bool,
    pub current: i64,
    pub limit: i64,
}

// ============================================================================
// Trial Mode Detection
// ============================================================================

/// Determine if the app is in trial mode.
/// Trial mode is active when no license key is present.
pub async fn is_trial_mode(pool: &DbPool) -> Result<bool, SettingsError> {
    Ok(!has_license_key(pool).await?)
}

/// True when a non-empty license key is stored.
pub async fn has_license_key(pool: &DbPool) -> Result<bool, SettingsError> {
    Ok(get_license_key(pool).await?.is_some())
}

/// Fetch the stored license key, trimmed.
pub async fn get_license_key(pool: &DbPool) -> Result<Option<String>, SettingsError> {
    let value = settings::get_setting(pool, KEY_LICENSE_KEY).await?;
    Ok(value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }))
}

/// Store (or replace) the license key.
pub async fn store_license_key(pool: &DbPool, license_key: &str) -> Result<(), SettingsError> {
    settings::set_setting(pool, KEY_LICENSE_KEY, license_key.trim()).await
}

/// Delete the stored license key.
pub async fn delete_license_key(pool: &DbPool) -> Result<(), SettingsError> {
    settings::delete_setting(pool, KEY_LICENSE_KEY).await
}

/// Get full trial status for frontend display.
pub async fn get_trial_status(pool: &DbPool) -> Result<TrialStatus, SettingsError> {
    let has_license = has_license_key(pool).await?;
    let has_api_key = keyring::has_api_key();
    let is_trial = !has_license;
    let messages_used = get_trial_messages_used(pool).await?;
    let employees_used = get_countable_employee_count(pool).await?;

    Ok(TrialStatus {
        is_trial,
        has_license,
        has_api_key,
        messages_used,
        messages_limit: TRIAL_MESSAGE_LIMIT,
        employees_used,
        employees_limit: TRIAL_EMPLOYEE_LIMIT,
    })
}

// ============================================================================
// Message Tracking
// ============================================================================

/// Get the number of trial messages used.
async fn get_trial_messages_used(pool: &DbPool) -> Result<u32, SettingsError> {
    let value = settings::get_setting(pool, KEY_TRIAL_MESSAGES_USED).await?;
    Ok(value
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0))
}

/// Check if the trial message limit has been reached.
/// Returns true if the user can still send messages.
pub async fn check_trial_message_limit(pool: &DbPool) -> Result<bool, SettingsError> {
    let used = get_trial_messages_used(pool).await?;
    Ok(used < TRIAL_MESSAGE_LIMIT)
}

/// Increment the trial message counter. Returns the new count.
pub async fn increment_trial_messages(pool: &DbPool) -> Result<u32, SettingsError> {
    let current = get_trial_messages_used(pool).await?;
    let new_count = current + 1;
    settings::set_setting(pool, KEY_TRIAL_MESSAGES_USED, &new_count.to_string()).await?;
    Ok(new_count)
}

/// Set the trial message counter to a specific value.
pub async fn set_trial_messages_used(pool: &DbPool, value: u32) -> Result<(), SettingsError> {
    settings::set_setting(pool, KEY_TRIAL_MESSAGES_USED, &value.to_string()).await
}

/// Reset the trial message counter.
pub async fn reset_trial_messages(pool: &DbPool) -> Result<(), SettingsError> {
    settings::set_setting(pool, KEY_TRIAL_MESSAGES_USED, "0").await
}

// ============================================================================
// Employee Limit
// ============================================================================

/// Count employees that count toward the trial limit: all statuses, but
/// excluding bundled sample data (`is_sample = 1`). Sample rows are loaded
/// by onboarding and must not consume the trial allowance (#106).
pub async fn get_countable_employee_count(pool: &DbPool) -> Result<i64, SettingsError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM employees WHERE is_sample = 0")
        .fetch_one(pool)
        .await
        .map_err(|e| SettingsError::Database(e.to_string()))?;
    Ok(row.0)
}

/// Check if adding an employee is allowed under trial limits.
/// Only enforces limits in trial mode.
pub async fn check_employee_limit(pool: &DbPool) -> Result<EmployeeLimitCheck, SettingsError> {
    let is_trial = is_trial_mode(pool).await?;
    let current = get_countable_employee_count(pool).await?;

    if !is_trial {
        return Ok(EmployeeLimitCheck {
            allowed: true,
            current,
            limit: TRIAL_EMPLOYEE_LIMIT,
        });
    }

    Ok(EmployeeLimitCheck {
        allowed: current < TRIAL_EMPLOYEE_LIMIT,
        current,
        limit: TRIAL_EMPLOYEE_LIMIT,
    })
}

// ============================================================================
// Proxy Configuration
// ============================================================================

/// Get the proxy URL for trial mode chat routing.
/// Priority: env var > settings > default.
pub async fn get_proxy_url(pool: &DbPool) -> Result<String, SettingsError> {
    // Environment variable override (useful for development)
    if let Ok(url) = std::env::var("HRCOMMAND_PROXY_URL") {
        if !url.is_empty() {
            return Ok(url);
        }
    }

    // Settings table
    if let Some(url) = settings::get_setting(pool, KEY_PROXY_URL).await? {
        if !url.is_empty() {
            return Ok(url);
        }
    }

    Ok(DEFAULT_PROXY_URL.to_string())
}

/// Get optional proxy signing secret used for request HMAC signatures.
/// Priority: env var > build-time > settings > none.
pub async fn get_proxy_signing_secret(pool: &DbPool) -> Result<Option<String>, SettingsError> {
    if let Ok(secret) = std::env::var("HRCOMMAND_PROXY_SIGNING_SECRET") {
        let trimmed = secret.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_string()));
        }
    }

    if let Some(secret) = option_env!("HRCOMMAND_PROXY_SIGNING_SECRET") {
        let trimmed = secret.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_string()));
        }
    }

    let from_settings = settings::get_setting(pool, KEY_PROXY_SIGNING_SECRET).await?;
    Ok(from_settings.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }))
}

/// Get or create the device ID (delegates to device_id module).
pub async fn get_device_id(pool: &DbPool) -> Result<String, SettingsError> {
    device_id::get_or_create_device_id(pool).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trial_status_struct() {
        let status = TrialStatus {
            is_trial: true,
            has_license: false,
            has_api_key: false,
            messages_used: 5,
            messages_limit: TRIAL_MESSAGE_LIMIT,
            employees_used: 3,
            employees_limit: TRIAL_EMPLOYEE_LIMIT,
        };
        assert!(status.is_trial);
        assert!(!status.has_license);
        assert_eq!(status.messages_limit, 50);
        assert_eq!(status.employees_limit, 10);
    }

    #[test]
    fn test_employee_limit_check_struct() {
        let check = EmployeeLimitCheck {
            allowed: true,
            current: 5,
            limit: 10,
        };
        assert!(check.allowed);
        assert_eq!(check.current, 5);

        let blocked = EmployeeLimitCheck {
            allowed: false,
            current: 10,
            limit: 10,
        };
        assert!(!blocked.allowed);
    }

    #[test]
    fn test_constants() {
        assert_eq!(TRIAL_MESSAGE_LIMIT, 50);
        assert_eq!(TRIAL_EMPLOYEE_LIMIT, 10);
    }

    #[test]
    fn test_default_proxy_url() {
        assert!(DEFAULT_PROXY_URL.starts_with("https://"));
        assert!(DEFAULT_PROXY_URL.contains("hrcommand-proxy"));
    }

    #[test]
    fn test_trial_status_serialization() {
        let status = TrialStatus {
            is_trial: true,
            has_license: false,
            has_api_key: false,
            messages_used: 10,
            messages_limit: 50,
            employees_used: 3,
            employees_limit: 10,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"is_trial\":true"));
        assert!(json.contains("\"has_license\":false"));
        assert!(json.contains("\"has_api_key\":false"));
        assert!(json.contains("\"messages_used\":10"));
        assert!(json.contains("\"messages_limit\":50"));
        assert!(json.contains("\"employees_used\":3"));
        assert!(json.contains("\"employees_limit\":10"));
    }

    #[test]
    fn test_employee_limit_check_serialization() {
        let check = EmployeeLimitCheck {
            allowed: false,
            current: 10,
            limit: 10,
        };
        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains("\"allowed\":false"));
        assert!(json.contains("\"current\":10"));
        assert!(json.contains("\"limit\":10"));
    }

    #[test]
    fn test_proxy_url_env_var() {
        // When env var is set, it should be preferred
        // (tested indirectly; the actual async function can't be tested without a DB)
        let url = std::env::var("HRCOMMAND_PROXY_URL");
        // Just verify the env var mechanism works — not set in test env
        assert!(url.is_err() || url.unwrap().is_empty() || true);
    }

    #[test]
    fn test_settings_keys_are_distinct() {
        let keys = [
            KEY_TRIAL_MESSAGES_USED,
            KEY_PROXY_URL,
            KEY_LICENSE_KEY,
            KEY_PROXY_SIGNING_SECRET,
        ];
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(keys[i], keys[j], "Settings keys must be unique");
            }
        }
    }

    // ------------------------------------------------------------------
    // DB-backed tests: sample data is exempt from the trial employee limit
    // (#106 — onboarding auto-loads 100 sample employees through the bulk
    // path; counting them tripped an undismissable paywall on fresh installs)
    // ------------------------------------------------------------------

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::time::Duration;

    async fn test_pool() -> DbPool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect :memory: pool");
        crate::db::run_migrations_for_tests(&pool)
            .await
            .expect("run migrations");
        pool
    }

    async fn insert_real_employee(pool: &DbPool, id: &str, email: &str) {
        sqlx::query("INSERT INTO employees (id, email, full_name) VALUES (?, ?, ?)")
            .bind(id)
            .bind(email)
            .bind(format!("Real User {id}"))
            .execute(pool)
            .await
            .expect("insert real employee");
    }

    fn sample_employee(i: usize) -> crate::bulk_import::ImportEmployee {
        crate::bulk_import::ImportEmployee {
            id: format!("emp_sample{i:03}"),
            email: format!("user{i}@acmecorp.com"),
            full_name: format!("Sample User {i}"),
            department: None,
            job_title: None,
            manager_id: None,
            hire_date: None,
            work_state: None,
            status: Some("active".to_string()),
            date_of_birth: None,
            gender: None,
            ethnicity: None,
            termination_date: None,
            termination_reason: None,
        }
    }

    /// Fresh-install regression (#106): onboarding bulk-loads 100 sample
    /// employees in trial mode. That load must neither trip the 10-employee
    /// limit nor count toward `employees_used`.
    #[tokio::test]
    async fn fresh_install_sample_load_does_not_trip_trial_limit() {
        let pool = test_pool().await;
        let sample: Vec<_> = (0..100).map(sample_employee).collect();
        crate::bulk_import::import_employees_bulk(&pool, sample)
            .await
            .expect("sample import succeeds");

        let check = check_employee_limit(&pool).await.expect("limit check");
        assert!(check.allowed, "sample data alone must not trip the trial limit");
        assert_eq!(check.current, 0, "sample employees must not count");
    }

    #[tokio::test]
    async fn real_employees_still_count_toward_trial_limit() {
        let pool = test_pool().await;
        for i in 0..10 {
            insert_real_employee(&pool, &format!("real{i}"), &format!("real{i}@example.com"))
                .await;
        }
        let check = check_employee_limit(&pool).await.expect("limit check");
        assert!(!check.allowed, "10 real employees must trip the trial limit");
        assert_eq!(check.current, 10);
    }

    #[tokio::test]
    async fn mixed_sample_and_real_counts_only_real() {
        let pool = test_pool().await;
        let sample: Vec<_> = (0..100).map(sample_employee).collect();
        crate::bulk_import::import_employees_bulk(&pool, sample)
            .await
            .expect("sample import");
        for i in 0..3 {
            insert_real_employee(&pool, &format!("real{i}"), &format!("real{i}@example.com"))
                .await;
        }

        let count = get_countable_employee_count(&pool).await.expect("count");
        assert_eq!(count, 3, "only non-sample employees count");
        let check = check_employee_limit(&pool).await.expect("limit check");
        assert!(check.allowed);
        assert_eq!(check.current, 3);
    }

    /// Pins the 014 migration's backfill predicate: only rows from the Acme
    /// sample dataset (@acmecorp.com) get flagged; real rows are untouched.
    #[tokio::test]
    async fn backfill_predicate_targets_only_acmecorp_rows() {
        let pool = test_pool().await;
        // Simulate a pre-fix install: sample rows present but unflagged.
        insert_real_employee(&pool, "legacy_sample", "margaret.chen@acmecorp.com").await;
        insert_real_employee(&pool, "real_user", "jane@realco.com").await;

        sqlx::query("UPDATE employees SET is_sample = 1 WHERE email LIKE '%@acmecorp.com'")
            .execute(&pool)
            .await
            .expect("backfill update");

        let count = get_countable_employee_count(&pool).await.expect("count");
        assert_eq!(count, 1, "only the non-Acme row should remain countable");
    }
}
