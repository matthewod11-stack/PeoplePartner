// HR Command Center - Rust Backend
// This file contains the core library code for Tauri commands

use std::collections::HashSet;
use tauri::Manager;

mod analytics;
mod analytics_templates;
mod audit;
mod backup;
mod bulk_import;
mod chat;
mod company;
mod context;
mod conversations;
mod data_quality;
mod db;
mod dei;
mod device_id;
mod employees;
mod enps;
mod file_parser;
mod highlights;
mod insight_canvas;
mod keyring;
mod memory;
mod network;
mod performance_ratings;
mod performance_reviews;
mod pii;
mod review_cycles;
mod settings;
mod signals;
mod trial;

use db::Database;

/// Greet command for testing - will be replaced with actual commands
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to HR Command Center.", name)
}

/// Check if database is initialized
#[tauri::command]
fn check_db(state: tauri::State<'_, Database>) -> bool {
    // If we can access the state, the database is initialized
    let _ = &state.pool;
    true
}

// ============================================================================
// API Key Management Commands
// ============================================================================

/// Store the Anthropic API key in macOS Keychain
#[tauri::command]
fn store_api_key(api_key: String) -> Result<(), keyring::KeyringError> {
    keyring::store_api_key(&api_key)
}

/// Check if an API key exists in the Keychain
#[tauri::command]
fn has_api_key() -> bool {
    keyring::has_api_key()
}

/// Delete the API key from the Keychain
#[tauri::command]
fn delete_api_key() -> Result<(), keyring::KeyringError> {
    keyring::delete_api_key()
}

/// Validate an API key format (does not store it)
#[tauri::command]
fn validate_api_key_format(api_key: String) -> bool {
    api_key.starts_with("sk-ant-") && api_key.len() > 20
}

