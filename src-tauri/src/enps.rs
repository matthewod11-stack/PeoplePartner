// People Partner - eNPS Module
// CRUD operations for Employee Net Promoter Score tracking

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::db::DbPool;

#[derive(Error, Debug, Serialize)]
pub enum EnpsError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Response not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<sqlx::Error> for EnpsError {
    fn from(err: sqlx::Error) -> Self {
        EnpsError::Database(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnpsResponse {
    pub id: String,
    pub employee_id: String,
    pub score: i32, // 0-10
    pub survey_date: String,
    pub survey_name: Option<String>,
    pub feedback_text: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEnps {
    pub employee_id: String,
    pub score: i32,
    pub survey_date: String,
    pub survey_name: Option<String>,
    pub feedback_text: Option<String>,
}

pub async fn create_enps(pool: &DbPool, input: CreateEnps) -> Result<EnpsResponse, EnpsError> {
    if input.employee_id.trim().is_empty() {
        return Err(EnpsError::Validation("employee_id is required".to_string()));
    }
    if input.score < 0 || input.score > 10 {
        return Err(EnpsError::Validation("score must be between 0 and 10".to_string()));
    }

    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO enps_responses (id, employee_id, score, survey_date, survey_name, feedback_text) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&input.employee_id)
    .bind(input.score)
    .bind(&input.survey_date)
    .bind(&input.survey_name)
    .bind(&input.feedback_text)
    .execute(pool)
    .await?;

    get_enps(pool, &id).await
}

pub async fn get_enps(pool: &DbPool, id: &str) -> Result<EnpsResponse, EnpsError> {
    sqlx::query_as::<_, EnpsResponse>("SELECT * FROM enps_responses WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EnpsError::NotFound(id.to_string()))
}

pub async fn get_enps_for_employee(pool: &DbPool, employee_id: &str) -> Result<Vec<EnpsResponse>, EnpsError> {
    Ok(sqlx::query_as::<_, EnpsResponse>(
        "SELECT * FROM enps_responses WHERE employee_id = ? ORDER BY survey_date DESC"
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_enps_for_survey(pool: &DbPool, survey_name: &str) -> Result<Vec<EnpsResponse>, EnpsError> {
    Ok(sqlx::query_as::<_, EnpsResponse>(
        "SELECT * FROM enps_responses WHERE survey_name = ? ORDER BY score DESC"
    )
    .bind(survey_name)
    .fetch_all(pool)
    .await?)
}

pub async fn delete_enps(pool: &DbPool, id: &str) -> Result<(), EnpsError> {
    let result = sqlx::query("DELETE FROM enps_responses WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(EnpsError::NotFound(id.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnpsScore {
    pub promoters: i64,
    pub passives: i64,
    pub detractors: i64,
    pub total: i64,
    pub score: f64, // eNPS = %promoters - %detractors
}

/// Calculate eNPS for a survey
pub async fn calculate_enps(pool: &DbPool, survey_name: &str) -> Result<EnpsScore, EnpsError> {
    let row = sqlx::query(
        r#"SELECT
            COUNT(CASE WHEN score >= 9 THEN 1 END) as promoters,
            COUNT(CASE WHEN score >= 7 AND score < 9 THEN 1 END) as passives,
            COUNT(CASE WHEN score < 7 THEN 1 END) as detractors,
            COUNT(*) as total
           FROM enps_responses WHERE survey_name = ?"#,
    )
    .bind(survey_name)
    .fetch_one(pool)
    .await?;

    let promoters: i64 = row.get("promoters");
    let passives: i64 = row.get("passives");
    let detractors: i64 = row.get("detractors");
    let total: i64 = row.get("total");

    let score = if total > 0 {
        ((promoters as f64 / total as f64) - (detractors as f64 / total as f64)) * 100.0
    } else {
        0.0
    };

    Ok(EnpsScore { promoters, passives, detractors, total, score })
}

/// Get latest eNPS score for an employee
pub async fn get_latest_enps(pool: &DbPool, employee_id: &str) -> Result<Option<EnpsResponse>, EnpsError> {
    Ok(sqlx::query_as::<_, EnpsResponse>(
        "SELECT * FROM enps_responses WHERE employee_id = ? ORDER BY survey_date DESC LIMIT 1"
    )
    .bind(employee_id)
    .fetch_optional(pool)
    .await?)
}
