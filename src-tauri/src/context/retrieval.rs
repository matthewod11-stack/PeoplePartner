//! Employee + company DB retrieval.
//!
//! Owns `EmployeeContext` and the wider per-employee shape, the batch +
//! per-row context fetchers (including #32's `get_employee_contexts`), and
//! all specialized retrieval routes (longest tenure, top performers, theme
//! search, etc.). `calculate_trend` is private to this module — only the
//! per-employee context builders use it.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};

use crate::db::DbPool;
use crate::highlights;

use super::query::{QueryMentions, TenureDirection, ThemeTarget};
use super::ContextError;

// ============================================================================
// Public Result Types
// ============================================================================

/// Employee with performance and eNPS data for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeContext {
    pub id: String,
    pub full_name: String,
    pub email: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub hire_date: Option<String>,
    pub work_state: Option<String>,
    pub status: String,
    pub manager_name: Option<String>,

    // Performance data
    pub latest_rating: Option<f64>,
    pub latest_rating_cycle: Option<String>,
    pub rating_trend: Option<String>, // "improving", "stable", "declining"
    pub all_ratings: Vec<RatingInfo>,

    // #154: Narrative reviews are stored separately from numeric ratings. An
    // employee can have reviews with no ratings; without these the context
    // renders nothing and the model asserts there is no review history.
    pub review_count: usize,
    pub latest_review_date: Option<String>,

    // eNPS data
    pub latest_enps: Option<i32>,
    pub latest_enps_date: Option<String>,
    pub enps_trend: Option<String>,
    pub all_enps: Vec<EnpsInfo>,

    // V2.2.1: Extracted highlights from performance reviews
    pub career_summary: Option<String>,
    pub key_strengths: Vec<String>,
    pub development_areas: Vec<String>,
    pub recent_highlights: Vec<CycleHighlight>,
}

/// Extracted highlight data for a single review cycle (V2.2.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleHighlight {
    pub cycle_name: String,
    pub strengths: Vec<String>,
    pub opportunities: Vec<String>,
    pub themes: Vec<String>,
    pub sentiment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingInfo {
    pub cycle_name: String,
    pub overall_rating: f64,
    pub rating_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnpsInfo {
    pub score: i32,
    pub survey_name: Option<String>,
    pub survey_date: String,
    pub feedback: Option<String>,
}

/// Company context for system prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyContext {
    pub name: String,
    pub state: String,
    pub industry: Option<String>,
    pub employee_count: i64,
    pub department_count: i64,
}

/// Lightweight employee summary for list queries (~70 chars each)
/// Used when showing rosters instead of full profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeSummary {
    pub id: String,
    pub full_name: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub status: String,
    pub hire_date: Option<String>,
}

/// Full context for building system prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatContext {
    pub company: Option<CompanyContext>,
    pub aggregates: Option<super::aggregates::OrgAggregates>, // Phase 2.7: org-wide stats
    pub query_type: super::query::QueryType,                  // Phase 2.7: classification result
    pub employees: Vec<EmployeeContext>,                      // Full profiles (for Individual/Comparison)
    pub employee_summaries: Vec<EmployeeSummary>,             // Brief roster (for List queries)
    pub employee_ids_used: Vec<String>,
    pub memory_summaries: Vec<String>,
    pub document_chunks: Vec<crate::documents::RetrievedChunk>, // V3.0: Document context
    pub metrics: super::prompt::RetrievalMetrics,              // V2.2.2: retrieval observability
}

// ============================================================================
// Internal Row Types
// ============================================================================

/// Internal struct for employee query result
#[derive(Debug, FromRow)]
struct EmployeeRow {
    id: String,
    email: String,
    full_name: String,
    department: Option<String>,
    job_title: Option<String>,
    hire_date: Option<String>,
    work_state: Option<String>,
    status: String,
    manager_id: Option<String>,
}

/// Internal struct for rating query result
#[derive(Debug, Clone, FromRow)]
struct RatingRow {
    overall_rating: f64,
    cycle_name: String,
    rating_date: Option<String>,
}

/// Internal struct for the #154 narrative-review count/date probe.
/// `review_date` is nullable, so an all-NULL review set yields `None` here.
#[derive(Debug, Clone, FromRow)]
struct ReviewMetaRow {
    review_count: i64,
    latest_review_date: Option<String>,
}

/// Internal struct for eNPS query result
#[derive(Debug, Clone, FromRow)]
struct EnpsRow {
    score: i32,
    survey_name: Option<String>,
    survey_date: String,
    feedback_text: Option<String>,
}

// ============================================================================
// Primary Retrieval Entrypoint
// ============================================================================

