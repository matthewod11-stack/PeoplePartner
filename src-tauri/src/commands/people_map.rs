//! People Map commands — prep-brief generation (FHR-109).
//!
//! Guards live here at the command boundary: sample employees are rejected
//! before any provider work (#118 — sample data never uses API credits), and
//! trial users ride the metered proxy path (T3 decision) through a transport
//! that suppresses chat-stream events so brief JSON never leaks into the
//! chat UI. Nothing is persisted — the brief is ephemeral (decision 9); the
//! seam's audit row is the only durable trace.

use async_trait::async_trait;

use crate::audit::EgressAudit;
use crate::chat::{self, ChatError, ChatMessage};
use crate::db::{Database, DbPool};
use crate::people_map::brief::{generate_brief, BriefTransport, SeamTransport};
use crate::people_map::schema::PrepBrief;
use crate::trial;

/// #118 guard: sample-flagged employees never reach a real provider.
async fn ensure_not_sample(pool: &DbPool, employee_id: &str) -> Result<(), String> {
    let is_sample: Option<i64> = sqlx::query_scalar("SELECT is_sample FROM employees WHERE id = ?")
        .bind(employee_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    match is_sample {
        None => Err(format!("Employee not found: {employee_id}")),
        Some(0) => Ok(()),
        Some(_) => Err(
            "Prep briefs are not available for sample employees — sample data never uses API credits."
                .to_string(),
        ),
    }
}

/// Trial transport: the brief prompt goes through the metered proxy path
/// (consumes a trial message; quota synced from proxy headers like the
/// interactive path), with chunk events suppressed. The stream registers in
/// the shared registry, so `cancel_stream` works on briefs too.
struct TrialTransport<'a> {
    app: tauri::AppHandle,
    registry: &'a chat::StreamRegistry,
    stream_id: String,
    proxy_url: String,
    device_id: String,
    signing_secret: Option<String>,
}

#[async_trait]
impl BriefTransport for TrialTransport<'_> {
    async fn complete(
        &self,
        pool: &DbPool,
        audit: EgressAudit,
        messages: Vec<ChatMessage>,
        system_prompt: String,
    ) -> Result<String, ChatError> {
        let result = chat::send_message_streaming_trial(
            self.app.clone(),
            self.registry,
            self.stream_id.clone(),
            pool,
            audit,
            messages,
            Some(system_prompt),
            &self.proxy_url,
            &self.device_id,
            self.signing_secret.as_deref(),
            None,
            None,
            false, // no chat-stream events: brief JSON must not reach the chat UI
        )
        .await;

        match result {
            Ok((usage, full_text)) => {
                // Mirror the interactive path's quota sync.
                if let Some(used) = usage.used {
                    let _ = trial::set_trial_messages_used(pool, used).await;
                } else {
                    let _ = trial::increment_trial_messages(pool).await;
                }
                Ok(full_text)
            }
            Err(ChatError::TrialLimitReached { used, limit }) => {
                if let Some(server_used) = used {
                    let _ = trial::set_trial_messages_used(pool, server_used).await;
                }
                Err(ChatError::TrialLimitReached { used, limit })
            }
            Err(other) => Err(other),
        }
    }
}

/// Generate an ephemeral prep brief for one employee.
///
/// `stream_id` is optional; when the frontend passes one it can cancel an
/// in-flight trial generation via the existing `cancel_stream` command.
#[tauri::command]
pub(crate) async fn people_map_generate_brief(
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    registry: tauri::State<'_, chat::StreamRegistry>,
    employee_id: String,
    stream_id: Option<String>,
) -> Result<PrepBrief, String> {
    ensure_not_sample(&state.pool, &employee_id).await?;

    let has_license = trial::has_license_key(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if has_license {
        generate_brief(&state.pool, &employee_id, &SeamTransport)
            .await
            .map_err(|e| e.to_string())
    } else {
        let proxy_url = trial::get_proxy_url(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
        let device_id = trial::get_device_id(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
        let signing_secret = trial::get_proxy_signing_secret(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
        let transport = TrialTransport {
            app,
            registry: registry.inner(),
            stream_id: stream_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            proxy_url,
            device_id,
            signing_secret,
        };
        generate_brief(&state.pool, &employee_id, &transport)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    async fn insert_employee(pool: &DbPool, id: &str, is_sample: i64) {
        sqlx::query(
            "INSERT INTO employees (id, email, full_name, is_sample) VALUES (?, ?, 'Ada Example', ?)",
        )
        .bind(id)
        .bind(format!("{id}@example.com"))
        .bind(is_sample)
        .execute(pool)
        .await
        .expect("insert employee");
    }

    #[tokio::test]
    async fn sample_employee_is_rejected_before_any_provider_work() {
        let pool = test_pool().await;
        insert_employee(&pool, "emp-sample", 1).await;
        let err = ensure_not_sample(&pool, "emp-sample")
            .await
            .expect_err("sample employee must be rejected");
        assert!(err.contains("sample"), "error names the reason: {err}");
    }

    #[tokio::test]
    async fn real_employee_passes_the_sample_guard() {
        let pool = test_pool().await;
        insert_employee(&pool, "emp-real", 0).await;
        ensure_not_sample(&pool, "emp-real")
            .await
            .expect("real employee passes");
    }

    #[tokio::test]
    async fn unknown_employee_is_a_not_found_error() {
        let pool = test_pool().await;
        let err = ensure_not_sample(&pool, "nope")
            .await
            .expect_err("unknown employee must error");
        assert!(err.contains("not found"), "{err}");
    }
}
