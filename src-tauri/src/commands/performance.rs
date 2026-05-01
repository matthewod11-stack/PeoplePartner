//! Performance-related commands — review cycles, performance ratings,
//! performance reviews, and review highlights / employee summaries (V2.2.1).

use crate::db::Database;
use crate::highlights;
use crate::performance_ratings;
use crate::performance_reviews;
use crate::review_cycles;

// ============================================================================
// Review Cycle Commands
// ============================================================================

/// Create a new review cycle
#[tauri::command]
pub(crate) async fn create_review_cycle(
    state: tauri::State<'_, Database>,
    input: review_cycles::CreateReviewCycle,
) -> Result<review_cycles::ReviewCycle, review_cycles::ReviewCycleError> {
    review_cycles::create_review_cycle(&state.pool, input).await
}

/// Get a review cycle by ID
#[tauri::command]
pub(crate) async fn get_review_cycle(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<review_cycles::ReviewCycle, review_cycles::ReviewCycleError> {
    review_cycles::get_review_cycle(&state.pool, &id).await
}

/// Update a review cycle
#[tauri::command]
pub(crate) async fn update_review_cycle(
    state: tauri::State<'_, Database>,
    id: String,
    input: review_cycles::UpdateReviewCycle,
) -> Result<review_cycles::ReviewCycle, review_cycles::ReviewCycleError> {
    review_cycles::update_review_cycle(&state.pool, &id, input).await
}

/// Delete a review cycle
#[tauri::command]
pub(crate) async fn delete_review_cycle(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), review_cycles::ReviewCycleError> {
    review_cycles::delete_review_cycle(&state.pool, &id).await
}

/// List all review cycles
#[tauri::command]
pub(crate) async fn list_review_cycles(
    state: tauri::State<'_, Database>,
    status_filter: Option<String>,
) -> Result<Vec<review_cycles::ReviewCycle>, review_cycles::ReviewCycleError> {
    review_cycles::list_review_cycles(&state.pool, status_filter).await
}

/// Get the current active review cycle
#[tauri::command]
pub(crate) async fn get_active_review_cycle(
    state: tauri::State<'_, Database>,
) -> Result<Option<review_cycles::ReviewCycle>, review_cycles::ReviewCycleError> {
    review_cycles::get_active_review_cycle(&state.pool).await
}

/// Close a review cycle
#[tauri::command]
pub(crate) async fn close_review_cycle(
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
pub(crate) async fn create_performance_rating(
    state: tauri::State<'_, Database>,
    input: performance_ratings::CreateRating,
) -> Result<performance_ratings::PerformanceRating, performance_ratings::RatingError> {
    performance_ratings::create_rating(&state.pool, input).await
}

/// Get a rating by ID
#[tauri::command]
pub(crate) async fn get_performance_rating(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<performance_ratings::PerformanceRating, performance_ratings::RatingError> {
    performance_ratings::get_rating(&state.pool, &id).await
}

/// Get all ratings for an employee
#[tauri::command]
pub(crate) async fn get_ratings_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<performance_ratings::PerformanceRating>, performance_ratings::RatingError> {
    performance_ratings::get_ratings_for_employee(&state.pool, &employee_id).await
}

/// Get all ratings for a review cycle
#[tauri::command]
pub(crate) async fn get_ratings_for_cycle(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<Vec<performance_ratings::PerformanceRating>, performance_ratings::RatingError> {
    performance_ratings::get_ratings_for_cycle(&state.pool, &review_cycle_id).await
}

/// Get the latest rating for an employee
#[tauri::command]
pub(crate) async fn get_latest_rating(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Option<performance_ratings::PerformanceRating>, performance_ratings::RatingError> {
    performance_ratings::get_latest_rating_for_employee(&state.pool, &employee_id).await
}

/// Update a rating
#[tauri::command]
pub(crate) async fn update_performance_rating(
    state: tauri::State<'_, Database>,
    id: String,
    input: performance_ratings::UpdateRating,
) -> Result<performance_ratings::PerformanceRating, performance_ratings::RatingError> {
    performance_ratings::update_rating(&state.pool, &id, input).await
}

/// Delete a rating
#[tauri::command]
pub(crate) async fn delete_performance_rating(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), performance_ratings::RatingError> {
    performance_ratings::delete_rating(&state.pool, &id).await
}

/// Get rating distribution for a cycle
#[tauri::command]
pub(crate) async fn get_rating_distribution(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<performance_ratings::RatingDistribution, performance_ratings::RatingError> {
    performance_ratings::get_rating_distribution(&state.pool, &review_cycle_id).await
}

/// Get average rating for a cycle
#[tauri::command]
pub(crate) async fn get_average_rating(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<Option<f64>, performance_ratings::RatingError> {
    performance_ratings::get_average_rating(&state.pool, &review_cycle_id).await
}

// ============================================================================
// Performance Review Commands
// ============================================================================

#[tauri::command]
pub(crate) async fn create_performance_review(
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    input: performance_reviews::CreateReview,
) -> Result<performance_reviews::PerformanceReview, performance_reviews::ReviewError> {
    performance_reviews::create_review(&state.pool, input, app).await
}

#[tauri::command]
pub(crate) async fn get_performance_review(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<performance_reviews::PerformanceReview, performance_reviews::ReviewError> {
    performance_reviews::get_review(&state.pool, &id).await
}

#[tauri::command]
pub(crate) async fn get_reviews_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<performance_reviews::PerformanceReview>, performance_reviews::ReviewError> {
    performance_reviews::get_reviews_for_employee(&state.pool, &employee_id).await
}

#[tauri::command]
pub(crate) async fn get_reviews_for_cycle(
    state: tauri::State<'_, Database>,
    review_cycle_id: String,
) -> Result<Vec<performance_reviews::PerformanceReview>, performance_reviews::ReviewError> {
    performance_reviews::get_reviews_for_cycle(&state.pool, &review_cycle_id).await
}

#[tauri::command]
pub(crate) async fn update_performance_review(
    state: tauri::State<'_, Database>,
    id: String,
    input: performance_reviews::UpdateReview,
) -> Result<performance_reviews::PerformanceReview, performance_reviews::ReviewError> {
    performance_reviews::update_review(&state.pool, &id, input).await
}

#[tauri::command]
pub(crate) async fn delete_performance_review(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), performance_reviews::ReviewError> {
    performance_reviews::delete_review(&state.pool, &id).await
}

#[tauri::command]
pub(crate) async fn search_performance_reviews(
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
pub(crate) async fn get_review_highlight(
    state: tauri::State<'_, Database>,
    review_id: String,
) -> Result<Option<highlights::ReviewHighlight>, highlights::HighlightsError> {
    highlights::get_highlight_for_review(&state.pool, &review_id).await
}

/// Get all highlights for an employee
#[tauri::command]
pub(crate) async fn get_highlights_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<highlights::ReviewHighlight>, highlights::HighlightsError> {
    highlights::get_highlights_for_employee(&state.pool, &employee_id).await
}

/// Extract highlights from a single review using Claude API
#[tauri::command]
pub(crate) async fn extract_review_highlight(
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
pub(crate) async fn extract_highlights_batch(
    state: tauri::State<'_, Database>,
    review_ids: Vec<String>,
) -> Result<highlights::BatchExtractionResult, highlights::HighlightsError> {
    highlights::extract_highlights_batch(&state.pool, review_ids).await
}

/// Find reviews that need highlights extracted
#[tauri::command]
pub(crate) async fn find_reviews_pending_extraction(
    state: tauri::State<'_, Database>,
) -> Result<Vec<String>, highlights::HighlightsError> {
    highlights::find_reviews_pending_extraction(&state.pool).await
}

/// Get employee summary
#[tauri::command]
pub(crate) async fn get_employee_summary(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Option<highlights::EmployeeSummary>, highlights::HighlightsError> {
    highlights::get_summary_for_employee(&state.pool, &employee_id).await
}

/// Generate employee career summary from highlights
#[tauri::command]
pub(crate) async fn generate_employee_summary(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<highlights::EmployeeSummary, highlights::HighlightsError> {
    highlights::generate_employee_summary(&state.pool, &employee_id).await
}

/// Invalidate highlight and summary when a review is updated
#[tauri::command]
pub(crate) async fn invalidate_review_highlight(
    state: tauri::State<'_, Database>,
    review_id: String,
    employee_id: String,
) -> Result<(), highlights::HighlightsError> {
    highlights::invalidate_for_review(&state.pool, &review_id, &employee_id).await
}
