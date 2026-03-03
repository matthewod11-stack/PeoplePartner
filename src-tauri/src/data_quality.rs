// People Partner - Data Quality Module (V2.5.1)
// Pre-import validation, deduplication detection, column mapping, and HRIS presets.
// All operations are in-memory on parsed data before import — no DB schema changes needed.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::db::DbPool;
use crate::employees;
use crate::file_parser::{
    normalize_header, ParsedRow, EMPLOYEE_COLUMN_MAPPINGS, ENPS_COLUMN_MAPPINGS,
    RATING_COLUMN_MAPPINGS,
};

// ============================================================================
// Column Mapping Types (V2.5.1a)
// ============================================================================

/// A user-defined or auto-detected mapping from source header to target field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMappingConfig {
    /// Map of target_field_name -> source_header_name
    /// e.g. { "email": "Employee Email", "first_name": "First Name" }
    pub mappings: HashMap<String, String>,
    /// Optional HRIS preset name that was used (for audit trail)
    pub preset_name: Option<String>,
}

/// Import type determines which target fields and validation rules apply
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportType {
    Employees,
    Ratings,
    Reviews,
    Enps,
}

// ============================================================================
// Header Normalization Preview Types (V2.5.1b)
// ============================================================================

/// Result of analyzing a single header column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderAnalysis {
    /// Original header as it appears in the file
    pub original: String,
    /// Normalized version (lowercase, underscored)
    pub normalized: String,
    /// Suggested target field name (if auto-detected), or None
    pub suggested_field: Option<String>,
    /// Confidence of the suggestion: "exact", "alias", "fuzzy", or "none"
    pub confidence: String,
    /// Sample values from first 3 rows (for UI preview)
    pub sample_values: Vec<String>,
}

/// Complete result of analyzing all headers in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderAnalysisResult {
    pub headers: Vec<HeaderAnalysis>,
    /// Which target fields are required but not yet mapped
    pub unmapped_required: Vec<String>,
    /// Which target fields are optional and not mapped
    pub unmapped_optional: Vec<String>,
    /// The import type that was analyzed against
    pub import_type: ImportType,
}

// ============================================================================
// Dedupe Detection Types (V2.5.1c)
// ============================================================================

/// A pair of rows that may be duplicates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatePair {
    /// Index of first row (0-based, within parsed rows)
    pub row_a: usize,
    /// Index of second row
    pub row_b: usize,
    /// Match type: "exact_email", "fuzzy_name_dob", "fuzzy_name_only"
    pub match_type: String,
    /// Confidence score 0.0-1.0
    pub confidence: f64,
    /// The field values that matched (for display)
    pub matched_fields: HashMap<String, (String, String)>,
}

/// Result of dedupe detection across all rows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeResult {
    /// Pairs of potential duplicates found
    pub duplicates: Vec<DuplicatePair>,
    /// Total rows analyzed
    pub total_rows: usize,
    /// Row indices involved in at least one duplicate pair
    pub affected_rows: Vec<usize>,
}

/// A parsed row that conflicts with an existing DB record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingConflict {
    /// Index of the import row (0-based)
    pub row_index: usize,
    /// Email that matched
    pub email: String,
    /// Existing employee ID in the database
    pub existing_employee_id: String,
    /// Existing employee name for display
    pub existing_employee_name: String,
    /// Suggested action: "update" (default for email matches)
    pub suggested_action: String,
}

/// Result of checking parsed rows against existing DB employees
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingConflictsResult {
    pub conflicts: Vec<ExistingConflict>,
    pub new_rows: usize,
    pub update_rows: usize,
}

// ============================================================================
// Validation Types (V2.5.1d)
// ============================================================================

/// Severity of a validation issue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

/// A single validation issue on a specific row+field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Row index (0-based within parsed rows)
    pub row_index: usize,
    /// The field name that has the issue (target field name)
    pub field: String,
    /// Current value (or empty string if missing)
    pub value: String,
    /// Human-readable description of the issue
    pub message: String,
    /// Error or Warning
    pub severity: ValidationSeverity,
    /// Machine-readable rule code
    pub rule: String,
    /// Suggested fix value (if deterministic), or None
    pub suggested_fix: Option<String>,
}

/// Complete validation result for an import batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
    pub error_count: usize,
    pub warning_count: usize,
    pub total_rows: usize,
    pub valid_rows: usize,
    /// Whether the batch can proceed (error_count == 0)
    pub can_import: bool,
}

// ============================================================================
// Fix-and-Retry Types (V2.5.1e)
// ============================================================================

/// A correction to a specific cell value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowCorrection {
    /// Row index (0-based)
    pub row_index: usize,
    /// Target field name
    pub field: String,
    /// New corrected value
    pub new_value: String,
}

// ============================================================================
// HRIS Preset Types (V2.5.1f)
// ============================================================================

/// A predefined mapping configuration for a known HRIS provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrisPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Header mappings: (target_field, vec_of_possible_source_headers)
    pub header_mappings: Vec<(String, Vec<String>)>,
}

// ============================================================================
// Constants
// ============================================================================

const REQUIRED_EMPLOYEE_FIELDS: &[&str] = &["email"];
const OPTIONAL_EMPLOYEE_FIELDS: &[&str] = &[
    "first_name", "last_name", "department", "title", "hire_date",
    "work_state", "manager_email", "status", "date_of_birth", "gender", "ethnicity",
];

const REQUIRED_RATING_FIELDS: &[&str] = &["employee_email", "rating"];
const OPTIONAL_RATING_FIELDS: &[&str] = &["cycle_name", "rated_at", "notes"];

const REQUIRED_ENPS_FIELDS: &[&str] = &["employee_email", "score"];
const OPTIONAL_ENPS_FIELDS: &[&str] = &["survey_name", "responded_at", "comment"];

