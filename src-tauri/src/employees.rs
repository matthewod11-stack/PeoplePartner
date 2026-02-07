// HR Command Center - Employees Module
// CRUD operations for employee data including demographics and termination tracking

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use crate::db::DbPool;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Serialize)]
pub enum EmployeeError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Employee not found: {0}")]
    NotFound(String),
    #[error("Duplicate email: {0}")]
    DuplicateEmail(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<sqlx::Error> for EmployeeError {
    fn from(err: sqlx::Error) -> Self {
        let err_str = err.to_string();
        if err_str.contains("UNIQUE constraint failed") && err_str.contains("email") {
            EmployeeError::DuplicateEmail("An employee with this email already exists".to_string())
        } else {
            eprintln!("[employees] Database error: {}", err_str);
            EmployeeError::Database("An internal database error occurred".to_string())
        }
    }
}

// ============================================================================
// Employee Struct
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Employee {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub manager_id: Option<String>,
    pub hire_date: Option<String>,
    pub work_state: Option<String>,
    pub status: String, // 'active' | 'terminated' | 'leave'

    // Demographics (V1 expansion)
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,

    // Termination details
    pub termination_date: Option<String>,
    pub termination_reason: Option<String>,

    // Flexibility
    pub extra_fields: Option<String>, // JSON string

    // Timestamps
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Input Types (for create/update operations)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEmployee {
    pub email: String,
    pub full_name: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub manager_id: Option<String>,
    pub hire_date: Option<String>,
    pub work_state: Option<String>,
    pub status: Option<String>,

    // Demographics
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,

    // Termination (usually not set on create)
    pub termination_date: Option<String>,
    pub termination_reason: Option<String>,

    pub extra_fields: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEmployee {
    pub email: Option<String>,
    pub full_name: Option<String>,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub manager_id: Option<String>,
    pub hire_date: Option<String>,
    pub work_state: Option<String>,
    pub status: Option<String>,

    // Demographics
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,

    // Termination
    pub termination_date: Option<String>,
    pub termination_reason: Option<String>,

    pub extra_fields: Option<String>,
}

// ============================================================================
// Filter/Query Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmployeeFilter {
    pub status: Option<String>,
    pub department: Option<String>,
    pub work_state: Option<String>,
    pub search: Option<String>, // Search by name or email
    // V2.3.2l: Additional filters for drilldown
    pub gender: Option<String>,
    pub ethnicity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeListResult {
    pub employees: Vec<Employee>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeWithLatestRating {
    #[serde(flatten)]
    pub employee: Employee,
    #[serde(rename = "latestRating")]
    pub latest_rating: Option<crate::performance_ratings::PerformanceRating>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeListWithRatingsResult {
    pub employees: Vec<EmployeeWithLatestRating>,
    pub total: i64,
}

fn build_employee_filter_where_clause(filter: &EmployeeFilter) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut bindings: Vec<String> = Vec::new();

    if let Some(status) = filter.status.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        conditions.push("status = ?".to_string());
        bindings.push(status.to_string());
    }

    if let Some(department) = filter
        .department
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        conditions.push("department = ?".to_string());
        bindings.push(department.to_string());
    }

    if let Some(work_state) = filter
        .work_state
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        conditions.push("work_state = ?".to_string());
        bindings.push(work_state.to_string());
    }

    if let Some(search) = filter.search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let pattern = format!("%{}%", search);
        conditions.push("(full_name LIKE ? OR email LIKE ?)".to_string());
        bindings.push(pattern.clone());
        bindings.push(pattern);
    }

    if let Some(gender) = filter.gender.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        conditions.push("gender = ?".to_string());
        bindings.push(gender.to_string());
    }

    if let Some(ethnicity) = filter
        .ethnicity
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        conditions.push("ethnicity = ?".to_string());
        bindings.push(ethnicity.to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bindings)
}

// ============================================================================
// CRUD Operations
// ============================================================================

