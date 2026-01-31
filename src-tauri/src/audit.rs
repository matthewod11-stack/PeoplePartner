// HR Command Center - Audit Logging Module
// Records all Claude API interactions for compliance tracking
//
// Key responsibilities:
// 1. Create audit entries after each Claude API interaction
// 2. List/filter audit entries for review
// 3. Export audit log to CSV format
//
// Design: Audit entries are created AFTER streaming completes.
// Failures are logged but never block the chat flow.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

use crate::db::DbPool;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum AuditError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Audit entry not found: {0}")]
    NotFound(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Export error: {0}")]
    ExportError(String),
}

impl From<sqlx::Error> for AuditError {
    fn from(err: sqlx::Error) -> Self {
        AuditError::Database(err.to_string())
    }
}

// Make AuditError serializable for Tauri commands
impl serde::Serialize for AuditError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ============================================================================
// Types
// ============================================================================

/// Full audit log entry from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEntry {
    pub id: String,
    pub conversation_id: Option<String>,
    pub request_redacted: String,
    pub response_text: String,
    pub context_used: Option<String>, // JSON array of employee IDs
    pub created_at: String,
    /// V2.4.2: Query category for filtering (e.g., "dei")
    pub query_category: Option<String>,
}

/// Lightweight audit entry for list display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditListItem {
    pub id: String,
    pub conversation_id: Option<String>,
    pub request_preview: String,  // First 100 chars
    pub response_preview: String, // First 100 chars
    pub employee_count: usize,
    pub created_at: String,
}

/// Input for creating an audit entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuditEntry {
    pub conversation_id: Option<String>,
    pub request_redacted: String,
    pub response_text: String,
    pub employee_ids_used: Vec<String>,
    /// V2.4.2: Optional category for filtering (e.g., "dei")
    pub query_category: Option<String>,
}

/// Filter options for listing/exporting audit entries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditFilter {
    pub conversation_id: Option<String>,
    pub start_date: Option<String>, // ISO 8601 format
    pub end_date: Option<String>,   // ISO 8601 format
    /// V2.4.2: Filter by query category (e.g., "dei")
    pub query_category: Option<String>,
}

/// CSV export result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub csv_content: String,
    pub row_count: usize,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Create a new audit log entry
///
/// Called by frontend after streaming response completes.
/// Employee IDs are serialized to JSON for storage.
pub async fn create_audit_entry(
    pool: &DbPool,
    input: CreateAuditEntry,
) -> Result<AuditEntry, AuditError> {
    let id = Uuid::new_v4().to_string();

    // Serialize employee IDs to JSON
    let context_used = if input.employee_ids_used.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&input.employee_ids_used).map_err(|e| {
            AuditError::InvalidInput(format!("Failed to serialize employee IDs: {}", e))
        })?)
    };

    sqlx::query(
        r#"
        INSERT INTO audit_log (id, conversation_id, request_redacted, response_text, context_used, query_category, created_at)
        VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
        "#,
    )
    .bind(&id)
    .bind(&input.conversation_id)
    .bind(&input.request_redacted)
    .bind(&input.response_text)
    .bind(&context_used)
    .bind(&input.query_category)
    .execute(pool)
    .await?;

    get_audit_entry(pool, &id).await
}