const US_STATE_CODES: &[&str] = &[
    "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "FL", "GA", "HI", "ID", "IL", "IN", "IA",
    "KS", "KY", "LA", "ME", "MD", "MA", "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ",
    "NM", "NY", "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC", "SD", "TN", "TX", "UT", "VT",
    "VA", "WA", "WV", "WI", "WY", "DC",
];

const FUZZY_THRESHOLD: f64 = 0.7;

// ============================================================================
// V2.5.1f — HRIS Presets
// ============================================================================

pub fn get_hris_presets() -> Vec<HrisPreset> {
    vec![
        HrisPreset {
            id: "bamboohr".to_string(),
            name: "BambooHR".to_string(),
            description: "Standard BambooHR employee export format".to_string(),
            header_mappings: vec![
                ("email".into(), vec!["work email".into(), "email address".into(), "email".into()]),
                ("first_name".into(), vec!["first name".into()]),
                ("last_name".into(), vec!["last name".into()]),
                ("department".into(), vec!["department".into()]),
                ("title".into(), vec!["job title".into()]),
                ("hire_date".into(), vec!["hire date".into(), "original hire date".into()]),
                ("work_state".into(), vec!["state".into(), "state/province".into()]),
                ("manager_email".into(), vec!["supervisor email".into(), "reports to email".into()]),
                ("status".into(), vec!["status".into(), "employment status".into()]),
                ("date_of_birth".into(), vec!["date of birth".into(), "birth date".into()]),
                ("gender".into(), vec!["gender".into()]),
                ("ethnicity".into(), vec!["ethnicity".into(), "race/ethnicity".into()]),
            ],
        },
        HrisPreset {
            id: "gusto".to_string(),
            name: "Gusto".to_string(),
            description: "Standard Gusto people export format".to_string(),
            header_mappings: vec![
                ("email".into(), vec!["email".into(), "work email".into()]),
                ("first_name".into(), vec!["first name".into(), "legal first name".into()]),
                ("last_name".into(), vec!["last name".into(), "legal last name".into()]),
                ("department".into(), vec!["department".into(), "team".into()]),
                ("title".into(), vec!["job title".into(), "title".into()]),
                ("hire_date".into(), vec!["start date".into(), "hire date".into()]),
                ("work_state".into(), vec!["work state".into(), "state".into()]),
                ("manager_email".into(), vec!["manager email".into(), "manager".into()]),
                ("status".into(), vec!["employment status".into(), "status".into()]),
                ("date_of_birth".into(), vec!["date of birth".into()]),
            ],
        },
        HrisPreset {
            id: "rippling".to_string(),
            name: "Rippling".to_string(),
            description: "Standard Rippling employee export format".to_string(),
            header_mappings: vec![
                ("email".into(), vec!["work email".into(), "company email".into(), "email".into()]),
                ("first_name".into(), vec!["preferred first name".into(), "first name".into(), "legal first name".into()]),
                ("last_name".into(), vec!["preferred last name".into(), "last name".into(), "legal last name".into()]),
                ("department".into(), vec!["department".into()]),
                ("title".into(), vec!["job title".into(), "business title".into()]),
                ("hire_date".into(), vec!["start date".into(), "original start date".into()]),
                ("work_state".into(), vec!["work state".into(), "work location state".into()]),
                ("manager_email".into(), vec!["manager work email".into(), "manager email".into()]),
                ("status".into(), vec!["employment status".into(), "status".into()]),
                ("date_of_birth".into(), vec!["date of birth".into()]),
                ("gender".into(), vec!["gender".into()]),
                ("ethnicity".into(), vec!["ethnicity".into(), "race".into()]),
            ],
        },
    ]
}

/// Try to auto-detect which HRIS preset matches the given headers.
/// Returns (preset_id, match_ratio) or None if no good match (>= 0.5).
pub fn detect_hris_preset(headers: &[String]) -> Option<(String, f64)> {
    let normalized_headers: Vec<String> = headers.iter().map(|h| normalize_header(h)).collect();
    let presets = get_hris_presets();
    let mut best: Option<(String, f64)> = None;

    for preset in &presets {
        let total_fields = preset.header_mappings.len() as f64;
        if total_fields == 0.0 {
            continue;
        }
        let mut matched = 0usize;
        for (_target, possible_headers) in &preset.header_mappings {
            let found = possible_headers.iter().any(|ph| {
                let normalized_ph = normalize_header(ph);
                normalized_headers.contains(&normalized_ph)
            });
            if found {
                matched += 1;
            }
        }
        let ratio = matched as f64 / total_fields;
        if ratio >= 0.5 {
            if best.as_ref().map_or(true, |(_, br)| ratio > *br) {
                best = Some((preset.id.clone(), ratio));
            }
        }
    }

    best
}

/// Apply an HRIS preset to generate a ColumnMappingConfig for the given headers.
pub fn apply_hris_preset(preset_id: &str, headers: &[String]) -> Option<ColumnMappingConfig> {
    let presets = get_hris_presets();
    let preset = presets.iter().find(|p| p.id == preset_id)?;
    let normalized_headers: Vec<String> = headers.iter().map(|h| normalize_header(h)).collect();
    let mut mappings = HashMap::new();

    for (target_field, possible_headers) in &preset.header_mappings {
        for ph in possible_headers {
            let normalized_ph = normalize_header(ph);
            if let Some(idx) = normalized_headers.iter().position(|nh| *nh == normalized_ph) {
                mappings.insert(target_field.clone(), headers[idx].clone());
                break;
            }
        }
    }

    Some(ColumnMappingConfig {
        mappings,
        preset_name: Some(preset.name.clone()),
    })
}

// ============================================================================
// V2.5.1b — Header Normalization Preview
// ============================================================================

