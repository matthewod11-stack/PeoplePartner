//! Document Ingestion commands (V3.0) — folder selection, watcher control,
//! rescans, and folder/document stats.

use crate::db::Database;
use crate::documents;

#[tauri::command]
pub(crate) async fn set_document_folder(
    state: tauri::State<'_, Database>,
    watcher: tauri::State<'_, documents::WatcherState>,
    path: String,
) -> Result<documents::DocumentFolderStats, String> {
    let pool = &state.pool;
    documents::set_document_folder(pool, &path)
        .await
        .map_err(|e| e.to_string())?;
    let stats = documents::scan_folder(pool)
        .await
        .map_err(|e| e.to_string())?;
    // Restart watcher for the new folder
    watcher.start(pool.clone()).await;
    Ok(stats)
}

#[tauri::command]
pub(crate) async fn remove_document_folder(
    state: tauri::State<'_, Database>,
    watcher: tauri::State<'_, documents::WatcherState>,
) -> Result<(), String> {
    documents::remove_document_folder(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
    // Stop watcher — no folder to watch
    watcher.stop().await;
    Ok(())
}

#[tauri::command]
pub(crate) async fn get_document_folder(
    state: tauri::State<'_, Database>,
) -> Result<Option<documents::DocumentFolderStats>, String> {
    documents::get_folder_stats(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn rescan_documents(
    state: tauri::State<'_, Database>,
) -> Result<documents::DocumentFolderStats, String> {
    documents::scan_folder(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn get_document_stats(
    state: tauri::State<'_, Database>,
) -> Result<documents::DocumentStats, String> {
    documents::get_document_stats(&state.pool)
        .await
        .map_err(|e| e.to_string())
}
