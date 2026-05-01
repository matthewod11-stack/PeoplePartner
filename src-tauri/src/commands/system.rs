//! System and infrastructure commands — settings, audit, PII, device ID,
//! data path, trial mode, backup, and the legacy greet/check_db handlers.

use tauri::Manager;

use crate::audit;
use crate::backup;
use crate::db::Database;
use crate::device_id;
use crate::pii;
use crate::settings;
use crate::trial;

/// Greet command for testing - will be replaced with actual commands
#[tauri::command]
pub(crate) fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to People Partner.", name)
}

/// Check if database is initialized
#[tauri::command]
pub(crate) fn check_db(state: tauri::State<'_, Database>) -> bool {
    // If we can access the state, the database is initialized
    let _ = &state.pool;
    true
}

/// Scan text for PII and return redaction result
/// Used by frontend before sending messages to Claude API
#[tauri::command]
pub(crate) fn scan_pii(text: String) -> pii::RedactionResult {
    pii::scan_and_redact(&text)
}

/// Create an audit log entry after a Claude API interaction
/// Called by frontend after streaming response completes
#[tauri::command]
pub(crate) async fn create_audit_entry(
    state: tauri::State<'_, Database>,
    input: audit::CreateAuditEntry,
) -> Result<audit::AuditEntry, audit::AuditError> {
    audit::create_audit_entry(&state.pool, input).await
}

/// Get a single audit entry by ID
#[tauri::command]
pub(crate) async fn get_audit_entry(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<audit::AuditEntry, audit::AuditError> {
    audit::get_audit_entry(&state.pool, &id).await
}

/// List audit entries with optional filtering
#[tauri::command]
pub(crate) async fn list_audit_entries(
    state: tauri::State<'_, Database>,
    filter: Option<audit::AuditFilter>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<audit::AuditListItem>, audit::AuditError> {
    audit::list_audit_entries(&state.pool, filter, limit, offset).await
}

/// Count audit entries matching filter (for pagination)
#[tauri::command]
pub(crate) async fn count_audit_entries(
    state: tauri::State<'_, Database>,
    filter: Option<audit::AuditFilter>,
) -> Result<i64, audit::AuditError> {
    audit::count_audit_entries(&state.pool, filter).await
}

/// Export audit log to CSV format
#[tauri::command]
pub(crate) async fn export_audit_log(
    state: tauri::State<'_, Database>,
    filter: Option<audit::AuditFilter>,
) -> Result<audit::ExportResult, audit::AuditError> {
    audit::export_to_csv(&state.pool, filter).await
}

/// Get a setting value by key
#[tauri::command]
pub(crate) async fn get_setting(
    state: tauri::State<'_, Database>,
    key: String,
) -> Result<Option<String>, settings::SettingsError> {
    settings::get_setting(&state.pool, &key).await
}

/// Set a setting value (creates or updates)
#[tauri::command]
pub(crate) async fn set_setting(
    state: tauri::State<'_, Database>,
    key: String,
    value: String,
) -> Result<(), settings::SettingsError> {
    settings::set_setting(&state.pool, &key, &value).await
}

/// Delete a setting by key
#[tauri::command]
pub(crate) async fn delete_setting(
    state: tauri::State<'_, Database>,
    key: String,
) -> Result<(), settings::SettingsError> {
    settings::delete_setting(&state.pool, &key).await
}

/// Check if a setting exists
#[tauri::command]
pub(crate) async fn has_setting(
    state: tauri::State<'_, Database>,
    key: String,
) -> Result<bool, settings::SettingsError> {
    settings::has_setting(&state.pool, &key).await
}

/// Get the app data directory path (where SQLite database is stored)
#[tauri::command]
pub(crate) fn get_data_path(app: tauri::AppHandle) -> Result<String, String> {
    let path = app.path().app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Get or create a stable device ID for trial quota tracking
#[tauri::command]
pub(crate) async fn get_device_id(
    state: tauri::State<'_, Database>,
) -> Result<String, settings::SettingsError> {
    device_id::get_or_create_device_id(&state.pool).await
}

/// Get current trial status (is_trial, messages used/limit, employees used/limit)
#[tauri::command]
pub(crate) async fn get_trial_status(
    state: tauri::State<'_, Database>,
) -> Result<trial::TrialStatus, String> {
    trial::get_trial_status(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Check if adding an employee is allowed under trial limits
#[tauri::command]
pub(crate) async fn check_employee_limit(
    state: tauri::State<'_, Database>,
) -> Result<trial::EmployeeLimitCheck, String> {
    trial::check_employee_limit(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Export all database tables to an encrypted backup file
#[tauri::command]
pub(crate) async fn export_backup(
    state: tauri::State<'_, Database>,
    password: String,
) -> Result<backup::ExportResult, backup::BackupError> {
    backup::export_backup(&state.pool, &password).await
}

/// Validate a backup file and return its metadata (without importing)
#[tauri::command]
pub(crate) fn validate_backup(
    encrypted_data: Vec<u8>,
    password: String,
) -> Result<backup::BackupMetadata, backup::BackupError> {
    backup::validate_backup(&encrypted_data, &password)
}

/// Import data from an encrypted backup, replacing all existing data
#[tauri::command]
pub(crate) async fn import_backup(
    state: tauri::State<'_, Database>,
    encrypted_data: Vec<u8>,
    password: String,
) -> Result<backup::ImportResult, backup::BackupError> {
    backup::import_backup(&state.pool, &encrypted_data, &password).await
}
