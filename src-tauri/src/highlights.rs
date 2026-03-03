// People Partner - Highlights Module
// Extracted structured data from performance reviews
// Session 1: Types and CRUD operations
// Session 2: Extraction pipeline with Claude API

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

use crate::chat::{ChatError, ChatMessage};
use crate::db::DbPool;
use crate::performance_reviews::PerformanceReview;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Serialize)]
pub enum HighlightsError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Highlight not found: {0}")]
    NotFound(String),
    #[error("Summary not found for employee: {0}")]
    SummaryNotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("JSON parse error: {0}")]
    JsonParse(String),
    #[error("Extraction error: {0}")]
    Extraction(String),
}

impl From<sqlx::Error> for HighlightsError {
    fn from(err: sqlx::Error) -> Self {
        HighlightsError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for HighlightsError {
    fn from(err: serde_json::Error) -> Self {
        HighlightsError::JsonParse(err.to_string())
    }
}

impl From<ChatError> for HighlightsError {
    fn from(err: ChatError) -> Self {
        HighlightsError::Extraction(err.to_string())
    }
}

// ============================================================================
// Core Types
// ============================================================================

/// A quote extracted from a performance review
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quote {
    pub sentiment: String, // positive, negative, neutral
    pub text: String,
}

/// Extracted highlights from a single performance review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewHighlight {
    pub id: String,
    pub review_id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub strengths: Vec<String>,
    pub opportunities: Vec<String>,
    pub themes: Vec<String>,
    pub quotes: Vec<Quote>,
    pub overall_sentiment: String,
    pub extraction_model: Option<String>,
    pub extraction_version: i32,
    pub token_count: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

/// Raw database row for ReviewHighlight (JSON fields as strings)
#[derive(Debug, Clone, FromRow)]
struct ReviewHighlightRow {
    id: String,
    review_id: String,
    employee_id: String,
    review_cycle_id: String,
    strengths: String,
    opportunities: String,
    themes: String,
    quotes: String,
    overall_sentiment: Option<String>,
    extraction_model: Option<String>,
    extraction_version: Option<i32>,
    token_count: Option<i32>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ReviewHighlightRow> for ReviewHighlight {
    type Error = HighlightsError;

    fn try_from(row: ReviewHighlightRow) -> Result<Self, Self::Error> {
        Ok(ReviewHighlight {
            id: row.id,
            review_id: row.review_id,
            employee_id: row.employee_id,
            review_cycle_id: row.review_cycle_id,
            strengths: parse_json_array(&row.strengths)?,
            opportunities: parse_json_array(&row.opportunities)?,
            themes: parse_json_array(&row.themes)?,
            quotes: parse_quotes_array(&row.quotes)?,
            overall_sentiment: row.overall_sentiment.unwrap_or_else(|| "neutral".to_string()),
            extraction_model: row.extraction_model,
            extraction_version: row.extraction_version.unwrap_or(1),
            token_count: row.token_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Aggregated career summary for an employee
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeSummary {
    pub id: String,
    pub employee_id: String,
    pub career_narrative: Option<String>,
    pub key_strengths: Vec<String>,
    pub development_areas: Vec<String>,
    pub notable_accomplishments: Vec<String>,
    pub reviews_analyzed: i32,
    pub last_review_date: Option<String>,
    pub generation_model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Raw database row for EmployeeSummary
#[derive(Debug, Clone, FromRow)]
struct EmployeeSummaryRow {
    id: String,
    employee_id: String,
    career_narrative: Option<String>,
    key_strengths: String,
    development_areas: String,
    notable_accomplishments: String,
    reviews_analyzed: Option<i32>,
    last_review_date: Option<String>,
    generation_model: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<EmployeeSummaryRow> for EmployeeSummary {
    type Error = HighlightsError;

    fn try_from(row: EmployeeSummaryRow) -> Result<Self, Self::Error> {
        Ok(EmployeeSummary {
            id: row.id,
            employee_id: row.employee_id,
            career_narrative: row.career_narrative,
            key_strengths: parse_json_array(&row.key_strengths)?,
            development_areas: parse_json_array(&row.development_areas)?,
            notable_accomplishments: parse_json_array(&row.notable_accomplishments)?,
            reviews_analyzed: row.reviews_analyzed.unwrap_or(0),
            last_review_date: row.last_review_date,
            generation_model: row.generation_model,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Input for creating a new highlight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHighlight {
    pub review_id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub strengths: Vec<String>,
    pub opportunities: Vec<String>,
    pub themes: Vec<String>,
    pub quotes: Vec<Quote>,
    pub overall_sentiment: String,
    pub extraction_model: Option<String>,
    pub token_count: Option<i32>,
}

/// Input for creating/updating an employee summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSummary {
    pub employee_id: String,
    pub career_narrative: Option<String>,
    pub key_strengths: Vec<String>,
    pub development_areas: Vec<String>,
    pub notable_accomplishments: Vec<String>,
    pub reviews_analyzed: i32,
    pub last_review_date: Option<String>,
    pub generation_model: Option<String>,
}

/// Progress tracking for batch extraction
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub is_running: bool,
}

// ============================================================================
// JSON Parsing Helpers
// ============================================================================

/// Parse a JSON string array, returning empty vec on error
fn parse_json_array(json_str: &str) -> Result<Vec<String>, HighlightsError> {
    if json_str.is_empty() || json_str == "[]" {
        return Ok(Vec::new());
    }
    serde_json::from_str(json_str).map_err(HighlightsError::from)
}

/// Parse a JSON quotes array
fn parse_quotes_array(json_str: &str) -> Result<Vec<Quote>, HighlightsError> {
    if json_str.is_empty() || json_str == "[]" {
        return Ok(Vec::new());
    }
    serde_json::from_str(json_str).map_err(HighlightsError::from)
}

/// Serialize a vec to JSON string
fn to_json_string<T: Serialize>(value: &T) -> Result<String, HighlightsError> {
    serde_json::to_string(value).map_err(HighlightsError::from)
}

// ============================================================================
// Valid Themes (whitelist)
// ============================================================================

/// Valid theme values that can be extracted
pub const VALID_THEMES: &[&str] = &[
    "leadership",
    "technical-growth",
    "communication",
    "collaboration",
    "execution",
    "learning",
    "innovation",
    "mentoring",
    "problem-solving",
    "customer-focus",
];

/// Filter themes to only include valid values
pub fn filter_valid_themes(themes: Vec<String>) -> Vec<String> {
    themes
        .into_iter()
        .filter(|t| VALID_THEMES.contains(&t.as_str()))
        .collect()
}

/// Validate sentiment value
pub fn validate_sentiment(sentiment: &str) -> bool {
    matches!(sentiment, "positive" | "neutral" | "mixed" | "negative")
}

// ============================================================================
// Extraction Constants
// ============================================================================

/// System prompt for extracting structured data from a performance review
const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are an HR data extraction system. Extract structured information from performance review text.

Output ONLY valid JSON matching this schema:
{
  "strengths": ["string array of 2-5 key strengths mentioned"],
  "opportunities": ["string array of 1-3 development areas mentioned"],
  "themes": ["leadership", "technical-growth", "communication", "collaboration", "execution", "learning", "innovation", "mentoring", "problem-solving", "customer-focus"],
  "quotes": [{"sentiment": "positive|negative|neutral", "text": "verbatim quote under 100 chars"}],
  "overall_sentiment": "positive|neutral|mixed|negative"
}

Guidelines:
- Extract actual content from the review, not generic statements
- Limit quotes to genuinely notable feedback (max 2)
- Themes must be from the provided list only
- If a section has no content, use an empty array []
- overall_sentiment should reflect the balance of positive vs negative feedback"#;

/// System prompt for generating employee career summaries
const SUMMARY_SYSTEM_PROMPT: &str = r#"You are an HR analyst synthesizing performance review data into a career narrative.

Output ONLY valid JSON matching this schema:
{
  "career_narrative": "3-5 sentences describing career trajectory and current standing",
  "key_strengths": ["top 3-5 consistent strengths across reviews"],
  "development_areas": ["1-3 persistent development themes"],
  "notable_accomplishments": ["2-4 standout achievements mentioned"]
}

Guidelines:
- Synthesize patterns across multiple reviews, not just the most recent
- Be specific and factual, not generic
- Focus on trajectory and growth over time
- Note any concerning patterns diplomatically"#;

/// Response structure from Claude for extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractionResponse {
    strengths: Option<Vec<String>>,
    opportunities: Option<Vec<String>>,
    themes: Option<Vec<String>>,
    quotes: Option<Vec<Quote>>,
    overall_sentiment: Option<String>,
}

/// Response structure from Claude for summary generation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SummaryResponse {
    career_narrative: Option<String>,
    key_strengths: Option<Vec<String>>,
    development_areas: Option<Vec<String>>,
    notable_accomplishments: Option<Vec<String>>,
}

// ============================================================================
// Extraction Functions
// ============================================================================

/// Extract highlights from a single performance review using Claude API
pub async fn extract_highlights_for_review(
    pool: &DbPool,
    review: &PerformanceReview,
) -> Result<ReviewHighlight, HighlightsError> {
    use crate::chat;

    // Build the user prompt with review content
    let user_prompt = format_review_for_extraction(review);

    // Call Claude API
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: user_prompt,
    }];

    let response = chat::send_message(messages, Some(EXTRACTION_SYSTEM_PROMPT.to_string()), "anthropic", None)
        .await
        .map_err(HighlightsError::from)?;

    // Parse the JSON response
    let extracted = parse_extraction_response(&response.content)?;

    // Record which model was actually used (catalog default)
    let model_used = crate::models::default_model_for_provider("anthropic")
        .unwrap_or("unknown")
        .to_string();

    // Create and save the highlight
    let input = CreateHighlight {
        review_id: review.id.clone(),
        employee_id: review.employee_id.clone(),
        review_cycle_id: review.review_cycle_id.clone(),
        strengths: extracted.strengths.unwrap_or_default(),
        opportunities: extracted.opportunities.unwrap_or_default(),
        themes: extracted.themes.unwrap_or_default(),
        quotes: extracted.quotes.unwrap_or_default(),
        overall_sentiment: extracted.overall_sentiment.unwrap_or_else(|| "neutral".to_string()),
        extraction_model: Some(model_used),
        token_count: Some(response.input_tokens as i32 + response.output_tokens as i32),
    };

    create_highlight(pool, input).await
}

/// Format a performance review into a prompt for extraction
fn format_review_for_extraction(review: &PerformanceReview) -> String {
    let mut parts = vec!["Extract structured data from this performance review:".to_string()];

    if let Some(ref strengths) = review.strengths {
        if !strengths.is_empty() {
            parts.push(format!("\nSTRENGTHS:\n{}", strengths));
        }
    }

    if let Some(ref areas) = review.areas_for_improvement {
        if !areas.is_empty() {
            parts.push(format!("\nAREAS FOR IMPROVEMENT:\n{}", areas));
        }
    }

    if let Some(ref accomplishments) = review.accomplishments {
        if !accomplishments.is_empty() {
            parts.push(format!("\nACCOMPLISHMENTS:\n{}", accomplishments));
        }
    }

    if let Some(ref manager_comments) = review.manager_comments {
        if !manager_comments.is_empty() {
            parts.push(format!("\nMANAGER COMMENTS:\n{}", manager_comments));
        }
    }

    if let Some(ref self_assessment) = review.self_assessment {
        if !self_assessment.is_empty() {
            parts.push(format!("\nSELF ASSESSMENT:\n{}", self_assessment));
        }
    }

    parts.join("\n")
}

/// Parse Claude's JSON response for extraction
fn parse_extraction_response(content: &str) -> Result<ExtractionResponse, HighlightsError> {
    // Claude may include markdown code blocks, strip them
    let json_str = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(json_str).map_err(|e| {
        HighlightsError::JsonParse(format!("Failed to parse extraction response: {}. Content: {}", e, content))
    })
}

/// Extract highlights for multiple reviews in batch
/// Returns results for each review (success or error message)
pub async fn extract_highlights_batch(
    pool: &DbPool,
    review_ids: Vec<String>,
) -> Result<BatchExtractionResult, HighlightsError> {
    use crate::performance_reviews;

    let mut result = BatchExtractionResult {
        total: review_ids.len(),
        succeeded: 0,
        failed: 0,
        errors: Vec::new(),
    };

    for review_id in review_ids {
        // Get the review
        let review = match performance_reviews::get_review(pool, &review_id).await {
            Ok(r) => r,
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("Review {}: {}", review_id, e));
                continue;
            }
        };

        // Check if highlight already exists
        if let Ok(Some(_)) = get_highlight_for_review(pool, &review_id).await {
            // Already extracted, skip
            result.succeeded += 1;
            continue;
        }

        // Extract highlights
        match extract_highlights_for_review(pool, &review).await {
            Ok(_) => result.succeeded += 1,
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!("Review {}: {}", review_id, e));
            }
        }

        // Small delay between API calls to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(result)
}

/// Result of batch extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExtractionResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// Generate a career summary for an employee from their review highlights
pub async fn generate_employee_summary(
    pool: &DbPool,
    employee_id: &str,
) -> Result<EmployeeSummary, HighlightsError> {
    use crate::chat;
    use crate::employees;

    // Get employee name for context
    let employee = employees::get_employee(pool, employee_id)
        .await
        .map_err(|e| HighlightsError::Database(e.to_string()))?;

    // Get all highlights for this employee
    let highlights = get_highlights_for_employee(pool, employee_id).await?;

    if highlights.is_empty() {
        return Err(HighlightsError::Validation(
            "No highlights found for employee. Run extraction first.".to_string(),
        ));
    }

    // Format highlights for summary generation
    let user_prompt = format_highlights_for_summary(&employee.full_name, &highlights);

    // Call Claude API
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: user_prompt,
    }];

    let response = chat::send_message(messages, Some(SUMMARY_SYSTEM_PROMPT.to_string()), "anthropic", None)
        .await
        .map_err(HighlightsError::from)?;

    // Parse the JSON response
    let summary_data = parse_summary_response(&response.content)?;

    // Find the most recent review date
    let last_review_date = highlights
        .first()
        .and_then(|h| Some(h.created_at.clone()));

    // Record which model was actually used (catalog default)
    let model_used = crate::models::default_model_for_provider("anthropic")
        .unwrap_or("unknown")
        .to_string();

    // Save the summary
    let input = CreateSummary {
        employee_id: employee_id.to_string(),
        career_narrative: summary_data.career_narrative,
        key_strengths: summary_data.key_strengths.unwrap_or_default(),
        development_areas: summary_data.development_areas.unwrap_or_default(),
        notable_accomplishments: summary_data.notable_accomplishments.unwrap_or_default(),
        reviews_analyzed: highlights.len() as i32,
        last_review_date,
        generation_model: Some(model_used),
    };

    save_summary(pool, input).await
}

/// Format highlights for summary generation prompt
fn format_highlights_for_summary(employee_name: &str, highlights: &[ReviewHighlight]) -> String {
    let mut parts = vec![format!(
        "Generate a career summary for {} based on these {} performance review highlights:",
        employee_name,
        highlights.len()
    )];

    for (i, h) in highlights.iter().enumerate() {
        parts.push(format!("\n--- Review {} ---", i + 1));
        parts.push(format!("Sentiment: {}", h.overall_sentiment));

        if !h.strengths.is_empty() {
            parts.push(format!("Strengths: {}", h.strengths.join(", ")));
        }
        if !h.opportunities.is_empty() {
            parts.push(format!("Development areas: {}", h.opportunities.join(", ")));
        }
        if !h.themes.is_empty() {
            parts.push(format!("Themes: {}", h.themes.join(", ")));
        }
        for quote in &h.quotes {
            parts.push(format!("Quote ({}): \"{}\"", quote.sentiment, quote.text));
        }
    }

    parts.join("\n")
}

/// Parse Claude's JSON response for summary generation
fn parse_summary_response(content: &str) -> Result<SummaryResponse, HighlightsError> {
    let json_str = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(json_str).map_err(|e| {
        HighlightsError::JsonParse(format!("Failed to parse summary response: {}. Content: {}", e, content))
    })
}

// ============================================================================
// CRUD Operations - Review Highlights
// ============================================================================

/// Create a new review highlight
pub async fn create_highlight(
    pool: &DbPool,
    input: CreateHighlight,
) -> Result<ReviewHighlight, HighlightsError> {
    // Validate
    if input.review_id.trim().is_empty() {
        return Err(HighlightsError::Validation("review_id is required".to_string()));
    }
    if input.employee_id.trim().is_empty() {
        return Err(HighlightsError::Validation("employee_id is required".to_string()));
    }
    if !validate_sentiment(&input.overall_sentiment) {
        return Err(HighlightsError::Validation(format!(
            "Invalid sentiment: {}. Must be positive, neutral, mixed, or negative",
            input.overall_sentiment
        )));
    }

    let id = Uuid::new_v4().to_string();
    let filtered_themes = filter_valid_themes(input.themes);

    sqlx::query(
        r#"
        INSERT INTO review_highlights (
            id, review_id, employee_id, review_cycle_id,
            strengths, opportunities, themes, quotes,
            overall_sentiment, extraction_model, token_count
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.review_id)
    .bind(&input.employee_id)
    .bind(&input.review_cycle_id)
    .bind(to_json_string(&input.strengths)?)
    .bind(to_json_string(&input.opportunities)?)
    .bind(to_json_string(&filtered_themes)?)
    .bind(to_json_string(&input.quotes)?)
    .bind(&input.overall_sentiment)
    .bind(&input.extraction_model)
    .bind(input.token_count)
    .execute(pool)
    .await?;

    get_highlight(pool, &id).await
}

/// Get a highlight by ID
pub async fn get_highlight(pool: &DbPool, id: &str) -> Result<ReviewHighlight, HighlightsError> {
    let row = sqlx::query_as::<_, ReviewHighlightRow>(
        "SELECT * FROM review_highlights WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| HighlightsError::NotFound(id.to_string()))?;

    row.try_into()
}

/// Get highlight by review ID
pub async fn get_highlight_for_review(
    pool: &DbPool,
    review_id: &str,
) -> Result<Option<ReviewHighlight>, HighlightsError> {
    let row = sqlx::query_as::<_, ReviewHighlightRow>(
        "SELECT * FROM review_highlights WHERE review_id = ?"
    )
    .bind(review_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Some(r.try_into()?)),
        None => Ok(None),
    }
}

/// Get all highlights for an employee
pub async fn get_highlights_for_employee(
    pool: &DbPool,
    employee_id: &str,
) -> Result<Vec<ReviewHighlight>, HighlightsError> {
    let rows = sqlx::query_as::<_, ReviewHighlightRow>(
        r#"SELECT h.* FROM review_highlights h
           JOIN review_cycles rc ON h.review_cycle_id = rc.id
           WHERE h.employee_id = ?
           ORDER BY rc.start_date DESC"#,
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

/// Get all highlights for a review cycle
pub async fn get_highlights_for_cycle(
    pool: &DbPool,
    review_cycle_id: &str,
) -> Result<Vec<ReviewHighlight>, HighlightsError> {
    let rows = sqlx::query_as::<_, ReviewHighlightRow>(
        "SELECT * FROM review_highlights WHERE review_cycle_id = ?"
    )
    .bind(review_cycle_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

/// Delete a highlight by ID
pub async fn delete_highlight(pool: &DbPool, id: &str) -> Result<(), HighlightsError> {
    let result = sqlx::query("DELETE FROM review_highlights WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(HighlightsError::NotFound(id.to_string()));
    }
    Ok(())
}

/// Delete highlight by review ID (for invalidation)
pub async fn delete_highlight_for_review(
    pool: &DbPool,
    review_id: &str,
) -> Result<(), HighlightsError> {
    sqlx::query("DELETE FROM review_highlights WHERE review_id = ?")
        .bind(review_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ============================================================================
// CRUD Operations - Employee Summaries
// ============================================================================

/// Create or update an employee summary (upsert)
pub async fn save_summary(
    pool: &DbPool,
    input: CreateSummary,
) -> Result<EmployeeSummary, HighlightsError> {
    if input.employee_id.trim().is_empty() {
        return Err(HighlightsError::Validation("employee_id is required".to_string()));
    }

    // Check if summary exists
    let existing = get_summary_for_employee(pool, &input.employee_id).await?;

    if existing.is_some() {
        // Update existing
        sqlx::query(
            r#"UPDATE employee_summaries SET
                career_narrative = ?,
                key_strengths = ?,
                development_areas = ?,
                notable_accomplishments = ?,
                reviews_analyzed = ?,
                last_review_date = ?,
                generation_model = ?,
                updated_at = datetime('now')
               WHERE employee_id = ?"#,
        )
        .bind(&input.career_narrative)
        .bind(to_json_string(&input.key_strengths)?)
        .bind(to_json_string(&input.development_areas)?)
        .bind(to_json_string(&input.notable_accomplishments)?)
        .bind(input.reviews_analyzed)
        .bind(&input.last_review_date)
        .bind(&input.generation_model)
        .bind(&input.employee_id)
        .execute(pool)
        .await?;
    } else {
        // Insert new
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO employee_summaries (
                id, employee_id, career_narrative,
                key_strengths, development_areas, notable_accomplishments,
                reviews_analyzed, last_review_date, generation_model
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(&input.employee_id)
        .bind(&input.career_narrative)
        .bind(to_json_string(&input.key_strengths)?)
        .bind(to_json_string(&input.development_areas)?)
        .bind(to_json_string(&input.notable_accomplishments)?)
        .bind(input.reviews_analyzed)
        .bind(&input.last_review_date)
        .bind(&input.generation_model)
        .execute(pool)
        .await?;
    }

    get_summary_for_employee(pool, &input.employee_id)
        .await?
        .ok_or_else(|| HighlightsError::SummaryNotFound(input.employee_id))
}

/// Get summary for an employee
pub async fn get_summary_for_employee(
    pool: &DbPool,
    employee_id: &str,
) -> Result<Option<EmployeeSummary>, HighlightsError> {
    let row = sqlx::query_as::<_, EmployeeSummaryRow>(
        "SELECT * FROM employee_summaries WHERE employee_id = ?"
    )
    .bind(employee_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Some(r.try_into()?)),
        None => Ok(None),
    }
}

/// Delete summary for an employee (for invalidation)
pub async fn delete_summary_for_employee(
    pool: &DbPool,
    employee_id: &str,
) -> Result<(), HighlightsError> {
    sqlx::query("DELETE FROM employee_summaries WHERE employee_id = ?")
        .bind(employee_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ============================================================================
// Invalidation Helpers
// ============================================================================

/// Invalidate highlight and summary for a review (call when review is updated)
pub async fn invalidate_for_review(
    pool: &DbPool,
    review_id: &str,
    employee_id: &str,
) -> Result<(), HighlightsError> {
    delete_highlight_for_review(pool, review_id).await?;
    delete_summary_for_employee(pool, employee_id).await?;
    Ok(())
}

/// Find reviews that don't have highlights yet
pub async fn find_reviews_pending_extraction(
    pool: &DbPool,
) -> Result<Vec<String>, HighlightsError> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"SELECT pr.id FROM performance_reviews pr
           LEFT JOIN review_highlights rh ON pr.id = rh.review_id
           WHERE rh.id IS NULL"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Find employees that need summary regeneration
/// (have highlights but no summary, or summary is outdated)
pub async fn find_employees_pending_summary(
    pool: &DbPool,
) -> Result<Vec<String>, HighlightsError> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"SELECT DISTINCT h.employee_id FROM review_highlights h
           LEFT JOIN employee_summaries s ON h.employee_id = s.employee_id
           WHERE s.id IS NULL
              OR s.updated_at < h.updated_at"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// ============================================================================
// Graceful Degradation Helpers
// ============================================================================

/// Get highlights for employee, returning empty vec on error
pub async fn get_highlights_or_empty(pool: &DbPool, employee_id: &str) -> Vec<ReviewHighlight> {
    get_highlights_for_employee(pool, employee_id)
        .await
        .unwrap_or_default()
}

/// Get summary for employee, returning None on error
pub async fn get_summary_or_none(pool: &DbPool, employee_id: &str) -> Option<EmployeeSummary> {
    get_summary_for_employee(pool, employee_id)
        .await
        .ok()
        .flatten()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- JSON Parsing Tests --------------------

    #[test]
    fn test_parse_json_array_valid() {
        let json = r#"["leadership", "communication", "execution"]"#;
        let result = parse_json_array(json).unwrap();
        assert_eq!(result, vec!["leadership", "communication", "execution"]);
    }

    #[test]
    fn test_parse_json_array_empty() {
        assert_eq!(parse_json_array("[]").unwrap(), Vec::<String>::new());
        assert_eq!(parse_json_array("").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn test_parse_json_array_invalid() {
        let result = parse_json_array("not valid json");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), HighlightsError::JsonParse(_)));
    }

    #[test]
    fn test_parse_quotes_array_valid() {
        let json = r#"[{"sentiment": "positive", "text": "Great work on the project"}]"#;
        let result = parse_quotes_array(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].sentiment, "positive");
        assert_eq!(result[0].text, "Great work on the project");
    }

    #[test]
    fn test_parse_quotes_array_multiple() {
        let json = r#"[
            {"sentiment": "positive", "text": "Great leadership"},
            {"sentiment": "negative", "text": "Needs improvement on deadlines"}
        ]"#;
        let result = parse_quotes_array(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].sentiment, "positive");
        assert_eq!(result[1].sentiment, "negative");
    }

    #[test]
    fn test_parse_quotes_array_empty() {
        assert_eq!(parse_quotes_array("[]").unwrap(), Vec::<Quote>::new());
        assert_eq!(parse_quotes_array("").unwrap(), Vec::<Quote>::new());
    }

    // -------------------- Sentiment Validation Tests --------------------

    #[test]
    fn test_validate_sentiment_valid() {
        assert!(validate_sentiment("positive"));
        assert!(validate_sentiment("neutral"));
        assert!(validate_sentiment("mixed"));
        assert!(validate_sentiment("negative"));
    }

    #[test]
    fn test_validate_sentiment_invalid() {
        assert!(!validate_sentiment("happy"));
        assert!(!validate_sentiment("Positive")); // case sensitive
        assert!(!validate_sentiment(""));
        assert!(!validate_sentiment("very_positive"));
    }

    // -------------------- Theme Filtering Tests --------------------

    #[test]
    fn test_filter_valid_themes() {
        let themes = vec![
            "leadership".to_string(),
            "invalid-theme".to_string(),
            "communication".to_string(),
            "not-a-theme".to_string(),
        ];
        let filtered = filter_valid_themes(themes);
        assert_eq!(filtered, vec!["leadership", "communication"]);
    }

    #[test]
    fn test_filter_valid_themes_all_valid() {
        let themes = vec!["leadership".to_string(), "execution".to_string()];
        let filtered = filter_valid_themes(themes);
        assert_eq!(filtered, vec!["leadership", "execution"]);
    }

    #[test]
    fn test_filter_valid_themes_all_invalid() {
        let themes = vec!["foo".to_string(), "bar".to_string()];
        let filtered = filter_valid_themes(themes);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_valid_themes_empty() {
        let filtered = filter_valid_themes(vec![]);
        assert!(filtered.is_empty());
    }

    // -------------------- JSON Serialization Tests --------------------

    #[test]
    fn test_to_json_string_array() {
        let arr = vec!["a".to_string(), "b".to_string()];
        let json = to_json_string(&arr).unwrap();
        assert_eq!(json, r#"["a","b"]"#);
    }

    #[test]
    fn test_to_json_string_quotes() {
        let quotes = vec![Quote {
            sentiment: "positive".to_string(),
            text: "Great work".to_string(),
        }];
        let json = to_json_string(&quotes).unwrap();
        assert!(json.contains("positive"));
        assert!(json.contains("Great work"));
    }

    // -------------------- Row Conversion Tests --------------------

    #[test]
    fn test_review_highlight_row_conversion() {
        let row = ReviewHighlightRow {
            id: "test-id".to_string(),
            review_id: "review-1".to_string(),
            employee_id: "emp-1".to_string(),
            review_cycle_id: "cycle-1".to_string(),
            strengths: r#"["leadership"]"#.to_string(),
            opportunities: r#"["communication"]"#.to_string(),
            themes: r#"["execution"]"#.to_string(),
            quotes: r#"[{"sentiment":"positive","text":"Great"}]"#.to_string(),
            overall_sentiment: Some("positive".to_string()),
            extraction_model: Some("claude-3-sonnet".to_string()),
            extraction_version: Some(1),
            token_count: Some(500),
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let highlight: ReviewHighlight = row.try_into().unwrap();
        assert_eq!(highlight.id, "test-id");
        assert_eq!(highlight.strengths, vec!["leadership"]);
        assert_eq!(highlight.opportunities, vec!["communication"]);
        assert_eq!(highlight.themes, vec!["execution"]);
        assert_eq!(highlight.quotes.len(), 1);
        assert_eq!(highlight.quotes[0].sentiment, "positive");
        assert_eq!(highlight.overall_sentiment, "positive");
    }

    #[test]
    fn test_employee_summary_row_conversion() {
        let row = EmployeeSummaryRow {
            id: "sum-1".to_string(),
            employee_id: "emp-1".to_string(),
            career_narrative: Some("Strong performer".to_string()),
            key_strengths: r#"["leadership", "execution"]"#.to_string(),
            development_areas: r#"["communication"]"#.to_string(),
            notable_accomplishments: r#"["Led project X"]"#.to_string(),
            reviews_analyzed: Some(3),
            last_review_date: Some("2024-01-01".to_string()),
            generation_model: Some("claude-3-sonnet".to_string()),
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let summary: EmployeeSummary = row.try_into().unwrap();
        assert_eq!(summary.employee_id, "emp-1");
        assert_eq!(summary.career_narrative, Some("Strong performer".to_string()));
        assert_eq!(summary.key_strengths, vec!["leadership", "execution"]);
        assert_eq!(summary.development_areas, vec!["communication"]);
        assert_eq!(summary.reviews_analyzed, 3);
    }

    #[test]
    fn test_row_conversion_with_empty_arrays() {
        let row = ReviewHighlightRow {
            id: "test-id".to_string(),
            review_id: "review-1".to_string(),
            employee_id: "emp-1".to_string(),
            review_cycle_id: "cycle-1".to_string(),
            strengths: "[]".to_string(),
            opportunities: "[]".to_string(),
            themes: "[]".to_string(),
            quotes: "[]".to_string(),
            overall_sentiment: None, // Should default to "neutral"
            extraction_model: None,
            extraction_version: None, // Should default to 1
            token_count: None,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let highlight: ReviewHighlight = row.try_into().unwrap();
        assert!(highlight.strengths.is_empty());
        assert!(highlight.opportunities.is_empty());
        assert!(highlight.themes.is_empty());
        assert!(highlight.quotes.is_empty());
        assert_eq!(highlight.overall_sentiment, "neutral");
        assert_eq!(highlight.extraction_version, 1);
    }

    // -------------------- Extraction Response Parsing Tests --------------------

    #[test]
    fn test_parse_extraction_response_valid() {
        let json = r#"{
            "strengths": ["leadership", "communication"],
            "opportunities": ["time management"],
            "themes": ["leadership", "execution"],
            "quotes": [{"sentiment": "positive", "text": "Great leader"}],
            "overall_sentiment": "positive"
        }"#;

        let result = parse_extraction_response(json).unwrap();
        assert_eq!(result.strengths.unwrap(), vec!["leadership", "communication"]);
        assert_eq!(result.opportunities.unwrap(), vec!["time management"]);
        assert_eq!(result.overall_sentiment.unwrap(), "positive");
    }

    #[test]
    fn test_parse_extraction_response_with_markdown() {
        let json = "```json\n{\"strengths\": [\"test\"], \"overall_sentiment\": \"positive\"}\n```";

        let result = parse_extraction_response(json).unwrap();
        assert_eq!(result.strengths.unwrap(), vec!["test"]);
    }

    #[test]
    fn test_parse_extraction_response_partial() {
        // Claude might return partial fields - all should be Option
        let json = r#"{"overall_sentiment": "neutral"}"#;

        let result = parse_extraction_response(json).unwrap();
        assert!(result.strengths.is_none());
        assert!(result.opportunities.is_none());
        assert_eq!(result.overall_sentiment.unwrap(), "neutral");
    }

    #[test]
    fn test_parse_summary_response_valid() {
        let json = r#"{
            "career_narrative": "Strong performer with growth trajectory.",
            "key_strengths": ["leadership", "execution"],
            "development_areas": ["communication"],
            "notable_accomplishments": ["Led major project"]
        }"#;

        let result = parse_summary_response(json).unwrap();
        assert!(result.career_narrative.unwrap().contains("Strong performer"));
        assert_eq!(result.key_strengths.unwrap().len(), 2);
    }

    #[test]
    fn test_format_review_for_extraction() {
        let review = PerformanceReview {
            id: "r1".to_string(),
            employee_id: "e1".to_string(),
            review_cycle_id: "c1".to_string(),
            strengths: Some("Great leader".to_string()),
            areas_for_improvement: Some("Time management".to_string()),
            accomplishments: None,
            goals_next_period: None,
            manager_comments: Some("Excellent work".to_string()),
            self_assessment: None,
            reviewer_id: None,
            review_date: None,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let formatted = format_review_for_extraction(&review);

        assert!(formatted.contains("STRENGTHS:"));
        assert!(formatted.contains("Great leader"));
        assert!(formatted.contains("AREAS FOR IMPROVEMENT:"));
        assert!(formatted.contains("Time management"));
        assert!(formatted.contains("MANAGER COMMENTS:"));
        assert!(formatted.contains("Excellent work"));
        // Should not include sections with no content
        assert!(!formatted.contains("ACCOMPLISHMENTS:"));
        assert!(!formatted.contains("SELF ASSESSMENT:"));
    }

    #[test]
    fn test_format_highlights_for_summary() {
        let highlights = vec![
            ReviewHighlight {
                id: "h1".to_string(),
                review_id: "r1".to_string(),
                employee_id: "e1".to_string(),
                review_cycle_id: "c1".to_string(),
                strengths: vec!["leadership".to_string()],
                opportunities: vec!["communication".to_string()],
                themes: vec!["execution".to_string()],
                quotes: vec![Quote {
                    sentiment: "positive".to_string(),
                    text: "Great work".to_string(),
                }],
                overall_sentiment: "positive".to_string(),
                extraction_model: None,
                extraction_version: 1,
                token_count: None,
                created_at: "2024-01-01".to_string(),
                updated_at: "2024-01-01".to_string(),
            },
        ];

        let formatted = format_highlights_for_summary("John Doe", &highlights);

        assert!(formatted.contains("John Doe"));
        assert!(formatted.contains("Review 1"));
        assert!(formatted.contains("Sentiment: positive"));
        assert!(formatted.contains("Strengths: leadership"));
        assert!(formatted.contains("Development areas: communication"));
        assert!(formatted.contains("Quote (positive): \"Great work\""));
    }
}
