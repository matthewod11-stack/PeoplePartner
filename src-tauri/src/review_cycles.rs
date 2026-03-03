// People Partner - Review Cycles Module
// CRUD operations for performance review cycles

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::db::DbPool;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Serialize)]
pub enum ReviewCycleError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Review cycle not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<sqlx::Error> for ReviewCycleError {
    fn from(err: sqlx::Error) -> Self {
        ReviewCycleError::Database(err.to_string())
    }
}

// ============================================================================
// Review Cycle Struct
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReviewCycle {
    pub id: String,
    pub name: String,
    pub cycle_type: String, // 'annual' | 'semi-annual' | 'quarterly'
    pub start_date: String,
    pub end_date: String,
    pub status: String, // 'active' | 'closed'
    pub created_at: String,
}

// ============================================================================
// Input Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReviewCycle {
    pub name: String,
    pub cycle_type: String,
    pub start_date: String,
    pub end_date: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReviewCycle {
    pub name: Option<String>,
    pub cycle_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: Option<String>,
}

// ============================================================================
// CRUD Operations
// ============================================================================

/// Create a new review cycle
pub async fn create_review_cycle(
    pool: &DbPool,
    input: CreateReviewCycle,
) -> Result<ReviewCycle, ReviewCycleError> {
    // Validate required fields
    if input.name.trim().is_empty() {
        return Err(ReviewCycleError::Validation("Name is required".to_string()));
    }

    // Validate cycle_type
    if !["annual", "semi-annual", "quarterly"].contains(&input.cycle_type.as_str()) {
        return Err(ReviewCycleError::Validation(format!(
            "Invalid cycle_type '{}'. Must be 'annual', 'semi-annual', or 'quarterly'",
            input.cycle_type
        )));
    }

    let id = Uuid::new_v4().to_string();
    let status = input.status.unwrap_or_else(|| "active".to_string());

    // Validate status
    if !["active", "closed"].contains(&status.as_str()) {
        return Err(ReviewCycleError::Validation(format!(
            "Invalid status '{}'. Must be 'active' or 'closed'",
            status
        )));
    }

    sqlx::query(
        r#"
        INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date, status)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.cycle_type)
    .bind(&input.start_date)
    .bind(&input.end_date)
    .bind(&status)
    .execute(pool)
    .await?;

    get_review_cycle(pool, &id).await
}

/// Get a review cycle by ID
pub async fn get_review_cycle(pool: &DbPool, id: &str) -> Result<ReviewCycle, ReviewCycleError> {
    let cycle = sqlx::query_as::<_, ReviewCycle>("SELECT * FROM review_cycles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ReviewCycleError::NotFound(id.to_string()))?;

    Ok(cycle)
}

/// Update a review cycle
pub async fn update_review_cycle(
    pool: &DbPool,
    id: &str,
    input: UpdateReviewCycle,
) -> Result<ReviewCycle, ReviewCycleError> {
    let existing = get_review_cycle(pool, id).await?;

    let name = input.name.unwrap_or(existing.name);
    let cycle_type = input.cycle_type.unwrap_or(existing.cycle_type);
    let start_date = input.start_date.unwrap_or(existing.start_date);
    let end_date = input.end_date.unwrap_or(existing.end_date);
    let status = input.status.unwrap_or(existing.status);

    // Validate cycle_type
    if !["annual", "semi-annual", "quarterly"].contains(&cycle_type.as_str()) {
        return Err(ReviewCycleError::Validation(format!(
            "Invalid cycle_type '{}'. Must be 'annual', 'semi-annual', or 'quarterly'",
            cycle_type
        )));
    }

    // Validate status
    if !["active", "closed"].contains(&status.as_str()) {
        return Err(ReviewCycleError::Validation(format!(
            "Invalid status '{}'. Must be 'active' or 'closed'",
            status
        )));
    }

    sqlx::query(
        r#"
        UPDATE review_cycles SET
            name = ?, cycle_type = ?, start_date = ?, end_date = ?, status = ?
        WHERE id = ?
        "#,
    )
    .bind(&name)
    .bind(&cycle_type)
    .bind(&start_date)
    .bind(&end_date)
    .bind(&status)
    .bind(id)
    .execute(pool)
    .await?;

    get_review_cycle(pool, id).await
}

/// Delete a review cycle
pub async fn delete_review_cycle(pool: &DbPool, id: &str) -> Result<(), ReviewCycleError> {
    let result = sqlx::query("DELETE FROM review_cycles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ReviewCycleError::NotFound(id.to_string()));
    }

    Ok(())
}

/// List all review cycles
pub async fn list_review_cycles(
    pool: &DbPool,
    status_filter: Option<String>,
) -> Result<Vec<ReviewCycle>, ReviewCycleError> {
    let cycles = if let Some(status) = status_filter {
        sqlx::query_as::<_, ReviewCycle>(
            "SELECT * FROM review_cycles WHERE status = ? ORDER BY start_date DESC",
        )
        .bind(status)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, ReviewCycle>(
            "SELECT * FROM review_cycles ORDER BY start_date DESC",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(cycles)
}

/// Get the current active review cycle (most recent by start_date)
pub async fn get_active_review_cycle(pool: &DbPool) -> Result<Option<ReviewCycle>, ReviewCycleError> {
    let cycle = sqlx::query_as::<_, ReviewCycle>(
        "SELECT * FROM review_cycles WHERE status = 'active' ORDER BY start_date DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(cycle)
}

/// Close a review cycle
pub async fn close_review_cycle(pool: &DbPool, id: &str) -> Result<ReviewCycle, ReviewCycleError> {
    update_review_cycle(
        pool,
        id,
        UpdateReviewCycle {
            name: None,
            cycle_type: None,
            start_date: None,
            end_date: None,
            status: Some("closed".to_string()),
        },
    )
    .await
}
