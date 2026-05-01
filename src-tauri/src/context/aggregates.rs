//! Organization-wide aggregate statistics: headcount, performance distribution,
//! eNPS roll-up, and YTD attrition. Owns the `OrgAggregates` shape used by the
//! verification + system-prompt builders.
//!
//! Also hosts `rating_label` (consumed by both this module's
//! `format_org_aggregates` and `prompt::format_single_employee_with_budget`).

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::DbPool;

use super::ContextError;

// ============================================================================
// Aggregate Types
// ============================================================================

/// Organization-wide aggregate statistics
/// Computed from full database for every query (~2K chars when formatted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgAggregates {
    // Headcount
    pub total_employees: i64,
    pub active_count: i64,
    pub terminated_count: i64,
    pub on_leave_count: i64,

    // By department (sorted by count descending)
    pub by_department: Vec<DepartmentCount>,

    // Performance (active employees only, most recent rating per employee)
    pub avg_rating: Option<f64>,
    pub rating_distribution: RatingDistribution,
    pub employees_with_no_rating: i64,

    // Engagement (reuses existing EnpsAggregate)
    pub enps: EnpsAggregate,

    // Attrition (YTD)
    pub attrition: AttritionStats,
}

/// Department headcount with percentage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentCount {
    pub name: String,
    pub count: i64,
    pub percentage: f64,
}

/// Performance rating distribution buckets
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RatingDistribution {
    /// Rating >= 4.5
    pub exceptional: i64,
    /// Rating 3.5 - 4.49
    pub exceeds: i64,
    /// Rating 2.5 - 3.49
    pub meets: i64,
    /// Rating < 2.5
    pub needs_improvement: i64,
}

/// Year-to-date attrition statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttritionStats {
    pub terminations_ytd: i64,
    pub voluntary: i64,
    pub involuntary: i64,
    pub avg_tenure_months: Option<f64>,
    pub turnover_rate_annualized: Option<f64>,
}

/// Aggregate eNPS calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnpsAggregate {
    /// eNPS score (-100 to +100)
    pub score: i32,
    /// Number of promoters (score >= 9)
    pub promoters: i64,
    /// Number of passives (score 7-8)
    pub passives: i64,
    /// Number of detractors (score <= 6)
    pub detractors: i64,
    /// Total survey responses
    pub total_responses: i64,
    /// Response rate vs active employees
    pub response_rate: f64,
}

// ============================================================================
// Aggregate Calculation
// ============================================================================

/// Build organization-wide aggregates from the full database
/// These are computed for every query to give Claude accurate org-level context
pub async fn build_org_aggregates(pool: &DbPool) -> Result<OrgAggregates, ContextError> {
    // 1. Headcount by status
    let headcount = fetch_headcount_by_status(pool).await?;

    // 2. Headcount by department
    let by_department = fetch_headcount_by_department(pool, headcount.active_count).await?;

    // 3. Performance distribution (most recent rating per active employee)
    let (avg_rating, rating_distribution, employees_with_no_rating) =
        fetch_performance_distribution(pool, headcount.active_count).await?;

    // 4. eNPS (reuse existing function)
    let enps = calculate_aggregate_enps(pool).await?;

    // 5. Attrition YTD
    let attrition = fetch_attrition_stats(pool, headcount.active_count).await?;

    Ok(OrgAggregates {
        total_employees: headcount.total,
        active_count: headcount.active_count,
        terminated_count: headcount.terminated_count,
        on_leave_count: headcount.on_leave_count,
        by_department,
        avg_rating,
        rating_distribution,
        employees_with_no_rating,
        enps,
        attrition,
    })
}

/// Internal struct for headcount query result
struct HeadcountResult {
    total: i64,
    active_count: i64,
    terminated_count: i64,
    on_leave_count: i64,
}

/// Fetch headcount by status
async fn fetch_headcount_by_status(pool: &DbPool) -> Result<HeadcountResult, ContextError> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) as total,
            SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END) as active,
            SUM(CASE WHEN status = 'terminated' THEN 1 ELSE 0 END) as terminated,
            SUM(CASE WHEN status = 'leave' THEN 1 ELSE 0 END) as on_leave
        FROM employees
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(HeadcountResult {
        total: row.get::<i64, _>("total"),
        active_count: row.get::<i64, _>("active"),
        terminated_count: row.get::<i64, _>("terminated"),
        on_leave_count: row.get::<i64, _>("on_leave"),
    })
}

