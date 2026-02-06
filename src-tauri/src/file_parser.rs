// HR Command Center - Unified File Parser
// Supports CSV, TSV, XLSX, and XLS file formats
// Returns a consistent structure regardless of input format

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use thiserror::Error;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Error, Serialize)]
pub enum ParseError {
    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("Failed to read file: {0}")]
    ReadError(String),

    #[error("Invalid file structure: {0}")]
    InvalidStructure(String),

    #[error("No data found in file")]
    NoData,

    #[error("No headers found in first row")]
    NoHeaders,
}

/// A single parsed row as column_name -> value
pub type ParsedRow = HashMap<String, String>;

/// Result of parsing a file
#[derive(Debug, Serialize, Deserialize)]
pub struct ParseResult {
    /// Column headers from the first row
    pub headers: Vec<String>,
    /// All data rows (excluding header)
    pub rows: Vec<ParsedRow>,
    /// Total number of data rows
    pub total_rows: usize,
    /// Detected file format (CSV, TSV, XLSX, XLS)
    pub file_format: String,
    /// Warnings during parsing (e.g., skipped rows)
    pub warnings: Vec<String>,
}

/// Preview result (limited rows for UI display)
#[derive(Debug, Serialize, Deserialize)]
pub struct ParsePreview {
    /// Column headers
    pub headers: Vec<String>,
    /// First N rows for preview
    pub preview_rows: Vec<ParsedRow>,
    /// Total rows in file (not just preview)
    pub total_rows: usize,
    /// Detected file format
    pub file_format: String,
}

/// Supported file formats
#[derive(Debug, Clone, Copy, PartialEq)]
enum FileFormat {
    Csv,
    Tsv,
    Xlsx,
    Xls,
}

impl FileFormat {
    fn as_str(&self) -> &'static str {
        match self {
            FileFormat::Csv => "CSV",
            FileFormat::Tsv => "TSV",
            FileFormat::Xlsx => "XLSX",
            FileFormat::Xls => "XLS",
        }
    }
}

// ============================================================================
// Format Detection
// ============================================================================

/// Detect file format from filename extension
fn detect_format(file_name: &str) -> Result<FileFormat, ParseError> {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" => Ok(FileFormat::Csv),
        "tsv" => Ok(FileFormat::Tsv),
        "xlsx" => Ok(FileFormat::Xlsx),
        "xls" => Ok(FileFormat::Xls),
        _ => Err(ParseError::UnsupportedFormat(format!(
            ".{} - supported formats: .csv, .tsv, .xlsx, .xls",
            ext
        ))),
    }
}

// ============================================================================
// CSV/TSV Parsing
// ============================================================================

/// Parse delimited text (CSV or TSV)
fn parse_delimited(data: &[u8], delimiter: u8, format: FileFormat) -> Result<ParseResult, ParseError> {
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true) // Allow rows with varying column counts
        .trim(csv::Trim::All)
        .from_reader(data);

    // Extract headers
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| ParseError::ReadError(format!("Failed to read headers: {}", e)))?
        .iter()
        .map(|h| normalize_header(h))
        .collect();

    if headers.is_empty() {
        return Err(ParseError::NoHeaders);
    }

    // Parse data rows
    let mut rows = Vec::new();
    let mut warnings = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        match result {
            Ok(record) => {
                let mut row = HashMap::new();
                for (i, value) in record.iter().enumerate() {
                    if i < headers.len() {
                        let trimmed = value.trim();
                        // Only include non-empty values
                        if !trimmed.is_empty() {
                            row.insert(headers[i].clone(), trimmed.to_string());
                        }
                    }
                }
                // Only include rows that have at least one value
                if !row.is_empty() {
                    rows.push(row);
                }
            }
            Err(e) => {
                warnings.push(format!("Row {}: {}", idx + 2, e)); // +2 for 1-indexed + header
            }
        }
    }

    if rows.is_empty() {
        return Err(ParseError::NoData);
    }

    Ok(ParseResult {
        headers,
        total_rows: rows.len(),
        rows,
        file_format: format.as_str().to_string(),
        warnings,
    })
}

