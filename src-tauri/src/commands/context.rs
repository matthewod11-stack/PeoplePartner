//! Context-builder commands (chat context, system prompt, employee/company
//! context, aggregate eNPS), HR persona switcher, cross-conversation memory,
//! and conversation CRUD.

use crate::context;
use crate::conversations;
use crate::db::Database;
use crate::memory;

// ---- Context builder ----

/// Build chat context for a user message (extracts mentions, finds employees)
/// If selected_employee_id is provided, that employee is always included first
#[tauri::command]
pub(crate) async fn build_chat_context(
    state: tauri::State<'_, Database>,
    user_message: String,
    selected_employee_id: Option<String>,
) -> Result<context::ChatContext, context::ContextError> {
    context::build_chat_context(&state.pool, &user_message, selected_employee_id.as_deref()).await
}

/// Get the system prompt for a chat message
/// If selected_employee_id is provided, that employee is always included first
///
/// V2.1.4: Now returns SystemPromptResult with aggregates and query_type for verification
#[tauri::command]
pub(crate) async fn get_system_prompt(
    state: tauri::State<'_, Database>,
    user_message: String,
    selected_employee_id: Option<String>,
) -> Result<context::SystemPromptResult, context::ContextError> {
    context::get_system_prompt_for_message(&state.pool, &user_message, selected_employee_id.as_deref()).await
}

/// Get employee context by ID (for debugging/display)
#[tauri::command]
pub(crate) async fn get_employee_context(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<context::EmployeeContext, context::ContextError> {
    context::get_employee_context(&state.pool, &employee_id).await
}

/// Get company context
#[tauri::command]
pub(crate) async fn get_company_context(
    state: tauri::State<'_, Database>,
) -> Result<Option<context::CompanyContext>, context::ContextError> {
    context::get_company_context(&state.pool).await
}

/// Get aggregate eNPS score for the organization
#[tauri::command]
pub(crate) async fn get_aggregate_enps(
    state: tauri::State<'_, Database>,
) -> Result<context::EnpsAggregate, context::ContextError> {
    context::calculate_aggregate_enps(&state.pool).await
}

// ---- Personas (V2.1.3) ----

/// Get all available HR personas for the persona switcher
#[tauri::command]
pub(crate) fn get_personas() -> Vec<context::Persona> {
    context::PERSONAS.to_vec()
}

// ---- Memory (cross-conversation) ----

/// Generate a summary for a conversation using the user's active provider (#108)
#[tauri::command]
pub(crate) async fn generate_conversation_summary(
    state: tauri::State<'_, Database>,
    messages_json: String,
) -> Result<String, memory::MemoryError> {
    memory::generate_summary(&state.pool, &messages_json).await
}

/// Save a summary to an existing conversation
#[tauri::command]
pub(crate) async fn save_conversation_summary(
    state: tauri::State<'_, Database>,
    conversation_id: String,
    summary: String,
) -> Result<(), memory::MemoryError> {
    memory::save_summary(&state.pool, &conversation_id, &summary).await
}

/// Search for relevant past conversation memories
#[tauri::command]
pub(crate) async fn search_memories(
    state: tauri::State<'_, Database>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<memory::ConversationSummary>, memory::MemoryError> {
    let limit = limit.unwrap_or(memory::DEFAULT_MEMORY_LIMIT);
    memory::find_relevant_memories(&state.pool, &query, limit).await
}

// ---- Conversation management ----

/// Create a new conversation
#[tauri::command]
pub(crate) async fn create_conversation(
    state: tauri::State<'_, Database>,
    input: conversations::CreateConversation,
) -> Result<conversations::Conversation, conversations::ConversationError> {
    conversations::create_conversation(&state.pool, input).await
}

/// Get a conversation by ID
#[tauri::command]
pub(crate) async fn get_conversation(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<conversations::Conversation, conversations::ConversationError> {
    conversations::get_conversation(&state.pool, &id).await
}

/// Update a conversation (title, messages, summary)
#[tauri::command]
pub(crate) async fn update_conversation(
    state: tauri::State<'_, Database>,
    id: String,
    input: conversations::UpdateConversation,
) -> Result<conversations::Conversation, conversations::ConversationError> {
    conversations::update_conversation(&state.pool, &id, input).await
}

/// List conversations for sidebar display
#[tauri::command]
pub(crate) async fn list_conversations(
    state: tauri::State<'_, Database>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<conversations::ConversationListItem>, conversations::ConversationError> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    conversations::list_conversations(&state.pool, limit, offset).await
}

/// Search conversations using FTS
#[tauri::command]
pub(crate) async fn search_conversations(
    state: tauri::State<'_, Database>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<conversations::ConversationListItem>, conversations::ConversationError> {
    let limit = limit.unwrap_or(20);
    conversations::search_conversations(&state.pool, &query, limit).await
}

/// Delete a conversation
#[tauri::command]
pub(crate) async fn delete_conversation(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), conversations::ConversationError> {
    conversations::delete_conversation(&state.pool, &id).await
}

/// Generate a title for a conversation
#[tauri::command]
pub(crate) async fn generate_conversation_title(
    state: tauri::State<'_, Database>,
    first_message: String,
) -> Result<String, conversations::ConversationError> {
    Ok(conversations::generate_title_with_fallback(&state.pool, &first_message).await)
}