/// Fetch headcount by department (active employees only)
async fn fetch_headcount_by_department(
    pool: &DbPool,
    total_active: i64,
) -> Result<Vec<DepartmentCount>, ContextError> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(department, 'Unassigned') as department,
            COUNT(*) as count
        FROM employees
        WHERE status = 'active'
        GROUP BY department
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let departments: Vec<DepartmentCount> = rows
        .iter()
        .map(|row| {
            let name: String = row.get("department");
            let count: i64 = row.get("count");
            let percentage = if total_active > 0 {
                (count as f64 / total_active as f64) * 100.0
            } else {
                0.0
            };
            DepartmentCount {
                name,
                count,
                percentage,
            }
        })
        .collect();

    Ok(departments)
}

/// Fetch performance rating distribution (most recent rating per active employee)
async fn fetch_performance_distribution(
    pool: &DbPool,
    total_active: i64,
) -> Result<(Option<f64>, RatingDistribution, i64), ContextError> {
    // Get most recent rating per active employee
    let row = sqlx::query(
        r#"
        WITH latest_ratings AS (
            SELECT
                pr.employee_id,
                pr.overall_rating,
                ROW_NUMBER() OVER (PARTITION BY pr.employee_id ORDER BY rc.end_date DESC) as rn
            FROM performance_ratings pr
            JOIN review_cycles rc ON pr.review_cycle_id = rc.id
            JOIN employees e ON pr.employee_id = e.id
            WHERE e.status = 'active'
        )
        SELECT
            AVG(overall_rating) as avg_rating,
            SUM(CASE WHEN overall_rating >= 4.5 THEN 1 ELSE 0 END) as exceptional,
            SUM(CASE WHEN overall_rating >= 3.5 AND overall_rating < 4.5 THEN 1 ELSE 0 END) as exceeds,
            SUM(CASE WHEN overall_rating >= 2.5 AND overall_rating < 3.5 THEN 1 ELSE 0 END) as meets,
            SUM(CASE WHEN overall_rating < 2.5 THEN 1 ELSE 0 END) as needs_improvement,
            COUNT(*) as rated_count
        FROM latest_ratings
        WHERE rn = 1
        "#,
    )
    .fetch_one(pool)
    .await?;

    let avg_rating: Option<f64> = row.get("avg_rating");
    let rated_count: i64 = row.get("rated_count");
    let employees_with_no_rating = total_active - rated_count;

    let distribution = RatingDistribution {
        exceptional: row.get("exceptional"),
        exceeds: row.get("exceeds"),
        meets: row.get("meets"),
        needs_improvement: row.get("needs_improvement"),
    };

    Ok((avg_rating, distribution, employees_with_no_rating))
}

/// Fetch attrition stats for YTD
async fn fetch_attrition_stats(
    pool: &DbPool,
    current_active: i64,
) -> Result<AttritionStats, ContextError> {
    // Get YTD termination stats
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) as terminations,
            SUM(CASE WHEN termination_reason = 'voluntary' THEN 1 ELSE 0 END) as voluntary,
            SUM(CASE WHEN termination_reason = 'involuntary' THEN 1 ELSE 0 END) as involuntary,
            AVG(
                CAST((julianday(termination_date) - julianday(hire_date)) / 30.0 AS REAL)
            ) as avg_tenure_months
        FROM employees
        WHERE status = 'terminated'
          AND termination_date >= date('now', 'start of year')
        "#,
    )
    .fetch_one(pool)
    .await?;

    let terminations_ytd: i64 = row.get("terminations");
    let voluntary: i64 = row.get("voluntary");
    let involuntary: i64 = row.get("involuntary");
    let avg_tenure_months: Option<f64> = row.get("avg_tenure_months");

    // Calculate annualized turnover rate
    // Formula: (terminations / avg headcount) * (12 / months elapsed) * 100
    let turnover_rate_annualized = calculate_turnover_rate(pool, terminations_ytd, current_active).await?;

    Ok(AttritionStats {
        terminations_ytd,
        voluntary,
        involuntary,
        avg_tenure_months,
        turnover_rate_annualized,
    })
}

/// Calculate annualized turnover rate
async fn calculate_turnover_rate(
    pool: &DbPool,
    terminations_ytd: i64,
    current_active: i64,
) -> Result<Option<f64>, ContextError> {
    if terminations_ytd == 0 {
        return Ok(Some(0.0));
    }

    // Get months elapsed this year
    let row = sqlx::query(
        r#"
        SELECT
            (julianday('now') - julianday(date('now', 'start of year'))) / 30.0 as months_elapsed
        "#,
    )
    .fetch_one(pool)
    .await?;

    let months_elapsed: f64 = row.get("months_elapsed");

    if months_elapsed <= 0.0 {
        return Ok(None);
    }

    // Approximate average headcount = current active + half of terminations
    let avg_headcount = current_active as f64 + (terminations_ytd as f64 / 2.0);

    if avg_headcount <= 0.0 {
        return Ok(None);
    }

    // Annualized rate = (terminations / avg headcount) * (12 / months elapsed) * 100
    let rate = (terminations_ytd as f64 / avg_headcount) * (12.0 / months_elapsed) * 100.0;

    Ok(Some(rate))
}