// ============================================================================
// Excel Parsing
// ============================================================================

/// Parse Excel file (XLSX or XLS)
fn parse_excel(data: &[u8], format: FileFormat) -> Result<ParseResult, ParseError> {
    // Create cursor for reading from bytes
    let cursor = Cursor::new(data);

    // Open workbook from bytes
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|e| ParseError::ReadError(format!("Failed to open Excel file: {}", e)))?;

    // Get first sheet
    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err(ParseError::NoData);
    }

    let range = workbook
        .worksheet_range(&sheet_names[0])
        .map_err(|e| ParseError::ReadError(format!("Failed to read worksheet: {}", e)))?;

    // Get dimensions
    let (row_count, col_count) = range.get_size();
    if row_count == 0 || col_count == 0 {
        return Err(ParseError::NoData);
    }

    // Extract headers from first row
    let mut headers: Vec<String> = Vec::new();
    for col in 0..col_count {
        let cell = range.get((0, col));
        let header = match cell {
            Some(Data::String(s)) => normalize_header(s),
            Some(Data::Int(n)) => normalize_header(&n.to_string()),
            Some(Data::Float(n)) => normalize_header(&n.to_string()),
            Some(Data::Bool(b)) => normalize_header(&b.to_string()),
            Some(Data::DateTime(dt)) => normalize_header(&dt.to_string()),
            Some(Data::Error(e)) => normalize_header(&format!("{:?}", e)),
            Some(Data::Empty) | Some(Data::DateTimeIso(_)) | Some(Data::DurationIso(_)) | None => {
                format!("column_{}", col + 1)
            }
        };
        headers.push(header);
    }

    if headers.is_empty() {
        return Err(ParseError::NoHeaders);
    }

    // Parse data rows (skip header row)
    let mut rows = Vec::new();
    let mut warnings = Vec::new();

    for row_idx in 1..row_count {
        let mut row = HashMap::new();
        let mut has_data = false;

        for col_idx in 0..col_count {
            let cell = range.get((row_idx, col_idx));
            let value = cell_to_string(cell);

            if !value.is_empty() && col_idx < headers.len() {
                row.insert(headers[col_idx].clone(), value);
                has_data = true;
            }
        }

        // Only include rows with actual data
        if has_data {
            rows.push(row);
        }
    }

    if rows.is_empty() {
        return Err(ParseError::NoData);
    }

    // Note which sheet was used if there are multiple
    if sheet_names.len() > 1 {
        warnings.push(format!(
            "File has {} sheets. Using first sheet: '{}'",
            sheet_names.len(),
            sheet_names[0]
        ));
    }

    Ok(ParseResult {
        headers,
        total_rows: rows.len(),
        rows,
        file_format: format.as_str().to_string(),
        warnings,
    })
}

/// Convert Excel cell to string
fn cell_to_string(cell: Option<&Data>) -> String {
    match cell {
        Some(Data::String(s)) => s.trim().to_string(),
        Some(Data::Int(n)) => n.to_string(),
        Some(Data::Float(n)) => {
            // Format floats nicely (remove trailing zeros)
            let formatted = format!("{}", n);
            formatted
        }
        Some(Data::Bool(b)) => b.to_string(),
        Some(Data::DateTime(dt)) => {
            // Convert Excel datetime to string using chrono
            excel_datetime_to_string(dt.as_f64())
        }
        Some(Data::DateTimeIso(s)) => s.clone(),
        Some(Data::DurationIso(s)) => s.clone(),
        Some(Data::Error(_)) | Some(Data::Empty) | None => String::new(),
    }
}

