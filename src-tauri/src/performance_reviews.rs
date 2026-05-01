// People Partner - Performance Reviews Module
// CRUD operations for review narratives with FTS search support

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use uuid::Uuid;

use crate::db::DbPool;

#[derive(Error, Debug, Serialize)]
pub enum ReviewError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Review not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Duplicate review: employee already has a review for this cycle")]
    DuplicateReview,
}

impl From<sqlx::Error> for ReviewError {
    fn from(err: sqlx::Error) -> Self {
        let err_str = err.to_string();
        if err_str.contains("UNIQUE constraint failed") {
            ReviewError::DuplicateReview
        } else {
            ReviewError::Database(err_str)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PerformanceReview {
    pub id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub strengths: Option<String>,
    pub areas_for_improvement: Option<String>,
    pub accomplishments: Option<String>,
    pub goals_next_period: Option<String>,
    pub manager_comments: Option<String>,
    pub self_assessment: Option<String>,
    pub reviewer_id: Option<String>,
    pub review_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReview {
    pub employee_id: String,
    pub review_cycle_id: String,
    pub strengths: Option<String>,
    pub areas_for_improvement: Option<String>,
    pub accomplishments: Option<String>,
    pub goals_next_period: Option<String>,
    pub manager_comments: Option<String>,
    pub self_assessment: Option<String>,
    pub reviewer_id: Option<String>,
    pub review_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReview {
    pub strengths: Option<String>,
    pub areas_for_improvement: Option<String>,
    pub accomplishments: Option<String>,
    pub goals_next_period: Option<String>,
    pub manager_comments: Option<String>,
    pub self_assessment: Option<String>,
    pub reviewer_id: Option<String>,
    pub review_date: Option<String>,
}

pub async fn create_review(pool: &DbPool, input: CreateReview, app: AppHandle) -> Result<PerformanceReview, ReviewError> {
    if input.employee_id.trim().is_empty() {
        return Err(ReviewError::Validation("employee_id is required".to_string()));
    }
    if input.review_cycle_id.trim().is_empty() {
        return Err(ReviewError::Validation("review_cycle_id is required".to_string()));
    }

    let id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO performance_reviews (
            id, employee_id, review_cycle_id, strengths, areas_for_improvement,
            accomplishments, goals_next_period, manager_comments, self_assessment,
            reviewer_id, review_date
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.employee_id)
    .bind(&input.review_cycle_id)
    .bind(&input.strengths)
    .bind(&input.areas_for_improvement)
    .bind(&input.accomplishments)
    .bind(&input.goals_next_period)
    .bind(&input.manager_comments)
    .bind(&input.self_assessment)
    .bind(&input.reviewer_id)
    .bind(&input.review_date)
    .execute(pool)
    .await?;

    let review = get_review(pool, &id).await?;

    // Auto-trigger: Extract highlights and regenerate summary in background
    // Emits an event to the frontend if extraction fails
    let pool_clone = pool.clone();
    let review_clone = review.clone();
    tokio::spawn(async move {
        let mut failures: Vec<String> = Vec::new();
        // Extract highlights from review text
        if let Err(e) = crate::highlights::extract_highlights_for_review(&pool_clone, &review_clone).await {
            let msg = format!("Highlight extraction failed for review {}: {}", review_clone.id, e);
            log::warn!("[Auto-extract] {}", msg);
            failures.push(msg);
        }
        // Regenerate employee summary with new highlight
        if let Err(e) = crate::highlights::generate_employee_summary(&pool_clone, &review_clone.employee_id).await {
            let msg = format!("Summary generation failed for employee {}: {}", review_clone.employee_id, e);
            log::warn!("[Auto-summary] {}", msg);
            failures.push(msg);
        }
        if !failures.is_empty() {
            let _ = app.emit("highlight-extraction-error", serde_json::json!({
                "failures": failures,
                "count": failures.len(),
            }));
        }
    });

    Ok(review)
}

pub async fn get_review(pool: &DbPool, id: &str) -> Result<PerformanceReview, ReviewError> {
    sqlx::query_as::<_, PerformanceReview>("SELECT * FROM performance_reviews WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ReviewError::NotFound(id.to_string()))
}

pub async fn get_reviews_for_employee(pool: &DbPool, employee_id: &str) -> Result<Vec<PerformanceReview>, ReviewError> {
    Ok(sqlx::query_as::<_, PerformanceReview>(
        r#"SELECT pr.* FROM performance_reviews pr
           JOIN review_cycles rc ON pr.review_cycle_id = rc.id
           WHERE pr.employee_id = ? ORDER BY rc.start_date DESC"#,
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_reviews_for_cycle(pool: &DbPool, review_cycle_id: &str) -> Result<Vec<PerformanceReview>, ReviewError> {
    Ok(sqlx::query_as::<_, PerformanceReview>(
        "SELECT * FROM performance_reviews WHERE review_cycle_id = ?"
    )
    .bind(review_cycle_id)
    .fetch_all(pool)
    .await?)
}

pub async fn update_review(pool: &DbPool, id: &str, input: UpdateReview) -> Result<PerformanceReview, ReviewError> {
    let existing = get_review(pool, id).await?;

    sqlx::query(
        r#"UPDATE performance_reviews SET
            strengths = ?, areas_for_improvement = ?, accomplishments = ?,
            goals_next_period = ?, manager_comments = ?, self_assessment = ?,
            reviewer_id = ?, review_date = ?, updated_at = datetime('now')
           WHERE id = ?"#,
    )
    .bind(input.strengths.or(existing.strengths))
    .bind(input.areas_for_improvement.or(existing.areas_for_improvement))
    .bind(input.accomplishments.or(existing.accomplishments))
    .bind(input.goals_next_period.or(existing.goals_next_period))
    .bind(input.manager_comments.or(existing.manager_comments))
    .bind(input.self_assessment.or(existing.self_assessment))
    .bind(input.reviewer_id.or(existing.reviewer_id))
    .bind(input.review_date.or(existing.review_date))
    .bind(id)
    .execute(pool)
    .await?;

    get_review(pool, id).await
}

pub async fn delete_review(pool: &DbPool, id: &str) -> Result<(), ReviewError> {
    let result = sqlx::query("DELETE FROM performance_reviews WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ReviewError::NotFound(id.to_string()));
    }
    Ok(())
}

/// Search reviews using FTS (strengths, areas_for_improvement, accomplishments, etc.)
pub async fn search_reviews(pool: &DbPool, query: &str) -> Result<Vec<PerformanceReview>, ReviewError> {
    Ok(sqlx::query_as::<_, PerformanceReview>(
        r#"SELECT pr.* FROM performance_reviews pr
           JOIN performance_reviews_fts fts ON pr.rowid = fts.rowid
           WHERE performance_reviews_fts MATCH ?
           ORDER BY rank"#,
    )
    .bind(query)
    .fetch_all(pool)
    .await?)
}
