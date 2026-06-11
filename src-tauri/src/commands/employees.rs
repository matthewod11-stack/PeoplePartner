//! Company profile + employee CRUD + work-states summary commands.

use std::collections::HashSet;

use crate::company;
use crate::db::Database;
use crate::employees;
use crate::trial;

// ---- Company profile ----

/// Check if a company profile exists
#[tauri::command]
pub(crate) async fn has_company(
    state: tauri::State<'_, Database>,
) -> Result<bool, company::CompanyError> {
    company::has_company(&state.pool).await
}

/// Get the company profile
#[tauri::command]
pub(crate) async fn get_company(
    state: tauri::State<'_, Database>,
) -> Result<company::Company, company::CompanyError> {
    company::get_company(&state.pool).await
}

/// Create or update the company profile
#[tauri::command]
pub(crate) async fn upsert_company(
    state: tauri::State<'_, Database>,
    input: company::UpsertCompany,
) -> Result<company::Company, company::CompanyError> {
    company::upsert_company(&state.pool, input).await
}

/// Get summary of states where employees work (operational footprint)
#[tauri::command]
pub(crate) async fn get_employee_work_states(
    state: tauri::State<'_, Database>,
) -> Result<company::EmployeeStatesSummary, company::CompanyError> {
    company::get_employee_work_states(&state.pool).await
}

// ---- Employee CRUD ----

/// Create a new employee (with trial mode limit check)
#[tauri::command]
pub(crate) async fn create_employee(
    state: tauri::State<'_, Database>,
    input: employees::CreateEmployee,
) -> Result<employees::Employee, employees::EmployeeError> {
    // Enforce trial employee limit (sample data is exempt — #106)
    if trial::is_trial_mode(&state.pool).await.unwrap_or(false) {
        let count = trial::get_countable_employee_count(&state.pool)
            .await
            .map_err(|e| employees::EmployeeError::Database(e.to_string()))?;
        if count >= trial::TRIAL_EMPLOYEE_LIMIT {
            return Err(employees::EmployeeError::Validation(
                "Trial is limited to 10 employees. Upgrade to add more.".to_string(),
            ));
        }
    }
    employees::create_employee(&state.pool, input).await
}

/// Get an employee by ID
#[tauri::command]
pub(crate) async fn get_employee(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<employees::Employee, employees::EmployeeError> {
    employees::get_employee(&state.pool, &id).await
}

/// Get an employee by email
#[tauri::command]
pub(crate) async fn get_employee_by_email(
    state: tauri::State<'_, Database>,
    email: String,
) -> Result<Option<employees::Employee>, employees::EmployeeError> {
    employees::get_employee_by_email(&state.pool, &email).await
}

/// Update an employee
#[tauri::command]
pub(crate) async fn update_employee(
    state: tauri::State<'_, Database>,
    id: String,
    input: employees::UpdateEmployee,
) -> Result<employees::Employee, employees::EmployeeError> {
    employees::update_employee(&state.pool, &id, input).await
}

/// Delete an employee
#[tauri::command]
pub(crate) async fn delete_employee(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), employees::EmployeeError> {
    employees::delete_employee(&state.pool, &id).await
}

/// List employees with filtering
#[tauri::command]
pub(crate) async fn list_employees(
    state: tauri::State<'_, Database>,
    filter: employees::EmployeeFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<employees::EmployeeListResult, employees::EmployeeError> {
    employees::list_employees(&state.pool, filter, limit, offset).await
}

/// List employees with latest ratings in one backend call
#[tauri::command]
pub(crate) async fn list_employees_with_ratings(
    state: tauri::State<'_, Database>,
    filter: employees::EmployeeFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<employees::EmployeeListWithRatingsResult, employees::EmployeeError> {
    employees::list_employees_with_ratings(&state.pool, filter, limit, offset).await
}

/// Get all unique departments
#[tauri::command]
pub(crate) async fn get_departments(
    state: tauri::State<'_, Database>,
) -> Result<Vec<String>, employees::EmployeeError> {
    employees::get_departments(&state.pool).await
}

/// Get employee counts by status
#[tauri::command]
pub(crate) async fn get_employee_counts(
    state: tauri::State<'_, Database>,
) -> Result<Vec<(String, i64)>, employees::EmployeeError> {
    employees::get_employee_counts(&state.pool).await
}

/// Bulk import employees (upsert by email, with trial mode limit check)
#[tauri::command]
pub(crate) async fn import_employees(
    state: tauri::State<'_, Database>,
    employees: Vec<employees::CreateEmployee>,
) -> Result<employees::ImportResult, employees::EmployeeError> {
    // Enforce trial employee limit for imports (sample data is exempt — #106)
    if trial::is_trial_mode(&state.pool).await.unwrap_or(false) {
        let current = trial::get_countable_employee_count(&state.pool)
            .await
            .map_err(|e| employees::EmployeeError::Database(e.to_string()))?;
        let mut unique_emails: HashSet<String> = HashSet::new();
        let mut net_new_count: i64 = 0;

        for employee in &employees {
            let normalized_email = employee.email.trim().to_lowercase();
            if !unique_emails.insert(normalized_email.clone()) {
                continue;
            }

            if employees::get_employee_by_email(&state.pool, &normalized_email)
                .await?
                .is_none()
            {
                net_new_count += 1;
            }
        }

        if current + net_new_count > trial::TRIAL_EMPLOYEE_LIMIT {
            return Err(employees::EmployeeError::Validation(format!(
                "Trial is limited to {} employees. You have {} and this import adds {} new records. Upgrade to add more.",
                trial::TRIAL_EMPLOYEE_LIMIT,
                current,
                net_new_count
            )));
        }
    }
    employees::import_employees(&state.pool, employees).await
}
