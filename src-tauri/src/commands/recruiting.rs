//! Tauri command bridge for the `recruiting` module.

use crate::db::Database;
use crate::recruiting::{self, RecruitingSearch};

/// Create a new recruiting search. Returns the generated row ID.
#[tauri::command]
pub(crate) async fn recruiting_create_search(
    state: tauri::State<'_, Database>,
    query: String,
    seed_employee_id: Option<String>,
) -> Result<String, String> {
    recruiting::create_search(&state.pool, &query, seed_employee_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// List all recruiting searches, newest first.
#[tauri::command]
pub(crate) async fn recruiting_list_searches(
    state: tauri::State<'_, Database>,
) -> Result<Vec<RecruitingSearch>, String> {
    recruiting::list_searches(&state.pool)
        .await
        .map_err(|e| e.to_string())
}
