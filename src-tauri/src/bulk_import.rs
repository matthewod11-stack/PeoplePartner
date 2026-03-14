// People Partner - Bulk Import Module
// Direct database inserts for test data with predefined IDs

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::DbPool;

#[derive(Error, Debug, Serialize)]
pub enum ImportError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<sqlx::Error> for ImportError {
    fn from(err: sqlx::Error) -> Self {
        ImportError::Database(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportResult {
    pub inserted: usize,
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

// ============================================================================
// Import Types (with explicit IDs)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEmployee {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub manager_id: Option<String>,
    pub hire_date: Option<String>,
    pub work_state: Option<String>,
    pub status: Option<String>,
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,
    pub termination_date: Option<String>,
    pub termination_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReviewCycle {
    pub id: String,
    pub name: String,
    pub cycle_type: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRating {
    pub id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub reviewer_id: Option<String>,
    pub overall_rating: f64,
    pub goals_rating: Option<f64>,
    pub competency_rating: Option<f64>,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReview {
    pub id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub reviewer_id: Option<String>,
    pub strengths: Option<String>,
    pub areas_for_improvement: Option<String>,
    pub accomplishments: Option<String>,
    pub manager_comments: Option<String>,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEnps {
    pub id: String,
    pub employee_id: String,
    pub survey_date: String,
    pub survey_name: String,
    pub score: i32,
    pub feedback_text: Option<String>,
    pub submitted_at: Option<String>,
}

// ============================================================================
// Bulk Import Functions
// ============================================================================

/// Clear all test data from the database
pub async fn clear_all_data(pool: &DbPool) -> Result<(), ImportError> {
    // Delete in order respecting foreign key constraints
    sqlx::query("DELETE FROM enps_responses").execute(pool).await?;
    sqlx::query("DELETE FROM performance_reviews").execute(pool).await?;
    sqlx::query("DELETE FROM performance_ratings").execute(pool).await?;
    sqlx::query("DELETE FROM employees").execute(pool).await?;
    sqlx::query("DELETE FROM review_cycles").execute(pool).await?;
    Ok(())
}

/// Import review cycles with predefined IDs
pub async fn import_review_cycles(
    pool: &DbPool,
    cycles: Vec<ImportReviewCycle>,
) -> Result<BulkImportResult, ImportError> {
    let mut inserted = 0;
    let mut errors = Vec::new();

    for cycle in cycles {
        let result = sqlx::query(
            r#"
            INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date, status)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&cycle.id)
        .bind(&cycle.name)
        .bind(&cycle.cycle_type)
        .bind(&cycle.start_date)
        .bind(&cycle.end_date)
        .bind(&cycle.status)
        .execute(pool)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => errors.push(format!("{}: {}", cycle.id, e)),
        }
    }

    Ok(BulkImportResult { inserted, errors, warnings: Vec::new() })
}

/// Import employees with predefined IDs (preserves foreign key references)
pub async fn import_employees_bulk(
    pool: &DbPool,
    employees: Vec<ImportEmployee>,
) -> Result<BulkImportResult, ImportError> {
    let mut inserted = 0;
    let mut errors = Vec::new();

    for emp in employees {
        let status = emp.status.unwrap_or_else(|| "active".to_string());

        let result = sqlx::query(
            r#"
            INSERT INTO employees (
                id, email, full_name, department, job_title, manager_id,
                hire_date, work_state, status, date_of_birth, gender, ethnicity,
                termination_date, termination_reason
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&emp.id)
        .bind(&emp.email)
        .bind(&emp.full_name)
        .bind(&emp.department)
        .bind(&emp.job_title)
        .bind(&emp.manager_id)
        .bind(&emp.hire_date)
        .bind(&emp.work_state)
        .bind(&status)
        .bind(&emp.date_of_birth)
        .bind(&emp.gender)
        .bind(&emp.ethnicity)
        .bind(&emp.termination_date)
        .bind(&emp.termination_reason)
        .execute(pool)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => errors.push(format!("{}: {}", emp.id, e)),
        }
    }

    Ok(BulkImportResult { inserted, errors, warnings: Vec::new() })
}

/// Import performance ratings with predefined IDs
pub async fn import_ratings_bulk(
    pool: &DbPool,
    ratings: Vec<ImportRating>,
) -> Result<BulkImportResult, ImportError> {
    let mut inserted = 0;
    let mut errors = Vec::new();

    for rating in ratings {
        let result = sqlx::query(
            r#"
            INSERT INTO performance_ratings (
                id, employee_id, review_cycle_id, reviewer_id,
                overall_rating, goals_rating, competencies_rating, rating_date
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&rating.id)
        .bind(&rating.employee_id)
        .bind(&rating.review_cycle_id)
        .bind(&rating.reviewer_id)
        .bind(rating.overall_rating)
        .bind(rating.goals_rating)
        .bind(rating.competency_rating)
        .bind(&rating.submitted_at)
        .execute(pool)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => errors.push(format!("{}: {}", rating.id, e)),
        }
    }

    Ok(BulkImportResult { inserted, errors, warnings: Vec::new() })
}

/// Import performance reviews with predefined IDs
pub async fn import_reviews_bulk(
    pool: &DbPool,
    reviews: Vec<ImportReview>,
) -> Result<BulkImportResult, ImportError> {
    let mut inserted = 0;
    let mut errors = Vec::new();

    // Track inserted reviews and affected employees for auto-extraction
    let mut inserted_review_ids: Vec<String> = Vec::new();
    let mut affected_employee_ids: HashSet<String> = HashSet::new();

    for review in reviews {
        let result = sqlx::query(
            r#"
            INSERT INTO performance_reviews (
                id, employee_id, review_cycle_id, reviewer_id,
                strengths, areas_for_improvement, accomplishments, manager_comments, review_date
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&review.id)
        .bind(&review.employee_id)
        .bind(&review.review_cycle_id)
        .bind(&review.reviewer_id)
        .bind(&review.strengths)
        .bind(&review.areas_for_improvement)
        .bind(&review.accomplishments)
        .bind(&review.manager_comments)
        .bind(&review.submitted_at)
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                inserted += 1;
                inserted_review_ids.push(review.id.clone());
                affected_employee_ids.insert(review.employee_id.clone());
            }
            Err(e) => errors.push(format!("{}: {}", review.id, e)),
        }
    }

    // Auto-trigger: Extract highlights and regenerate summaries after import
    // Runs inline so failures are surfaced as warnings in the result
    let mut warnings = Vec::new();
    if !inserted_review_ids.is_empty() {
        let employee_ids: Vec<String> = affected_employee_ids.into_iter().collect();
        // Batch extract with rate limiting (100ms between API calls)
        if let Err(e) = crate::highlights::extract_highlights_batch(pool, inserted_review_ids).await {
            let msg = format!("[Auto-extract batch] Failed: {}", e);
            eprintln!("{}", msg);
            warnings.push(msg);
        }
        // Regenerate summaries for all affected employees
        for emp_id in &employee_ids {
            if let Err(e) = crate::highlights::generate_employee_summary(pool, emp_id).await {
                let msg = format!("[Auto-summary] Failed for employee {}: {}", emp_id, e);
                eprintln!("{}", msg);
                warnings.push(msg);
            }
        }
    }

    Ok(BulkImportResult { inserted, errors, warnings })
}

/// Import eNPS responses with predefined IDs
pub async fn import_enps_bulk(
    pool: &DbPool,
    responses: Vec<ImportEnps>,
) -> Result<BulkImportResult, ImportError> {
    let mut inserted = 0;
    let mut errors = Vec::new();

    for enps in responses {
        let result = sqlx::query(
            r#"
            INSERT INTO enps_responses (
                id, employee_id, survey_date, survey_name, score, feedback_text
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&enps.id)
        .bind(&enps.employee_id)
        .bind(&enps.survey_date)
        .bind(&enps.survey_name)
        .bind(enps.score)
        .bind(&enps.feedback_text)
        .execute(pool)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => errors.push(format!("{}: {}", enps.id, e)),
        }
    }

    Ok(BulkImportResult { inserted, errors, warnings: Vec::new() })
}

// ============================================================================
// Verification Queries
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheckResult {
    pub check_name: String,
    pub passed: bool,
    pub expected: i64,
    pub actual: i64,
    pub details: Option<String>,
}

/// Verify relational integrity of imported data
pub async fn verify_integrity(pool: &DbPool) -> Result<Vec<IntegrityCheckResult>, ImportError> {
    let mut results = Vec::new();

    // Check 1: Employee count
    let emp_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees")
        .fetch_one(pool)
        .await?;
    results.push(IntegrityCheckResult {
        check_name: "Employee count".to_string(),
        passed: emp_count == 100,
        expected: 100,
        actual: emp_count,
        details: None,
    });

    // Check 2: Review cycle count
    let cycle_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM review_cycles")
        .fetch_one(pool)
        .await?;
    results.push(IntegrityCheckResult {
        check_name: "Review cycle count".to_string(),
        passed: cycle_count == 3,
        expected: 3,
        actual: cycle_count,
        details: None,
    });

    // Check 3: All rating employee_ids exist in employees
    let orphan_rating_emps: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM performance_ratings pr
        WHERE NOT EXISTS (SELECT 1 FROM employees e WHERE e.id = pr.employee_id)
        "#
    )
    .fetch_one(pool)
    .await?;
    results.push(IntegrityCheckResult {
        check_name: "Rating employee_id integrity".to_string(),
        passed: orphan_rating_emps == 0,
        expected: 0,
        actual: orphan_rating_emps,
        details: Some("Orphan ratings with missing employee_id".to_string()),
    });

    // Check 4: All rating reviewer_ids exist in employees
    let orphan_rating_reviewers: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM performance_ratings pr
        WHERE pr.reviewer_id IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM employees e WHERE e.id = pr.reviewer_id)
        "#
    )
    .fetch_one(pool)
    .await?;
    results.push(IntegrityCheckResult {
        check_name: "Rating reviewer_id integrity".to_string(),
        passed: orphan_rating_reviewers == 0,
        expected: 0,
        actual: orphan_rating_reviewers,
        details: Some("Orphan ratings with missing reviewer_id".to_string()),
    });

    // Check 5: All rating review_cycle_ids exist in review_cycles
    let orphan_rating_cycles: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM performance_ratings pr
        WHERE NOT EXISTS (SELECT 1 FROM review_cycles rc WHERE rc.id = pr.review_cycle_id)
        "#
    )
    .fetch_one(pool)
    .await?;
    results.push(IntegrityCheckResult {
        check_name: "Rating review_cycle_id integrity".to_string(),
        passed: orphan_rating_cycles == 0,
        expected: 0,
        actual: orphan_rating_cycles,
        details: Some("Orphan ratings with missing review_cycle_id".to_string()),
    });

