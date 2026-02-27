// HR Command Center - Cross-Conversation Memory Module
// Generates summaries and retrieves relevant past conversations
//
// Key responsibilities:
// 1. Generate Claude-powered conversation summaries
// 2. Store summaries in the conversations table
// 3. Search past summaries for context (hybrid: summary-only → full FTS fallback)

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;

use crate::chat::{ChatMessage, ChatResponse};
use crate::db::DbPool;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("API key not configured")]
    NoApiKey,
    #[error("Failed to parse messages: {0}")]
    ParseError(String),
    #[error("Conversation not found: {0}")]
    NotFound(String),
}

impl From<sqlx::Error> for MemoryError {
    fn from(err: sqlx::Error) -> Self {
        MemoryError::Database(err.to_string())
    }
}

impl From<crate::chat::ChatError> for MemoryError {
    fn from(err: crate::chat::ChatError) -> Self {
        match err {
            crate::chat::ChatError::NoApiKey => MemoryError::NoApiKey,
            other => MemoryError::ApiError(other.to_string()),
        }
    }
}

// Make MemoryError serializable for Tauri commands
impl serde::Serialize for MemoryError {
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

/// A conversation summary for cross-conversation memory
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationSummary {
    /// The conversation ID this summary belongs to
    #[sqlx(rename = "id")]
    pub conversation_id: String,
    /// The 2-3 sentence summary of the conversation
    pub summary: String,
    /// When the conversation was created
    pub created_at: String,
}

/// Message format used in messages_json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

// ============================================================================
// Constants
// ============================================================================

/// System prompt for generating summaries
const SUMMARY_SYSTEM_PROMPT: &str = r#"You are a concise HR conversation summarizer. Your summaries are used to help recall past conversations when relevant topics come up again.

Guidelines:
- Write exactly 2-3 sentences
- Include: main topic discussed, any employee names mentioned, key decisions or outcomes
- Be specific and factual, not generic
- If employees were discussed, include their names
- Focus on actionable information that would help recall this conversation later"#;

/// Maximum tokens to request for summary (summaries should be short)
const SUMMARY_MAX_TOKENS: u32 = 200;

/// Default number of memories to retrieve
pub const DEFAULT_MEMORY_LIMIT: usize = 3;

// ============================================================================
// Core Functions
// ============================================================================

/// Generate a summary for a conversation using Claude
///
/// Takes the messages_json from the conversations table and returns
/// a 2-3 sentence summary focusing on topic, employees mentioned, and outcomes.
pub async fn generate_summary(messages_json: &str) -> Result<String, MemoryError> {
    // Parse the messages from JSON
    let messages: Vec<StoredMessage> = serde_json::from_str(messages_json)
        .map_err(|e| MemoryError::ParseError(e.to_string()))?;

    if messages.is_empty() {
        return Ok("Empty conversation - no summary generated.".to_string());
    }

    // Format the conversation for summarization
    let conversation_text = format_conversation_for_summary(&messages);

    // Build the summarization request
    let summary_request = vec![ChatMessage {
        role: "user".to_string(),
        content: format!(
            "Please summarize this HR conversation:\n\n{}",
            conversation_text
        ),
    }];

    // Call Claude for summary (using existing chat module)
    let response = generate_summary_internal(summary_request).await?;

    Ok(response.content.trim().to_string())
}

/// Internal function to call Claude API for summary generation
/// Separated for testability
async fn generate_summary_internal(
    messages: Vec<ChatMessage>,
) -> Result<ChatResponse, MemoryError> {
    use crate::chat;

    // Use a simpler, direct API call for summaries
    // This avoids the conversation trimming logic meant for longer chats
    chat::send_message(messages, Some(SUMMARY_SYSTEM_PROMPT.to_string()), "anthropic")
        .await
        .map_err(MemoryError::from)
}

/// Format conversation messages into a readable string for summarization
fn format_conversation_for_summary(messages: &[StoredMessage]) -> String {
    messages
        .iter()
        .map(|msg| {
            let role_label = if msg.role == "user" { "User" } else { "Assistant" };
            format!("{}: {}", role_label, msg.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Save a summary to an existing conversation
///
/// Updates the summary field in the conversations table and keeps FTS in sync.
pub async fn save_summary(
    pool: &DbPool,
    conversation_id: &str,
    summary: &str,
) -> Result<(), MemoryError> {
    let result = sqlx::query(
        r#"
        UPDATE conversations
        SET summary = ?, updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(summary)
    .bind(conversation_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(MemoryError::NotFound(conversation_id.to_string()));
    }

    Ok(())
}

/// Find relevant memories for a query using hybrid search
///
/// Strategy:
/// 1. First try summary-only search (more focused results)
/// 2. Fall back to full FTS if no summary matches found
pub async fn find_relevant_memories(
    pool: &DbPool,
    query: &str,
    limit: usize,
) -> Result<Vec<ConversationSummary>, MemoryError> {
    // Skip search for very short queries
    if query.trim().len() < 3 {
        return Ok(Vec::new());
    }

    // Step 1: Try summary-only search (more focused)
    let results = search_summaries_only(pool, query, limit).await?;

    if !results.is_empty() {
        return Ok(results);
    }

    // Step 2: Fall back to full FTS search
    search_full_conversation_fts(pool, query, limit).await
}

/// Search only in summary field using LIKE (case-insensitive substring match)
async fn search_summaries_only(
    pool: &DbPool,
    query: &str,
    limit: usize,
) -> Result<Vec<ConversationSummary>, MemoryError> {
    // Extract meaningful keywords from query (skip common words)
    let keywords = extract_search_keywords(query);

    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    // Build a query that matches any keyword in the summary
    // For simplicity, we'll search for the first meaningful keyword
    let search_term = format!("%{}%", keywords[0]);

    let summaries = sqlx::query_as::<_, ConversationSummary>(
        r#"
        SELECT id, summary, created_at
        FROM conversations
        WHERE summary IS NOT NULL
          AND summary != ''
          AND summary LIKE ?
        ORDER BY updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(&search_term)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(summaries)
}

/// Search using full-text search on title, messages, and summary
async fn search_full_conversation_fts(
    pool: &DbPool,
    query: &str,
    limit: usize,
) -> Result<Vec<ConversationSummary>, MemoryError> {
    // Prepare FTS query (escape special characters)
    let fts_query = prepare_fts_query(query);

    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let summaries = sqlx::query_as::<_, ConversationSummary>(
        r#"
        SELECT c.id, c.summary, c.created_at
        FROM conversations c
        INNER JOIN conversations_fts fts ON c.rowid = fts.rowid
        WHERE c.summary IS NOT NULL
          AND c.summary != ''
          AND conversations_fts MATCH ?
        ORDER BY rank
        LIMIT ?
        "#,
    )
    .bind(&fts_query)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(summaries)
}

/// Extract meaningful search keywords from a query
fn extract_search_keywords(query: &str) -> Vec<String> {
    // Common words to skip
    let stop_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "about", "with", "from", "for", "on",
        "in", "to", "of", "and", "or", "but", "if", "then", "so", "what",
        "when", "where", "who", "how", "any", "all", "each", "every", "some",
        "me", "my", "we", "our", "you", "your", "their", "this", "that",
    ];

    query
        .to_lowercase()
        .split_whitespace()
        .map(|word| {
            // Strip punctuation from start and end of words
            word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        })
        .filter(|word| {
            word.len() >= 3 && !stop_words.contains(&word.as_ref())
        })
        .map(|s| s.to_string())
        .collect()
}

/// Prepare a query string for FTS5 MATCH
fn prepare_fts_query(query: &str) -> String {
    // Extract keywords and join with OR for broader matching
    let keywords = extract_search_keywords(query);

    if keywords.is_empty() {
        return String::new();
    }

    // Escape special FTS5 characters and wrap in quotes for phrase matching
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
    fn test_format_conversation_for_summary() {
        let messages = vec![
            StoredMessage {
                id: "1".to_string(),
                role: "user".to_string(),
                content: "What should I do about Sarah's attendance?".to_string(),
                timestamp: "2024-01-15T10:00:00Z".to_string(),
            },
            StoredMessage {
                id: "2".to_string(),
                role: "assistant".to_string(),
                content: "I recommend documenting the incidents first.".to_string(),
                timestamp: "2024-01-15T10:00:30Z".to_string(),
            },
        ];

        let formatted = format_conversation_for_summary(&messages);

        assert!(formatted.contains("User: What should I do about Sarah's attendance?"));
        assert!(formatted.contains("Assistant: I recommend documenting the incidents first."));
    }

    #[test]
    fn test_extract_search_keywords() {
        let keywords = extract_search_keywords("What about Sarah's performance review?");

        assert!(keywords.contains(&"sarah's".to_string()));
        assert!(keywords.contains(&"performance".to_string()));
        assert!(keywords.contains(&"review".to_string()));
        // Should not contain stop words
        assert!(!keywords.contains(&"what".to_string()));
        assert!(!keywords.contains(&"about".to_string()));
    }

    #[test]
    fn test_extract_search_keywords_short_words() {
        let keywords = extract_search_keywords("I am at HR");

        // Should filter out words shorter than 3 chars
        assert!(keywords.is_empty() || !keywords.contains(&"am".to_string()));
        assert!(keywords.is_empty() || !keywords.contains(&"at".to_string()));
        assert!(keywords.is_empty() || !keywords.contains(&"i".to_string()));
    }

    #[test]
    fn test_prepare_fts_query() {
        let fts = prepare_fts_query("Sarah performance review");

        assert!(fts.contains("\"sarah\""));
        assert!(fts.contains("\"performance\""));
        assert!(fts.contains("\"review\""));
        assert!(fts.contains(" OR "));
    }

    #[test]
    fn test_prepare_fts_query_escapes_quotes() {
        let fts = prepare_fts_query("test \"quoted\" word");

        // Should not have unescaped quotes that break the query
        assert!(!fts.contains("\"\""));
    }

    #[test]
    fn test_prepare_fts_query_empty_on_stop_words() {
        let fts = prepare_fts_query("the a an is");

        // All stop words should result in empty query
        assert!(fts.is_empty());
    }

    #[test]
    fn test_summary_system_prompt_is_concise() {
        // Verify the system prompt fits within reasonable token budget
        let prompt_len = SUMMARY_SYSTEM_PROMPT.len();
        // At ~4 chars per token, should be under 200 tokens
        assert!(prompt_len < 800, "System prompt too long: {} chars", prompt_len);
    }

    #[test]
    fn test_stored_message_deserialization() {
        let json = r#"[
            {"id": "1", "role": "user", "content": "Hello", "timestamp": "2024-01-01T00:00:00Z"},
            {"id": "2", "role": "assistant", "content": "Hi there!", "timestamp": "2024-01-01T00:00:01Z"}
        ]"#;

        let messages: Vec<StoredMessage> = serde_json::from_str(json).unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }
}
