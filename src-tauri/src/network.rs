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

    /// Whether the Anthropic API is specifically reachable
    pub api_reachable: bool,

    /// Optional error message if offline
    pub error_message: Option<String>,
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

/// Check if the Anthropic API is reachable
///
/// This performs a lightweight HTTP request to the Anthropic API
/// with a short timeout to quickly determine network availability.
///
/// Returns a NetworkStatus struct with connectivity details.
pub async fn check_network() -> NetworkStatus {
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

    // Try to reach the Anthropic API
    // We use HEAD to api.anthropic.com to minimize data transfer
    // The API will return a 4xx without auth, but that confirms reachability
    let result = client
        .head("https://api.anthropic.com/v1/messages")
        .send()
        .await;

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
pub async fn is_online() -> bool {
    check_network().await.is_online
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let status = check_network().await;
        // If you're running this test with network, it should pass
        assert!(status.is_online);
        assert!(status.api_reachable);
    }
}
