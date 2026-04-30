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

/// Clear all test data from the database.
///
/// Wrapped in a transaction so an interrupted run (panic, IPC drop, manual
/// kill mid-execution) cannot leave the DB in a half-deleted state where
/// e.g. `employees` is empty but `performance_reviews` still references the
/// vanished IDs.
pub async fn clear_all_data(pool: &DbPool) -> Result<(), ImportError> {
    let mut tx = pool.begin().await?;
    // Delete in order respecting foreign key constraints
    sqlx::query("DELETE FROM enps_responses").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM performance_reviews").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM performance_ratings").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM employees").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM review_cycles").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Import review cycles with predefined IDs.
///
/// All-or-nothing: any per-row failure rolls the whole batch back. Previous
/// behavior persisted the rows that succeeded before the first failure,
/// leaving the DB in a partial state that complicated re-imports.
pub async fn import_review_cycles(
    pool: &DbPool,
    cycles: Vec<ImportReviewCycle>,
) -> Result<BulkImportResult, ImportError> {
    let mut tx = pool.begin().await?;
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
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => {
                errors.push(format!("{}: {}", cycle.id, e));
                // Drop tx without commit → automatic rollback. Returning
                // inserted: 0 reflects the post-rollback DB state honestly.
                return Ok(BulkImportResult { inserted: 0, errors, warnings: Vec::new() });
            }
        }
    }

    tx.commit().await?;
    Ok(BulkImportResult { inserted, errors, warnings: Vec::new() })
}

/// Import employees with predefined IDs (preserves foreign key references).
///
/// All-or-nothing transactional. See `import_review_cycles` for rationale.
pub async fn import_employees_bulk(
    pool: &DbPool,
    employees: Vec<ImportEmployee>,
) -> Result<BulkImportResult, ImportError> {
    let mut tx = pool.begin().await?;
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
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => {
                errors.push(format!("{}: {}", emp.id, e));
                return Ok(BulkImportResult { inserted: 0, errors, warnings: Vec::new() });
            }
        }
    }

    tx.commit().await?;
    Ok(BulkImportResult { inserted, errors, warnings: Vec::new() })
}

/// Import performance ratings with predefined IDs.
///
/// All-or-nothing transactional. See `import_review_cycles` for rationale.
pub async fn import_ratings_bulk(
    pool: &DbPool,
    ratings: Vec<ImportRating>,
) -> Result<BulkImportResult, ImportError> {
    let mut tx = pool.begin().await?;
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
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => {
                errors.push(format!("{}: {}", rating.id, e));
                return Ok(BulkImportResult { inserted: 0, errors, warnings: Vec::new() });
            }
        }
    }

    tx.commit().await?;
    Ok(BulkImportResult { inserted, errors, warnings: Vec::new() })
}

/// Import performance reviews with predefined IDs.
///
/// All-or-nothing transactional for the INSERTs. The post-commit auto-extract
/// step (highlights + summaries) intentionally runs *after* commit so the
/// highlights pipeline reads from a stable, persisted view of the new rows.
/// Auto-extract failures are surfaced as warnings, not errors — the import
/// itself has already succeeded.
pub async fn import_reviews_bulk(
    pool: &DbPool,
    reviews: Vec<ImportReview>,
) -> Result<BulkImportResult, ImportError> {
    let mut tx = pool.begin().await?;
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
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => {
                inserted += 1;
                inserted_review_ids.push(review.id.clone());
                affected_employee_ids.insert(review.employee_id.clone());
            }
            Err(e) => {
                errors.push(format!("{}: {}", review.id, e));
                // Drop tx → rollback. Skip auto-extract entirely.
                return Ok(BulkImportResult { inserted: 0, errors, warnings: Vec::new() });
            }
        }
    }

    tx.commit().await?;

    // Auto-trigger: Extract highlights and regenerate summaries after import
    // Runs inline so failures are surfaced as warnings in the result
    let mut warnings = Vec::new();
    if !inserted_review_ids.is_empty() {
        let employee_ids: Vec<String> = affected_employee_ids.into_iter().collect();
        // Batch extract with rate limiting (100ms between API calls)
        if let Err(e) = crate::highlights::extract_highlights_batch(pool, inserted_review_ids).await {
            let msg = format!("[Auto-extract batch] Failed: {}", e);
            log::warn!("{}", msg);
            warnings.push(msg);
        }
        // Regenerate summaries for all affected employees
        for emp_id in &employee_ids {
            if let Err(e) = crate::highlights::generate_employee_summary(pool, emp_id).await {
                let msg = format!("[Auto-summary] Failed for employee {}: {}", emp_id, e);
                log::warn!("{}", msg);
                warnings.push(msg);
            }
        }
    }

    Ok(BulkImportResult { inserted, errors, warnings })
}