/// Get an audit entry by ID
pub async fn get_audit_entry(pool: &DbPool, id: &str) -> Result<AuditEntry, AuditError> {
    let entry = sqlx::query_as::<_, AuditEntry>(
        r#"
        SELECT id, conversation_id, request_redacted, response_text, context_used, created_at, query_category
        FROM audit_log
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    entry.ok_or_else(|| AuditError::NotFound(id.to_string()))
}

/// List audit entries with optional filtering
///
/// Returns lightweight items sorted by created_at (most recent first)
pub async fn list_audit_entries(
    pool: &DbPool,
    filter: Option<AuditFilter>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AuditListItem>, AuditError> {
    let filter = filter.unwrap_or_default();
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    // Build dynamic query with filters
    let mut conditions = vec!["1=1".to_string()];
    let mut bindings: Vec<String> = vec![];

    if let Some(conv_id) = &filter.conversation_id {
        conditions.push("conversation_id = ?".to_string());
        bindings.push(conv_id.clone());
    }

    if let Some(start) = &filter.start_date {
        conditions.push("created_at >= ?".to_string());
        bindings.push(start.clone());
    }

    if let Some(end) = &filter.end_date {
        conditions.push("created_at <= ?".to_string());
        bindings.push(end.clone());
    }

    // V2.4.2: Add query_category filter
    if let Some(category) = &filter.query_category {
        conditions.push("query_category = ?".to_string());
        bindings.push(category.clone());
    }

    let query = format!(
        r#"
        SELECT id, conversation_id, request_redacted, response_text, context_used, created_at, query_category
        FROM audit_log
        WHERE {}
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
        "#,
        conditions.join(" AND ")
    );

    // Build query with dynamic bindings
    let mut sqlx_query = sqlx::query_as::<_, AuditEntry>(&query);
    for binding in &bindings {
        sqlx_query = sqlx_query.bind(binding);
    }
    sqlx_query = sqlx_query.bind(limit).bind(offset);

    let entries = sqlx_query.fetch_all(pool).await?;

    // Transform to list items with previews
    let list_items = entries
        .into_iter()
        .map(|e| {
            let employee_count = e
                .context_used
                .as_ref()
                .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
                .map(|ids| ids.len())
                .unwrap_or(0);

            AuditListItem {
                id: e.id,
                conversation_id: e.conversation_id,
                request_preview: truncate_preview(&e.request_redacted, 100),
                response_preview: truncate_preview(&e.response_text, 100),
                employee_count,
                created_at: e.created_at,
            }
        })
        .collect();

    Ok(list_items)
}

/// Count audit entries matching filter (for pagination)
pub async fn count_audit_entries(
    pool: &DbPool,
    filter: Option<AuditFilter>,
) -> Result<i64, AuditError> {
    let filter = filter.unwrap_or_default();

    // Build dynamic query with filters
    let mut conditions = vec!["1=1".to_string()];
    let mut bindings: Vec<String> = vec![];

    if let Some(conv_id) = &filter.conversation_id {
        conditions.push("conversation_id = ?".to_string());
        bindings.push(conv_id.clone());
    }

    if let Some(start) = &filter.start_date {
        conditions.push("created_at >= ?".to_string());
        bindings.push(start.clone());
    }

    if let Some(end) = &filter.end_date {
        conditions.push("created_at <= ?".to_string());
        bindings.push(end.clone());
    }

    // V2.4.2: Add query_category filter
    if let Some(category) = &filter.query_category {
        conditions.push("query_category = ?".to_string());
        bindings.push(category.clone());
    }

    let query = format!(
        "SELECT COUNT(*) FROM audit_log WHERE {}",
        conditions.join(" AND ")
    );

    let mut sqlx_query = sqlx::query_as::<_, (i64,)>(&query);
    for binding in &bindings {
        sqlx_query = sqlx_query.bind(binding);
    }

    let result = sqlx_query.fetch_one(pool).await?;
    Ok(result.0)
}

/// Export audit log to CSV format
///
/// Returns CSV content as a string for download.
/// Response is truncated to first 500 chars to keep file size reasonable.
pub async fn export_to_csv(
    pool: &DbPool,
    filter: Option<AuditFilter>,
) -> Result<ExportResult, AuditError> {
    let filter = filter.unwrap_or_default();

    // Build dynamic query with filters
    let mut conditions = vec!["1=1".to_string()];
    let mut bindings: Vec<String> = vec![];

    if let Some(conv_id) = &filter.conversation_id {
        conditions.push("conversation_id = ?".to_string());
        bindings.push(conv_id.clone());
    }

    if let Some(start) = &filter.start_date {
        conditions.push("created_at >= ?".to_string());
        bindings.push(start.clone());
    }

    if let Some(end) = &filter.end_date {
        conditions.push("created_at <= ?".to_string());
        bindings.push(end.clone());
    }

    // V2.4.2: Add query_category filter
    if let Some(category) = &filter.query_category {
        conditions.push("query_category = ?".to_string());
        bindings.push(category.clone());
    }

    let query = format!(
        r#"
        SELECT id, conversation_id, request_redacted, response_text, context_used, created_at, query_category
        FROM audit_log
        WHERE {}
        ORDER BY created_at DESC
        "#,
        conditions.join(" AND ")
    );

    let mut sqlx_query = sqlx::query_as::<_, AuditEntry>(&query);
    for binding in &bindings {
        sqlx_query = sqlx_query.bind(binding);
    }

    let entries = sqlx_query.fetch_all(pool).await?;
    let row_count = entries.len();

    // Build CSV content
    let mut csv = String::new();

    // Header row (V2.4.2: added query_category)
    csv.push_str("id,timestamp,conversation_id,query_category,request_redacted,response_preview,employee_ids_used\n");

    // Data rows
    for entry in &entries {
        let employee_ids = entry
            .context_used
            .as_ref()
            .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
            .map(|ids| ids.join(";"))
            .unwrap_or_default();

        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            escape_csv(&entry.id),
            escape_csv(&entry.created_at),
            escape_csv(&entry.conversation_id.clone().unwrap_or_default()),
            escape_csv(&entry.query_category.clone().unwrap_or_default()),
            escape_csv(&entry.request_redacted),
            escape_csv(&truncate_preview(&entry.response_text, 500)),
            escape_csv(&employee_ids),
        ));
    }

    Ok(ExportResult { csv_content: csv, row_count })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Truncate text to a preview length, adding ellipsis if truncated
fn truncate_preview(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max_len.saturating_sub(3)])
    }
}

