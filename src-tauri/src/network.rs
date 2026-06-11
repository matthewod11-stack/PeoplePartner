// People Partner - Network Detection Module
// Provides network connectivity checking for the Tauri backend

use reqwest::Client;
use std::time::Duration;
use thiserror::Error;

/// Error types for network operations (reserved for future use)
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum NetworkError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Connection timeout")]
    Timeout,
}

/// Result of a network status check
#[derive(Debug, Clone, serde::Serialize)]
pub struct NetworkStatus {
    /// Whether the network is available
    pub is_online: bool,

    /// Whether the active mode's chat endpoint is reachable (trial → proxy,
    /// BYOK → the active provider's API; #114)
    pub api_reachable: bool,

    /// Optional error message if offline
    pub error_message: Option<String>,
}

/// Generic-connectivity fallback when the active mode can't be determined.
/// Kept at the pre-#114 target so indeterminate cases contact no new hosts.
const GENERIC_PROBE_URL: &str = "https://api.anthropic.com/v1/messages";

/// Pick the endpoint the offline gate should probe (#114): the one the active
/// mode actually talks to. Probing only api.anthropic.com falsely locked out
/// trial users (who chat via the Cloudflare proxy) and OpenAI/Gemini BYOK
/// customers, while missing real proxy outages. Pure function for testability.
///
/// Any HTTP response from the target — including 401/403/404 — counts as
/// reachable; only connection-level failures mean offline.
pub fn probe_target(has_license: bool, provider_id: &str, proxy_url: Option<&str>) -> String {
    if !has_license {
        // Trial chats go through the proxy regardless of provider settings.
        if let Some(url) = proxy_url {
            return url.trim_end_matches('/').to_string();
        }
        return GENERIC_PROBE_URL.to_string();
    }
    match provider_id {
        "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
        "gemini" => "https://generativelanguage.googleapis.com/v1beta".to_string(),
        "anthropic" => "https://api.anthropic.com/v1/messages".to_string(),
        _ => GENERIC_PROBE_URL.to_string(),
    }
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self {
            is_online: false,
            api_reachable: false,
            error_message: None,
        }
    }
}

/// Check if the active mode's chat endpoint is reachable (#114)
///
/// Resolves what this install actually talks to — the trial proxy when
/// unlicensed, the active provider's API otherwise — and performs a
/// lightweight HEAD request with a short timeout. Resolution failures
/// degrade to the generic probe rather than reporting offline.
///
/// Returns a NetworkStatus struct with connectivity details.
pub async fn check_network(pool: &crate::db::DbPool) -> NetworkStatus {
    let has_license = crate::trial::has_license_key(pool).await.unwrap_or(false);
    let provider_id = crate::chat::resolve_active_provider(pool)
        .await
        .map(|active| active.provider_id)
        .unwrap_or_else(|_| "anthropic".to_string());
    let proxy_url = if has_license {
        None
    } else {
        crate::trial::get_proxy_url(pool).await.ok()
    };
    let target = probe_target(has_license, &provider_id, proxy_url.as_deref());

    // Create a client with a short timeout for quick checks
    let client = match Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            return NetworkStatus {
                is_online: false,
                api_reachable: false,
                error_message: Some(format!("Failed to create HTTP client: {}", e)),
            };
        }
    };

    // HEAD to minimize data transfer. The endpoint will return a 4xx without
    // auth (or 403 invalid_origin from the proxy), but that confirms reachability.
    let result = client.head(&target).send().await;

    match result {
        Ok(_response) => {
            // Any response (even 401/403) means the API is reachable
            // This is expected without proper authentication headers
            NetworkStatus {
                is_online: true,
                api_reachable: true,
                error_message: None,
            }
        }
        Err(e) => {
            // Determine if this is a timeout or other network error
            let error_msg = if e.is_timeout() {
                "Connection timeout - check your internet connection".to_string()
            } else if e.is_connect() {
                "Unable to connect - network may be unavailable".to_string()
            } else {
                format!("Network error: {}", e)
            };

            NetworkStatus {
                is_online: false,
                api_reachable: false,
                error_message: Some(error_msg),
            }
        }
    }
}

/// Quick check that returns just a boolean for simple use cases
pub async fn is_online(pool: &crate::db::DbPool) -> bool {
    check_network(pool).await.is_online
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- probe-target selection (#114) -----

    #[test]
    fn trial_mode_probes_the_proxy() {
        let target = probe_target(false, "anthropic", Some("https://proxy.example.workers.dev/"));
        assert_eq!(target, "https://proxy.example.workers.dev");
    }

    #[test]
    fn trial_mode_without_proxy_url_falls_back_to_generic() {
        let target = probe_target(false, "anthropic", None);
        assert_eq!(target, GENERIC_PROBE_URL);
    }

    #[test]
    fn licensed_anthropic_probes_anthropic() {
        let target = probe_target(true, "anthropic", None);
        assert!(target.contains("api.anthropic.com"), "{target}");
    }

    #[test]
    fn licensed_openai_probes_openai() {
        let target = probe_target(true, "openai", None);
        assert!(target.contains("api.openai.com"), "{target}");
    }

    #[test]
    fn licensed_gemini_probes_gemini() {
        let target = probe_target(true, "gemini", None);
        assert!(target.contains("generativelanguage.googleapis.com"), "{target}");
    }

    #[test]
    fn licensed_unknown_provider_falls_back_to_generic() {
        let target = probe_target(true, "mystery-llm", None);
        assert_eq!(target, GENERIC_PROBE_URL);
    }

    #[test]
    fn licensed_user_ignores_proxy_url() {
        // A licensed OpenAI user must not be gated on proxy reachability.
        let target = probe_target(true, "openai", Some("https://proxy.example.workers.dev"));
        assert!(target.contains("api.openai.com"), "{target}");
    }

    #[tokio::test]
    async fn test_network_status_default() {
        let status = NetworkStatus::default();
        assert!(!status.is_online);
        assert!(!status.api_reachable);
        assert!(status.error_message.is_none());
    }

    // Note: Network tests are integration tests and may fail without internet
    // They are included here for documentation purposes
    #[tokio::test]
    #[ignore] // Run with --ignored flag when you have network
    async fn test_check_network_when_online() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect :memory: pool");
        crate::db::run_migrations_for_tests(&pool)
            .await
            .expect("run migrations");
        let status = check_network(&pool).await;
        // If you're running this test with network, it should pass
        assert!(status.is_online);
        assert!(status.api_reachable);
    }
}
