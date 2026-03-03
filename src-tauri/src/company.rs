// People Partner - Company Module
// CRUD operations for company profile (single-row table)
// Company state = HQ/incorporation state (legal jurisdiction)
// Employee work states are tracked separately per employee

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;

use crate::db::DbPool;

/// Default ID for the single company row
const COMPANY_ID: &str = "default";

#[derive(Error, Debug, Serialize)]
pub enum CompanyError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Company profile not found")]
    NotFound,
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<sqlx::Error> for CompanyError {
    fn from(err: sqlx::Error) -> Self {
        CompanyError::Database(err.to_string())
    }
}

/// Company profile from the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Company {
    pub id: String,
    pub name: String,
    pub state: String, // HQ/incorporation state
    pub industry: Option<String>,
    pub created_at: String,
}

/// Input for creating or updating the company profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCompany {
    pub name: String,
    pub state: String,
    pub industry: Option<String>,
}

/// Summary of employee work states (derived from employees table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeStatesSummary {
    pub states: Vec<String>,
    pub counts: Vec<StateCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCount {
    pub state: String,
    pub count: i64,
}

/// Check if a company profile exists
pub async fn has_company(pool: &DbPool) -> Result<bool, CompanyError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT COUNT(*) FROM company WHERE id = ?")
        .bind(COMPANY_ID)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|(count,)| count > 0).unwrap_or(false))
}

/// Get the company profile
pub async fn get_company(pool: &DbPool) -> Result<Company, CompanyError> {
    sqlx::query_as::<_, Company>("SELECT * FROM company WHERE id = ?")
        .bind(COMPANY_ID)
        .fetch_optional(pool)
        .await?
        .ok_or(CompanyError::NotFound)
}

/// Create or update the company profile (upsert)
pub async fn upsert_company(pool: &DbPool, input: UpsertCompany) -> Result<Company, CompanyError> {
    // Validate inputs
    let name = input.name.trim();
    let state = input.state.trim().to_uppercase();

    if name.is_empty() {
        return Err(CompanyError::Validation("Company name is required".to_string()));
    }
    if state.is_empty() {
        return Err(CompanyError::Validation("State is required".to_string()));
    }
    if state.len() != 2 {
        return Err(CompanyError::Validation(
            "State must be a 2-letter code (e.g., CA, NY, TX)".to_string(),
        ));
    }

    // Validate state is a valid US state code
    if !is_valid_us_state(&state) {
        return Err(CompanyError::Validation(format!(
            "'{}' is not a valid US state code",
            state
        )));
    }

    // Use INSERT OR REPLACE for upsert behavior
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO company (id, name, state, industry, created_at)
        VALUES (?, ?, ?, ?, COALESCE(
            (SELECT created_at FROM company WHERE id = ?),
            datetime('now')
        ))
        "#,
    )
    .bind(COMPANY_ID)
    .bind(name)
    .bind(&state)
    .bind(&input.industry)
    .bind(COMPANY_ID)
    .execute(pool)
    .await?;

    get_company(pool).await
}

/// Get summary of employee work states (operational footprint)
/// This is derived from the employees table, not stored in company
pub async fn get_employee_work_states(pool: &DbPool) -> Result<EmployeeStatesSummary, CompanyError> {
    let counts: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT work_state, COUNT(*) as count
        FROM employees
        WHERE work_state IS NOT NULL AND work_state != '' AND status = 'active'
        GROUP BY work_state
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let states: Vec<String> = counts.iter().map(|(s, _)| s.clone()).collect();
    let count_items: Vec<StateCount> = counts
        .into_iter()
        .map(|(state, count)| StateCount { state, count })
        .collect();

    Ok(EmployeeStatesSummary {
        states,
        counts: count_items,
    })
}

/// Validate US state codes
fn is_valid_us_state(code: &str) -> bool {
    const US_STATES: [&str; 50] = [
        "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "FL", "GA",
        "HI", "ID", "IL", "IN", "IA", "KS", "KY", "LA", "ME", "MD",
        "MA", "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ",
        "NM", "NY", "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC",
        "SD", "TN", "TX", "UT", "VT", "VA", "WA", "WV", "WI", "WY",
    ];
    US_STATES.contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_us_states() {
        assert!(is_valid_us_state("CA"));
        assert!(is_valid_us_state("NY"));
        assert!(is_valid_us_state("TX"));
        assert!(!is_valid_us_state("XX"));
        assert!(!is_valid_us_state("California"));
        assert!(!is_valid_us_state("ca")); // Must be uppercase
    }
}
