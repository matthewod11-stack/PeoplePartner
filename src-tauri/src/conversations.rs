// People Partner - Conversation Management Module
// CRUD operations for conversation persistence and browsing
//
// Key responsibilities:
// 1. Create and update conversations with messages
// 2. List conversations for sidebar display
// 3. Search conversations using FTS5
// 4. Generate titles for new conversations

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;

use crate::db::DbPool;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum ConversationError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Conversation not found: {0}")]
    NotFound(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl From<sqlx::Error> for ConversationError {
    fn from(err: sqlx::Error) -> Self {
        ConversationError::Database(err.to_string())
    }
}

// Make ConversationError serializable for Tauri commands
impl serde::Serialize for ConversationError {
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

/// Full conversation record from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Conversation {
    pub id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub messages_json: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight conversation item for sidebar list
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationListItem {
    pub id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub message_count: i64,
    pub first_message_preview: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for creating a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversation {
    pub id: String,
    pub title: Option<String>,
    pub messages_json: Option<String>,
}

/// Input for updating a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConversation {
    pub title: Option<String>,
    pub messages_json: Option<String>,
    pub summary: Option<String>,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Create a new conversation
///
/// Called when a conversation begins (before first message is sent)
pub async fn create_conversation(
    pool: &DbPool,
    input: CreateConversation,
) -> Result<Conversation, ConversationError> {
    let messages_json = input.messages_json.unwrap_or_else(|| "[]".to_string());

    sqlx::query(
        r#"
        INSERT INTO conversations (id, title, messages_json, created_at, updated_at)
        VALUES (?, ?, ?, datetime('now'), datetime('now'))
        "#,
    )
    .bind(&input.id)
    .bind(&input.title)
    .bind(&messages_json)
    .execute(pool)
    .await?;

    get_conversation(pool, &input.id).await
}

/// Get a conversation by ID
pub async fn get_conversation(
    pool: &DbPool,
    id: &str,
) -> Result<Conversation, ConversationError> {
    let conversation = sqlx::query_as::<_, Conversation>(
        r#"
        SELECT id, title, summary, messages_json, created_at, updated_at
        FROM conversations
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    conversation.ok_or_else(|| ConversationError::NotFound(id.to_string()))
}

/// Update a conversation (title, messages, or summary)
///
/// Called after each message exchange to persist the conversation
pub async fn update_conversation(
    pool: &DbPool,
    id: &str,
    input: UpdateConversation,
) -> Result<Conversation, ConversationError> {
    // Build dynamic UPDATE query based on provided fields
    let mut set_clauses = vec!["updated_at = datetime('now')".to_string()];
    let mut bindings: Vec<String> = vec![];

    if let Some(title) = &input.title {
        set_clauses.push("title = ?".to_string());
        bindings.push(title.clone());
    }

    if let Some(messages_json) = &input.messages_json {
        set_clauses.push("messages_json = ?".to_string());
        bindings.push(messages_json.clone());
    }

    if let Some(summary) = &input.summary {
        set_clauses.push("summary = ?".to_string());
        bindings.push(summary.clone());
    }

    let query = format!(
        "UPDATE conversations SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    // Build the query with bindings
    let mut sqlx_query = sqlx::query(&query);
    for binding in &bindings {
        sqlx_query = sqlx_query.bind(binding);
    }
    sqlx_query = sqlx_query.bind(id);

    let result = sqlx_query.execute(pool).await?;

    if result.rows_affected() == 0 {
        // Conversation doesn't exist - create it
        return create_conversation(
            pool,
            CreateConversation {
                id: id.to_string(),
                title: input.title,
                messages_json: input.messages_json,
            },
        )
        .await;
    }

    get_conversation(pool, id).await
}

/// List conversations for sidebar display
///
/// Returns lightweight items sorted by updated_at (most recent first)
pub async fn list_conversations(
    pool: &DbPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<ConversationListItem>, ConversationError> {
    // Use a subquery to count messages and extract first message preview
    let conversations = sqlx::query_as::<_, ConversationListItem>(
        r#"
        SELECT
            id,
            title,
            summary,
            json_array_length(messages_json) as message_count,
            CASE
                WHEN json_array_length(messages_json) > 0
                THEN substr(json_extract(messages_json, '$[0].content'), 1, 100)
                ELSE NULL
            END as first_message_preview,
            created_at,
            updated_at
        FROM conversations
        WHERE json_array_length(messages_json) > 0
        ORDER BY updated_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(conversations)
}

/// Search conversations using FTS5
///
/// Searches across title, messages_json, and summary fields
pub async fn search_conversations(
    pool: &DbPool,
    query: &str,
    limit: i64,
) -> Result<Vec<ConversationListItem>, ConversationError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    // Prepare FTS5 query - wrap each word in quotes for phrase matching
    let fts_query = prepare_fts_query(trimmed);

    if fts_query.is_empty() {
        return Ok(vec![]);
    }

    let conversations = sqlx::query_as::<_, ConversationListItem>(
        r#"
        SELECT
            c.id,
            c.title,
            c.summary,
            json_array_length(c.messages_json) as message_count,
            CASE
                WHEN json_array_length(c.messages_json) > 0
                THEN substr(json_extract(c.messages_json, '$[0].content'), 1, 100)
                ELSE NULL
            END as first_message_preview,
            c.created_at,
            c.updated_at
        FROM conversations c
        INNER JOIN conversations_fts fts ON c.rowid = fts.rowid
        WHERE conversations_fts MATCH ?
          AND json_array_length(c.messages_json) > 0
        ORDER BY rank
        LIMIT ?
        "#,
    )
    .bind(&fts_query)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(conversations)
}

/// Delete a conversation by ID.
///
/// Audit rows are deliberately NOT deleted here. Migration 011 dropped the
/// `audit_log.conversation_id` FK, so the audit trail survives the conversation
/// (with its `conversation_id` now a dangling reference — accepted because no
/// code JOINs audit_log back to conversations). Removing the audit rows would
/// be a tamper vector (user deletes conversation → hides what they asked
/// Claude), which the append-only audit log explicitly defends against.
pub async fn delete_conversation(
    pool: &DbPool,
    id: &str,
) -> Result<(), ConversationError> {
    let result = sqlx::query(
        r#"
        DELETE FROM conversations
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ConversationError::NotFound(id.to_string()));
    }

    Ok(())
}

/// Get total count of conversations (for pagination)
pub async fn count_conversations(pool: &DbPool) -> Result<i64, ConversationError> {
    let result: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM conversations
        WHERE json_array_length(messages_json) > 0
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(result.0)
}

// ============================================================================
// Title Generation
// ============================================================================

/// System prompt for generating conversation titles
const TITLE_SYSTEM_PROMPT: &str = r#"Generate a very short title (3-5 words max) for this HR conversation.
The title should capture the main topic or question.
Do not use quotes or punctuation at the end.
Just respond with the title, nothing else."#;

/// Generate a title for a conversation using Claude
///
/// Takes the first user message and generates a 3-5 word title
pub async fn generate_title(first_message: &str) -> Result<String, ConversationError> {
    use crate::chat::{send_message, ChatMessage};

    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: format!("Generate a title for: {}", first_message),
    }];

    let response = send_message(messages, Some(TITLE_SYSTEM_PROMPT.to_string()), "anthropic", None)
        .await
        .map_err(|e| ConversationError::Database(format!("Title generation failed: {}", e)))?;

    // Clean up the response - remove quotes, periods, extra whitespace
    let title = response
        .content
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches('.')
        .to_string();

    // Truncate if too long (fallback safety)
    if title.len() > 60 {
        let end = title
            .char_indices()
            .take_while(|(i, _)| *i <= 60)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(title.len());
        Ok(format!("{}...", &title[..end]))
    } else {
        Ok(title)
    }
}

/// Generate a title from the first message (fallback: truncation)
///
/// Tries Claude first, falls back to simple truncation if that fails
pub async fn generate_title_with_fallback(first_message: &str) -> String {
    match generate_title(first_message).await {
        Ok(title) => title,
        Err(_) => {
            // Fallback: truncate first message
            let truncated = first_message.chars().take(40).collect::<String>();
            if truncated.len() < first_message.len() {
                format!("{}...", truncated.trim())
            } else {
                truncated
            }
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Prepare a query string for FTS5 MATCH
fn prepare_fts_query(query: &str) -> String {
    // Common words to skip
    let stop_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "about", "with", "from", "for", "on",
        "in", "to", "of", "and", "or", "but", "if", "then", "so", "what",
        "when", "where", "who", "how", "any", "all", "each", "every", "some",
        "me", "my", "we", "our", "you", "your", "their", "this", "that",
    ];

    let keywords: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| word.len() >= 3 && !stop_words.contains(&word.as_ref()))
        .map(|s| s.to_string())
        .collect();

    if keywords.is_empty() {
        return String::new();
    }

    // Escape special FTS5 characters and wrap in quotes
    keywords
        .iter()
        .map(|k| format!("\"{}\"", k.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_fts_query_basic() {
        let result = prepare_fts_query("Sarah performance review");
        assert!(result.contains("\"sarah\""));
        assert!(result.contains("\"performance\""));
        assert!(result.contains("\"review\""));
        assert!(result.contains(" OR "));
    }

    #[test]
    fn test_prepare_fts_query_filters_stop_words() {
        let result = prepare_fts_query("what is the status");
        // "what", "is", "the" are stop words, "status" should remain
        assert!(result.contains("\"status\""));
        assert!(!result.contains("\"what\""));
        assert!(!result.contains("\"the\""));
    }

    #[test]
    fn test_prepare_fts_query_empty_on_all_stop_words() {
        let result = prepare_fts_query("the a an is");
        assert!(result.is_empty());
    }

    #[test]
    fn test_prepare_fts_query_filters_short_words() {
        let result = prepare_fts_query("I am HR");
        // "I", "am", "HR" - only "HR" is 2 chars (filtered), all filtered
        assert!(result.is_empty() || !result.contains("\"am\""));
    }

    #[test]
    fn test_prepare_fts_query_escapes_quotes() {
        let result = prepare_fts_query("test \"quoted\" word");
        // Should not have broken quotes
        assert!(!result.contains("\"\""));
    }

    #[test]
    fn test_conversation_error_serialization() {
        let err = ConversationError::NotFound("test-id".to_string());
        let serialized = serde_json::to_string(&err).unwrap();
        assert!(serialized.contains("Conversation not found"));
    }

    #[test]
    fn test_title_system_prompt_is_concise() {
        // Verify the system prompt fits within reasonable token budget
        let prompt_len = TITLE_SYSTEM_PROMPT.len();
        // At ~4 chars per token, should be under 100 tokens
        assert!(prompt_len < 400, "Title system prompt too long: {} chars", prompt_len);
    }
}