/// Calculate aggregate eNPS score for the organization
pub async fn calculate_aggregate_enps(pool: &DbPool) -> Result<EnpsAggregate, ContextError> {
    // Get the most recent survey response per employee to avoid double-counting
    let stats: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        WITH latest_responses AS (
            SELECT employee_id, score, survey_date,
                   ROW_NUMBER() OVER (PARTITION BY employee_id ORDER BY survey_date DESC) as rn
            FROM enps_responses
        )
        SELECT
            COUNT(*) as total,
            SUM(CASE WHEN score >= 9 THEN 1 ELSE 0 END) as promoters,
            SUM(CASE WHEN score >= 7 AND score <= 8 THEN 1 ELSE 0 END) as passives,
            SUM(CASE WHEN score <= 6 THEN 1 ELSE 0 END) as detractors
        FROM latest_responses
        WHERE rn = 1
        "#
    )
    .fetch_one(pool)
    .await?;

    let (total, promoters, passives, detractors) = stats;

    // Get active employee count for response rate
    let active_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM employees WHERE status = 'active'")
        .fetch_one(pool)
        .await?
        .get("count");

    let score = if total > 0 {
        ((promoters - detractors) * 100 / total) as i32
    } else {
        0
    };

    let response_rate = if active_count > 0 {
        (total as f64 / active_count as f64) * 100.0
    } else {
        0.0
    };

    Ok(EnpsAggregate {
        score,
        promoters,
        passives,
        detractors,
        total_responses: total,
        response_rate,
    })
}

// ============================================================================
// Aggregate Formatting
// ============================================================================

/// Format aggregate eNPS for inclusion in context
pub fn format_aggregate_enps(enps: &EnpsAggregate) -> String {
    format!(
        "Company eNPS: {} (Promoters: {}, Passives: {}, Detractors: {}) — {} responses ({:.0}% response rate)",
        enps.score, enps.promoters, enps.passives, enps.detractors,
        enps.total_responses, enps.response_rate
    )
}

/// Format organization aggregates for inclusion in system prompt
/// Produces a compact (~1.5-2K chars) summary of org-wide stats
pub fn format_org_aggregates(agg: &OrgAggregates, company_name: Option<&str>) -> String {
    let mut lines = Vec::new();

    // Header
    lines.push("ORGANIZATION DATA:".to_string());
    lines.push(String::new());

    // Workforce summary
    if let Some(name) = company_name {
        lines.push(format!("COMPANY: {}", name));
    }
    lines.push(format!(
        "WORKFORCE: {} employees",
        agg.total_employees
    ));
    lines.push(format!(
        "• Active: {} | Terminated: {} | On Leave: {}",
        agg.active_count, agg.terminated_count, agg.on_leave_count
    ));
    lines.push(String::new());

    // Departments (compact format for space efficiency)
    if !agg.by_department.is_empty() {
        lines.push("DEPARTMENTS:".to_string());
        let dept_strs: Vec<String> = agg
            .by_department
            .iter()
            .take(8) // Limit to 8 departments to save space
            .map(|d| format!("{}: {} ({:.0}%)", d.name, d.count, d.percentage))
            .collect();
        // Group 3 departments per line for compactness
        for chunk in dept_strs.chunks(3) {
            lines.push(format!("• {}", chunk.join(" • ")));
        }
        lines.push(String::new());
    }

    // Performance
    lines.push(format!(
        "PERFORMANCE ({} active employees):",
        agg.active_count
    ));
    if let Some(avg) = agg.avg_rating {
        let label = rating_label(avg);
        lines.push(format!("• Avg rating: {:.1} ({})", avg, label));
    } else {
        lines.push("• No performance data available".to_string());
    }
    let dist = &agg.rating_distribution;
    if dist.exceptional > 0 || dist.exceeds > 0 || dist.meets > 0 || dist.needs_improvement > 0 {
        lines.push(format!(
            "• Distribution: Exceptional: {} | Exceeds: {} | Meets: {} | Needs Improvement: {}",
            dist.exceptional, dist.exceeds, dist.meets, dist.needs_improvement
        ));
    }
    if agg.employees_with_no_rating > 0 {
        lines.push(format!(
            "• Employees with no rating: {}",
            agg.employees_with_no_rating
        ));
    }
    lines.push(String::new());

    // Engagement (eNPS)
    lines.push("ENGAGEMENT:".to_string());
    let sign = if agg.enps.score >= 0 { "+" } else { "" };
    lines.push(format!(
        "• eNPS: {}{} (Promoters: {}, Passives: {}, Detractors: {})",
        sign, agg.enps.score, agg.enps.promoters, agg.enps.passives, agg.enps.detractors
    ));
    lines.push(format!(
        "• Response rate: {:.0}% ({} of {} active)",
        agg.enps.response_rate, agg.enps.total_responses, agg.active_count
    ));
    lines.push(String::new());

    // Attrition
    lines.push("ATTRITION (YTD):".to_string());
    if agg.attrition.terminations_ytd > 0 {
        lines.push(format!(
            "• Terminations: {} (Voluntary: {}, Involuntary: {})",
            agg.attrition.terminations_ytd,
            agg.attrition.voluntary,
            agg.attrition.involuntary
        ));
        if let Some(tenure) = agg.attrition.avg_tenure_months {
            let years = tenure / 12.0;
            lines.push(format!("• Avg tenure at exit: {:.1} years", years));
        }
        if let Some(rate) = agg.attrition.turnover_rate_annualized {
            lines.push(format!("• Turnover rate: {:.1}% annualized", rate));
        }
    } else {
        lines.push("• No terminations YTD".to_string());
    }

    lines.join("\n")
}