/// Analyze headers from a parsed file and suggest mappings.
pub fn analyze_headers(
    headers: &[String],
    sample_rows: &[ParsedRow],
    import_type: &ImportType,
) -> HeaderAnalysisResult {
    let column_mappings = get_column_mappings_for_type(import_type);
    let (required_fields, optional_fields) = get_field_lists_for_type(import_type);
    let mut mapped_targets: HashSet<String> = HashSet::new();
    let mut header_analyses = Vec::new();

    for header in headers {
        let normalized = normalize_header(header);
        let sample_values: Vec<String> = sample_rows
            .iter()
            .take(3)
            .filter_map(|row| row.get(header).cloned())
            .collect();

        let (suggested_field, confidence) =
            find_best_field_match(&normalized, column_mappings, &mapped_targets);

        if let Some(ref field) = suggested_field {
            mapped_targets.insert(field.clone());
        }

        header_analyses.push(HeaderAnalysis {
            original: header.clone(),
            normalized,
            suggested_field,
            confidence,
            sample_values,
        });
    }

    let unmapped_required: Vec<String> = required_fields
        .iter()
        .filter(|f| !mapped_targets.contains(**f))
        .map(|f| f.to_string())
        .collect();

    let unmapped_optional: Vec<String> = optional_fields
        .iter()
        .filter(|f| !mapped_targets.contains(**f))
        .map(|f| f.to_string())
        .collect();

    HeaderAnalysisResult {
        headers: header_analyses,
        unmapped_required,
        unmapped_optional,
        import_type: import_type.clone(),
    }
}

fn find_best_field_match(
    normalized_header: &str,
    column_mappings: &[(&str, &[&str])],
    already_mapped: &HashSet<String>,
) -> (Option<String>, String) {
    // 1. Exact match against target field name
    for (target_field, aliases) in column_mappings {
        if already_mapped.contains(*target_field) {
            continue;
        }
        if normalized_header == *target_field {
            return (Some(target_field.to_string()), "exact".to_string());
        }
        if aliases.contains(&normalized_header) {
            return (Some(target_field.to_string()), "alias".to_string());
        }
    }

    // 2. Fuzzy match using Levenshtein similarity
    let mut best_score = 0.0f64;
    let mut best_field: Option<String> = None;

    for (target_field, aliases) in column_mappings {
        if already_mapped.contains(*target_field) {
            continue;
        }
        let score = similarity_score(normalized_header, target_field);
        if score > best_score {
            best_score = score;
            best_field = Some(target_field.to_string());
        }
        for alias in *aliases {
            let score = similarity_score(normalized_header, alias);
            if score > best_score {
                best_score = score;
                best_field = Some(target_field.to_string());
            }
        }
    }

    if best_score >= FUZZY_THRESHOLD {
        if let Some(field) = best_field {
            return (Some(field), "fuzzy".to_string());
        }
    }

    (None, "none".to_string())
}

// ============================================================================
// V2.5.1a — Column Mapping Application
// ============================================================================

/// Apply a column mapping config to parsed rows.
/// Remaps row keys from source headers to target field names.
pub fn apply_column_mapping(rows: &[ParsedRow], mapping: &ColumnMappingConfig) -> Vec<ParsedRow> {
    let reverse: HashMap<String, String> = mapping
        .mappings
        .iter()
        .map(|(target, source)| (source.clone(), target.clone()))
        .collect();

    rows.iter()
        .map(|row| {
            let mut new_row = HashMap::new();
            for (key, value) in row {
                if let Some(target_field) = reverse.get(key) {
                    new_row.insert(target_field.clone(), value.clone());
                }
            }
            new_row
        })
        .collect()
}

// ============================================================================
// V2.5.1d — Validation Rules
// ============================================================================

/// Validate all rows against the rules for the given import type.
pub fn validate_rows(
    rows: &[ParsedRow],
    mapping: &ColumnMappingConfig,
    import_type: &ImportType,
) -> ValidationResult {
    let mapped_rows = apply_column_mapping(rows, mapping);
    let mut all_issues: Vec<ValidationIssue> = Vec::new();

    for (index, row) in mapped_rows.iter().enumerate() {
        let row_issues = match import_type {
            ImportType::Employees => validate_employee_row(row, index),
            ImportType::Ratings => validate_rating_row(row, index),
            ImportType::Enps => validate_enps_row(row, index),
            ImportType::Reviews => Vec::new(),
        };
        all_issues.extend(row_issues);
    }

    // Cross-row: duplicate emails within batch (employees only)
    if matches!(import_type, ImportType::Employees) {
        let email_dupes = find_duplicate_emails_in_batch(&mapped_rows, "email");
        all_issues.extend(email_dupes);
    }

    let error_count = all_issues.iter().filter(|i| i.severity == ValidationSeverity::Error).count();
    let warning_count = all_issues.iter().filter(|i| i.severity == ValidationSeverity::Warning).count();
    let rows_with_issues: HashSet<usize> = all_issues.iter().map(|i| i.row_index).collect();
    let valid_rows = mapped_rows.len() - rows_with_issues.len();

    ValidationResult {
        issues: all_issues,
        error_count,
        warning_count,
        total_rows: mapped_rows.len(),
        valid_rows,
        can_import: error_count == 0,
    }
}