/// Convert Excel serial datetime to ISO date string
fn excel_datetime_to_string(serial: f64) -> String {
    // Excel epoch is 1899-12-30 (with a bug where 1900 is considered a leap year)
    // Serial 1 = 1900-01-01
    use chrono::{Duration, NaiveDate};

    // Handle the Excel leap year bug (dates >= 60 need adjustment)
    let adjusted = if serial >= 60.0 { serial - 1.0 } else { serial };

    // Excel epoch
    if let Some(epoch) = NaiveDate::from_ymd_opt(1899, 12, 30) {
        let days = adjusted.floor() as i64;
        if let Some(date) = epoch.checked_add_signed(Duration::days(days)) {
            return date.format("%Y-%m-%d").to_string();
        }
    }

    // Fallback to raw number
    format!("{}", serial)
}

// ============================================================================
// Header Normalization
// ============================================================================

/// Normalize header names for consistent field matching
pub fn normalize_header(header: &str) -> String {
    header
        .trim()
        .to_lowercase()
        .replace(' ', "_")
        .replace('-', "_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

// ============================================================================
// Public API
// ============================================================================

/// Parse a file from raw bytes
///
/// # Arguments
/// * `data` - Raw file bytes
/// * `file_name` - Original filename (used for format detection)
///
/// # Returns
/// * `ParseResult` with headers, rows, and metadata
pub fn parse_file(data: &[u8], file_name: &str) -> Result<ParseResult, ParseError> {
    let format = detect_format(file_name)?;

    match format {
        FileFormat::Csv => parse_delimited(data, b',', format),
        FileFormat::Tsv => parse_delimited(data, b'\t', format),
        FileFormat::Xlsx | FileFormat::Xls => parse_excel(data, format),
    }
}

/// Parse a file and return only a preview (first N rows)
/// Useful for showing users what they're importing before committing
///
/// # Arguments
/// * `data` - Raw file bytes
/// * `file_name` - Original filename
/// * `preview_rows` - Number of rows to include in preview (default: 5)
pub fn parse_file_preview(
    data: &[u8],
    file_name: &str,
    preview_rows: Option<usize>,
) -> Result<ParsePreview, ParseError> {
    let limit = preview_rows.unwrap_or(5);
    let result = parse_file(data, file_name)?;

    Ok(ParsePreview {
        headers: result.headers,
        preview_rows: result.rows.into_iter().take(limit).collect(),
        total_rows: result.total_rows,
        file_format: result.file_format,
    })
}

/// Get list of supported file extensions
pub fn supported_extensions() -> Vec<&'static str> {
    vec!["csv", "tsv", "xlsx", "xls"]
}

/// Check if a filename has a supported extension
pub fn is_supported_file(file_name: &str) -> bool {
    detect_format(file_name).is_ok()
}

// ============================================================================
// Column Mapping Helpers
// ============================================================================

/// Standard column names we look for when importing employees
pub const EMPLOYEE_COLUMN_MAPPINGS: &[(&str, &[&str])] = &[
    ("email", &["email", "email_address", "e_mail", "emailaddress", "work_email"]),
    ("first_name", &["first_name", "firstname", "first", "given_name", "givenname"]),
    ("last_name", &["last_name", "lastname", "last", "surname", "family_name", "familyname"]),
    ("department", &["department", "dept", "team", "division", "group"]),
    ("title", &["title", "job_title", "jobtitle", "position", "role"]),
    ("hire_date", &["hire_date", "hiredate", "start_date", "startdate", "date_hired", "joined"]),
    ("work_state", &["work_state", "workstate", "state", "location_state", "work_location"]),
    ("manager_email", &["manager_email", "manageremail", "manager", "reports_to", "reportsto"]),
    ("status", &["status", "employment_status", "employmentstatus", "active"]),
    ("date_of_birth", &["date_of_birth", "dateofbirth", "dob", "birth_date", "birthdate"]),
    ("gender", &["gender", "sex"]),
    ("ethnicity", &["ethnicity", "race", "race_ethnicity"]),
];

/// Try to map parsed headers to standard employee fields
/// Returns a map of standard_field -> parsed_header
pub fn map_employee_columns(headers: &[String]) -> HashMap<String, String> {
    let mut mapping = HashMap::new();

    for (standard_field, alternatives) in EMPLOYEE_COLUMN_MAPPINGS {
        for header in headers {
            let normalized = normalize_header(header);
            if alternatives.contains(&normalized.as_str()) {
                mapping.insert(standard_field.to_string(), header.clone());
                break;
            }
        }
    }

    mapping
}

/// Standard column names for performance ratings import
pub const RATING_COLUMN_MAPPINGS: &[(&str, &[&str])] = &[
    ("employee_email", &["email", "employee_email", "employeeemail", "employee"]),
    ("rating", &["rating", "score", "performance_rating", "performancerating", "overall_rating"]),
    ("cycle_name", &["cycle", "cycle_name", "cyclename", "review_cycle", "period", "review_period"]),
    ("rated_at", &["rated_at", "ratedat", "date", "rating_date", "ratingdate"]),
    ("notes", &["notes", "comments", "feedback", "rating_notes"]),
];

/// Try to map parsed headers to rating fields
pub fn map_rating_columns(headers: &[String]) -> HashMap<String, String> {
    let mut mapping = HashMap::new();

    for (standard_field, alternatives) in RATING_COLUMN_MAPPINGS {
        for header in headers {
            let normalized = normalize_header(header);
            if alternatives.contains(&normalized.as_str()) {
                mapping.insert(standard_field.to_string(), header.clone());
                break;
            }
        }
    }

    mapping
}

/// Standard column names for eNPS import
pub const ENPS_COLUMN_MAPPINGS: &[(&str, &[&str])] = &[
    ("employee_email", &["email", "employee_email", "employeeemail", "employee"]),
    ("score", &["score", "enps_score", "enpsscore", "rating", "nps_score"]),
    ("survey_name", &["survey", "survey_name", "surveyname", "survey_id", "period"]),
    ("responded_at", &["responded_at", "respondedat", "date", "response_date", "responsedate", "submitted_at"]),
    ("comment", &["comment", "comments", "feedback", "verbatim", "open_ended"]),
];

/// Try to map parsed headers to eNPS fields
pub fn map_enps_columns(headers: &[String]) -> HashMap<String, String> {
    let mut mapping = HashMap::new();

    for (standard_field, alternatives) in ENPS_COLUMN_MAPPINGS {
        for header in headers {
            let normalized = normalize_header(header);
            if alternatives.contains(&normalized.as_str()) {
                mapping.insert(standard_field.to_string(), header.clone());
                break;
            }
        }
    }

    mapping
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_header() {
        assert_eq!(normalize_header("First Name"), "first_name");
        assert_eq!(normalize_header("E-Mail"), "e_mail");
        assert_eq!(normalize_header("  Hire Date  "), "hire_date");
        assert_eq!(normalize_header("Department #1"), "department_1");
    }

    #[test]
    fn test_detect_format() {
        assert!(matches!(detect_format("employees.csv"), Ok(FileFormat::Csv)));
        assert!(matches!(detect_format("data.XLSX"), Ok(FileFormat::Xlsx)));
        assert!(matches!(detect_format("report.tsv"), Ok(FileFormat::Tsv)));
        assert!(detect_format("readme.txt").is_err());
    }

    #[test]
    fn test_parse_csv() {
        let csv_data = b"email,first_name,last_name\njohn@acme.com,John,Doe\njane@acme.com,Jane,Smith";
        let result = parse_file(csv_data, "employees.csv").unwrap();

        assert_eq!(result.headers.len(), 3);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.file_format, "CSV");
        assert_eq!(result.rows[0].get("email"), Some(&"john@acme.com".to_string()));
    }

    #[test]
    fn test_column_mapping() {
        let headers = vec![
            "E-Mail".to_string(),
            "First Name".to_string(),
            "Dept".to_string(),
            "Start Date".to_string(),
        ];
        let mapping = map_employee_columns(&headers);

        assert_eq!(mapping.get("email"), Some(&"E-Mail".to_string()));
        assert_eq!(mapping.get("first_name"), Some(&"First Name".to_string()));
        assert_eq!(mapping.get("department"), Some(&"Dept".to_string()));
        assert_eq!(mapping.get("hire_date"), Some(&"Start Date".to_string()));
    }
}