/// Get human-readable rating label.
/// `pub(super)` so `prompt::format_single_employee_with_budget` can also use it.
pub(super) fn rating_label(rating: f64) -> &'static str {
    if rating >= 4.5 {
        "Exceptional"
    } else if rating >= 3.5 {
        "Exceeds Expectations"
    } else if rating >= 2.5 {
        "Meets Expectations"
    } else if rating >= 1.5 {
        "Developing"
    } else {
        "Unsatisfactory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rating_label() {
        assert_eq!(rating_label(4.8), "Exceptional");
        assert_eq!(rating_label(3.7), "Exceeds Expectations");
        assert_eq!(rating_label(3.0), "Meets Expectations");
        assert_eq!(rating_label(2.2), "Developing");
        assert_eq!(rating_label(1.2), "Unsatisfactory");
    }

    #[test]
    fn test_format_org_aggregates_basic() {
        let agg = OrgAggregates {
            total_employees: 100,
            active_count: 82,
            terminated_count: 12,
            on_leave_count: 6,
            by_department: vec![
                DepartmentCount { name: "Engineering".to_string(), count: 28, percentage: 34.1 },
                DepartmentCount { name: "Sales".to_string(), count: 18, percentage: 22.0 },
                DepartmentCount { name: "Marketing".to_string(), count: 12, percentage: 14.6 },
            ],
            avg_rating: Some(3.4),
            rating_distribution: RatingDistribution {
                exceptional: 8,
                exceeds: 32,
                meets: 38,
                needs_improvement: 4,
            },
            employees_with_no_rating: 12,
            enps: EnpsAggregate {
                score: 12,
                promoters: 34,
                passives: 28,
                detractors: 20,
                total_responses: 67,
                response_rate: 81.7,
            },
            attrition: AttritionStats {
                terminations_ytd: 12,
                voluntary: 8,
                involuntary: 4,
                avg_tenure_months: Some(27.6),
                turnover_rate_annualized: Some(14.6),
            },
        };

        let formatted = format_org_aggregates(&agg, Some("Acme Corp"));

        assert!(formatted.contains("ORGANIZATION DATA:"));
        assert!(formatted.contains("COMPANY: Acme Corp"));
        assert!(formatted.contains("WORKFORCE: 100 employees"));
        assert!(formatted.contains("Active: 82"));
        assert!(formatted.contains("Terminated: 12"));
        assert!(formatted.contains("On Leave: 6"));

        assert!(formatted.contains("DEPARTMENTS:"));
        assert!(formatted.contains("Engineering: 28"));
        assert!(formatted.contains("Sales: 18"));

        assert!(formatted.contains("PERFORMANCE (82 active employees):"));
        assert!(formatted.contains("Avg rating: 3.4 (Meets Expectations)"));
        assert!(formatted.contains("Exceptional: 8"));

        assert!(formatted.contains("ENGAGEMENT:"));
        assert!(formatted.contains("eNPS: +12"));
        assert!(formatted.contains("Promoters: 34"));

        assert!(formatted.contains("ATTRITION (YTD):"));
        assert!(formatted.contains("Terminations: 12"));
        assert!(formatted.contains("Voluntary: 8"));
        assert!(formatted.contains("Turnover rate: 14.6%"));
    }

    #[test]
    fn test_format_org_aggregates_empty_data() {
        let agg = OrgAggregates {
            total_employees: 0,
            active_count: 0,
            terminated_count: 0,
            on_leave_count: 0,
            by_department: vec![],
            avg_rating: None,
            rating_distribution: RatingDistribution::default(),
            employees_with_no_rating: 0,
            enps: EnpsAggregate {
                score: 0,
                promoters: 0,
                passives: 0,
                detractors: 0,
                total_responses: 0,
                response_rate: 0.0,
            },
            attrition: AttritionStats::default(),
        };

        let formatted = format_org_aggregates(&agg, None);

        assert!(formatted.contains("ORGANIZATION DATA:"));
        assert!(formatted.contains("WORKFORCE: 0 employees"));
        assert!(formatted.contains("No performance data available"));
        assert!(formatted.contains("No terminations YTD"));
    }

    #[test]
    fn test_format_org_aggregates_negative_enps() {
        let agg = OrgAggregates {
            total_employees: 50,
            active_count: 45,
            terminated_count: 5,
            on_leave_count: 0,
            by_department: vec![],
            avg_rating: Some(2.8),
            rating_distribution: RatingDistribution {
                exceptional: 2,
                exceeds: 10,
                meets: 25,
                needs_improvement: 8,
            },
            employees_with_no_rating: 0,
            enps: EnpsAggregate {
                score: -15,
                promoters: 10,
                passives: 15,
                detractors: 20,
                total_responses: 45,
                response_rate: 100.0,
            },
            attrition: AttritionStats::default(),
        };

        let formatted = format_org_aggregates(&agg, Some("Test Corp"));

        // Negative eNPS should not have + sign
        assert!(formatted.contains("eNPS: -15"));
        assert!(!formatted.contains("eNPS: +-15"));
    }

    #[test]
    fn test_rating_distribution_default() {
        let dist = RatingDistribution::default();
        assert_eq!(dist.exceptional, 0);
        assert_eq!(dist.exceeds, 0);
        assert_eq!(dist.meets, 0);
        assert_eq!(dist.needs_improvement, 0);
    }

    #[test]
    fn test_attrition_stats_default() {
        let stats = AttritionStats::default();
        assert_eq!(stats.terminations_ytd, 0);
        assert_eq!(stats.voluntary, 0);
        assert_eq!(stats.involuntary, 0);
        assert!(stats.avg_tenure_months.is_none());
        assert!(stats.turnover_rate_annualized.is_none());
    }

    #[test]
    fn test_format_org_aggregates_size_budget() {
        // Verify formatted output stays within reasonable size (~2K chars)
        let agg = OrgAggregates {
            total_employees: 500,
            active_count: 450,
            terminated_count: 40,
            on_leave_count: 10,
            by_department: vec![
                DepartmentCount { name: "Engineering".to_string(), count: 150, percentage: 33.3 },
                DepartmentCount { name: "Sales".to_string(), count: 100, percentage: 22.2 },
                DepartmentCount { name: "Marketing".to_string(), count: 60, percentage: 13.3 },
                DepartmentCount { name: "Operations".to_string(), count: 50, percentage: 11.1 },
                DepartmentCount { name: "Finance".to_string(), count: 40, percentage: 8.9 },
                DepartmentCount { name: "HR".to_string(), count: 30, percentage: 6.7 },
                DepartmentCount { name: "Legal".to_string(), count: 15, percentage: 3.3 },
                DepartmentCount { name: "Executive".to_string(), count: 5, percentage: 1.1 },
            ],
            avg_rating: Some(3.6),
            rating_distribution: RatingDistribution {
                exceptional: 45,
                exceeds: 180,
                meets: 200,
                needs_improvement: 25,
            },
            employees_with_no_rating: 50,
            enps: EnpsAggregate {
                score: 25,
                promoters: 180,
                passives: 150,
                detractors: 70,
                total_responses: 400,
                response_rate: 88.9,
            },
            attrition: AttritionStats {
                terminations_ytd: 40,
                voluntary: 30,
                involuntary: 10,
                avg_tenure_months: Some(36.0),
                turnover_rate_annualized: Some(8.5),
            },
        };

        let formatted = format_org_aggregates(&agg, Some("Large Enterprise Corp"));

        // Should stay under 2500 chars for reasonable context budget
        assert!(
            formatted.len() < 2500,
            "Formatted output too large: {} chars",
            formatted.len()
        );
    }
}
