//! Import-pipeline commands — file parser, bulk-import (test data),
//! and the V2.5.1 Data Quality Center.

use crate::bulk_import;
use crate::data_quality;
use crate::db::Database;
use crate::employees;
use crate::file_parser;

// ============================================================================
// File Parser Commands
// ============================================================================

/// Parse a file (CSV, TSV, XLSX, XLS) and return all rows
#[tauri::command]
pub(crate) fn parse_file(
    data: Vec<u8>,
    file_name: String,
) -> Result<file_parser::ParseResult, file_parser::ParseError> {
    file_parser::parse_file(&data, &file_name)
}

/// Parse a file and return only a preview (first N rows)
#[tauri::command]
pub(crate) fn parse_file_preview(
    data: Vec<u8>,
    file_name: String,
    preview_rows: Option<usize>,
) -> Result<file_parser::ParsePreview, file_parser::ParseError> {
    file_parser::parse_file_preview(&data, &file_name, preview_rows)
}

/// Get list of supported file extensions
#[tauri::command]
pub(crate) fn get_supported_extensions() -> Vec<&'static str> {
    file_parser::supported_extensions()
}

/// Check if a file is supported for import
#[tauri::command]
pub(crate) fn is_supported_file(file_name: String) -> bool {
    file_parser::is_supported_file(&file_name)
}

/// Map parsed headers to standard employee fields
#[tauri::command]
pub(crate) fn map_employee_columns(
    headers: Vec<String>,
) -> std::collections::HashMap<String, String> {
    file_parser::map_employee_columns(&headers)
}

/// Map parsed headers to rating fields
#[tauri::command]
pub(crate) fn map_rating_columns(
    headers: Vec<String>,
) -> std::collections::HashMap<String, String> {
    file_parser::map_rating_columns(&headers)
}

/// Map parsed headers to eNPS fields
#[tauri::command]
pub(crate) fn map_enps_columns(
    headers: Vec<String>,
) -> std::collections::HashMap<String, String> {
    file_parser::map_enps_columns(&headers)
}

// ============================================================================
// Bulk Import Commands (Test Data)
// ============================================================================

/// Clear all data from the database (for test data reset)
#[tauri::command]
pub(crate) async fn bulk_clear_data(
    state: tauri::State<'_, Database>,
) -> Result<(), bulk_import::ImportError> {
    bulk_import::clear_all_data(&state.pool).await
}

/// Bulk import review cycles with predefined IDs
#[tauri::command]
pub(crate) async fn bulk_import_review_cycles(
    state: tauri::State<'_, Database>,
    cycles: Vec<bulk_import::ImportReviewCycle>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_review_cycles(&state.pool, cycles).await
}

/// Bulk import employees with predefined IDs
#[tauri::command]
pub(crate) async fn bulk_import_employees(
    state: tauri::State<'_, Database>,
    employees: Vec<bulk_import::ImportEmployee>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_employees_bulk(&state.pool, employees).await
}

/// Bulk import performance ratings with predefined IDs
#[tauri::command]
pub(crate) async fn bulk_import_ratings(
    state: tauri::State<'_, Database>,
    ratings: Vec<bulk_import::ImportRating>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_ratings_bulk(&state.pool, ratings).await
}

/// Bulk import performance reviews with predefined IDs
#[tauri::command]
pub(crate) async fn bulk_import_reviews(
    state: tauri::State<'_, Database>,
    reviews: Vec<bulk_import::ImportReview>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_reviews_bulk(&state.pool, reviews).await
}

/// Bulk import eNPS responses with predefined IDs
#[tauri::command]
pub(crate) async fn bulk_import_enps(
    state: tauri::State<'_, Database>,
    responses: Vec<bulk_import::ImportEnps>,
) -> Result<bulk_import::BulkImportResult, bulk_import::ImportError> {
    bulk_import::import_enps_bulk(&state.pool, responses).await
}

/// Verify data integrity after import
#[tauri::command]
pub(crate) async fn verify_data_integrity(
    state: tauri::State<'_, Database>,
) -> Result<Vec<bulk_import::IntegrityCheckResult>, bulk_import::ImportError> {
    bulk_import::verify_integrity(&state.pool).await
}

// ============================================================================
// Data Quality Center Commands (V2.5.1)
// ============================================================================

/// Analyze headers from a parsed file and suggest target field mappings
#[tauri::command]
pub(crate) fn analyze_import_headers(
    headers: Vec<String>,
    sample_rows: Vec<file_parser::ParsedRow>,
    import_type: data_quality::ImportType,
) -> data_quality::HeaderAnalysisResult {
    data_quality::analyze_headers(&headers, &sample_rows, &import_type)
}

/// Apply a column mapping config to remap parsed row keys to target fields
#[tauri::command]
pub(crate) fn apply_column_mapping(
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
) -> Vec<file_parser::ParsedRow> {
    data_quality::apply_column_mapping(&rows, &mapping)
}

/// Detect potential duplicates within parsed rows (in-file)
#[tauri::command]
pub(crate) fn detect_duplicates(
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
) -> data_quality::DedupeResult {
    data_quality::detect_duplicates(&rows, &mapping)
}

/// Detect conflicts between parsed rows and existing DB employees
#[tauri::command]
pub(crate) async fn detect_existing_conflicts(
    state: tauri::State<'_, Database>,
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
) -> Result<data_quality::ExistingConflictsResult, employees::EmployeeError> {
    data_quality::detect_existing_conflicts(&state.pool, &rows, &mapping).await
}

/// Validate all rows against rules for the given import type
#[tauri::command]
pub(crate) fn validate_import_rows(
    rows: Vec<file_parser::ParsedRow>,
    mapping: data_quality::ColumnMappingConfig,
    import_type: data_quality::ImportType,
) -> data_quality::ValidationResult {
    data_quality::validate_rows(&rows, &mapping, &import_type)
}

/// Apply corrections to rows and re-validate
#[tauri::command]
pub(crate) fn apply_corrections_and_revalidate(
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
pub(crate) fn get_hris_presets() -> Vec<data_quality::HrisPreset> {
    data_quality::get_hris_presets()
}

/// Auto-detect which HRIS preset matches the given headers
#[tauri::command]
pub(crate) fn detect_hris_preset(headers: Vec<String>) -> Option<(String, f64)> {
    data_quality::detect_hris_preset(&headers)
}

/// Apply an HRIS preset to generate a column mapping config
#[tauri::command]
pub(crate) fn apply_hris_preset(
    preset_id: String,
    headers: Vec<String>,
) -> Option<data_quality::ColumnMappingConfig> {
    data_quality::apply_hris_preset(&preset_id, &headers)
}