/// Find employees matching the extracted mentions
/// Routes to specialized retrieval functions based on query type (primary intent)
/// If selected_employee_id is provided, that employee is always included first
pub async fn find_relevant_employees(
    pool: &DbPool,
    mentions: &QueryMentions,
    limit: usize,
    selected_employee_id: Option<&str>,
) -> Result<Vec<EmployeeContext>, ContextError> {
    // If a specific employee is selected, always include them first
    let (selected_employee, remaining_limit) = if let Some(id) = selected_employee_id {
        match get_employee_context(pool, id).await {
            Ok(emp) => (Some(emp), limit.saturating_sub(1)),
            Err(_) => (None, limit), // ID not found, continue without
        }
    } else {
        (None, limit)
    };
    // Helper to prepend selected employee and filter duplicates
    let finalize_results = |mut employees: Vec<EmployeeContext>| {
        if let Some(ref selected) = selected_employee {
            // Remove selected employee if already in list (avoid duplicates)
            employees.retain(|e| e.id != selected.id);
            // Prepend selected employee
            let mut result = vec![selected.clone()];
            result.extend(employees);
            result
        } else {
            employees
        }
    };

    // Priority 1: Underperformer queries (most specific)
    if mentions.is_underperformer_query {
        let employees = find_underperformers(pool, remaining_limit).await?;
        return Ok(finalize_results(employees));
    }

    // Priority 2: Top performer queries
    if mentions.is_top_performer_query {
        let employees = find_top_performers(pool, remaining_limit).await?;
        return Ok(finalize_results(employees));
    }

    // Priority 3: Tenure queries with direction
    if mentions.is_tenure_query {
        let employees = match mentions.tenure_direction {
            Some(TenureDirection::Longest) => find_longest_tenure(pool, remaining_limit).await?,
            Some(TenureDirection::Newest) => find_newest_employees(pool, remaining_limit).await?,
            Some(TenureDirection::Anniversary) => find_upcoming_anniversaries(pool, remaining_limit).await?,
            None => find_longest_tenure(pool, remaining_limit).await?, // Default to longest if direction unclear
        };
        return Ok(finalize_results(employees));
    }

    // Priority 4: Name-based search (explicit employee mentions)
    let mut employee_ids: Vec<String> = Vec::new();

    // Get selected employee info for smart filtering
    let selected_id = selected_employee.as_ref().map(|e| e.id.as_str());
    let selected_name_lower = selected_employee
        .as_ref()
        .map(|e| e.full_name.to_lowercase());

    for name in &mentions.names {
        // If an employee is selected AND their name matches this query name,
        // skip searching for other employees with the same name.
        if let Some(ref sel_name) = selected_name_lower {
            let name_lower = name.to_lowercase();
            if sel_name.contains(&name_lower) || name_lower.contains(sel_name.split_whitespace().next().unwrap_or("")) {
                continue;
            }
        }

        let pattern = format!("%{}%", name);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM employees WHERE full_name LIKE ? LIMIT 5"
        )
        .bind(&pattern)
        .fetch_all(pool)
        .await?;

        for (id,) in rows {
            if !employee_ids.contains(&id) && Some(id.as_str()) != selected_id {
                employee_ids.push(id);
            }
        }
    }

    // Priority 5: Department-based search
    for dept in &mentions.departments {
        let pattern = format!("%{}%", dept);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM employees WHERE department LIKE ? AND status = 'active' LIMIT 10"
        )
        .bind(&pattern)
        .fetch_all(pool)
        .await?;

        for (id,) in rows {
            if !employee_ids.contains(&id) && Some(id.as_str()) != selected_id {
                employee_ids.push(id);
            }
        }
    }

    // Priority 6: Aggregate query fallback (random sample)
    if employee_ids.is_empty() && mentions.is_aggregate_query {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM employees WHERE status = 'active' ORDER BY RANDOM() LIMIT ?"
        )
        .bind(remaining_limit as i64)
        .fetch_all(pool)
        .await?;

        for (id,) in rows {
            if Some(id.as_str()) != selected_id {
                employee_ids.push(id);
            }
        }
    }

    // Limit results
    employee_ids.truncate(remaining_limit);

    let employees = get_employee_contexts(pool, &employee_ids).await?;
    Ok(finalize_results(employees))
}

