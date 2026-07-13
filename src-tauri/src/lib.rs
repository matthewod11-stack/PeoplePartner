// People Partner - Rust Backend
//
// Tauri command handlers are organized by domain in `src/commands/`. This
// file handles module wiring, app setup, and the master
// `tauri::generate_handler!` registration list.

use tauri::Manager;

mod audit;
mod backup;
mod bulk_import;
mod chat;
mod commands;
mod company;
mod context;
mod conversations;
mod data_quality;
mod db;
mod dei;
mod device_id;
mod documents;
mod employees;
mod enps;
mod file_parser;
mod grounding;
mod highlights;
mod keyring;
mod license_cache;
mod license_signing;
mod logging;
mod memory;
mod models;
mod network;
mod people_map;
mod performance_ratings;
mod performance_reviews;
mod pii;
mod provider;
mod providers;
mod recruiting;
mod review_cycles;
mod settings;
mod signals;
mod trial;

use db::Database;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(logging::plugin())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::system::greet,
            commands::system::check_db,
            commands::api_keys::store_api_key,
            commands::api_keys::has_api_key,
            commands::api_keys::delete_api_key,
            commands::api_keys::validate_api_key_format,
            // Provider management
            commands::api_keys::get_active_provider,
            commands::api_keys::set_active_provider,
            commands::api_keys::list_providers,
            commands::api_keys::validate_provider_api_key_format,
            commands::api_keys::store_provider_api_key,
            commands::api_keys::has_provider_api_key,
            commands::api_keys::delete_provider_api_key,
            commands::api_keys::has_any_provider_api_key,
            // Model selection
            commands::api_keys::get_models_for_provider,
            commands::api_keys::get_active_model,
            commands::api_keys::set_active_model,
            // License
            commands::license::store_license_key,
            commands::license::has_license_key,
            commands::license::delete_license_key,
            commands::license::validate_license_key_format,
            commands::license::revalidate_license,
            // Chat
            commands::chat::send_chat_message,
            commands::chat::send_chat_message_streaming,
            commands::chat::cancel_stream,
            commands::chat::check_network_status,
            commands::chat::is_online,
            // Company profile
            commands::employees::has_company,
            commands::employees::get_company,
            commands::employees::upsert_company,
            commands::employees::get_employee_work_states,
            // Employee management
            commands::employees::create_employee,
            commands::employees::get_employee,
            commands::employees::get_employee_by_email,
            commands::employees::update_employee,
            commands::employees::delete_employee,
            commands::employees::list_employees,
            commands::employees::list_employees_with_ratings,
            commands::employees::get_departments,
            commands::employees::get_employee_counts,
            commands::employees::import_employees,
            // Review cycles
            commands::performance::create_review_cycle,
            commands::performance::get_review_cycle,
            commands::performance::update_review_cycle,
            commands::performance::delete_review_cycle,
            commands::performance::list_review_cycles,
            commands::performance::get_active_review_cycle,
            commands::performance::close_review_cycle,
            // Performance ratings
            commands::performance::create_performance_rating,
            commands::performance::get_performance_rating,
            commands::performance::get_ratings_for_employee,
            commands::performance::get_ratings_for_cycle,
            commands::performance::get_latest_rating,
            commands::performance::update_performance_rating,
            commands::performance::delete_performance_rating,
            commands::performance::get_rating_distribution,
            commands::performance::get_average_rating,
            // Performance reviews
            commands::performance::create_performance_review,
            commands::performance::get_performance_review,
            commands::performance::get_reviews_for_employee,
            commands::performance::get_reviews_for_cycle,
            commands::performance::update_performance_review,
            commands::performance::delete_performance_review,
            commands::performance::search_performance_reviews,
            // Review highlights (V2.2.1)
            commands::performance::get_review_highlight,
            commands::performance::get_highlights_for_employee,
            commands::performance::extract_review_highlight,
            commands::performance::extract_highlights_batch,
            commands::performance::find_reviews_pending_extraction,
            commands::performance::get_employee_summary,
            commands::performance::generate_employee_summary,
            commands::performance::invalidate_review_highlight,
            // eNPS
            commands::enps::create_enps_response,
            commands::enps::get_enps_response,
            commands::enps::get_enps_for_employee,
            commands::enps::get_enps_for_survey,
            commands::enps::delete_enps_response,
            commands::enps::calculate_enps_score,
            commands::enps::get_latest_enps_for_employee,
            // File parser
            commands::import::parse_file,
            commands::import::parse_file_preview,
            commands::import::get_supported_extensions,
            commands::import::is_supported_file,
            commands::import::map_employee_columns,
            commands::import::map_rating_columns,
            commands::import::map_enps_columns,
            // Bulk import (test data)
            commands::import::bulk_clear_data,
            commands::import::bulk_import_review_cycles,
            commands::import::bulk_import_employees,
            commands::import::bulk_import_ratings,
            commands::import::bulk_import_reviews,
            commands::import::bulk_import_enps,
            commands::import::verify_data_integrity,
            // Context builder
            commands::context::build_chat_context,
            commands::context::get_system_prompt,
            commands::context::get_employee_context,
            commands::context::get_company_context,
            commands::context::get_aggregate_enps,
            // Attention Signals (V2.4.1)
            commands::enps::is_signals_enabled,
            commands::enps::get_attention_signals,
            commands::enps::get_team_themes,
            // DEI & Fairness Lens (V2.4.2)
            commands::enps::is_fairness_lens_enabled,
            commands::enps::get_representation_breakdown,
            commands::enps::get_rating_parity,
            commands::enps::get_promotion_rates,
            commands::enps::get_fairness_lens_summary,
            // Monday Digest
            commands::enps::get_digest_data,
            // Memory (cross-conversation)
            commands::context::generate_conversation_summary,
            commands::context::save_conversation_summary,
            commands::context::search_memories,
            // Conversation management
            commands::context::create_conversation,
            commands::context::get_conversation,
            commands::context::update_conversation,
            commands::context::list_conversations,
            commands::context::search_conversations,
            commands::context::delete_conversation,
            commands::context::generate_conversation_title,
            // Settings
            commands::system::get_setting,
            commands::system::set_setting,
            commands::system::delete_setting,
            commands::system::has_setting,
            // Personas (V2.1.3)
            commands::context::get_personas,
            // PII scanning
            commands::system::scan_pii,
            // Audit logging (read-only from the frontend — #112 moved the
            // write path backend-side to the chat seam)
            commands::system::get_audit_entry,
            commands::system::list_audit_entries,
            commands::system::count_audit_entries,
            commands::system::export_audit_log,
            // Device ID (trial mode)
            commands::system::get_device_id,
            // Data path
            commands::system::get_data_path,
            // Backup & restore
            commands::system::export_backup,
            commands::system::validate_backup,
            commands::system::import_backup,
            // Data Quality Center (V2.5.1)
            commands::import::analyze_import_headers,
            commands::import::apply_column_mapping,
            commands::import::detect_duplicates,
            commands::import::detect_existing_conflicts,
            commands::import::validate_import_rows,
            commands::import::apply_corrections_and_revalidate,
            commands::import::get_hris_presets,
            commands::import::detect_hris_preset,
            commands::import::apply_hris_preset,
            // Trial mode
            commands::system::get_trial_status,
            commands::system::check_employee_limit,
            // Document ingestion (V3.0)
            commands::documents::set_document_folder,
            commands::documents::remove_document_folder,
            commands::documents::get_document_folder,
            commands::documents::rescan_documents,
            commands::documents::get_document_stats,
            // Recruiting (Sourcerer module) — FHR-71, FHR-72
            commands::recruiting::recruiting_create_search,
            commands::recruiting::recruiting_list_searches,
            commands::recruiting::recruiting_search_exa,
            commands::recruiting::recruiting_has_exa_key,
            commands::recruiting::recruiting_store_exa_key,
            commands::recruiting::recruiting_delete_exa_key,
            commands::recruiting::recruiting_intake_start,
            commands::recruiting::recruiting_intake_step,
            commands::recruiting::recruiting_intake_extract,
            commands::recruiting::recruiting_intake_start_from_seed,
            // Full pipeline command (FHR-73 S1.1 Task 7)
            commands::recruiting::recruiting_run_search,
            commands::recruiting::recruiting_score_candidates,
        ])
        .setup(|app| {
            // Register updater plugin for auto-updates via GitHub Releases
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let handle = app.handle().clone();

            // Initialize database asynchronously
            tauri::async_runtime::block_on(async move {
                match db::init_db(&handle).await {
                    Ok(pool) => {
                        // Start document folder watcher (V3.0). Async since
                        // issue #38 — safe to await inside the existing
                        // tauri::async_runtime::block_on(async move { ... }).
                        let watcher_state =
                            documents::start_watcher(pool.clone(), handle.clone()).await;
                        handle.manage(watcher_state);

                        // Store database pool in app state
                        handle.manage(Database::new(pool));
                        // Registry of in-flight streams for cancel_stream (#25)
                        handle.manage(chat::StreamRegistry::new());
                        log::info!("Database initialized successfully");
                    }
                    Err(e) => {
                        log::error!("FATAL: Database initialization failed: {}", e);
                        // Finder-launched apps drop stderr; write a user-visible
                        // file the user can email to support.
                        if let Ok(dir) = handle.path().app_data_dir() {
                            logging::write_crash_file(&dir, "db_init", &e.to_string());
                        }
                        std::process::exit(1);
                    }
                }
            });

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