fn validate_employee_row(row: &ParsedRow, index: usize) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // Required: email
    let email = row.get("email").map(|s| s.as_str()).unwrap_or("");
    if email.trim().is_empty() {
        issues.push(ValidationIssue {
            row_index: index, field: "email".into(), value: String::new(),
            message: "Email is required".into(),
            severity: ValidationSeverity::Error, rule: "required_field".into(),
            suggested_fix: None,
        });
    } else if !is_valid_email(email) {
        issues.push(ValidationIssue {
            row_index: index, field: "email".into(), value: email.to_string(),
            message: "Invalid email format".into(),
            severity: ValidationSeverity::Error, rule: "invalid_email".into(),
            suggested_fix: None,
        });
    }

    // Required: at least one name field
    let first_name = row.get("first_name").map(|s| s.as_str()).unwrap_or("");
    let full_name = row.get("full_name").map(|s| s.as_str()).unwrap_or("");
    if first_name.trim().is_empty() && full_name.trim().is_empty() {
        issues.push(ValidationIssue {
            row_index: index, field: "first_name".into(), value: String::new(),
            message: "A name is required (first_name or full_name)".into(),
            severity: ValidationSeverity::Error, rule: "required_field".into(),
            suggested_fix: None,
        });
    }

    // Date fields
    for field_name in &["hire_date", "date_of_birth", "termination_date"] {
        if let Some(val) = row.get(*field_name) {
            if !val.trim().is_empty() {
                match try_parse_date(val) {
                    None => {
                        issues.push(ValidationIssue {
                            row_index: index, field: field_name.to_string(), value: val.clone(),
                            message: format!("Invalid date format for {}", field_name),
                            severity: ValidationSeverity::Warning, rule: "invalid_date".into(),
                            suggested_fix: None,
                        });
                    }
                    Some(ref normalized) if normalized != val.trim() => {
                        issues.push(ValidationIssue {
                            row_index: index, field: field_name.to_string(), value: val.clone(),
                            message: format!("Date will be normalized to {}", normalized),
                            severity: ValidationSeverity::Warning, rule: "date_format".into(),
                            suggested_fix: Some(normalized.clone()),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // hire_date: not too far in the future
    if let Some(val) = row.get("hire_date") {
        if let Some(normalized) = try_parse_date(val) {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
                let today = chrono::Utc::now().date_naive();
                if date > today + chrono::Duration::days(30) {
                    issues.push(ValidationIssue {
                        row_index: index, field: "hire_date".into(), value: val.clone(),
                        message: "Hire date is more than 30 days in the future".into(),
                        severity: ValidationSeverity::Warning, rule: "future_date".into(),
                        suggested_fix: None,
                    });
                }
            }
        }
    }

    // date_of_birth: reasonable range
    if let Some(val) = row.get("date_of_birth") {
        if let Some(normalized) = try_parse_date(val) {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
                let year = date.year();
                let current_year = chrono::Utc::now().date_naive().year();
                if year < 1930 || year > current_year - 14 {
                    issues.push(ValidationIssue {
                        row_index: index, field: "date_of_birth".into(), value: val.clone(),
                        message: format!("Date of birth year {} seems unreasonable (expected 1930-{})", year, current_year - 14),
                        severity: ValidationSeverity::Warning, rule: "dob_range".into(),
                        suggested_fix: None,
                    });
                }
            }
        }
    }

    // Status validation
    if let Some(val) = row.get("status") {
        let status = val.trim().to_lowercase();
        if !status.is_empty() && !["active", "terminated", "leave"].contains(&status.as_str()) {
            issues.push(ValidationIssue {
                row_index: index, field: "status".into(), value: val.clone(),
                message: "Status must be 'active', 'terminated', or 'leave'".into(),
                severity: ValidationSeverity::Warning, rule: "invalid_status".into(),
                suggested_fix: None,
            });
        }
    }

    // State validation
    if let Some(val) = row.get("work_state") {
        let state = val.trim().to_uppercase();
        if !state.is_empty() && !US_STATE_CODES.contains(&state.as_str()) {
            issues.push(ValidationIssue {
                row_index: index, field: "work_state".into(), value: val.clone(),
                message: "Invalid US state code".into(),
                severity: ValidationSeverity::Warning, rule: "invalid_state".into(),
                suggested_fix: None,
            });
        }
    }

    issues
}

fn validate_rating_row(row: &ParsedRow, index: usize) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    let email = row.get("employee_email").map(|s| s.as_str()).unwrap_or("");
    if email.trim().is_empty() {
        issues.push(ValidationIssue {
            row_index: index, field: "employee_email".into(), value: String::new(),
            message: "Employee email is required".into(),
            severity: ValidationSeverity::Error, rule: "required_field".into(),
            suggested_fix: None,
        });
    } else if !is_valid_email(email) {
        issues.push(ValidationIssue {
            row_index: index, field: "employee_email".into(), value: email.to_string(),
            message: "Invalid email format".into(),
            severity: ValidationSeverity::Error, rule: "invalid_email".into(),
            suggested_fix: None,
        });
    }

    let rating_str = row.get("rating").map(|s| s.as_str()).unwrap_or("");
    if rating_str.trim().is_empty() {
        issues.push(ValidationIssue {
            row_index: index, field: "rating".into(), value: String::new(),
            message: "Rating is required".into(),
            severity: ValidationSeverity::Error, rule: "required_field".into(),
            suggested_fix: None,
        });
    } else if let Ok(rating) = rating_str.trim().parse::<f64>() {
        if !(1.0..=5.0).contains(&rating) {
            issues.push(ValidationIssue {
                row_index: index, field: "rating".into(), value: rating_str.to_string(),
                message: "Rating must be between 1.0 and 5.0".into(),
                severity: ValidationSeverity::Error, rule: "invalid_rating".into(),
                suggested_fix: None,
            });
        }
    } else {
        issues.push(ValidationIssue {
            row_index: index, field: "rating".into(), value: rating_str.to_string(),
            message: "Rating must be a number between 1.0 and 5.0".into(),
            severity: ValidationSeverity::Error, rule: "invalid_rating".into(),
            suggested_fix: None,
        });
    }

    if let Some(val) = row.get("rated_at") {
        if !val.trim().is_empty() && try_parse_date(val).is_none() {
            issues.push(ValidationIssue {
                row_index: index, field: "rated_at".into(), value: val.clone(),
                message: "Invalid date format".into(),
                severity: ValidationSeverity::Warning, rule: "invalid_date".into(),
                suggested_fix: None,
            });
        }
    }

    issues
}

fn validate_enps_row(row: &ParsedRow, index: usize) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    let email = row.get("employee_email").map(|s| s.as_str()).unwrap_or("");
    if email.trim().is_empty() {
        issues.push(ValidationIssue {
            row_index: index, field: "employee_email".into(), value: String::new(),
            message: "Employee email is required".into(),
            severity: ValidationSeverity::Error, rule: "required_field".into(),
            suggested_fix: None,
        });
    } else if !is_valid_email(email) {
        issues.push(ValidationIssue {
            row_index: index, field: "employee_email".into(), value: email.to_string(),
            message: "Invalid email format".into(),
            severity: ValidationSeverity::Error, rule: "invalid_email".into(),
            suggested_fix: None,
        });
    }

    let score_str = row.get("score").map(|s| s.as_str()).unwrap_or("");
    if score_str.trim().is_empty() {
        issues.push(ValidationIssue {
            row_index: index, field: "score".into(), value: String::new(),
            message: "eNPS score is required".into(),
            severity: ValidationSeverity::Error, rule: "required_field".into(),
            suggested_fix: None,
        });
    } else if let Ok(score) = score_str.trim().parse::<i32>() {
        if !(0..=10).contains(&score) {
            issues.push(ValidationIssue {
                row_index: index, field: "score".into(), value: score_str.to_string(),
                message: "eNPS score must be between 0 and 10".into(),
                severity: ValidationSeverity::Error, rule: "invalid_enps_score".into(),
                suggested_fix: None,
            });
        }
    } else {
        issues.push(ValidationIssue {
            row_index: index, field: "score".into(), value: score_str.to_string(),
            message: "eNPS score must be an integer between 0 and 10".into(),
            severity: ValidationSeverity::Error, rule: "invalid_enps_score".into(),
            suggested_fix: None,
        });
    }

    if let Some(val) = row.get("responded_at") {
        if !val.trim().is_empty() && try_parse_date(val).is_none() {
            issues.push(ValidationIssue {
                row_index: index, field: "responded_at".into(), value: val.clone(),
                message: "Invalid date format".into(),
                severity: ValidationSeverity::Warning, rule: "invalid_date".into(),
                suggested_fix: None,
            });
        }
    }

    issues
}

fn find_duplicate_emails_in_batch(rows: &[ParsedRow], email_field: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for (index, row) in rows.iter().enumerate() {
        if let Some(email) = row.get(email_field) {
            let normalized = email.trim().to_lowercase();
            if normalized.is_empty() {
                continue;
            }
            if let Some(&first_index) = seen.get(&normalized) {
                issues.push(ValidationIssue {
                    row_index: index, field: email_field.to_string(), value: email.clone(),
                    message: format!("Duplicate email '{}' (also in row {})", email, first_index + 1),
                    severity: ValidationSeverity::Error, rule: "duplicate_email_in_batch".into(),
                    suggested_fix: None,
                });
            } else {
                seen.insert(normalized, index);
            }
        }
    }

    issues
}

// ============================================================================
// V2.5.1e — Fix-and-Retry
// ============================================================================

/// Apply corrections to parsed rows and re-validate.
pub fn apply_corrections_and_revalidate(
    rows: &mut Vec<ParsedRow>,
    corrections: &[RowCorrection],
    mapping: &ColumnMappingConfig,
    import_type: &ImportType,
) -> ValidationResult {
    let target_to_source: HashMap<String, String> = mapping
        .mappings
        .iter()
        .map(|(target, source)| (target.clone(), source.clone()))
        .collect();

    for correction in corrections {
        if correction.row_index < rows.len() {
            if let Some(source_header) = target_to_source.get(&correction.field) {
                rows[correction.row_index].insert(source_header.clone(), correction.new_value.clone());
            }
        }
    }

    validate_rows(rows, mapping, import_type)
}

// ============================================================================
// V2.5.1c — Dedupe Detection
// ============================================================================

/// Detect potential duplicates within parsed rows.
pub fn detect_duplicates(rows: &[ParsedRow], mapping: &ColumnMappingConfig) -> DedupeResult {
    let mapped_rows = apply_column_mapping(rows, mapping);
    let mut duplicates: Vec<DuplicatePair> = Vec::new();
    let mut affected: HashSet<usize> = HashSet::new();

    for i in 0..mapped_rows.len() {
        for j in (i + 1)..mapped_rows.len() {
            if let Some(pair) = check_duplicate_pair(&mapped_rows, i, j) {
                affected.insert(pair.row_a);
                affected.insert(pair.row_b);
                duplicates.push(pair);
            }
        }
    }

    let mut affected_rows: Vec<usize> = affected.into_iter().collect();
    affected_rows.sort();

    DedupeResult {
        duplicates,
        total_rows: rows.len(),
        affected_rows,
    }
}

fn check_duplicate_pair(rows: &[ParsedRow], i: usize, j: usize) -> Option<DuplicatePair> {
    let row_a = &rows[i];
    let row_b = &rows[j];

    // 1. Exact email match
    let email_a = row_a.get("email").or_else(|| row_a.get("employee_email"))
        .map(|s| s.trim().to_lowercase()).unwrap_or_default();
    let email_b = row_b.get("email").or_else(|| row_b.get("employee_email"))
        .map(|s| s.trim().to_lowercase()).unwrap_or_default();

    if !email_a.is_empty() && email_a == email_b {
        let mut matched_fields = HashMap::new();
        matched_fields.insert("email".to_string(), (email_a.clone(), email_b.clone()));
        return Some(DuplicatePair {
            row_a: i, row_b: j,
            match_type: "exact_email".into(), confidence: 1.0,
            matched_fields,
        });
    }

    // 2. Name matching
    let name_a = get_normalized_name(row_a);
    let name_b = get_normalized_name(row_b);

    if !name_a.is_empty() && name_a == name_b {
        let dob_a = row_a.get("date_of_birth").map(|s| s.trim().to_lowercase()).unwrap_or_default();
        let dob_b = row_b.get("date_of_birth").map(|s| s.trim().to_lowercase()).unwrap_or_default();

        let mut matched_fields = HashMap::new();
        matched_fields.insert("name".to_string(), (name_a.clone(), name_b.clone()));

        if !dob_a.is_empty() && dob_a == dob_b {
            matched_fields.insert("date_of_birth".to_string(), (dob_a, dob_b));
            return Some(DuplicatePair {
                row_a: i, row_b: j,
                match_type: "fuzzy_name_dob".into(), confidence: 0.9,
                matched_fields,
            });
        }

        return Some(DuplicatePair {
            row_a: i, row_b: j,
            match_type: "fuzzy_name_only".into(), confidence: 0.6,
            matched_fields,
        });
    }

    None
}

fn get_normalized_name(row: &ParsedRow) -> String {
    if let Some(full_name) = row.get("full_name") {
        return normalize_name(full_name);
    }
    let first = row.get("first_name").map(|s| s.as_str()).unwrap_or("");
    let last = row.get("last_name").map(|s| s.as_str()).unwrap_or("");
    normalize_name(&format!("{} {}", first, last))
}

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Check parsed rows against existing employees in the database.
pub async fn detect_existing_conflicts(
    pool: &DbPool,
    rows: &[ParsedRow],
    mapping: &ColumnMappingConfig,
) -> Result<ExistingConflictsResult, employees::EmployeeError> {
    let mapped_rows = apply_column_mapping(rows, mapping);
    let mut conflicts = Vec::new();
    let mut new_rows = 0usize;
    let mut update_rows = 0usize;

    for (index, row) in mapped_rows.iter().enumerate() {
        let email = row.get("email").or_else(|| row.get("employee_email"))
            .map(|s| s.trim().to_string()).unwrap_or_default();

        if email.is_empty() {
            new_rows += 1;
            continue;
        }

        match employees::get_employee_by_email(pool, &email).await? {
            Some(existing) => {
                conflicts.push(ExistingConflict {
                    row_index: index, email: email.clone(),
                    existing_employee_id: existing.id,
                    existing_employee_name: existing.full_name,
                    suggested_action: "update".to_string(),
                });
                update_rows += 1;
            }
            None => { new_rows += 1; }
        }
    }

    Ok(ExistingConflictsResult { conflicts, new_rows, update_rows })
}

// ============================================================================
// Helper Functions
// ============================================================================

fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if email.is_empty() { return false; }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 { return false; }
    let (local, domain) = (parts[0], parts[1]);
    if local.is_empty() || domain.is_empty() { return false; }
    if !domain.contains('.') { return false; }
    !email.contains(' ')
}

fn try_parse_date(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { return None; }

    if let Ok(date) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Some(date.format("%Y-%m-%d").to_string());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(trimmed, "%m/%d/%Y") {
        return Some(date.format("%Y-%m-%d").to_string());
    }
    // Flexible M/D/YYYY
    if trimmed.contains('/') {
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() == 3 {
            if let (Ok(m), Ok(d), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>(), parts[2].parse::<i32>()) {
                if let Some(date) = chrono::NaiveDate::from_ymd_opt(y, m, d) {
                    return Some(date.format("%Y-%m-%d").to_string());
                }
            }
        }
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(trimmed, "%d-%b-%Y") {
        return Some(date.format("%Y-%m-%d").to_string());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(trimmed, "%Y/%m/%d") {
        return Some(date.format("%Y-%m-%d").to_string());
    }
    None
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr_row[j + 1] = (prev_row[j + 1] + 1).min(curr_row[j] + 1).min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }
    prev_row[b_len]
}

fn similarity_score(a: &str, b: &str) -> f64 {
    let max_len = a.len().max(b.len());
    if max_len == 0 { return 1.0; }
    let dist = levenshtein_distance(a, b);
    1.0 - (dist as f64 / max_len as f64)
}

fn get_column_mappings_for_type(import_type: &ImportType) -> &'static [(&'static str, &'static [&'static str])] {
    match import_type {
        ImportType::Employees => EMPLOYEE_COLUMN_MAPPINGS,
        ImportType::Ratings => RATING_COLUMN_MAPPINGS,
        ImportType::Enps => ENPS_COLUMN_MAPPINGS,
        ImportType::Reviews => &[],
    }
}