/// Create a new employee
pub async fn create_employee(
    pool: &DbPool,
    input: CreateEmployee,
) -> Result<Employee, EmployeeError> {
    // Validate required fields
    if input.email.trim().is_empty() {
        return Err(EmployeeError::Validation("Email is required".to_string()));
    }
    if input.full_name.trim().is_empty() {
        return Err(EmployeeError::Validation("Full name is required".to_string()));
    }

    let id = Uuid::new_v4().to_string();
    let status = input.status.unwrap_or_else(|| "active".to_string());

    // Validate status
    if !["active", "terminated", "leave"].contains(&status.as_str()) {
        return Err(EmployeeError::Validation(format!(
            "Invalid status '{}'. Must be 'active', 'terminated', or 'leave'",
            status
        )));
    }

    sqlx::query(
        r#"
        INSERT INTO employees (
            id, email, full_name, department, job_title, manager_id,
            hire_date, work_state, status, date_of_birth, gender, ethnicity,
            termination_date, termination_reason, extra_fields
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.email)
    .bind(&input.full_name)
    .bind(&input.department)
    .bind(&input.job_title)
    .bind(&input.manager_id)
    .bind(&input.hire_date)
    .bind(&input.work_state)
    .bind(&status)
    .bind(&input.date_of_birth)
    .bind(&input.gender)
    .bind(&input.ethnicity)
    .bind(&input.termination_date)
    .bind(&input.termination_reason)
    .bind(&input.extra_fields)
    .execute(pool)
    .await?;

    // Fetch and return the created employee
    get_employee(pool, &id).await
}

/// Get an employee by ID
pub async fn get_employee(pool: &DbPool, id: &str) -> Result<Employee, EmployeeError> {
    let employee = sqlx::query_as::<_, Employee>(
        "SELECT * FROM employees WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EmployeeError::NotFound(id.to_string()))?;

    Ok(employee)
}

/// Get an employee by email
pub async fn get_employee_by_email(pool: &DbPool, email: &str) -> Result<Option<Employee>, EmployeeError> {
    let employee = sqlx::query_as::<_, Employee>(
        "SELECT * FROM employees WHERE email = ?"
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(employee)
}

/// Update an employee
pub async fn update_employee(
    pool: &DbPool,
    id: &str,
    input: UpdateEmployee,
) -> Result<Employee, EmployeeError> {
    // First check if employee exists
    let existing = get_employee(pool, id).await?;

    // Build dynamic update - only update fields that are provided
    let email = input.email.unwrap_or(existing.email);
    let full_name = input.full_name.unwrap_or(existing.full_name);
    let department = input.department.or(existing.department);
    let job_title = input.job_title.or(existing.job_title);
    let manager_id = input.manager_id.or(existing.manager_id);
    let hire_date = input.hire_date.or(existing.hire_date);
    let work_state = input.work_state.or(existing.work_state);
    let status = input.status.unwrap_or(existing.status);
    let date_of_birth = input.date_of_birth.or(existing.date_of_birth);
    let gender = input.gender.or(existing.gender);
    let ethnicity = input.ethnicity.or(existing.ethnicity);
    let termination_date = input.termination_date.or(existing.termination_date);
    let termination_reason = input.termination_reason.or(existing.termination_reason);
    let extra_fields = input.extra_fields.or(existing.extra_fields);

    // Validate status
    if !["active", "terminated", "leave"].contains(&status.as_str()) {
        return Err(EmployeeError::Validation(format!(
            "Invalid status '{}'. Must be 'active', 'terminated', or 'leave'",
            status
        )));
    }

    sqlx::query(
        r#"
        UPDATE employees SET
            email = ?, full_name = ?, department = ?, job_title = ?,
            manager_id = ?, hire_date = ?, work_state = ?, status = ?,
            date_of_birth = ?, gender = ?, ethnicity = ?,
            termination_date = ?, termination_reason = ?, extra_fields = ?,
            updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(&email)
    .bind(&full_name)
    .bind(&department)
    .bind(&job_title)
    .bind(&manager_id)
    .bind(&hire_date)
    .bind(&work_state)
    .bind(&status)
    .bind(&date_of_birth)
    .bind(&gender)
    .bind(&ethnicity)
    .bind(&termination_date)
    .bind(&termination_reason)
    .bind(&extra_fields)
    .bind(id)
    .execute(pool)
    .await?;

    // Return updated employee
    get_employee(pool, id).await
}

/// Delete an employee
pub async fn delete_employee(pool: &DbPool, id: &str) -> Result<(), EmployeeError> {
    let result = sqlx::query("DELETE FROM employees WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(EmployeeError::NotFound(id.to_string()));
    }

    Ok(())
}

/// List employees with optional filtering
pub async fn list_employees(
    pool: &DbPool,
    filter: EmployeeFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<EmployeeListResult, EmployeeError> {
    let limit = limit.unwrap_or(100);
    let offset = offset.unwrap_or(0);

    let (where_clause, bindings) = build_employee_filter_where_clause(&filter);

    // Get total count
    let count_query = format!("SELECT COUNT(*) as count FROM employees{}", where_clause);
    let mut count_query_builder = sqlx::query(&count_query);
    for binding in &bindings {
        count_query_builder = count_query_builder.bind(binding);
    }
    let total: i64 = count_query_builder.fetch_one(pool).await?.get("count");

    // Get paginated results
    let query = format!(
        "SELECT * FROM employees{} ORDER BY full_name ASC LIMIT ? OFFSET ?",
        where_clause
    );

    let mut list_query_builder = sqlx::query_as::<_, Employee>(&query);
    for binding in &bindings {
        list_query_builder = list_query_builder.bind(binding);
    }
    let employees = list_query_builder.bind(limit).bind(offset).fetch_all(pool).await?;

    Ok(EmployeeListResult { employees, total })
}

/// List employees with latest performance rating in a single backend call
pub async fn list_employees_with_ratings(
    pool: &DbPool,
    filter: EmployeeFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<EmployeeListWithRatingsResult, EmployeeError> {
    let list_result = list_employees(pool, filter, limit, offset).await?;

    if list_result.employees.is_empty() {
        return Ok(EmployeeListWithRatingsResult {
            employees: Vec::new(),
            total: list_result.total,
        });
    }

    let employee_ids: Vec<String> = list_result
        .employees
        .iter()
        .map(|employee| employee.id.clone())
        .collect();
    let placeholders = vec!["?"; employee_ids.len()].join(", ");

    let latest_ratings_query = format!(
        r#"
        WITH ranked_ratings AS (
            SELECT
                pr.*,
                ROW_NUMBER() OVER (
                    PARTITION BY pr.employee_id
                    ORDER BY COALESCE(rc.start_date, pr.rating_date, pr.created_at) DESC, pr.created_at DESC
                ) AS rn
            FROM performance_ratings pr
            LEFT JOIN review_cycles rc ON rc.id = pr.review_cycle_id
            WHERE pr.employee_id IN ({})
        )
        SELECT
            id,
            employee_id,
            review_cycle_id,
            overall_rating,
            goals_rating,
            competencies_rating,
            reviewer_id,
            rating_date,
            created_at,
            updated_at
        FROM ranked_ratings
        WHERE rn = 1
        "#,
        placeholders
    );

    let mut ratings_query =
        sqlx::query_as::<_, crate::performance_ratings::PerformanceRating>(&latest_ratings_query);
    for employee_id in &employee_ids {
        ratings_query = ratings_query.bind(employee_id);
    }

    let latest_ratings = ratings_query.fetch_all(pool).await?;
    let mut latest_rating_by_employee: HashMap<String, crate::performance_ratings::PerformanceRating> =
        HashMap::new();
    for rating in latest_ratings {
        latest_rating_by_employee.insert(rating.employee_id.clone(), rating);
    }

    let employees = list_result
        .employees
        .into_iter()
        .map(|employee| EmployeeWithLatestRating {
            latest_rating: latest_rating_by_employee.remove(&employee.id),
            employee,
        })
        .collect();

    Ok(EmployeeListWithRatingsResult {
        employees,
        total: list_result.total,
    })
}

/// Get all unique departments
pub async fn get_departments(pool: &DbPool) -> Result<Vec<String>, EmployeeError> {
    let rows = sqlx::query("SELECT DISTINCT department FROM employees WHERE department IS NOT NULL ORDER BY department")
        .fetch_all(pool)
        .await?;

    let departments: Vec<String> = rows
        .iter()
        .filter_map(|row| row.get::<Option<String>, _>("department"))
        .collect();

    Ok(departments)
}

/// Get total employee count (all statuses)
pub async fn get_total_employee_count(pool: &DbPool) -> Result<i64, EmployeeError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM employees")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Get employee count by status
pub async fn get_employee_counts(pool: &DbPool) -> Result<Vec<(String, i64)>, EmployeeError> {
    let rows = sqlx::query(
        "SELECT status, COUNT(*) as count FROM employees GROUP BY status ORDER BY status"
    )
    .fetch_all(pool)
    .await?;

    let counts: Vec<(String, i64)> = rows
        .iter()
        .map(|row| {
            let status: String = row.get("status");
            let count: i64 = row.get("count");
            (status, count)
        })
        .collect();

    Ok(counts)
}

/// Bulk import employees (upsert by email)
pub async fn import_employees(
    pool: &DbPool,
    employees: Vec<CreateEmployee>,
) -> Result<ImportResult, EmployeeError> {
    let mut created = 0;
    let mut updated = 0;
    let mut errors: Vec<String> = Vec::new();

    for (index, input) in employees.into_iter().enumerate() {
        // Check if employee with this email exists
        match get_employee_by_email(pool, &input.email).await? {
            Some(existing) => {
                // Update existing employee
                let update = UpdateEmployee {
                    email: Some(input.email),
                    full_name: Some(input.full_name),
                    department: input.department,
                    job_title: input.job_title,
                    manager_id: input.manager_id,
                    hire_date: input.hire_date,
                    work_state: input.work_state,
                    status: input.status,
                    date_of_birth: input.date_of_birth,
                    gender: input.gender,
                    ethnicity: input.ethnicity,
                    termination_date: input.termination_date,
                    termination_reason: input.termination_reason,
                    extra_fields: input.extra_fields,
                };
                match update_employee(pool, &existing.id, update).await {
                    Ok(_) => updated += 1,
                    Err(e) => errors.push(format!("Row {}: {}", index + 1, e)),
                }
            }
            None => {
                // Create new employee
                match create_employee(pool, input).await {
                    Ok(_) => created += 1,
                    Err(e) => errors.push(format!("Row {}: {}", index + 1, e)),
                }
            }
        }
    }

    Ok(ImportResult {
        created,
        updated,
        errors,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub created: i64,
    pub updated: i64,
    pub errors: Vec<String>,
}
