//! eNPS responses + Attention Signals (V2.4.1) + DEI Fairness Lens (V2.4.2)
//! + Monday Digest commands.
//!
//! Includes the `DigestEmployee` / `DigestData` payload types used only by
//! `get_digest_data`.

use crate::db::Database;
use crate::dei;
use crate::enps;
use crate::context;
use crate::signals;

// ============================================================================
// eNPS Commands
// ============================================================================

#[tauri::command]
pub(crate) async fn create_enps_response(
    state: tauri::State<'_, Database>,
    input: enps::CreateEnps,
) -> Result<enps::EnpsResponse, enps::EnpsError> {
    enps::create_enps(&state.pool, input).await
}

#[tauri::command]
pub(crate) async fn get_enps_response(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<enps::EnpsResponse, enps::EnpsError> {
    enps::get_enps(&state.pool, &id).await
}

#[tauri::command]
pub(crate) async fn get_enps_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Vec<enps::EnpsResponse>, enps::EnpsError> {
    enps::get_enps_for_employee(&state.pool, &employee_id).await
}

#[tauri::command]
pub(crate) async fn get_enps_for_survey(
    state: tauri::State<'_, Database>,
    survey_name: String,
) -> Result<Vec<enps::EnpsResponse>, enps::EnpsError> {
    enps::get_enps_for_survey(&state.pool, &survey_name).await
}

#[tauri::command]
pub(crate) async fn delete_enps_response(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), enps::EnpsError> {
    enps::delete_enps(&state.pool, &id).await
}

#[tauri::command]
pub(crate) async fn calculate_enps_score(
    state: tauri::State<'_, Database>,
    survey_name: String,
) -> Result<enps::EnpsScore, enps::EnpsError> {
    enps::calculate_enps(&state.pool, &survey_name).await
}

#[tauri::command]
pub(crate) async fn get_latest_enps_for_employee(
    state: tauri::State<'_, Database>,
    employee_id: String,
) -> Result<Option<enps::EnpsResponse>, enps::EnpsError> {
    enps::get_latest_enps(&state.pool, &employee_id).await
}

// ============================================================================
// Attention Signals Commands (V2.4.1)
// ============================================================================

/// Check if the attention signals feature is enabled
#[tauri::command]
pub(crate) async fn is_signals_enabled(
    state: tauri::State<'_, Database>,
) -> Result<bool, String> {
    signals::is_signals_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Get team attention signals for all departments
/// Returns teams sorted by attention score, filtered to MIN_TEAM_SIZE
#[tauri::command]
pub(crate) async fn get_attention_signals(
    state: tauri::State<'_, Database>,
) -> Result<signals::AttentionAreasSummary, String> {
    // Check if feature is enabled first
    let enabled = signals::is_signals_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Attention signals feature is not enabled".to_string());
    }

    signals::get_team_attention_signals(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Get common themes for a specific team from review highlights
#[tauri::command]
pub(crate) async fn get_team_themes(
    state: tauri::State<'_, Database>,
    department: String,
) -> Result<Vec<signals::ThemeOccurrence>, String> {
    signals::get_common_themes_for_team(&state.pool, &department)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// DEI & Fairness Lens Commands (V2.4.2)
// ============================================================================

/// Check if the fairness lens feature is enabled
#[tauri::command]
pub(crate) async fn is_fairness_lens_enabled(
    state: tauri::State<'_, Database>,
) -> Result<bool, String> {
    dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Get representation breakdown by demographic field
/// @param group_by - "gender" or "ethnicity"
/// @param filter_department - Optional department filter
#[tauri::command]
pub(crate) async fn get_representation_breakdown(
    state: tauri::State<'_, Database>,
    group_by: String,
    filter_department: Option<String>,
) -> Result<dei::RepresentationResult, String> {
    // Check if feature is enabled first
    let enabled = dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Fairness lens feature is not enabled".to_string());
    }

    dei::get_representation_breakdown(&state.pool, &group_by, filter_department.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Get rating parity by demographic field
/// @param group_by - "gender" or "ethnicity"
#[tauri::command]
pub(crate) async fn get_rating_parity(
    state: tauri::State<'_, Database>,
    group_by: String,
) -> Result<dei::RatingParityResult, String> {
    // Check if feature is enabled first
    let enabled = dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Fairness lens feature is not enabled".to_string());
    }

    dei::get_rating_parity(&state.pool, &group_by)
        .await
        .map_err(|e| e.to_string())
}

/// Get promotion rates by demographic field
/// Infers promotions from job title keywords
/// @param group_by - "gender" or "ethnicity"
#[tauri::command]
pub(crate) async fn get_promotion_rates(
    state: tauri::State<'_, Database>,
    group_by: String,
) -> Result<dei::PromotionRatesResult, String> {
    // Check if feature is enabled first
    let enabled = dei::is_fairness_lens_enabled(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    if !enabled {
        return Err("Fairness lens feature is not enabled".to_string());
    }

    dei::get_promotion_rates(&state.pool, &group_by)
        .await
        .map_err(|e| e.to_string())
}

/// Get complete fairness lens summary (all DEI metrics)
#[tauri::command]
pub(crate) async fn get_fairness_lens_summary(
    state: tauri::State<'_, Database>,
) -> Result<dei::FairnessLensSummary, String> {
    dei::get_fairness_lens_summary(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// Monday Digest Commands
// ============================================================================

/// Employee data for the Monday Digest (simplified for display)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DigestEmployee {
    pub id: String,
    pub full_name: String,
    pub department: Option<String>,
    pub hire_date: String,
    /// Years of tenure (for anniversaries)
    pub years_tenure: Option<i32>,
    /// Days since hire (for new hires)
    pub days_since_start: Option<i32>,
}

/// Data for the Monday Digest
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DigestData {
    /// Employees with work anniversaries this week (within 7 days)
    pub anniversaries: Vec<DigestEmployee>,
    /// New hires (hired within last 90 days)
    pub new_hires: Vec<DigestEmployee>,
}

/// Get Monday Digest data (anniversaries and new hires)
#[tauri::command]
pub(crate) async fn get_digest_data(
    state: tauri::State<'_, Database>,
) -> Result<DigestData, context::ContextError> {
    use chrono::{NaiveDate, Utc, Datelike};

    let today = Utc::now().date_naive();

    // Get anniversaries (within 7 days) - existing function returns 30-day window
    let anniversary_contexts = context::find_upcoming_anniversaries(&state.pool, 50).await?;

    // Filter to 7 days and convert to DigestEmployee
    let anniversaries: Vec<DigestEmployee> = anniversary_contexts
        .into_iter()
        .filter_map(|emp| {
            let hire_date = emp.hire_date.as_ref()?;
            let hire = NaiveDate::parse_from_str(hire_date, "%Y-%m-%d").ok()?;

            // Calculate this year's anniversary date
            let this_year_anniversary = NaiveDate::from_ymd_opt(today.year(), hire.month(), hire.day())?;

            // Check if anniversary is within 7 days (handles year boundary)
            let days_until = if this_year_anniversary >= today {
                (this_year_anniversary - today).num_days()
            } else {
                // Anniversary already passed this year, check next year
                let next_year_anniversary = NaiveDate::from_ymd_opt(today.year() + 1, hire.month(), hire.day())?;
                (next_year_anniversary - today).num_days()
            };

            if days_until > 7 {
                return None;
            }

            // Calculate years of tenure
            let years = today.year() - hire.year();
            let years_tenure = if this_year_anniversary > today { years } else { years + 1 };

            Some(DigestEmployee {
                id: emp.id,
                full_name: emp.full_name,
                department: emp.department,
                hire_date: hire_date.clone(),
                years_tenure: Some(years_tenure),
                days_since_start: None,
            })
        })
        .collect();

    // Get new hires (last 90 days)
    let new_hire_contexts = context::find_recent_hires(&state.pool, 90, 20).await?;

    let new_hires: Vec<DigestEmployee> = new_hire_contexts
        .into_iter()
        .filter_map(|emp| {
            let hire_date = emp.hire_date.as_ref()?;
            let hire = NaiveDate::parse_from_str(hire_date, "%Y-%m-%d").ok()?;
            let days = (today - hire).num_days() as i32;

            Some(DigestEmployee {
                id: emp.id,
                full_name: emp.full_name,
                department: emp.department,
                hire_date: hire_date.clone(),
                years_tenure: None,
                days_since_start: Some(days),
            })
        })
        .collect();

    Ok(DigestData {
        anniversaries,
        new_hires,
    })
}