fn get_field_lists_for_type(import_type: &ImportType) -> (&'static [&'static str], &'static [&'static str]) {
    match import_type {
        ImportType::Employees => (REQUIRED_EMPLOYEE_FIELDS, OPTIONAL_EMPLOYEE_FIELDS),
        ImportType::Ratings => (REQUIRED_RATING_FIELDS, OPTIONAL_RATING_FIELDS),
        ImportType::Enps => (REQUIRED_ENPS_FIELDS, OPTIONAL_ENPS_FIELDS),
        ImportType::Reviews => (&[], &[]),
    }
}

use chrono::Datelike;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(fields: &[(&str, &str)]) -> ParsedRow {
        fields.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn simple_employee_mapping() -> ColumnMappingConfig {
        ColumnMappingConfig {
            mappings: HashMap::from([
                ("email".into(), "email".into()),
                ("first_name".into(), "first_name".into()),
                ("last_name".into(), "last_name".into()),
                ("department".into(), "department".into()),
                ("hire_date".into(), "hire_date".into()),
                ("status".into(), "status".into()),
                ("work_state".into(), "work_state".into()),
                ("date_of_birth".into(), "date_of_birth".into()),
            ]),
            preset_name: None,
        }
    }

    // ---- Column Mapping ----

    #[test]
    fn test_apply_column_mapping_basic() {
        let mapping = ColumnMappingConfig {
            mappings: HashMap::from([
                ("email".into(), "Employee Email".into()),
                ("first_name".into(), "First Name".into()),
            ]),
            preset_name: None,
        };
        let rows = vec![make_row(&[("Employee Email", "john@acme.com"), ("First Name", "John"), ("Extra", "ignored")])];
        let result = apply_column_mapping(&rows, &mapping);
        assert_eq!(result[0].get("email").unwrap(), "john@acme.com");
        assert_eq!(result[0].get("first_name").unwrap(), "John");
        assert!(!result[0].contains_key("Extra"));
    }

    #[test]
    fn test_apply_column_mapping_unmapped_fields_dropped() {
        let mapping = ColumnMappingConfig {
            mappings: HashMap::from([("email".into(), "email".into())]),
            preset_name: None,
        };
        let rows = vec![make_row(&[("email", "j@a.com"), ("department", "Eng")])];
        let result = apply_column_mapping(&rows, &mapping);
        assert_eq!(result[0].len(), 1);
        assert!(result[0].contains_key("email"));
    }

    // ---- Header Analysis ----

    #[test]
    fn test_analyze_headers_exact_match() {
        let headers = vec!["email".into(), "first_name".into()];
        let samples = vec![make_row(&[("email", "j@a.com"), ("first_name", "John")])];
        let result = analyze_headers(&headers, &samples, &ImportType::Employees);
        assert_eq!(result.headers[0].confidence, "exact");
        assert_eq!(result.headers[0].suggested_field, Some("email".into()));
    }

    #[test]
    fn test_analyze_headers_alias_match() {
        let headers = vec!["dept".into(), "dob".into()];
        let samples = vec![make_row(&[("dept", "Eng"), ("dob", "1990-01-01")])];
        let result = analyze_headers(&headers, &samples, &ImportType::Employees);
        assert_eq!(result.headers[0].confidence, "alias");
        assert_eq!(result.headers[0].suggested_field, Some("department".into()));
    }

    #[test]
    fn test_analyze_headers_no_match() {
        let headers = vec!["zzz_unknown_field".into()];
        let samples = vec![make_row(&[("zzz_unknown_field", "val")])];
        let result = analyze_headers(&headers, &samples, &ImportType::Employees);
        assert_eq!(result.headers[0].confidence, "none");
        assert_eq!(result.headers[0].suggested_field, None);
    }

    #[test]
    fn test_analyze_headers_unmapped_required() {
        let headers = vec!["department".into()];
        let samples = vec![make_row(&[("department", "Eng")])];
        let result = analyze_headers(&headers, &samples, &ImportType::Employees);
        assert!(result.unmapped_required.contains(&"email".to_string()));
    }

    // ---- Dedupe Detection ----

    #[test]
    fn test_detect_duplicates_exact_email() {
        let mapping = simple_employee_mapping();
        let rows = vec![
            make_row(&[("email", "john@acme.com"), ("first_name", "John")]),
            make_row(&[("email", "john@acme.com"), ("first_name", "Jonathan")]),
        ];
        let result = detect_duplicates(&rows, &mapping);
        assert_eq!(result.duplicates.len(), 1);
        assert_eq!(result.duplicates[0].match_type, "exact_email");
        assert_eq!(result.duplicates[0].confidence, 1.0);
    }

    #[test]
    fn test_detect_duplicates_name_and_dob() {
        let mapping = simple_employee_mapping();
        let rows = vec![
            make_row(&[("email", "john@acme.com"), ("first_name", "John"), ("last_name", "Doe"), ("date_of_birth", "1990-01-15")]),
            make_row(&[("email", "jdoe@acme.com"), ("first_name", "John"), ("last_name", "Doe"), ("date_of_birth", "1990-01-15")]),
        ];
        let result = detect_duplicates(&rows, &mapping);
        assert_eq!(result.duplicates.len(), 1);
        assert_eq!(result.duplicates[0].match_type, "fuzzy_name_dob");
        assert_eq!(result.duplicates[0].confidence, 0.9);
    }

    #[test]
    fn test_detect_duplicates_name_only() {
        let mapping = simple_employee_mapping();
        let rows = vec![
            make_row(&[("email", "john@acme.com"), ("first_name", "John"), ("last_name", "Doe")]),
            make_row(&[("email", "jdoe@other.com"), ("first_name", "John"), ("last_name", "Doe")]),
        ];
        let result = detect_duplicates(&rows, &mapping);
        assert_eq!(result.duplicates.len(), 1);
        assert_eq!(result.duplicates[0].match_type, "fuzzy_name_only");
        assert_eq!(result.duplicates[0].confidence, 0.6);
    }

    #[test]
    fn test_detect_duplicates_no_dupes() {
        let mapping = simple_employee_mapping();
        let rows = vec![
            make_row(&[("email", "alice@acme.com"), ("first_name", "Alice")]),
            make_row(&[("email", "bob@acme.com"), ("first_name", "Bob")]),
        ];
        let result = detect_duplicates(&rows, &mapping);
        assert!(result.duplicates.is_empty());
    }

    // ---- Validation ----

    #[test]
    fn test_validate_employees_missing_email() {
        let mapping = simple_employee_mapping();
        let rows = vec![make_row(&[("first_name", "John")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(!result.can_import);
        assert!(result.issues.iter().any(|i| i.rule == "required_field" && i.field == "email"));
    }

    #[test]
    fn test_validate_employees_invalid_email() {
        let mapping = simple_employee_mapping();
        let rows = vec![make_row(&[("email", "not-an-email"), ("first_name", "John")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(!result.can_import);
        assert!(result.issues.iter().any(|i| i.rule == "invalid_email"));
    }

    #[test]
    fn test_validate_employees_missing_name() {
        let mapping = simple_employee_mapping();
        let rows = vec![make_row(&[("email", "john@acme.com")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(!result.can_import);
        assert!(result.issues.iter().any(|i| i.rule == "required_field" && i.field == "first_name"));
    }

    #[test]
    fn test_validate_employees_invalid_date() {
        let mapping = simple_employee_mapping();
        let rows = vec![make_row(&[("email", "john@acme.com"), ("first_name", "John"), ("hire_date", "not-a-date")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(result.issues.iter().any(|i| i.rule == "invalid_date"));
    }

    #[test]
    fn test_validate_employees_date_normalization() {
        let mapping = simple_employee_mapping();
        let rows = vec![make_row(&[("email", "john@acme.com"), ("first_name", "John"), ("hire_date", "01/15/2024")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        let date_issue = result.issues.iter().find(|i| i.rule == "date_format");
        assert!(date_issue.is_some());
        assert_eq!(date_issue.unwrap().suggested_fix, Some("2024-01-15".to_string()));
    }

    #[test]
    fn test_validate_employees_future_hire_date() {
        let mapping = simple_employee_mapping();
        let rows = vec![make_row(&[("email", "john@acme.com"), ("first_name", "John"), ("hire_date", "2099-01-01")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(result.issues.iter().any(|i| i.rule == "future_date"));
    }

    #[test]
    fn test_validate_employees_duplicate_email_in_batch() {
        let mapping = simple_employee_mapping();
        let rows = vec![
            make_row(&[("email", "john@acme.com"), ("first_name", "John")]),
            make_row(&[("email", "john@acme.com"), ("first_name", "Jonathan")]),
        ];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(!result.can_import);
        assert!(result.issues.iter().any(|i| i.rule == "duplicate_email_in_batch"));
    }

    #[test]
    fn test_validate_employees_valid_row() {
        let mapping = simple_employee_mapping();
        let rows = vec![make_row(&[
            ("email", "john@acme.com"), ("first_name", "John"), ("last_name", "Doe"),
            ("department", "Engineering"), ("hire_date", "2024-01-15"), ("status", "active"), ("work_state", "CA"),
        ])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(result.can_import);
        assert_eq!(result.error_count, 0);
        assert_eq!(result.valid_rows, 1);
    }

    #[test]
    fn test_validate_ratings_invalid_score() {
        let mapping = ColumnMappingConfig {
            mappings: HashMap::from([("employee_email".into(), "employee_email".into()), ("rating".into(), "rating".into())]),
            preset_name: None,
        };
        let rows = vec![make_row(&[("employee_email", "john@acme.com"), ("rating", "6.0")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Ratings);
        assert!(!result.can_import);
        assert!(result.issues.iter().any(|i| i.rule == "invalid_rating"));
    }

    #[test]
    fn test_validate_enps_invalid_score() {
        let mapping = ColumnMappingConfig {
            mappings: HashMap::from([("employee_email".into(), "employee_email".into()), ("score".into(), "score".into())]),
            preset_name: None,
        };
        let rows = vec![make_row(&[("employee_email", "john@acme.com"), ("score", "11")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Enps);
        assert!(!result.can_import);
        assert!(result.issues.iter().any(|i| i.rule == "invalid_enps_score"));
    }

    // ---- Fix-and-Retry ----

    #[test]
    fn test_apply_corrections() {
        let mapping = simple_employee_mapping();
        let mut rows = vec![make_row(&[("email", "bad-email"), ("first_name", "John")])];
        let result = validate_rows(&rows, &mapping, &ImportType::Employees);
        assert!(!result.can_import);

        let corrections = vec![RowCorrection { row_index: 0, field: "email".into(), new_value: "john@acme.com".into() }];
        let result = apply_corrections_and_revalidate(&mut rows, &corrections, &mapping, &ImportType::Employees);
        assert!(result.can_import);
        assert_eq!(result.error_count, 0);
    }

    // ---- HRIS Presets ----

    #[test]
    fn test_hris_preset_detection_bamboohr() {
        let headers = vec!["Work Email".into(), "First Name".into(), "Last Name".into(), "Department".into(), "Job Title".into(), "Hire Date".into(), "Gender".into(), "Supervisor Email".into(), "Race/Ethnicity".into()];
        let result = detect_hris_preset(&headers);
        assert!(result.is_some());
        let (id, ratio) = result.unwrap();
        assert_eq!(id, "bamboohr");
        assert!(ratio >= 0.5);
    }

    #[test]
    fn test_hris_preset_detection_gusto() {
        let headers = vec!["Email".into(), "Legal First Name".into(), "Legal Last Name".into(), "Team".into(), "Start Date".into(), "Employment Status".into()];
        let result = detect_hris_preset(&headers);
        assert!(result.is_some());
        let (id, _) = result.unwrap();
        assert_eq!(id, "gusto");
    }

    #[test]
    fn test_hris_preset_detection_none() {
        let headers = vec!["zzz_random".into(), "aaa_field".into(), "bbb_column".into()];
        let result = detect_hris_preset(&headers);
        assert!(result.is_none());
    }

    #[test]
    fn test_apply_hris_preset() {
        let headers = vec!["Work Email".into(), "First Name".into(), "Last Name".into(), "Department".into()];
        let config = apply_hris_preset("bamboohr", &headers);
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.mappings.get("email"), Some(&"Work Email".to_string()));
        assert_eq!(config.mappings.get("first_name"), Some(&"First Name".to_string()));
        assert_eq!(config.preset_name, Some("BambooHR".to_string()));
    }

    // ---- Helpers ----

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
    }

    #[test]
    fn test_date_normalization() {
        assert_eq!(try_parse_date("2024-01-15"), Some("2024-01-15".into()));
        assert_eq!(try_parse_date("01/15/2024"), Some("2024-01-15".into()));
        assert_eq!(try_parse_date("1/5/2024"), Some("2024-01-05".into()));
        assert_eq!(try_parse_date("15-Jan-2024"), Some("2024-01-15".into()));
        assert_eq!(try_parse_date("not-a-date"), None);
        assert_eq!(try_parse_date(""), None);
    }

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("john@acme.com"));
        assert!(is_valid_email("jane.doe@company.co.uk"));
        assert!(!is_valid_email("not-an-email"));
        assert!(!is_valid_email("@domain.com"));
        assert!(!is_valid_email("user@"));
        assert!(!is_valid_email("user@domain"));
        assert!(!is_valid_email(""));
    }
}