/// Import eNPS responses with predefined IDs.
///
/// All-or-nothing transactional. See `import_review_cycles` for rationale.
pub async fn import_enps_bulk(
    pool: &DbPool,
    responses: Vec<ImportEnps>,
) -> Result<BulkImportResult, ImportError> {
    let mut tx = pool.begin().await?;
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
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => inserted += 1,
            Err(e) => {
                errors.push(format!("{}: {}", enps.id, e));
                return Ok(BulkImportResult { inserted: 0, errors, warnings: Vec::new() });
            }
        }
    }

    tx.commit().await?;
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

// ============================================================================
// Tests (issue #33: transaction wrapping)
// ============================================================================

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

    fn make_employee(id: &str, email: &str) -> ImportEmployee {
        ImportEmployee {
            id: id.to_string(),
            email: email.to_string(),
            full_name: format!("Test User {id}"),
            department: Some("Engineering".to_string()),
            job_title: None,
            manager_id: None,
            hire_date: Some("2024-01-15".to_string()),
            work_state: Some("CA".to_string()),
            status: Some("active".to_string()),
            date_of_birth: None,
            gender: None,
            ethnicity: None,
            termination_date: None,
            termination_reason: None,
        }
    }

    #[tokio::test]
    async fn clear_all_data_succeeds_on_empty_db() {
        let pool = test_pool().await;
        clear_all_data(&pool).await.expect("clear empty db");
    }

    #[tokio::test]
    async fn import_employees_bulk_happy_path() {
        let pool = test_pool().await;
        let employees = vec![
            make_employee("emp-1", "alice@example.com"),
            make_employee("emp-2", "bob@example.com"),
        ];

        let result = import_employees_bulk(&pool, employees)
            .await
            .expect("import succeeded");
        assert_eq!(result.inserted, 2);
        assert!(result.errors.is_empty(), "no errors expected: {:?}", result.errors);

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn import_employees_bulk_rolls_back_on_duplicate_email() {
        // Two valid + one that collides on email UNIQUE — without the
        // transaction wrap, the first 2 used to persist before the 3rd
        // failed, leaving the DB in a partial state. Now: zero rows persist.
        let pool = test_pool().await;
        let employees = vec![
            make_employee("emp-1", "alice@example.com"),
            make_employee("emp-2", "bob@example.com"),
            make_employee("emp-3", "alice@example.com"), // duplicate email
        ];

        let result = import_employees_bulk(&pool, employees)
            .await
            .expect("call returns Ok with partial-failure result");
        assert_eq!(result.inserted, 0, "rollback must report zero inserted");
        assert_eq!(result.errors.len(), 1);
        assert!(
            result.errors[0].contains("emp-3"),
            "error should mention failing row: {}",
            result.errors[0]
        );

        // The critical assertion: nothing made it to disk.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "transaction must roll back the first 2 rows too");
    }

    #[tokio::test]
    async fn clear_all_data_then_import_is_atomic_pair() {
        // Pre-populate, then exercise the full clear-and-reimport flow that
        // a CSV refresh hits in production.
        let pool = test_pool().await;
        let original = vec![make_employee("emp-1", "alice@example.com")];
        import_employees_bulk(&pool, original).await.unwrap();

        clear_all_data(&pool).await.expect("clear succeeded");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);

        // And we can re-import using the same id without collision.
        let reimport = vec![make_employee("emp-1", "alice@example.com")];
        let result = import_employees_bulk(&pool, reimport).await.unwrap();
        assert_eq!(result.inserted, 1);
    }
}