/// Get full context for a single employee including performance and eNPS
pub async fn get_employee_context(
    pool: &DbPool,
    employee_id: &str,
) -> Result<EmployeeContext, ContextError> {
    // Get employee basic info
    let emp: EmployeeRow = sqlx::query_as(
        "SELECT id, email, full_name, department, job_title, hire_date, work_state, status, manager_id FROM employees WHERE id = ?"
    )
    .bind(employee_id)
    .fetch_one(pool)
    .await?;

    // Get manager name if exists
    let manager_name: Option<String> = if let Some(ref manager_id) = emp.manager_id {
        sqlx::query("SELECT full_name FROM employees WHERE id = ?")
            .bind(manager_id)
            .fetch_optional(pool)
            .await?
            .map(|row| row.get("full_name"))
    } else {
        None
    };

    // Get performance ratings with cycle names
    let ratings: Vec<RatingRow> = sqlx::query_as(
        r#"
        SELECT pr.overall_rating, rc.name as cycle_name, pr.rating_date
        FROM performance_ratings pr
        JOIN review_cycles rc ON pr.review_cycle_id = rc.id
        WHERE pr.employee_id = ?
        ORDER BY rc.start_date DESC
        "#
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;

    // #154: Narrative reviews — count + latest date only. The full text is
    // deliberately not loaded here; the employee-context section is token-budgeted
    // and this only needs to establish that a review history exists.
    let review_meta: Option<ReviewMetaRow> = sqlx::query_as(
        r#"
        SELECT COUNT(*) as review_count, MAX(review_date) as latest_review_date
        FROM performance_reviews
        WHERE employee_id = ?
        "#
    )
    .bind(employee_id)
    .fetch_optional(pool)
    .await?;
    let (review_count, latest_review_date) = review_meta
        .map(|r| (r.review_count.max(0) as usize, r.latest_review_date))
        .unwrap_or((0, None));

    // Get eNPS responses
    let enps_responses: Vec<EnpsRow> = sqlx::query_as(
        "SELECT score, survey_name, survey_date, feedback_text FROM enps_responses WHERE employee_id = ? ORDER BY survey_date DESC"
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;

    // Calculate rating trend
    let rating_trend = calculate_trend(&ratings.iter().map(|r| r.overall_rating).collect::<Vec<_>>());

    // Calculate eNPS trend
    let enps_trend = calculate_trend(
        &enps_responses.iter().map(|e| e.score as f64).collect::<Vec<_>>()
    );

    // Build rating info list
    let all_ratings: Vec<RatingInfo> = ratings
        .iter()
        .map(|r| RatingInfo {
            cycle_name: r.cycle_name.clone(),
            overall_rating: r.overall_rating,
            rating_date: r.rating_date.clone(),
        })
        .collect();

    // Build eNPS info list
    let all_enps: Vec<EnpsInfo> = enps_responses
        .iter()
        .map(|e| EnpsInfo {
            score: e.score,
            survey_name: e.survey_name.clone(),
            survey_date: e.survey_date.clone(),
            feedback: e.feedback_text.clone(),
        })
        .collect();

    // V2.2.1: Get extracted highlights and summary (graceful degradation)
    let raw_highlights = highlights::get_highlights_or_empty(pool, employee_id).await;
    let summary = highlights::get_summary_or_none(pool, employee_id).await;

    // Build cycle name lookup from review_cycles
    let cycle_names: std::collections::HashMap<String, String> = if !raw_highlights.is_empty() {
        let cycle_ids: Vec<String> = raw_highlights.iter().map(|h| h.review_cycle_id.clone()).collect();
        let placeholders = cycle_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("SELECT id, name FROM review_cycles WHERE id IN ({})", placeholders);

        let mut query_builder = sqlx::query(&query);
        for id in &cycle_ids {
            query_builder = query_builder.bind(id);
        }

        query_builder
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|row| (row.get::<String, _>("id"), row.get::<String, _>("name")))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Build CycleHighlight list from raw highlights
    let recent_highlights: Vec<CycleHighlight> = raw_highlights
        .into_iter()
        .take(3) // Limit to 3 most recent cycles for context
        .map(|h| CycleHighlight {
            cycle_name: cycle_names
                .get(&h.review_cycle_id)
                .cloned()
                .unwrap_or_else(|| "Review".to_string()),
            strengths: h.strengths,
            opportunities: h.opportunities,
            themes: h.themes,
            sentiment: h.overall_sentiment,
        })
        .collect();

    // Extract summary data
    let career_summary = summary.as_ref().and_then(|s| s.career_narrative.clone());
    let key_strengths = summary.as_ref().map(|s| s.key_strengths.clone()).unwrap_or_default();
    let development_areas = summary.as_ref().map(|s| s.development_areas.clone()).unwrap_or_default();

    Ok(EmployeeContext {
        id: emp.id,
        full_name: emp.full_name,
        email: emp.email,
        department: emp.department,
        job_title: emp.job_title,
        hire_date: emp.hire_date,
        work_state: emp.work_state,
        status: emp.status,
        manager_name,
        latest_rating: ratings.first().map(|r| r.overall_rating),
        latest_rating_cycle: ratings.first().map(|r| r.cycle_name.clone()),
        rating_trend,
        all_ratings,
        review_count,
        latest_review_date,
        latest_enps: enps_responses.first().map(|e| e.score),
        latest_enps_date: enps_responses.first().map(|e| e.survey_date.clone()),
        enps_trend,
        all_enps,
        // V2.2.1: Highlights data
        career_summary,
        key_strengths,
        development_areas,
        recent_highlights,
    })
}

/// Batch variant of `get_employee_context`. Issues 4 IN-clause queries (basic
/// info, manager names, ratings + cycles, eNPS) instead of N×4 sequential
/// per-employee queries, then assembles the EmployeeContext list in input-ID
/// order.
///
/// IDs not found in the employees table are silently skipped — matches the
/// per-row `if let Ok(emp) = ...` pattern at every caller in this module.
///
/// Highlights/summary lookups remain sequential per-employee; their batching
/// would require extending `highlights::*` and is out of scope for #32.
pub async fn get_employee_contexts(
    pool: &DbPool,
    employee_ids: &[String],
) -> Result<Vec<EmployeeContext>, ContextError> {
    if employee_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = employee_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // 1) Batch: employee basic info
    let basic_query = format!(
        "SELECT id, email, full_name, department, job_title, hire_date, work_state, status, manager_id \
         FROM employees WHERE id IN ({})",
        placeholders
    );
    let mut q = sqlx::query_as::<_, EmployeeRow>(&basic_query);
    for id in employee_ids {
        q = q.bind(id);
    }
    let basic_rows: Vec<EmployeeRow> = q.fetch_all(pool).await?;
    let basic_by_id: std::collections::HashMap<String, EmployeeRow> =
        basic_rows.into_iter().map(|r| (r.id.clone(), r)).collect();

    // 2) Batch: manager names (deduplicate manager_ids; one row per distinct manager)
    let manager_ids: Vec<String> = {
        let mut ids: Vec<String> = basic_by_id
            .values()
            .filter_map(|e| e.manager_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    };
    let manager_names_by_id: std::collections::HashMap<String, String> = if manager_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let mgr_placeholders = manager_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let mgr_query = format!(
            "SELECT id, full_name FROM employees WHERE id IN ({})",
            mgr_placeholders
        );
        let mut q = sqlx::query(&mgr_query);
        for id in &manager_ids {
            q = q.bind(id);
        }
        q.fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| (row.get::<String, _>("id"), row.get::<String, _>("full_name")))
            .collect()
    };

    // 3) Batch: ratings JOIN cycles, ORDER BY rc.start_date DESC.
    // Local row type because grouping needs employee_id (per-employee RatingRow doesn't carry it).
    #[derive(FromRow)]
    struct RatingRowWithEmpId {
        employee_id: String,
        overall_rating: f64,
        cycle_name: String,
        rating_date: Option<String>,
    }
    let ratings_query = format!(
        r#"SELECT pr.employee_id, pr.overall_rating, rc.name as cycle_name, pr.rating_date
        FROM performance_ratings pr
        JOIN review_cycles rc ON pr.review_cycle_id = rc.id
        WHERE pr.employee_id IN ({})
        ORDER BY rc.start_date DESC"#,
        placeholders
    );
    let mut q = sqlx::query_as::<_, RatingRowWithEmpId>(&ratings_query);
    for id in employee_ids {
        q = q.bind(id);
    }
    let rating_rows: Vec<RatingRowWithEmpId> = q.fetch_all(pool).await?;
    let mut ratings_by_emp: std::collections::HashMap<String, Vec<RatingRow>> =
        std::collections::HashMap::new();
    for r in rating_rows {
        ratings_by_emp
            .entry(r.employee_id)
            .or_default()
            .push(RatingRow {
                overall_rating: r.overall_rating,
                cycle_name: r.cycle_name,
                rating_date: r.rating_date,
            });
    }

    // 4) Batch: eNPS responses, ORDER BY survey_date DESC.
    #[derive(FromRow)]
    struct EnpsRowWithEmpId {
        employee_id: String,
        score: i32,
        survey_name: Option<String>,
        survey_date: String,
        feedback_text: Option<String>,
    }
    let enps_query = format!(
        "SELECT employee_id, score, survey_name, survey_date, feedback_text \
         FROM enps_responses WHERE employee_id IN ({}) \
         ORDER BY survey_date DESC",
        placeholders
    );
    let mut q = sqlx::query_as::<_, EnpsRowWithEmpId>(&enps_query);
    for id in employee_ids {
        q = q.bind(id);
    }
    let enps_rows: Vec<EnpsRowWithEmpId> = q.fetch_all(pool).await?;
    let mut enps_by_emp: std::collections::HashMap<String, Vec<EnpsRow>> =
        std::collections::HashMap::new();
    for e in enps_rows {
        enps_by_emp.entry(e.employee_id).or_default().push(EnpsRow {
            score: e.score,
            survey_name: e.survey_name,
            survey_date: e.survey_date,
            feedback_text: e.feedback_text,
        });
    }

    // 5) Batch: #154 narrative-review count + latest date, grouped per employee.
    #[derive(FromRow)]
    struct ReviewMetaRowWithEmpId {
        employee_id: String,
        review_count: i64,
        latest_review_date: Option<String>,
    }
    let reviews_query = format!(
        r#"SELECT employee_id, COUNT(*) as review_count, MAX(review_date) as latest_review_date
        FROM performance_reviews
        WHERE employee_id IN ({})
        GROUP BY employee_id"#,
        placeholders
    );
    let mut q = sqlx::query_as::<_, ReviewMetaRowWithEmpId>(&reviews_query);
    for id in employee_ids {
        q = q.bind(id);
    }
    let review_rows: Vec<ReviewMetaRowWithEmpId> = q.fetch_all(pool).await?;
    let mut reviews_by_emp: std::collections::HashMap<String, (usize, Option<String>)> =
        std::collections::HashMap::new();
    for r in review_rows {
        reviews_by_emp.insert(
            r.employee_id,
            (r.review_count.max(0) as usize, r.latest_review_date),
        );
    }

    // Assemble in input-ID order, dropping IDs not found in basic_by_id.
    let mut out: Vec<EmployeeContext> = Vec::with_capacity(employee_ids.len());
    for id in employee_ids {
        let Some(emp) = basic_by_id.get(id) else {
            continue;
        };

        let manager_name = emp
            .manager_id
            .as_ref()
            .and_then(|mid| manager_names_by_id.get(mid))
            .cloned();

        let ratings = ratings_by_emp.get(id).cloned().unwrap_or_default();
        let enps_responses = enps_by_emp.get(id).cloned().unwrap_or_default();
        let (review_count, latest_review_date) =
            reviews_by_emp.get(id).cloned().unwrap_or((0, None));

        let rating_trend = calculate_trend(
            &ratings.iter().map(|r| r.overall_rating).collect::<Vec<_>>(),
        );
        let enps_trend = calculate_trend(
            &enps_responses
                .iter()
                .map(|e| e.score as f64)
                .collect::<Vec<_>>(),
        );

        let all_ratings: Vec<RatingInfo> = ratings
            .iter()
            .map(|r| RatingInfo {
                cycle_name: r.cycle_name.clone(),
                overall_rating: r.overall_rating,
                rating_date: r.rating_date.clone(),
            })
            .collect();
        let all_enps: Vec<EnpsInfo> = enps_responses
            .iter()
            .map(|e| EnpsInfo {
                score: e.score,
                survey_name: e.survey_name.clone(),
                survey_date: e.survey_date.clone(),
                feedback: e.feedback_text.clone(),
            })
            .collect();

        // Highlights/summary remain per-employee — out of scope for this batch.
        let raw_highlights = highlights::get_highlights_or_empty(pool, id).await;
        let summary = highlights::get_summary_or_none(pool, id).await;

        let cycle_names: std::collections::HashMap<String, String> = if !raw_highlights.is_empty() {
            let cycle_ids: Vec<String> = raw_highlights
                .iter()
                .map(|h| h.review_cycle_id.clone())
                .collect();
            let cyc_placeholders = cycle_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let cyc_query = format!(
                "SELECT id, name FROM review_cycles WHERE id IN ({})",
                cyc_placeholders
            );
            let mut qb = sqlx::query(&cyc_query);
            for cid in &cycle_ids {
                qb = qb.bind(cid);
            }
            qb.fetch_all(pool)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|row| (row.get::<String, _>("id"), row.get::<String, _>("name")))
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        let recent_highlights: Vec<CycleHighlight> = raw_highlights
            .into_iter()
            .take(3)
            .map(|h| CycleHighlight {
                cycle_name: cycle_names
                    .get(&h.review_cycle_id)
                    .cloned()
                    .unwrap_or_else(|| "Review".to_string()),
                strengths: h.strengths,
                opportunities: h.opportunities,
                themes: h.themes,
                sentiment: h.overall_sentiment,
            })
            .collect();

        let career_summary = summary.as_ref().and_then(|s| s.career_narrative.clone());
        let key_strengths = summary
            .as_ref()
            .map(|s| s.key_strengths.clone())
            .unwrap_or_default();
        let development_areas = summary
            .as_ref()
            .map(|s| s.development_areas.clone())
            .unwrap_or_default();

        out.push(EmployeeContext {
            id: emp.id.clone(),
            full_name: emp.full_name.clone(),
            email: emp.email.clone(),
            department: emp.department.clone(),
            job_title: emp.job_title.clone(),
            hire_date: emp.hire_date.clone(),
            work_state: emp.work_state.clone(),
            status: emp.status.clone(),
            manager_name,
            latest_rating: ratings.first().map(|r| r.overall_rating),
            latest_rating_cycle: ratings.first().map(|r| r.cycle_name.clone()),
            rating_trend,
            review_count,
            latest_review_date,
            all_ratings,
            latest_enps: enps_responses.first().map(|e| e.score),
            latest_enps_date: enps_responses.first().map(|e| e.survey_date.clone()),
            enps_trend,
            all_enps,
            career_summary,
            key_strengths,
            development_areas,
            recent_highlights,
        });
    }

    Ok(out)
}

/// Calculate trend from a series of values (most recent first)
fn calculate_trend(values: &[f64]) -> Option<String> {
    if values.len() < 2 {
        return None;
    }

    let recent = values[0];
    let older = values[values.len() - 1];
    let diff = recent - older;

    // Use a small threshold to avoid noise
    if diff > 0.3 {
        Some("improving".to_string())
    } else if diff < -0.3 {
        Some("declining".to_string())
    } else {
        Some("stable".to_string())
    }
}

/// Get company context
pub async fn get_company_context(pool: &DbPool) -> Result<Option<CompanyContext>, ContextError> {
    let company: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT name, state, industry FROM company WHERE id = 'default'"
    )
    .fetch_optional(pool)
    .await?;

    let Some((name, state, industry)) = company else {
        return Ok(None);
    };

    // Get employee and department counts
    let employee_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM employees WHERE status = 'active'")
        .fetch_one(pool)
        .await?
        .get("count");

    let department_count: i64 = sqlx::query(
        "SELECT COUNT(DISTINCT department) as count FROM employees WHERE department IS NOT NULL AND status = 'active'"
    )
    .fetch_one(pool)
    .await?
    .get("count");

    Ok(Some(CompanyContext {
        name,
        state,
        industry,
        employee_count,
        department_count,
    }))
}