/// Escape a string for CSV format
///
/// Wraps in quotes if contains comma, quote, or newline.
/// Doubles any internal quotes.
fn escape_csv(s: &str) -> String {
    let needs_quoting = s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r');

    if needs_quoting {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_preview_short() {
        let text = "Hello world";
        let result = truncate_preview(text, 100);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_truncate_preview_long() {
        let text = "This is a very long text that should be truncated";
        let result = truncate_preview(text, 20);
        assert_eq!(result, "This is a very lo...");
        assert!(result.len() <= 20);
    }

    #[test]
    fn test_truncate_preview_trims_whitespace() {
        let text = "  Hello world  ";
        let result = truncate_preview(text, 100);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_escape_csv_simple() {
        assert_eq!(escape_csv("hello"), "hello");
    }

    #[test]
    fn test_escape_csv_with_comma() {
        assert_eq!(escape_csv("hello, world"), "\"hello, world\"");
    }

    #[test]
    fn test_escape_csv_with_quotes() {
        assert_eq!(escape_csv("say \"hello\""), "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_escape_csv_with_newline() {
        assert_eq!(escape_csv("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_audit_error_serialization() {
        let err = AuditError::NotFound("test-id".to_string());
        let serialized = serde_json::to_string(&err).unwrap();
        assert!(serialized.contains("Audit entry not found"));
    }

    #[test]
    fn test_audit_filter_default() {
        let filter = AuditFilter::default();
        assert!(filter.conversation_id.is_none());
        assert!(filter.start_date.is_none());
        assert!(filter.end_date.is_none());
        assert!(filter.query_category.is_none());
    }

    #[test]
    fn test_create_audit_entry_input() {
        let input = CreateAuditEntry {
            conversation_id: Some("conv-123".to_string()),
            request_redacted: "What is Sarah's rating?".to_string(),
            response_text: "Sarah has a rating of 4.2".to_string(),
            employee_ids_used: vec!["emp-1".to_string(), "emp-2".to_string()],
            query_category: Some("dei".to_string()),
        };

        // Verify serialization works
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("conv-123"));
        assert!(json.contains("emp-1"));
        assert!(json.contains("dei"));
    }

    #[test]
    fn test_employee_ids_json_serialization() {
        let ids = vec!["emp-1".to_string(), "emp-2".to_string(), "emp-3".to_string()];
        let json = serde_json::to_string(&ids).unwrap();
        assert_eq!(json, r#"["emp-1","emp-2","emp-3"]"#);

        // Verify deserialization
        let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], "emp-1");
    }
}