    // Check 6: All review employee_ids exist in employees
    let orphan_review_emps: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM performance_reviews pr
        WHERE NOT EXISTS (SELECT 1 FROM employees e WHERE e.id = pr.employee_id)
        "#
    )
    .fetch_one(pool)
    .await?;
    results.push(IntegrityCheckResult {
        check_name: "Review employee_id integrity".to_string(),
        passed: orphan_review_emps == 0,
        expected: 0,
        actual: orphan_review_emps,
        details: Some("Orphan reviews with missing employee_id".to_string()),
    });

    // Check 7: All eNPS employee_ids exist in employees
    let orphan_enps: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM enps_responses er
        WHERE NOT EXISTS (SELECT 1 FROM employees e WHERE e.id = er.employee_id)
        "#
    )
    .fetch_one(pool)
    .await?;
    results.push(IntegrityCheckResult {
        check_name: "eNPS employee_id integrity".to_string(),
        passed: orphan_enps == 0,
        expected: 0,
        actual: orphan_enps,
        details: Some("Orphan eNPS responses with missing employee_id".to_string()),
    });

    // Check 8: All manager_ids (except CEO) exist in employees
    let orphan_managers: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM employees e
        WHERE e.manager_id IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM employees m WHERE m.id = e.manager_id)
        "#
    )
    .fetch_one(pool)
    .await?;
    results.push(IntegrityCheckResult {
        check_name: "Employee manager_id integrity".to_string(),
        passed: orphan_managers == 0,
        expected: 0,
        actual: orphan_managers,
        details: Some("Employees with invalid manager_id".to_string()),
    });

    Ok(results)
}