// ============================================================================
// Specialized Retrieval Functions
// ============================================================================

/// Find employees with longest tenure (sorted by hire_date ASC)
pub async fn find_longest_tenure(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'active' AND hire_date IS NOT NULL ORDER BY hire_date ASC LIMIT ?"
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// Find newest employees (sorted by hire_date DESC)
pub async fn find_newest_employees(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'active' AND hire_date IS NOT NULL ORDER BY hire_date DESC LIMIT ?"
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// Find employees hired within the last N days (for new hires digest)
pub async fn find_recent_hires(
    pool: &DbPool,
    days: i64,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'active' AND hire_date IS NOT NULL AND hire_date >= date('now', ? || ' days') ORDER BY hire_date DESC LIMIT ?"
    )
    .bind(-days)  // Negative to go back in time
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// Find underperforming employees (rating < 2.5 in recent cycles)
pub async fn find_underperformers(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT e.id
        FROM employees e
        JOIN performance_ratings pr ON e.id = pr.employee_id
        WHERE e.status = 'active' AND pr.overall_rating < 2.5
        GROUP BY e.id
        ORDER BY COUNT(*) DESC, MIN(pr.overall_rating) ASC
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// Find top performers (rating >= 4.5 in recent cycles)
pub async fn find_top_performers(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT e.id
        FROM employees e
        JOIN performance_ratings pr ON e.id = pr.employee_id
        WHERE e.status = 'active' AND pr.overall_rating >= 4.5
        GROUP BY e.id
        ORDER BY COUNT(*) DESC, MAX(pr.overall_rating) DESC
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// V2.2.2b: Find employees by theme from extracted review highlights
/// Searches themes, strengths, or opportunities based on ThemeTarget
pub async fn find_employees_by_theme(
    pool: &DbPool,
    themes: &[String],
    department: Option<&str>,
    _target: ThemeTarget,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    if themes.is_empty() {
        return Ok(vec![]);
    }

    // Theme tags are stored in the `themes` column as JSON arrays like '["leadership", "mentoring"]'.
    // Build one `rh.themes LIKE ?` per theme and bind each pattern below — never interpolate
    // user-derived strings into SQL (theme values originate from AI extraction + user queries).
    let placeholders: Vec<&str> = themes.iter().map(|_| "rh.themes LIKE ?").collect();
    let theme_where = placeholders.join(" OR ");

    let dept_filter = if department.is_some() {
        "AND e.department = ?"
    } else {
        ""
    };

    let query = format!(
        r#"
        SELECT e.id, COUNT(*) as match_count
        FROM employees e
        JOIN review_highlights rh ON e.id = rh.employee_id
        WHERE e.status = 'active'
          AND ({theme_where})
          {dept_filter}
        GROUP BY e.id
        ORDER BY match_count DESC
        LIMIT ?
        "#,
    );

    let mut q = sqlx::query_as::<_, (String, i64)>(&query);
    for theme in themes {
        // JSON-array substring match — the leading quote is intentional so "leadership"
        // matches but "nonleadership" does not.
        q = q.bind(format!("%\"{}%", theme));
    }
    if let Some(dept) = department {
        q = q.bind(dept);
    }
    q = q.bind(limit as i64);

    let rows: Vec<(String, i64)> = q.fetch_all(pool).await?;

    let ids: Vec<String> = rows.into_iter().map(|(id, _)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// Find employees with upcoming work anniversaries (within next 30 days)
pub async fn find_upcoming_anniversaries(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT id FROM employees
        WHERE status = 'active'
        AND hire_date IS NOT NULL
        AND (
            (strftime('%m-%d', hire_date) >= strftime('%m-%d', 'now')
             AND strftime('%m-%d', hire_date) <= strftime('%m-%d', 'now', '+30 days'))
            OR
            (strftime('%m-%d', 'now', '+30 days') < strftime('%m-%d', 'now')
             AND (strftime('%m-%d', hire_date) >= strftime('%m-%d', 'now')
                  OR strftime('%m-%d', hire_date) <= strftime('%m-%d', 'now', '+30 days')))
        )
        ORDER BY strftime('%m-%d', hire_date)
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// Find recently terminated employees for attrition queries
/// Returns full EmployeeContext with termination details
pub async fn find_recent_terminations(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'terminated' ORDER BY termination_date DESC LIMIT ?"
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    get_employee_contexts(pool, &ids).await
}

/// Build a lightweight employee list for roster queries
/// Returns EmployeeSummary (name, dept, title, status, hire date) without full perf data
pub async fn build_employee_list(
    pool: &DbPool,
    mentions: &QueryMentions,
    limit: usize,
) -> Result<Vec<EmployeeSummary>, ContextError> {
    // Build query based on department filter
    let rows = if !mentions.departments.is_empty() {
        let dept = &mentions.departments[0];
        let pattern = format!("%{}%", dept);
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
            r#"
            SELECT id, full_name, department, job_title, status, hire_date
            FROM employees
            WHERE department LIKE ? AND status = 'active'
            ORDER BY full_name
            LIMIT ?
            "#
        )
        .bind(&pattern)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?
    } else {
        // No department filter - return active employees
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
            r#"
            SELECT id, full_name, department, job_title, status, hire_date
            FROM employees
            WHERE status = 'active'
            ORDER BY full_name
            LIMIT ?
            "#
        )
        .bind(limit as i64)
        .fetch_all(pool)
        .await?
    };

    let summaries: Vec<EmployeeSummary> = rows
        .into_iter()
        .map(|(id, full_name, department, job_title, status, hire_date)| EmployeeSummary {
            id,
            full_name,
            department,
            job_title,
            status,
            hire_date,
        })
        .collect();

    Ok(summaries)
}

/// Build a list of terminated employees for attrition list queries
pub async fn build_termination_list(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeSummary>, ContextError> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
        r#"
        SELECT id, full_name, department, job_title, status, hire_date
        FROM employees
        WHERE status = 'terminated'
        ORDER BY termination_date DESC
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let summaries: Vec<EmployeeSummary> = rows
        .into_iter()
        .map(|(id, full_name, department, job_title, status, hire_date)| EmployeeSummary {
            id,
            full_name,
            department,
            job_title,
            status,
            hire_date,
        })
        .collect();

    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_trend() {
        // Improving (most recent is higher)
        assert_eq!(calculate_trend(&[4.0, 3.5, 3.0]), Some("improving".to_string()));
        // Declining (most recent is lower)
        assert_eq!(calculate_trend(&[3.0, 3.5, 4.0]), Some("declining".to_string()));
        // Stable
        assert_eq!(calculate_trend(&[3.5, 3.4, 3.5]), Some("stable".to_string()));
        // Not enough data
        assert_eq!(calculate_trend(&[3.5]), None);
    }

    /// Regression test for SQL injection via theme strings.
    #[tokio::test]
    async fn find_employees_by_theme_rejects_sql_injection() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::time::Duration;

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

        sqlx::query(
            "INSERT INTO employees (id, email, full_name) VALUES ('canary', 'c@x.com', 'Canary')",
        )
        .execute(&pool)
        .await
        .expect("insert canary employee");

        let evil_themes = vec![
            "'; DROP TABLE employees; --".to_string(),
            "\" OR 1=1 --".to_string(),
            "' UNION SELECT id,0 FROM employees --".to_string(),
        ];

        let result = find_employees_by_theme(&pool, &evil_themes, None, ThemeTarget::Any, 10).await;
        assert!(result.is_ok(), "injection attempt produced an error: {:?}", result);
        assert!(result.unwrap().is_empty(), "injection attempt returned rows");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees WHERE id = 'canary'")
            .fetch_one(&pool)
            .await
            .expect("employees table must still exist after injection attempt");
        assert_eq!(count, 1, "canary employee vanished — SQL injection executed");
    }

    /// Regression test for #32: the batch `get_employee_contexts` must
    /// preserve input ID order, silently skip IDs absent from the employees
    /// table, resolve manager_name when the manager is inside *and* outside
    /// the batch, default missing rating/eNPS data to None, and return an
    /// empty Vec for empty input.
    #[tokio::test]
    async fn get_employee_contexts_preserves_order_and_resolves_managers() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::time::Duration;

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

        sqlx::query(
            r#"
            INSERT INTO employees (id, email, full_name, manager_id) VALUES
                ('emp_a', 'a@x.com', 'Alice', 'emp_c'),
                ('emp_b', 'b@x.com', 'Bob',   'ghost'),
                ('emp_c', 'c@x.com', 'Carol', NULL),
                ('ghost', 'g@x.com', 'Ghost', NULL)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert employees");

        sqlx::query(
            "INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date) \
             VALUES ('cyc1', '2024 Annual', 'annual', '2024-01-01', '2024-12-31')",
        )
        .execute(&pool)
        .await
        .expect("insert cycle");

        sqlx::query(
            "INSERT INTO performance_ratings (id, employee_id, review_cycle_id, overall_rating) \
             VALUES ('r1', 'emp_a', 'cyc1', 4.5)",
        )
        .execute(&pool)
        .await
        .expect("insert rating");

        sqlx::query(
            "INSERT INTO enps_responses (id, employee_id, score, survey_date) \
             VALUES ('e1', 'emp_a', 9, '2024-12-01')",
        )
        .execute(&pool)
        .await
        .expect("insert enps");

        let input_ids = vec![
            "emp_b".to_string(),
            "emp_a".to_string(),
            "missing".to_string(),
            "emp_c".to_string(),
        ];
        let result = get_employee_contexts(&pool, &input_ids)
            .await
            .expect("batch query");

        assert_eq!(result.len(), 3, "missing id should be silently skipped");
        assert_eq!(result[0].id, "emp_b", "input order must be preserved");
        assert_eq!(result[1].id, "emp_a");
        assert_eq!(result[2].id, "emp_c");

        assert_eq!(result[1].latest_rating, Some(4.5));
        assert_eq!(result[1].latest_enps, Some(9));
        assert_eq!(
            result[1].manager_name.as_deref(),
            Some("Carol"),
            "manager_name must resolve when manager is within the batch"
        );

        assert!(result[0].latest_rating.is_none());
        assert!(result[0].latest_enps.is_none());
        assert_eq!(
            result[0].manager_name.as_deref(),
            Some("Ghost"),
            "manager_name must resolve when manager is outside the batch but in employees"
        );

        assert_eq!(result[2].manager_name, None);

        let empty: Vec<String> = Vec::new();
        let empty_result = get_employee_contexts(&pool, &empty)
            .await
            .expect("empty batch");
        assert!(empty_result.is_empty(), "empty input must return empty Vec");
    }

    /// Regression test for #154: narrative reviews live in `performance_reviews`,
    /// numeric scores in `performance_ratings`. The context module read only the
    /// latter, so an employee with reviews-but-no-ratings surfaced zero
    /// performance context and chat asserted "no review history" while Prep Brief
    /// cited the same reviews. Exercises BOTH the single and batch query paths
    /// against the real migrated schema.
    #[tokio::test]
    async fn reviews_without_ratings_are_counted_in_context() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::time::Duration;

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

        sqlx::query(
            r#"
            INSERT INTO employees (id, email, full_name) VALUES
                ('maya',  'maya@x.com',  'Maya Patel'),
                ('noone', 'noone@x.com', 'No Reviews')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert employees");

        // UNIQUE(employee_id, review_cycle_id) — two reviews need two cycles.
        sqlx::query(
            r#"
            INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date) VALUES
                ('c1', '2025 Annual', 'annual', '2025-01-01', '2025-12-31'),
                ('c2', '2026 H1',     'annual', '2026-01-01', '2026-06-30')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert cycles");

        // Reviews only — deliberately NO performance_ratings rows for Maya.
        sqlx::query(
            r#"
            INSERT INTO performance_reviews
                (id, employee_id, review_cycle_id, manager_comments, review_date) VALUES
                ('r1', 'maya', 'c1', 'Strong systems thinker.', '2025-12-01'),
                ('r2', 'maya', 'c2', 'Led the redesign.',       '2026-03-01')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert reviews");

        // Single-employee path.
        let ctx = get_employee_context(&pool, "maya")
            .await
            .expect("single-employee context");
        assert!(
            ctx.all_ratings.is_empty(),
            "fixture must have no ratings — that is the whole point of #154"
        );
        assert_eq!(ctx.review_count, 2, "both narrative reviews must be counted");
        assert_eq!(
            ctx.latest_review_date.as_deref(),
            Some("2026-03-01"),
            "latest_review_date must be MAX(review_date)"
        );

        // Batch path must agree with the single path.
        let batch = get_employee_contexts(&pool, &["maya".to_string(), "noone".to_string()])
            .await
            .expect("batch context");
        assert_eq!(batch[0].review_count, 2);
        assert_eq!(batch[0].latest_review_date.as_deref(), Some("2026-03-01"));

        // An employee with no reviews must report zero, not a phantom count.
        assert_eq!(batch[1].review_count, 0);
        assert_eq!(batch[1].latest_review_date, None);

        let solo = get_employee_context(&pool, "noone")
            .await
            .expect("single-employee context for reviewless employee");
        assert_eq!(solo.review_count, 0);
        assert_eq!(solo.latest_review_date, None);
    }
}
