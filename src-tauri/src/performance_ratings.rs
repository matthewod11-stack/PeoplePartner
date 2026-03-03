// People Partner - Performance Ratings Module
// CRUD operations for numeric performance ratings (1.0-5.0 scale)

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::db::DbPool;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Serialize)]
pub enum RatingError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Rating not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Duplicate rating: employee already has a rating for this cycle")]
    DuplicateRating,
}

impl From<sqlx::Error> for RatingError {
    fn from(err: sqlx::Error) -> Self {
        let err_str = err.to_string();
        if err_str.contains("UNIQUE constraint failed") {
            RatingError::DuplicateRating
        } else {
            RatingError::Database(err_str)
        }
    }
}

// ============================================================================
// Performance Rating Struct
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PerformanceRating {
    pub id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub overall_rating: f64,
    pub goals_rating: Option<f64>,
    pub competencies_rating: Option<f64>,
    pub reviewer_id: Option<String>,
    pub rating_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Input Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRating {
    pub employee_id: String,
    pub review_cycle_id: String,
    pub overall_rating: f64,
    pub goals_rating: Option<f64>,
    pub competencies_rating: Option<f64>,
    pub reviewer_id: Option<String>,
    pub rating_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRating {
    pub overall_rating: Option<f64>,
    pub goals_rating: Option<f64>,
    pub competencies_rating: Option<f64>,
    pub reviewer_id: Option<String>,
    pub rating_date: Option<String>,
}

// ============================================================================
// Helper: Validate rating value (1.0 - 5.0)
// ============================================================================

fn validate_rating(value: f64, field_name: &str) -> Result<(), RatingError> {
    if value < 1.0 || value > 5.0 {
        return Err(RatingError::Validation(format!(
            "{} must be between 1.0 and 5.0, got {}",
            field_name, value
        )));
    }
    Ok(())
}

// ============================================================================
// CRUD Operations
// ============================================================================

/// Create a new performance rating
pub async fn create_rating(pool: &DbPool, input: CreateRating) -> Result<PerformanceRating, RatingError> {
    // Validate required fields
    if input.employee_id.trim().is_empty() {
        return Err(RatingError::Validation("employee_id is required".to_string()));
    }
    if input.review_cycle_id.trim().is_empty() {
        return Err(RatingError::Validation("review_cycle_id is required".to_string()));
    }

    // Validate rating values
    validate_rating(input.overall_rating, "overall_rating")?;
    if let Some(goals) = input.goals_rating {
        validate_rating(goals, "goals_rating")?;
    }
    if let Some(comp) = input.competencies_rating {
        validate_rating(comp, "competencies_rating")?;
    }

    let id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO performance_ratings (
            id, employee_id, review_cycle_id, overall_rating,
            goals_rating, competencies_rating, reviewer_id, rating_date
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.employee_id)
    .bind(&input.review_cycle_id)
    .bind(input.overall_rating)
    .bind(input.goals_rating)
    .bind(input.competencies_rating)
    .bind(&input.reviewer_id)
    .bind(&input.rating_date)
    .execute(pool)
    .await?;

    get_rating(pool, &id).await
}

/// Get a rating by ID
pub async fn get_rating(pool: &DbPool, id: &str) -> Result<PerformanceRating, RatingError> {
    let rating = sqlx::query_as::<_, PerformanceRating>(
        "SELECT * FROM performance_ratings WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| RatingError::NotFound(id.to_string()))?;

    Ok(rating)
}

/// Get ratings for an employee
pub async fn get_ratings_for_employee(
    pool: &DbPool,
    employee_id: &str,
) -> Result<Vec<PerformanceRating>, RatingError> {
    let ratings = sqlx::query_as::<_, PerformanceRating>(
        r#"
        SELECT pr.* FROM performance_ratings pr
        JOIN review_cycles rc ON pr.review_cycle_id = rc.id
        WHERE pr.employee_id = ?
        ORDER BY rc.start_date DESC
        "#,
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;

    Ok(ratings)
}

/// Get ratings for a review cycle
pub async fn get_ratings_for_cycle(
    pool: &DbPool,
    review_cycle_id: &str,
) -> Result<Vec<PerformanceRating>, RatingError> {
    let ratings = sqlx::query_as::<_, PerformanceRating>(
        "SELECT * FROM performance_ratings WHERE review_cycle_id = ? ORDER BY overall_rating DESC"
    )
    .bind(review_cycle_id)
    .fetch_all(pool)
    .await?;

    Ok(ratings)
}

/// Get latest rating for an employee
pub async fn get_latest_rating_for_employee(
    pool: &DbPool,
    employee_id: &str,
) -> Result<Option<PerformanceRating>, RatingError> {
    let rating = sqlx::query_as::<_, PerformanceRating>(
        r#"
        SELECT pr.* FROM performance_ratings pr
        JOIN review_cycles rc ON pr.review_cycle_id = rc.id
        WHERE pr.employee_id = ?
        ORDER BY rc.start_date DESC
        LIMIT 1
        "#,
    )
    .bind(employee_id)
    .fetch_optional(pool)
    .await?;

    Ok(rating)
}

/// Update a rating
pub async fn update_rating(
    pool: &DbPool,
    id: &str,
    input: UpdateRating,
) -> Result<PerformanceRating, RatingError> {
    let existing = get_rating(pool, id).await?;

    let overall_rating = input.overall_rating.unwrap_or(existing.overall_rating);
    let goals_rating = input.goals_rating.or(existing.goals_rating);
    let competencies_rating = input.competencies_rating.or(existing.competencies_rating);
    let reviewer_id = input.reviewer_id.or(existing.reviewer_id);
    let rating_date = input.rating_date.or(existing.rating_date);

    // Validate rating values
    validate_rating(overall_rating, "overall_rating")?;
    if let Some(goals) = goals_rating {
        validate_rating(goals, "goals_rating")?;
    }
    if let Some(comp) = competencies_rating {
        validate_rating(comp, "competencies_rating")?;
    }

    sqlx::query(
        r#"
        UPDATE performance_ratings SET
            overall_rating = ?, goals_rating = ?, competencies_rating = ?,
            reviewer_id = ?, rating_date = ?, updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(overall_rating)
    .bind(goals_rating)
    .bind(competencies_rating)
    .bind(&reviewer_id)
    .bind(&rating_date)
    .bind(id)
    .execute(pool)
    .await?;

    get_rating(pool, id).await
}

/// Delete a rating
pub async fn delete_rating(pool: &DbPool, id: &str) -> Result<(), RatingError> {
    let result = sqlx::query("DELETE FROM performance_ratings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RatingError::NotFound(id.to_string()));
    }

    Ok(())
}

/// Get rating distribution for a cycle (for analytics)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingDistribution {
    pub exceptional: i64,    // 5.0
    pub exceeds: i64,        // 4.0-4.9
    pub meets: i64,          // 3.0-3.9
    pub developing: i64,     // 2.0-2.9
    pub unsatisfactory: i64, // 1.0-1.9
    pub total: i64,
}

pub async fn get_rating_distribution(
    pool: &DbPool,
    review_cycle_id: &str,
) -> Result<RatingDistribution, RatingError> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(CASE WHEN overall_rating >= 5.0 THEN 1 END) as exceptional,
            COUNT(CASE WHEN overall_rating >= 4.0 AND overall_rating < 5.0 THEN 1 END) as exceeds,
            COUNT(CASE WHEN overall_rating >= 3.0 AND overall_rating < 4.0 THEN 1 END) as meets,
            COUNT(CASE WHEN overall_rating >= 2.0 AND overall_rating < 3.0 THEN 1 END) as developing,
            COUNT(CASE WHEN overall_rating < 2.0 THEN 1 END) as unsatisfactory,
            COUNT(*) as total
        FROM performance_ratings
        WHERE review_cycle_id = ?
        "#,
    )
    .bind(review_cycle_id)
    .fetch_one(pool)
    .await?;

    Ok(RatingDistribution {
        exceptional: row.get("exceptional"),
        exceeds: row.get("exceeds"),
        meets: row.get("meets"),
        developing: row.get("developing"),
        unsatisfactory: row.get("unsatisfactory"),
        total: row.get("total"),
    })
}

/// Get average rating for a cycle
pub async fn get_average_rating(
    pool: &DbPool,
    review_cycle_id: &str,
) -> Result<Option<f64>, RatingError> {
    let row = sqlx::query(
        "SELECT AVG(overall_rating) as avg FROM performance_ratings WHERE review_cycle_id = ?"
    )
    .bind(review_cycle_id)
    .fetch_one(pool)
    .await?;

    Ok(row.get("avg"))
}
