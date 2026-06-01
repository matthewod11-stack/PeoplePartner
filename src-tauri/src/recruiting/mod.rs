//! Recruiting (Sourcerer) module — talent sourcing inside People Partner.
//!
//! Skeleton for FHR-71 (S0.2). Owns the `recruiting_searches` row and the
//! create / list operations the first command exposes. Submodules
//! (`adapters`, `intake`, `scoring`, `enrichment`) land in S1+.
//!
//! Architecture:
//!   - Domain helpers (`create_search`, `list_searches`) live here as the
//!     library API.
//!   - Tauri command wrappers live in `crate::commands::recruiting` so the
//!     bridge layer stays separate from the domain layer (mirrors how
//!     `chat`, `employees`, `documents` are organized).
//!   - The migration that backs this module is `013_recruiting.sql`.

pub mod adapters;
pub mod intake;

use crate::db::DbPool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Keyring provider ID for the Exa search API key. Used by the recruiting
/// search command to look up the user's BYOK Exa key:
/// `keyring::get_provider_api_key(EXA_PROVIDER_ID)`.
pub const EXA_PROVIDER_ID: &str = "exa";

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct RecruitingSearch {
    pub id: String,
    pub query: String,
    pub status: String,
    pub seed_employee_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Create a new recruiting search. Returns the generated row ID.
///
/// `seed_employee_id`, when set, points at an existing employee whose
/// profile seeds context-aware discovery ("find people like this person").
/// The FK is `ON DELETE SET NULL` — deleting the seed employee never
/// cascades into recruiting history.
pub async fn create_search(
    pool: &DbPool,
    query: &str,
    seed_employee_id: Option<&str>,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO recruiting_searches (id, query, seed_employee_id) VALUES (?, ?, ?)",
    )
    .bind(&id)
    .bind(query)
    .bind(seed_employee_id)
    .execute(pool)
    .await?;
    Ok(id)
}

/// List all recruiting searches, newest first.
pub async fn list_searches(pool: &DbPool) -> Result<Vec<RecruitingSearch>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, query, status, seed_employee_id, created_at, updated_at \
         FROM recruiting_searches \
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::time::Duration;

    /// In-memory pool with the same connect options as production (minus
    /// WAL, which `:memory:` doesn't support) + full migration suite.
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
            .expect("connect to :memory: pool");

        crate::db::run_migrations_for_tests(&pool)
            .await
            .expect("run migrations");
        pool
    }

    #[tokio::test]
    async fn create_and_list_round_trip() {
        let pool = test_pool().await;

        let id = create_search(&pool, "rust engineers in Berlin", None)
            .await
            .expect("create search");

        let rows = list_searches(&pool).await.expect("list searches");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].query, "rust engineers in Berlin");
        assert_eq!(
            rows[0].status, "pending",
            "default status from the migration must be 'pending'"
        );
        assert!(rows[0].seed_employee_id.is_none());
    }

    /// Regression guard: the FK is `ON DELETE SET NULL`, not `ON DELETE
    /// CASCADE`. Deleting the seed employee must never destroy the
    /// recruiting history that referenced them — that's the architectural
    /// HR/recruiting decoupling (DECISIONS.md finding #1).
    #[tokio::test]
    async fn seed_employee_id_is_set_null_on_employee_delete() {
        let pool = test_pool().await;

        sqlx::query(
            "INSERT INTO employees (id, email, full_name) \
             VALUES ('emp-1', 'sarah@example.com', 'Sarah')",
        )
        .execute(&pool)
        .await
        .expect("insert employee");

        let id = create_search(&pool, "find people like Sarah", Some("emp-1"))
            .await
            .expect("create search");

        sqlx::query("DELETE FROM employees WHERE id = 'emp-1'")
            .execute(&pool)
            .await
            .expect("delete employee");

        let rows = list_searches(&pool).await.expect("list searches");
        assert_eq!(rows.len(), 1, "search must survive employee delete");
        assert_eq!(rows[0].id, id);
        assert!(
            rows[0].seed_employee_id.is_none(),
            "FK must be SET NULL on delete; CASCADE would lose recruiting history"
        );
    }
}