/// Store a license key after basic local format validation.
/// This is local-only storage until server-side validation is wired.
#[tauri::command]
async fn store_license_key(
    state: tauri::State<'_, Database>,
    license_key: String,
) -> Result<(), String> {
    let normalized = license_key.trim().to_string();
    if !validate_license_key_format(normalized.clone()) {
        return Err(
            "License key format is invalid. Use letters, numbers, and dashes only.".to_string(),
        );
    }

    trial::store_license_key(&state.pool, &normalized)
        .await
        .map_err(|e| e.to_string())?;

    // Purchased installs should not keep stale trial usage counts.
    trial::reset_trial_messages(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Remove stored license key.
#[tauri::command]
async fn delete_license_key(state: tauri::State<'_, Database>) -> Result<(), String> {
    trial::delete_license_key(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Check whether a license key is present.
#[tauri::command]
async fn has_license_key(state: tauri::State<'_, Database>) -> Result<bool, String> {
    trial::has_license_key(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Validate license key format without storing it.
#[tauri::command]
fn validate_license_key_format(license_key: String) -> bool {
    let trimmed = license_key.trim();
    let len = trimmed.len();
    if len < 12 || len > 80 {
        return false;
    }
    if !trimmed.contains('-') {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

// ============================================================================
// Chat Commands
// ============================================================================

/// Send a message to Claude and get a response (non-streaming)
#[tauri::command]
async fn send_chat_message(
    messages: Vec<chat::ChatMessage>,
    system_prompt: Option<String>,
) -> Result<chat::ChatResponse, chat::ChatError> {
    chat::send_message(messages, system_prompt).await
}

/// Send a message to Claude with streaming response
/// Emits "chat-stream" events as response chunks arrive
///
/// Dual-path routing: trial mode goes through proxy, paid mode uses BYOK key directly.
#[tauri::command]
async fn send_chat_message_streaming(
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    messages: Vec<chat::ChatMessage>,
    system_prompt: Option<String>,
    aggregates: Option<context::OrgAggregates>,
    query_type: Option<context::QueryType>,
) -> Result<(), chat::ChatError> {
    let has_license = trial::has_license_key(&state.pool)
        .await
        .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;
    let has_api_key = keyring::has_api_key();

    if !has_license {
        // Get proxy config
        let proxy_url = trial::get_proxy_url(&state.pool)
            .await
            .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;
        let device_id = trial::get_device_id(&state.pool)
            .await
            .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;
        let proxy_signing_secret = trial::get_proxy_signing_secret(&state.pool)
            .await
            .map_err(|e| chat::ChatError::TrialError(e.to_string()))?;

        // Route through proxy
        let trial_usage = chat::send_message_streaming_trial(
            app,
            messages,
            system_prompt,
            &proxy_url,
            &device_id,
            proxy_signing_secret.as_deref(),
            aggregates,
            query_type,
        )
        .await;

        let trial_usage = match trial_usage {
            Ok(usage) => usage,
            Err(chat::ChatError::TrialLimitReached { used, limit }) => {
                if let Some(server_used) = used {
                    let _ = trial::set_trial_messages_used(&state.pool, server_used).await;
                }
                return Err(chat::ChatError::TrialLimitReached { used, limit });
            }
            Err(other) => return Err(other),
        };

        // Sync local counter from proxy metadata when available.
        if let Some(used) = trial_usage.used {
            let _ = trial::set_trial_messages_used(&state.pool, used).await;
        } else {
            let _ = trial::increment_trial_messages(&state.pool).await;
        }

        Ok(())
    } else if !has_api_key {
        Err(chat::ChatError::NoApiKey)
    } else {
        // Paid mode: direct to Anthropic API with BYOK key
        chat::send_message_streaming(app, messages, system_prompt, aggregates, query_type).await
    }
}

// ============================================================================
// Network Status Commands
// ============================================================================

/// Check if the network and Anthropic API are reachable
#[tauri::command]
async fn check_network_status() -> network::NetworkStatus {
    network::check_network().await
}

/// Quick check if online (returns just a boolean)
#[tauri::command]
async fn is_online() -> bool {
    network::is_online().await
}

// ============================================================================
// PII Scanning Commands
// ============================================================================

/// Scan text for PII and return redaction result
/// Used by frontend before sending messages to Claude API
#[tauri::command]
fn scan_pii(text: String) -> pii::RedactionResult {
    pii::scan_and_redact(&text)
}

// ============================================================================
// Audit Logging Commands
// ============================================================================

/// Create an audit log entry after a Claude API interaction
/// Called by frontend after streaming response completes
#[tauri::command]
async fn create_audit_entry(
    state: tauri::State<'_, Database>,
    input: audit::CreateAuditEntry,
) -> Result<audit::AuditEntry, audit::AuditError> {
    audit::create_audit_entry(&state.pool, input).await
}

/// Get a single audit entry by ID
#[tauri::command]
async fn get_audit_entry(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<audit::AuditEntry, audit::AuditError> {
    audit::get_audit_entry(&state.pool, &id).await
}

/// List audit entries with optional filtering
#[tauri::command]
async fn list_audit_entries(
    state: tauri::State<'_, Database>,
    filter: Option<audit::AuditFilter>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<audit::AuditListItem>, audit::AuditError> {
    audit::list_audit_entries(&state.pool, filter, limit, offset).await
}

/// Count audit entries matching filter (for pagination)
#[tauri::command]
async fn count_audit_entries(
    state: tauri::State<'_, Database>,
    filter: Option<audit::AuditFilter>,
) -> Result<i64, audit::AuditError> {
    audit::count_audit_entries(&state.pool, filter).await
}

/// Export audit log to CSV format
#[tauri::command]
async fn export_audit_log(
    state: tauri::State<'_, Database>,
    filter: Option<audit::AuditFilter>,
) -> Result<audit::ExportResult, audit::AuditError> {
    audit::export_to_csv(&state.pool, filter).await
}

// ============================================================================
// Company Profile Commands
// ============================================================================

/// Check if a company profile exists
#[tauri::command]
async fn has_company(
    state: tauri::State<'_, Database>,
) -> Result<bool, company::CompanyError> {
    company::has_company(&state.pool).await
}

/// Get the company profile
#[tauri::command]
async fn get_company(
    state: tauri::State<'_, Database>,
) -> Result<company::Company, company::CompanyError> {
    company::get_company(&state.pool).await
}

/// Create or update the company profile
#[tauri::command]
async fn upsert_company(
    state: tauri::State<'_, Database>,
    input: company::UpsertCompany,
) -> Result<company::Company, company::CompanyError> {
    company::upsert_company(&state.pool, input).await
}

/// Get summary of states where employees work (operational footprint)
#[tauri::command]
async fn get_employee_work_states(
    state: tauri::State<'_, Database>,
) -> Result<company::EmployeeStatesSummary, company::CompanyError> {
    company::get_employee_work_states(&state.pool).await
}

// ============================================================================
// Employee Management Commands
// ============================================================================

/// Create a new employee (with trial mode limit check)
#[tauri::command]
async fn create_employee(
    state: tauri::State<'_, Database>,
    input: employees::CreateEmployee,
) -> Result<employees::Employee, employees::EmployeeError> {
    // Enforce trial employee limit
    if trial::is_trial_mode(&state.pool).await.unwrap_or(false) {
        let count = employees::get_total_employee_count(&state.pool).await?;
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
async fn get_employee(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<employees::Employee, employees::EmployeeError> {
    employees::get_employee(&state.pool, &id).await
}

/// Get an employee by email
#[tauri::command]
async fn get_employee_by_email(
    state: tauri::State<'_, Database>,
    email: String,
) -> Result<Option<employees::Employee>, employees::EmployeeError> {
    employees::get_employee_by_email(&state.pool, &email).await
}

/// Update an employee
#[tauri::command]
async fn update_employee(
    state: tauri::State<'_, Database>,
    id: String,
    input: employees::UpdateEmployee,
) -> Result<employees::Employee, employees::EmployeeError> {
    employees::update_employee(&state.pool, &id, input).await
}

/// Delete an employee
#[tauri::command]
async fn delete_employee(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), employees::EmployeeError> {
    employees::delete_employee(&state.pool, &id).await
}

/// List employees with filtering
#[tauri::command]
async fn list_employees(
    state: tauri::State<'_, Database>,
    filter: employees::EmployeeFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<employees::EmployeeListResult, employees::EmployeeError> {
    employees::list_employees(&state.pool, filter, limit, offset).await
}

/// List employees with latest ratings in one backend call
#[tauri::command]
async fn list_employees_with_ratings(
    state: tauri::State<'_, Database>,
    filter: employees::EmployeeFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<employees::EmployeeListWithRatingsResult, employees::EmployeeError> {
    employees::list_employees_with_ratings(&state.pool, filter, limit, offset).await
}

/// Get all unique departments
#[tauri::command]
async fn get_departments(
    state: tauri::State<'_, Database>,
) -> Result<Vec<String>, employees::EmployeeError> {
    employees::get_departments(&state.pool).await
}

/// Get employee counts by status
#[tauri::command]
async fn get_employee_counts(
    state: tauri::State<'_, Database>,
) -> Result<Vec<(String, i64)>, employees::EmployeeError> {
    employees::get_employee_counts(&state.pool).await
}

/// Bulk import employees (upsert by email, with trial mode limit check)
#[tauri::command]
async fn import_employees(
    state: tauri::State<'_, Database>,
    employees: Vec<employees::CreateEmployee>,
) -> Result<employees::ImportResult, employees::EmployeeError> {
    // Enforce trial employee limit for imports
    if trial::is_trial_mode(&state.pool).await.unwrap_or(false) {
        let current = employees::get_total_employee_count(&state.pool).await?;
        let mut unique_emails: HashSet<String> = HashSet::new();
        let mut net_new_count: i64 = 0;

        for employee in &employees {
            let normalized_email = employee.email.trim().to_lowercase();
            if !unique_emails.insert(normalized_email.clone()) {
                continue;
            }

            if employees::get_employee_by_email(&state.pool, &employee.email)
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

// ============================================================================
// Review Cycle Commands
// ============================================================================

/// Create a new review cycle
#[tauri::command]
async fn create_review_cycle(
    state: tauri::State<'_, Database>,
    input: review_cycles::CreateReviewCycle,
) -> Result<review_cycles::ReviewCycle, review_cycles::ReviewCycleError> {
    review_cycles::create_review_cycle(&state.pool, input).await
}

/// Get a review cycle by ID
#[tauri::command]
async fn get_review_cycle(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<review_cycles::ReviewCycle, review_cycles::ReviewCycleError> {
    review_cycles::get_review_cycle(&state.pool, &id).await
}

/// Update a review cycle
#[tauri::command]
async fn update_review_cycle(
    state: tauri::State<'_, Database>,
    id: String,
    input: review_cycles::UpdateReviewCycle,
) -> Result<review_cycles::ReviewCycle, review_cycles::ReviewCycleError> {
    review_cycles::update_review_cycle(&state.pool, &id, input).await
}

/// Delete a review cycle
#[tauri::command]
async fn delete_review_cycle(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), review_cycles::ReviewCycleError> {
    review_cycles::delete_review_cycle(&state.pool, &id).await
}

/// List all review cycles
#[tauri::command]
async fn list_review_cycles(
    state: tauri::State<'_, Database>,
    status_filter: Option<String>,
) -> Result<Vec<review_cycles::ReviewCycle>, review_cycles::ReviewCycleError> {
    review_cycles::list_review_cycles(&state.pool, status_filter).await
}

/// Get the current active review cycle
#[tauri::command]
async fn get_active_review_cycle(
    state: tauri::State<'_, Database>,
) -> Result<Option<review_cycles::ReviewCycle>, review_cycles::ReviewCycleError> {
    review_cycles::get_active_review_cycle(&state.pool).await
}

/// Close a review cycle
#[tauri::command]
async fn close_review_cycle(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<review_cycles::ReviewCycle, review_cycles::ReviewCycleError> {
    review_cycles::close_review_cycle(&state.pool, &id).await
}

// ============================================================================
// Performance Rating Commands
// ============================================================================

/// Create a performance rating
#[tauri::command]
async fn create_performance_rating(
    state: tauri::State<'_, Database>,
    input: performance_ratings::CreateRating,
) -> Result<performance_ratings::PerformanceRating, performance_ratings::RatingError> {
    performance_ratings::create_rating(&state.pool, input).await
}

/// Get a rating by ID
#[tauri::command]
async fn get_performance_rating(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<performance_ratings::PerformanceRating, performance_ratings::RatingError> {
    performance_ratings::get_rating(&state.pool, &id).await
}

/// Get all ratings for an employee
#[tauri::command]
async fn get_ratings_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<performance_ratings::PerformanceRating>, performance_ratings::RatingError> {
    performance_ratings::get_ratings_for_employee(&state.pool, &employee_id).await
}

/// Get all ratings for a review cycle
#[tauri::command]
async fn get_ratings_for_cycle(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<Vec<performance_ratings::PerformanceRating>, performance_ratings::RatingError> {
    performance_ratings::get_ratings_for_cycle(&state.pool, &review_cycle_id).await
}

/// Get the latest rating for an employee
#[tauri::command]
async fn get_latest_rating(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Option<performance_ratings::PerformanceRating>, performance_ratings::RatingError> {
    performance_ratings::get_latest_rating_for_employee(&state.pool, &employee_id).await
}

/// Update a rating
#[tauri::command]
async fn update_performance_rating(
    state: tauri::State<'_, Database>,
    id: String,
    input: performance_ratings::UpdateRating,
) -> Result<performance_ratings::PerformanceRating, performance_ratings::RatingError> {
    performance_ratings::update_rating(&state.pool, &id, input).await
}

/// Delete a rating
#[tauri::command]
async fn delete_performance_rating(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), performance_ratings::RatingError> {
    performance_ratings::delete_rating(&state.pool, &id).await
}

/// Get rating distribution for a cycle
#[tauri::command]
async fn get_rating_distribution(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<performance_ratings::RatingDistribution, performance_ratings::RatingError> {
    performance_ratings::get_rating_distribution(&state.pool, &review_cycle_id).await
}

/// Get average rating for a cycle
#[tauri::command]
async fn get_average_rating(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<Option<f64>, performance_ratings::RatingError> {
    performance_ratings::get_average_rating(&state.pool, &review_cycle_id).await
}

// ============================================================================
// Performance Review Commands
// ============================================================================

#[tauri::command]
async fn create_performance_review(
    state: tauri::State<'_, Database>,
    input: performance_reviews::CreateReview,
) -> Result<performance_reviews::PerformanceReview, performance_reviews::ReviewError> {
    performance_reviews::create_review(&state.pool, input).await
}

#[tauri::command]
async fn get_performance_review(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<performance_reviews::PerformanceReview, performance_reviews::ReviewError> {
    performance_reviews::get_review(&state.pool, &id).await
}

#[tauri::command]
async fn get_reviews_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<performance_reviews::PerformanceReview>, performance_reviews::ReviewError> {
    performance_reviews::get_reviews_for_employee(&state.pool, &employee_id).await
}

#[tauri::command]
async fn get_reviews_for_cycle(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<Vec<performance_reviews::PerformanceReview>, performance_reviews::ReviewError> {
    performance_reviews::get_reviews_for_cycle(&state.pool, &review_cycle_id).await
}

#[tauri::command]
async fn update_performance_review(
    state: tauri::State<'_, Database>,
    id: String,
    input: performance_reviews::UpdateReview,
) -> Result<performance_reviews::PerformanceReview, performance_reviews::ReviewError> {
    performance_reviews::update_review(&state.pool, &id, input).await
}

#[tauri::command]
async fn delete_performance_review(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), performance_reviews::ReviewError> {
    performance_reviews::delete_review(&state.pool, &id).await
}

#[tauri::command]
async fn search_performance_reviews(
    state: tauri::State<'_, Database>,
    query: String,
) -> Result<Vec<performance_reviews::PerformanceReview>, performance_reviews::ReviewError> {
    performance_reviews::search_reviews(&state.pool, &query).await
}

// ============================================================================
// Review Highlights Commands (V2.2.1)
// ============================================================================

/// Get highlight for a specific review
#[tauri::command]
async fn get_review_highlight(
    state: tauri::State<'_, Database>,
    review_id: String,
) -> Result<Option<highlights::ReviewHighlight>, highlights::HighlightsError> {
    highlights::get_highlight_for_review(&state.pool, &review_id).await
}

/// Get all highlights for an employee
#[tauri::command]
async fn get_highlights_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<highlights::ReviewHighlight>, highlights::HighlightsError> {
    highlights::get_highlights_for_employee(&state.pool, &employee_id).await
}

/// Extract highlights from a single review using Claude API
#[tauri::command]
async fn extract_review_highlight(
    state: tauri::State<'_, Database>,
    review_id: String,
) -> Result<highlights::ReviewHighlight, highlights::HighlightsError> {
    let review = performance_reviews::get_review(&state.pool, &review_id)
        .await
        .map_err(|e| highlights::HighlightsError::Database(e.to_string()))?;
    highlights::extract_highlights_for_review(&state.pool, &review).await
}

/// Extract highlights for multiple reviews in batch
#[tauri::command]
async fn extract_highlights_batch(
    state: tauri::State<'_, Database>,
    review_ids: Vec<String>,
) -> Result<highlights::BatchExtractionResult, highlights::HighlightsError> {
    highlights::extract_highlights_batch(&state.pool, review_ids).await
}

/// Find reviews that need highlights extracted
#[tauri::command]
async fn find_reviews_pending_extraction(
    state: tauri::State<'_, Database>,
) -> Result<Vec<String>, highlights::HighlightsError> {
    highlights::find_reviews_pending_extraction(&state.pool).await
}

/// Get employee summary
#[tauri::command]
async fn get_employee_summary(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Option<highlights::EmployeeSummary>, highlights::HighlightsError> {
    highlights::get_summary_for_employee(&state.pool, &employee_id).await
}

/// Generate employee career summary from highlights
#[tauri::command]
async fn generate_employee_summary(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<highlights::EmployeeSummary, highlights::HighlightsError> {
    highlights::generate_employee_summary(&state.pool, &employee_id).await
}

/// Invalidate highlight and summary when a review is updated
#[tauri::command]
async fn invalidate_review_highlight(
    state: tauri::State<'_, Database>,
    review_id: String,
    employee_id: String,
) -> Result<(), highlights::HighlightsError> {
    highlights::invalidate_for_review(&state.pool, &review_id, &employee_id).await
}

// ============================================================================
// eNPS Commands
// ============================================================================

#[tauri::command]
async fn create_enps_response(
    state: tauri::State<'_, Database>,
    input: enps::CreateEnps,
) -> Result<enps::EnpsResponse, enps::EnpsError> {
    enps::create_enps(&state.pool, input).await
}

#[tauri::command]
async fn get_enps_response(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<enps::EnpsResponse, enps::EnpsError> {
    enps::get_enps(&state.pool, &id).await
}

#[tauri::command]
async fn get_enps_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<enps::EnpsResponse>, enps::EnpsError> {
    enps::get_enps_for_employee(&state.pool, &employee_id).await
}

#[tauri::command]
async fn get_enps_for_survey(
    state: tauri::State<'_, Database>,
    survey_name: String,
) -> Result<Vec<enps::EnpsResponse>, enps::EnpsError> {
    enps::get_enps_for_survey(&state.pool, &survey_name).await
}

#[tauri::command]
async fn delete_enps_response(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), enps::EnpsError> {
    enps::delete_enps(&state.pool, &id).await
}

#[tauri::command]
async fn calculate_enps_score(
    state: tauri::State<'_, Database>,
    survey_name: String,
) -> Result<enps::EnpsScore, enps::EnpsError> {
    enps::calculate_enps(&state.pool, &survey_name).await
}

#[tauri::command]
async fn get_latest_enps_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Option<enps::EnpsResponse>, enps::EnpsError> {
    enps::get_latest_enps(&state.pool, &employee_id).await
}

// ============================================================================
// Bulk Import Commands (Test Data)
// ============================================================================

/// Clear all data from the database (for test data reset)
#[tauri::command]
async fn bulk_clear_data(
    state: tauri::State<'_, Database>,
) -> Result<(), bulk_import::ImportError> {
    bulk_import::clear_all_data(&state.pool).await
}

/// Bulk import review cycles with predefined IDs
#[tauri::command]
async fn bulk_import_review_cycles(
    state: tauri::State<'_, Database>,
    cycles: Vec<bulk_import::ImportReviewCycle>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_review_cycles(&state.pool, cycles).await
}

/// Bulk import employees with predefined IDs
#[tauri::command]
async fn bulk_import_employees(
    state: tauri::State<'_, Database>,
    employees: Vec<bulk_import::ImportEmployee>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_employees_bulk(&state.pool, employees).await
}

/// Bulk import performance ratings with predefined IDs
#[tauri::command]
async fn bulk_import_ratings(
    state: tauri::State<'_, Database>,
    ratings: Vec<bulk_import::ImportRating>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_ratings_bulk(&state.pool, ratings).await
}

/// Bulk import performance reviews with predefined IDs
#[tauri::command]
async fn bulk_import_reviews(
    state: tauri::State<'_, Database>,
    reviews: Vec<bulk_import::ImportReview>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_reviews_bulk(&state.pool, reviews).await
}

/// Bulk import eNPS responses with predefined IDs
#[tauri::command]
async fn bulk_import_enps(
    state: tauri::State<'_, Database>,
    responses: Vec<bulk_import::ImportEnps>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_enps_bulk(&state.pool, responses).await
}

/// Verify data integrity after import
#[tauri::command]
async fn verify_data_integrity(
    state: tauri::State<'_, Database>,
) -> Result<Vec<bulk_import::IntegrityCheckResult>, bulk_import::ImportError> {
    bulk_import::verify_integrity(&state.pool).await
}

// ============================================================================
// File Parser Commands
// ============================================================================

/// Parse a file (CSV, TSV, XLSX, XLS) and return all rows
#[tauri::command]
fn parse_file(
    data: Vec<u8>,
    file_name: String,
) -> Result<file_parser::ParseResult, file_parser::ParseError> {
    file_parser::parse_file(&data, &file_name)
}

/// Parse a file and return only a preview (first N rows)
#[tauri::command]
fn parse_file_preview(
    data: Vec<u8>,
    file_name: String,
    preview_rows: Option<usize>,
) -> Result<file_parser::ParsePreview, file_parser::ParseError> {
    file_parser::parse_file_preview(&data, &file_name, preview_rows)
}

/// Get list of supported file extensions
#[tauri::command]
fn get_supported_extensions() -> Vec<&'static str> {
    file_parser::supported_extensions()
}

/// Check if a file is supported for import
#[tauri::command]
fn is_supported_file(file_name: String) -> bool {
    file_parser::is_supported_file(&file_name)
}

/// Map parsed headers to standard employee fields
#[tauri::command]
fn map_employee_columns(
    headers: Vec<String>,
) -> std::collections::HashMap<String, String> {
    file_parser::map_employee_columns(&headers)
}

/// Map parsed headers to rating fields
#[tauri::command]
fn map_rating_columns(
    headers: Vec<String>,
) -> std::collections::HashMap<String, String> {
    file_parser::map_rating_columns(&headers)
}

/// Map parsed headers to eNPS fields
#[tauri::command]
fn map_enps_columns(
    headers: Vec<String>,
) -> std::collections::HashMap<String, String> {
    file_parser::map_enps_columns(&headers)
}

// ============================================================================
// Context Builder Commands
// ============================================================================

/// Build chat context for a user message (extracts mentions, finds employees)
/// If selected_employee_id is provided, that employee is always included first
#[tauri::command]
async fn build_chat_context(
    state: tauri::State<'_, Database>,
    user_message: String,
    selected_employee_id: Option<String>,
) -> Result<context::ChatContext, context::ContextError> {
    context::build_chat_context(&state.pool, &user_message, selected_employee_id.as_deref()).await
}

/// Get the system prompt for a chat message
/// If selected_employee_id is provided, that employee is always included first
///
/// V2.1.4: Now returns SystemPromptResult with aggregates and query_type for verification
#[tauri::command]
async fn get_system_prompt(
    state: tauri::State<'_, Database>,
    user_message: String,
    selected_employee_id: Option<String>,
) -> Result<context::SystemPromptResult, context::ContextError> {
    context::get_system_prompt_for_message(&state.pool, &user_message, selected_employee_id.as_deref()).await
}

/// Get employee context by ID (for debugging/display)
#[tauri::command]
async fn get_employee_context(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<context::EmployeeContext, context::ContextError> {
    context::get_employee_context(&state.pool, &employee_id).await
}

/// Get company context
#[tauri::command]
async fn get_company_context(
    state: tauri::State<'_, Database>,
) -> Result<Option<context::CompanyContext>, context::ContextError> {
    context::get_company_context(&state.pool).await
}

/// Get aggregate eNPS score for the organization
#[tauri::command]
async fn get_aggregate_enps(
    state: tauri::State<'_, Database>,
) -> Result<context::EnpsAggregate, context::ContextError> {
    context::calculate_aggregate_enps(&state.pool).await
}

// ============================================================================
// Analytics Commands (V2.3.2)
// ============================================================================

/// Execute an analytics request and return chart data
#[tauri::command]
async fn execute_analytics(
    state: tauri::State<'_, Database>,
    request: analytics::AnalyticsRequest,
) -> Result<analytics::ChartResult, String> {
    analytics_templates::execute_analytics(&state.pool, &request)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Insight Canvas Commands (V2.3.2g-l)
// ============================================================================

/// Create a new insight board
#[tauri::command]
async fn create_insight_board(
    state: tauri::State<'_, Database>,
    input: insight_canvas::CreateBoardInput,
) -> Result<insight_canvas::InsightBoard, insight_canvas::InsightCanvasError> {
    insight_canvas::create_board(&state.pool, input).await
}

/// Get an insight board by ID
#[tauri::command]
async fn get_insight_board(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<insight_canvas::InsightBoard, insight_canvas::InsightCanvasError> {
    insight_canvas::get_board(&state.pool, &id).await
}

/// Update an insight board
#[tauri::command]
async fn update_insight_board(
    state: tauri::State<'_, Database>,
    id: String,
    input: insight_canvas::UpdateBoardInput,
) -> Result<insight_canvas::InsightBoard, insight_canvas::InsightCanvasError> {
    insight_canvas::update_board(&state.pool, &id, input).await
}

/// Delete an insight board (and all its charts)
#[tauri::command]
async fn delete_insight_board(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), insight_canvas::InsightCanvasError> {
    insight_canvas::delete_board(&state.pool, &id).await
}

/// List all insight boards
#[tauri::command]
async fn list_insight_boards(
    state: tauri::State<'_, Database>,
) -> Result<Vec<insight_canvas::InsightBoardListItem>, insight_canvas::InsightCanvasError> {
    insight_canvas::list_boards(&state.pool).await
}

/// Pin a chart to a board
#[tauri::command]
async fn pin_chart(
    state: tauri::State<'_, Database>,
    input: insight_canvas::PinChartInput,
) -> Result<insight_canvas::PinnedChart, insight_canvas::InsightCanvasError> {
    insight_canvas::pin_chart(&state.pool, input).await
}

/// Get all charts for a board
#[tauri::command]
async fn get_charts_for_board(
    state: tauri::State<'_, Database>,
    board_id: String,
) -> Result<Vec<insight_canvas::PinnedChart>, insight_canvas::InsightCanvasError> {
    insight_canvas::get_charts_for_board(&state.pool, &board_id).await
}

/// Update a pinned chart
#[tauri::command]
async fn update_pinned_chart(
    state: tauri::State<'_, Database>,
    id: String,
    input: insight_canvas::UpdatePinnedChartInput,
) -> Result<insight_canvas::PinnedChart, insight_canvas::InsightCanvasError> {
    insight_canvas::update_pinned_chart(&state.pool, &id, input).await
}

/// Remove a chart from a board
#[tauri::command]
async fn unpin_chart(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), insight_canvas::InsightCanvasError> {
    insight_canvas::unpin_chart(&state.pool, &id).await
}

/// Create an annotation on a chart
#[tauri::command]
async fn create_chart_annotation(
    state: tauri::State<'_, Database>,
    input: insight_canvas::CreateAnnotationInput,
) -> Result<insight_canvas::ChartAnnotation, insight_canvas::InsightCanvasError> {
    insight_canvas::create_annotation(&state.pool, input).await
}

/// Get all annotations for a chart
#[tauri::command]
async fn get_annotations_for_chart(
    state: tauri::State<'_, Database>,
    chart_id: String,
) -> Result<Vec<insight_canvas::ChartAnnotation>, insight_canvas::InsightCanvasError> {
    insight_canvas::get_annotations_for_chart(&state.pool, &chart_id).await
}

/// Update an annotation
#[tauri::command]
async fn update_chart_annotation(
    state: tauri::State<'_, Database>,
    id: String,
    content: String,
) -> Result<insight_canvas::ChartAnnotation, insight_canvas::InsightCanvasError> {
    insight_canvas::update_annotation(&state.pool, &id, &content).await
}

/// Delete an annotation
#[tauri::command]
async fn delete_chart_annotation(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), insight_canvas::InsightCanvasError> {
    insight_canvas::delete_annotation(&state.pool, &id).await
}

// ============================================================================
// Attention Signals Commands (V2.4.1)
// ============================================================================

/// Check if the attention signals feature is enabled
#[tauri::command]
async fn is_signals_enabled(
    state: tauri::State<'_, Database>,
) -> Result<bool, String> {
    signals::is_signals_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Get team attention signals for all departments
/// Returns teams sorted by attention score, filtered to MIN_TEAM_SIZE
#[tauri::command]
async fn get_attention_signals(
    state: tauri::State<'_, Database>,
) -> Result<signals::AttentionAreasSummary, String> {
    // Check if feature is enabled first
    let enabled = signals::is_signals_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Attention signals feature is not enabled".to_string());
    }

    signals::get_team_attention_signals(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Get common themes for a specific team from review highlights
#[tauri::command]
async fn get_team_themes(
    state: tauri::State<'_, Database>,
    department: String,
) -> Result<Vec<signals::ThemeOccurrence>, String> {
    signals::get_common_themes_for_team(&state.pool, &department)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// DEI & Fairness Lens Commands (V2.4.2)
// ============================================================================

/// Check if the fairness lens feature is enabled
#[tauri::command]
async fn is_fairness_lens_enabled(
    state: tauri::State<'_, Database>,
) -> Result<bool, String> {
    dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Get representation breakdown by demographic field
/// @param group_by - "gender" or "ethnicity"
/// @param filter_department - Optional department filter
#[tauri::command]
async fn get_representation_breakdown(
    state: tauri::State<'_, Database>,
    group_by: String,
    filter_department: Option<String>,
) -> Result<dei::RepresentationResult, String> {
    // Check if feature is enabled first
    let enabled = dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Fairness lens feature is not enabled".to_string());
    }

    dei::get_representation_breakdown(&state.pool, &group_by, filter_department.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Get rating parity by demographic field
/// @param group_by - "gender" or "ethnicity"
#[tauri::command]
async fn get_rating_parity(
    state: tauri::State<'_, Database>,
    group_by: String,
) -> Result<dei::RatingParityResult, String> {
    // Check if feature is enabled first
    let enabled = dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Fairness lens feature is not enabled".to_string());
    }

    dei::get_rating_parity(&state.pool, &group_by)
        .await
        .map_err(|e| e.to_string())
}

/// Get promotion rates by demographic field
/// Infers promotions from job title keywords
/// @param group_by - "gender" or "ethnicity"
#[tauri::command]
async fn get_promotion_rates(
    state: tauri::State<'_, Database>,
    group_by: String,
) -> Result<dei::PromotionRatesResult, String> {
    // Check if feature is enabled first
    let enabled = dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Fairness lens feature is not enabled".to_string());
    }

    dei::get_promotion_rates(&state.pool, &group_by)
        .await
        .map_err(|e| e.to_string())
}

/// Get complete fairness lens summary (all DEI metrics)
#[tauri::command]
async fn get_fairness_lens_summary(
    state: tauri::State<'_, Database>,
) -> Result<dei::FairnessLensSummary, String> {
    dei::get_fairness_lens_summary(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Monday Digest Commands
// ============================================================================

/// Employee data for the Monday Digest (simplified for display)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DigestEmployee {
    pub id: String,
    pub full_name: String,
    pub department: Option<String>,
    pub hire_date: String,
    /// Years of tenure (for anniversaries)
    pub years_tenure: Option<i32>,
    /// Days since hire (for new hires)
    pub days_since_start: Option<i32>,
}

/// Data for the Monday Digest
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DigestData {
    /// Employees with work anniversaries this week (within 7 days)
    pub anniversaries: Vec<DigestEmployee>,
    /// New hires (hired within last 90 days)
    pub new_hires: Vec<DigestEmployee>,
}

/// Get Monday Digest data (anniversaries and new hires)
#[tauri::command]
async fn get_digest_data(
    state: tauri::State<'_, Database>,
) -> Result<DigestData, context::ContextError> {
    use chrono::{NaiveDate, Utc, Datelike};

    let today = Utc::now().date_naive();

    // Get anniversaries (within 7 days) - existing function returns 30-day window
    let anniversary_contexts = context::find_upcoming_anniversaries(&state.pool, 50).await?;

    // Filter to 7 days and convert to DigestEmployee
    let anniversaries: Vec<DigestEmployee> = anniversary_contexts
        .into_iter()
        .filter_map(|emp| {
            let hire_date = emp.hire_date.as_ref()?;
            let hire = NaiveDate::parse_from_str(hire_date, "%Y-%m-%d").ok()?;

            // Calculate this year's anniversary date
            let this_year_anniversary = NaiveDate::from_ymd_opt(today.year(), hire.month(), hire.day())?;

            // Check if anniversary is within 7 days (handles year boundary)
            let days_until = if this_year_anniversary >= today {
                (this_year_anniversary - today).num_days()
            } else {
                // Anniversary already passed this year, check next year
                let next_year_anniversary = NaiveDate::from_ymd_opt(today.year() + 1, hire.month(), hire.day())?;
                (next_year_anniversary - today).num_days()
            };

            if days_until > 7 {
                return None;
            }

            // Calculate years of tenure
            let years = today.year() - hire.year();
            let years_tenure = if this_year_anniversary > today { years } else { years + 1 };

            Some(DigestEmployee {
                id: emp.id,
                full_name: emp.full_name,
                department: emp.department,
                hire_date: hire_date.clone(),
                years_tenure: Some(years_tenure),
                days_since_start: None,
            })
        })
        .collect();

    // Get new hires (last 90 days)
    let new_hire_contexts = context::find_recent_hires(&state.pool, 90, 20).await?;

    let new_hires: Vec<DigestEmployee> = new_hire_contexts
        .into_iter()
        .filter_map(|emp| {
            let hire_date = emp.hire_date.as_ref()?;
            let hire = NaiveDate::parse_from_str(hire_date, "%Y-%m-%d").ok()?;
            let days = (today - hire).num_days() as i32;

            Some(DigestEmployee {
                id: emp.id,
                full_name: emp.full_name,
                department: emp.department,
                hire_date: hire_date.clone(),
                years_tenure: None,
                days_since_start: Some(days),
            })
        })
        .collect();

    Ok(DigestData {
        anniversaries,
        new_hires,
    })
}

// ============================================================================
// Memory Commands (Cross-Conversation Memory)
// ============================================================================

/// Generate a summary for a conversation using Claude
#[tauri::command]
async fn generate_conversation_summary(
    messages_json: String,
) -> Result<String, memory::MemoryError> {
    memory::generate_summary(&messages_json).await
}

/// Save a summary to an existing conversation
#[tauri::command]
async fn save_conversation_summary(
    state: tauri::State<'_, Database>,
    conversation_id: String,
    summary: String,
) -> Result<(), memory::MemoryError> {
    memory::save_summary(&state.pool, &conversation_id, &summary).await
}

/// Search for relevant past conversation memories
#[tauri::command]
async fn search_memories(
    state: tauri::State<'_, Database>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<memory::ConversationSummary>, memory::MemoryError> {
    let limit = limit.unwrap_or(memory::DEFAULT_MEMORY_LIMIT);
    memory::find_relevant_memories(&state.pool, &query, limit).await
}

// ============================================================================
// Conversation Management Commands
// ============================================================================

/// Create a new conversation
#[tauri::command]
async fn create_conversation(
    state: tauri::State<'_, Database>,
    input: conversations::CreateConversation,
) -> Result<conversations::Conversation, conversations::ConversationError> {
    conversations::create_conversation(&state.pool, input).await
}

/// Get a conversation by ID
#[tauri::command]
async fn get_conversation(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<conversations::Conversation, conversations::ConversationError> {
    conversations::get_conversation(&state.pool, &id).await
}

/// Update a conversation (title, messages, summary)
#[tauri::command]
async fn update_conversation(
    state: tauri::State<'_, Database>,
    id: String,
    input: conversations::UpdateConversation,
) -> Result<conversations::Conversation, conversations::ConversationError> {
    conversations::update_conversation(&state.pool, &id, input).await
}

/// List conversations for sidebar display
#[tauri::command]
async fn list_conversations(
    state: tauri::State<'_, Database>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<conversations::ConversationListItem>, conversations::ConversationError> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    conversations::list_conversations(&state.pool, limit, offset).await
}

/// Search conversations using FTS
#[tauri::command]
async fn search_conversations(
    state: tauri::State<'_, Database>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<conversations::ConversationListItem>, conversations::ConversationError> {
    let limit = limit.unwrap_or(20);
    conversations::search_conversations(&state.pool, &query, limit).await
}

/// Delete a conversation
#[tauri::command]
async fn delete_conversation(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), conversations::ConversationError> {
    conversations::delete_conversation(&state.pool, &id).await
}

/// Generate a title for a conversation
#[tauri::command]
async fn generate_conversation_title(
    first_message: String,
) -> Result<String, conversations::ConversationError> {
    Ok(conversations::generate_title_with_fallback(&first_message).await)
}

// ============================================================================
// Settings Commands
// ============================================================================

/// Get a setting value by key
#[tauri::command]
async fn get_setting(
    state: tauri::State<'_, Database>,
    key: String,
) -> Result<Option<String>, settings::SettingsError> {
    settings::get_setting(&state.pool, &key).await
}

/// Set a setting value (creates or updates)
#[tauri::command]
async fn set_setting(
    state: tauri::State<'_, Database>,
    key: String,
    value: String,
) -> Result<(), settings::SettingsError> {
    settings::set_setting(&state.pool, &key, &value).await
}

/// Delete a setting by key
#[tauri::command]
async fn delete_setting(
    state: tauri::State<'_, Database>,
    key: String,
) -> Result<(), settings::SettingsError> {
    settings::delete_setting(&state.pool, &key).await
}

/// Check if a setting exists
#[tauri::command]
async fn has_setting(
    state: tauri::State<'_, Database>,
    key: String,
) -> Result<bool, settings::SettingsError> {
    settings::has_setting(&state.pool, &key).await
}

// ============================================================================
// Persona Commands (V2.1.3)
// ============================================================================

/// Get all available HR personas for the persona switcher
#[tauri::command]
fn get_personas() -> Vec<context::Persona> {
    context::PERSONAS.to_vec()
}

// ============================================================================
// Data Path Commands
// ============================================================================

/// Get the app data directory path (where SQLite database is stored)
#[tauri::command]
fn get_data_path(app: tauri::AppHandle) -> Result<String, String> {
    let path = app.path().app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

// ============================================================================
// Device ID Commands (Trial Mode)
// ============================================================================

/// Get or create a stable device ID for trial quota tracking
#[tauri::command]
async fn get_device_id(
    state: tauri::State<'_, Database>,
) -> Result<String, settings::SettingsError> {
    device_id::get_or_create_device_id(&state.pool).await
}

// ============================================================================
// Trial Mode Commands
// ============================================================================

/// Get current trial status (is_trial, messages used/limit, employees used/limit)
#[tauri::command]
async fn get_trial_status(
    state: tauri::State<'_, Database>,
) -> Result<trial::TrialStatus, String> {
    trial::get_trial_status(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Check if adding an employee is allowed under trial limits
#[tauri::command]
async fn check_employee_limit(
    state: tauri::State<'_, Database>,
) -> Result<trial::EmployeeLimitCheck, String> {
    trial::check_employee_limit(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Backup & Restore Commands
// ============================================================================

/// Export all database tables to an encrypted backup file
#[tauri::command]
async fn export_backup(
    state: tauri::State<'_, Database>,
    password: String,
) -> Result<backup::ExportResult, backup::BackupError> {
    backup::export_backup(&state.pool, &password).await
}

/// Validate a backup file and return its metadata (without importing)
#[tauri::command]
fn validate_backup(
    encrypted_data: Vec<u8>,
    password: String,
) -> Result<backup::BackupMetadata, backup::BackupError> {
    backup::validate_backup(&encrypted_data, &password)
}

/// Import data from an encrypted backup, replacing all existing data
#[tauri::command]
async fn import_backup(
    state: tauri::State<'_, Database>,
    encrypted_data: Vec<u8>,
    password: String,
) -> Result<backup::ImportResult, backup::BackupError> {
    backup::import_backup(&state.pool, &encrypted_data, &password).await
}

// ============================================================================
// Data Quality Center Commands (V2.5.1)
// ============================================================================

/// Analyze headers from a parsed file and suggest target field mappings
#[tauri::command]
fn analyze_import_headers(
    headers: Vec<String>,
    sample_rows: Vec<file_parser::ParsedRow>,
    import_type: data_quality::ImportType,
) -> data_quality::HeaderAnalysisResult {
    data_quality::analyze_headers(&headers, &sample_rows, &import_type)
}

/// Apply a column mapping config to remap parsed row keys to target fields
#[tauri::command]
fn apply_column_mapping(
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
) -> Vec<file_parser::ParsedRow> {
    data_quality::apply_column_mapping(&rows, &mapping)
}

/// Detect potential duplicates within parsed rows (in-file)
#[tauri::command]
fn detect_duplicates(
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
) -> data_quality::DedupeResult {
    data_quality::detect_duplicates(&rows, &mapping)
}

/// Detect conflicts between parsed rows and existing DB employees
#[tauri::command]
async fn detect_existing_conflicts(
    state: tauri::State<'_, Database>,
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
) -> Result<data_quality::ExistingConflictsResult, employees::EmployeeError> {
    data_quality::detect_existing_conflicts(&state.pool, &rows, &mapping).await
}

/// Validate all rows against rules for the given import type
#[tauri::command]
fn validate_import_rows(
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
    import_type: data_quality::ImportType,
) -> data_quality::ValidationResult {
    data_quality::validate_rows(&rows, &mapping, &import_type)
}

/// Apply corrections to rows and re-validate
#[tauri::command]
fn apply_corrections_and_revalidate(
    mut rows: Vec<file_parser::ParsedRow>,
    corrections: Vec<data_quality::RowCorrection>,
    mapping: data_quality::ColumnMappingConfig,
    import_type: data_quality::ImportType,
) -> (Vec<file_parser::ParsedRow>, data_quality::ValidationResult) {
    let result = data_quality::apply_corrections_and_revalidate(
        &mut rows, &corrections, &mapping, &import_type,
    );
    (rows, result)
}

/// Get all available HRIS presets
#[tauri::command]
fn get_hris_presets() -> Vec<data_quality::HrisPreset> {
    data_quality::get_hris_presets()
}

/// Auto-detect which HRIS preset matches the given headers
#[tauri::command]
fn detect_hris_preset(headers: Vec<String>) -> Option<(String, f64)> {
    data_quality::detect_hris_preset(&headers)
}

/// Apply an HRIS preset to generate a column mapping config
#[tauri::command]
fn apply_hris_preset(
    preset_id: String,
    headers: Vec<String>,
) -> Option<data_quality::ColumnMappingConfig> {
    data_quality::apply_hris_preset(&preset_id, &headers)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            check_db,
            store_api_key,
            has_api_key,
            delete_api_key,
            validate_api_key_format,
            store_license_key,
            has_license_key,
            delete_license_key,
            validate_license_key_format,
            send_chat_message,
            send_chat_message_streaming,
            check_network_status,
            is_online,
            // Company profile
            has_company,
            get_company,
            upsert_company,
            get_employee_work_states,
            // Employee management
            create_employee,
            get_employee,
            get_employee_by_email,
            update_employee,
            delete_employee,
            list_employees,
            list_employees_with_ratings,
            get_departments,
            get_employee_counts,
            import_employees,
            // Review cycles
            create_review_cycle,
            get_review_cycle,
            update_review_cycle,
            delete_review_cycle,
            list_review_cycles,
            get_active_review_cycle,
            close_review_cycle,
            // Performance ratings
            create_performance_rating,
            get_performance_rating,
            get_ratings_for_employee,
            get_ratings_for_cycle,
            get_latest_rating,
            update_performance_rating,
            delete_performance_rating,
            get_rating_distribution,
            get_average_rating,
            // Performance reviews
            create_performance_review,
            get_performance_review,
            get_reviews_for_employee,
            get_reviews_for_cycle,
            update_performance_review,
            delete_performance_review,
            search_performance_reviews,
            // Review highlights (V2.2.1)
            get_review_highlight,
            get_highlights_for_employee,
            extract_review_highlight,
            extract_highlights_batch,
            find_reviews_pending_extraction,
            get_employee_summary,
            generate_employee_summary,
            invalidate_review_highlight,
            // eNPS
            create_enps_response,
            get_enps_response,
            get_enps_for_employee,
            get_enps_for_survey,
            delete_enps_response,
            calculate_enps_score,
            get_latest_enps_for_employee,
            // File parser
            parse_file,
            parse_file_preview,
            get_supported_extensions,
            is_supported_file,
            map_employee_columns,
            map_rating_columns,
            map_enps_columns,
            // Bulk import (test data)
            bulk_clear_data,
            bulk_import_review_cycles,
            bulk_import_employees,
            bulk_import_ratings,
            bulk_import_reviews,
            bulk_import_enps,
            verify_data_integrity,
            // Context builder
            build_chat_context,
            get_system_prompt,
            get_employee_context,
            get_company_context,
            get_aggregate_enps,
            // Analytics (V2.3.2)
            execute_analytics,
            // Insight Canvas (V2.3.2g-l)
            create_insight_board,
            get_insight_board,
            update_insight_board,
            delete_insight_board,
            list_insight_boards,
            pin_chart,
            get_charts_for_board,
            update_pinned_chart,
            unpin_chart,
            create_chart_annotation,
            get_annotations_for_chart,
            update_chart_annotation,
            delete_chart_annotation,
            // Attention Signals (V2.4.1)
            is_signals_enabled,
            get_attention_signals,
            get_team_themes,
            // DEI & Fairness Lens (V2.4.2)
            is_fairness_lens_enabled,
            get_representation_breakdown,
            get_rating_parity,
            get_promotion_rates,
            get_fairness_lens_summary,
            // Monday Digest
            get_digest_data,
            // Memory (cross-conversation)
            generate_conversation_summary,
            save_conversation_summary,
            search_memories,
            // Conversation management
            create_conversation,
            get_conversation,
            update_conversation,
            list_conversations,
            search_conversations,
            delete_conversation,
            generate_conversation_title,
            // Settings
            get_setting,
            set_setting,
            delete_setting,
            has_setting,
            // Personas (V2.1.3)
            get_personas,
            // PII scanning
            scan_pii,
            // Audit logging
            create_audit_entry,
            get_audit_entry,
            list_audit_entries,
            count_audit_entries,
            export_audit_log,
            // Device ID (trial mode)
            get_device_id,
            // Data path
            get_data_path,
            // Backup & restore
            export_backup,
            validate_backup,
            import_backup,
            // Data Quality Center (V2.5.1)
            analyze_import_headers,
            apply_column_mapping,
            detect_duplicates,
            detect_existing_conflicts,
            validate_import_rows,
            apply_corrections_and_revalidate,
            get_hris_presets,
            detect_hris_preset,
            apply_hris_preset,
            // Trial mode
            get_trial_status,
            check_employee_limit
        ])
        .setup(|app| {
            // Register updater plugin for auto-updates via GitHub Releases
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;

            let handle = app.handle().clone();

            // Initialize database asynchronously
            tauri::async_runtime::block_on(async move {
                match db::init_db(&handle).await {
                    Ok(pool) => {
                        // Store database pool in app state
                        handle.manage(Database::new(pool));
                        println!("Database initialized successfully");
                    }
                    Err(e) => {
                        eprintln!("Failed to initialize database: {}", e);
                        // In production, we might want to show an error dialog
                        // For now, we'll let the app continue and handle errors gracefully
                    }
                }
            });

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
