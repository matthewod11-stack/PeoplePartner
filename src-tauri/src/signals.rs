// People Partner - Signals Module
// Team-level attrition risk and sentiment analysis (V2.4.1)
//
// Key guardrails:
// - Team-level only (department), never individual predictions
// - Opt-in required with first-use disclaimer
// - Strong disclaimers on all outputs
// - Factor transparency (show what contributed)

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;

use crate::db::DbPool;

// ============================================================================
// Constants
// ============================================================================

/// Weight for tenure factor in composite score
pub const TENURE_WEIGHT: f64 = 0.35;

/// Weight for performance factor in composite score
pub const PERFORMANCE_WEIGHT: f64 = 0.35;

/// Weight for engagement factor in composite score
pub const ENGAGEMENT_WEIGHT: f64 = 0.30;

/// Minimum team size for privacy protection
pub const MIN_TEAM_SIZE: i64 = 5;

/// Disclaimer text for all signal outputs
pub const DISCLAIMER: &str = "These are heuristic indicators based on aggregate team patterns, not predictions about individuals. Use as conversation starters, not conclusions.";

/// Settings key for signals feature
pub const SIGNALS_ENABLED_KEY: &str = "signals_enabled";

/// Settings key for first-use acknowledgment
pub const SIGNALS_ACKNOWLEDGED_KEY: &str = "signals_acknowledged";

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Serialize)]
pub enum SignalsError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Feature not enabled")]
    NotEnabled,
    #[error("Calculation error: {0}")]
    Calculation(String),
}

impl From<sqlx::Error> for SignalsError {
    fn from(err: sqlx::Error) -> Self {
        SignalsError::Database(err.to_string())
    }
}

// ============================================================================
// Types
// ============================================================================

/// Attention level based on composite score
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttentionLevel {
    High,     // 70-100
    Moderate, // 50-69
    Monitor,  // 30-49
    Low,      // 0-29
}

impl AttentionLevel {
    /// Convert a score (0-100) to an attention level
    pub fn from_score(score: f64) -> Self {
        match score as i32 {
            70..=100 => AttentionLevel::High,
            50..=69 => AttentionLevel::Moderate,
            30..=49 => AttentionLevel::Monitor,
            _ => AttentionLevel::Low,
        }
    }
}

/// Tenure factor breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenureFactor {
    /// Percentage of team with < 1 year tenure
    pub pct_under_1yr: f64,
    /// Percentage of team with 3-5 years tenure (career plateau window)
    pub pct_3_to_5yr: f64,
    /// Calculated factor score (0-100)
    pub score: f64,
}

impl TenureFactor {
    /// Calculate tenure factor: (pct_under_1yr × 0.6) + (pct_3_to_5yr × 0.4)
    pub fn calculate(pct_under_1yr: f64, pct_3_to_5yr: f64) -> Self {
        let score = (pct_under_1yr * 0.6 + pct_3_to_5yr * 0.4).min(100.0);
        Self {
            pct_under_1yr,
            pct_3_to_5yr,
            score,
        }
    }
}

/// Performance factor breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceFactor {
    /// Percentage of team with declining ratings (latest < previous)
    pub pct_declining: f64,
    /// Percentage of team with "needs improvement" rating (< 3.0)
    pub pct_needs_improvement: f64,
    /// Calculated factor score (0-100)
    pub score: f64,
}

impl PerformanceFactor {
    /// Calculate performance factor: (pct_declining × 0.7) + (pct_needs_improvement × 0.3)
    pub fn calculate(pct_declining: f64, pct_needs_improvement: f64) -> Self {
        let score = (pct_declining * 0.7 + pct_needs_improvement * 0.3).min(100.0);
        Self {
            pct_declining,
            pct_needs_improvement,
            score,
        }
    }
}

/// Engagement factor breakdown (based on eNPS)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementFactor {
    /// Percentage of team that are detractors (eNPS <= 6)
    pub pct_detractors: f64,
    /// Percentage of team that are passives (eNPS 7-8)
    pub pct_passives: f64,
    /// Calculated factor score (0-100)
    pub score: f64,
}

impl EngagementFactor {
    /// Calculate engagement factor: (pct_detractors × 0.8) + (pct_passives × 0.2)
    pub fn calculate(pct_detractors: f64, pct_passives: f64) -> Self {
        let score = (pct_detractors * 0.8 + pct_passives * 0.2).min(100.0);
        Self {
            pct_detractors,
            pct_passives,
            score,
        }
    }
}

/// A theme occurrence from review highlights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeOccurrence {
    /// Theme name (from VALID_THEMES)
    pub theme: String,
    /// Dominant sentiment for this theme
    pub sentiment: String,
    /// Number of occurrences in the team's reviews
    pub count: i32,
}

/// Complete attention signal for a team
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamAttentionSignal {
    /// Team/department name
    pub team: String,
    /// Number of active employees
    pub headcount: i64,
    /// Composite attention score (0-100)
    pub attention_score: f64,
    /// Attention level category
    pub attention_level: AttentionLevel,
    /// Tenure factor breakdown
    pub tenure_factor: TenureFactor,
    /// Performance factor breakdown
    pub performance_factor: PerformanceFactor,
    /// Engagement factor breakdown
    pub engagement_factor: EngagementFactor,
    /// Common themes from recent reviews (top 3)
    pub common_themes: Vec<ThemeOccurrence>,
}

impl TeamAttentionSignal {
    /// Calculate composite attention score from factors
    pub fn calculate_score(
        tenure: &TenureFactor,
        performance: &PerformanceFactor,
        engagement: &EngagementFactor,
    ) -> f64 {
        (tenure.score * TENURE_WEIGHT
            + performance.score * PERFORMANCE_WEIGHT
            + engagement.score * ENGAGEMENT_WEIGHT)
            .min(100.0)
    }
}

/// Summary of attention areas across all teams
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionAreasSummary {
    /// Teams with attention signals (filtered by MIN_TEAM_SIZE)
    pub teams: Vec<TeamAttentionSignal>,
    /// Disclaimer text
    pub disclaimer: String,
    /// When this was computed
    pub computed_at: String,
}

// ============================================================================
// Database Row Types
// ============================================================================

/// Raw row from the team aggregation query
#[derive(Debug, Clone, FromRow)]
struct TeamAggregateRow {
    department: String,
    headcount: i64,
    pct_under_1yr: f64,
    pct_3_to_5yr: f64,
    pct_declining: f64,
    pct_needs_improvement: f64,
    pct_detractors: f64,
    pct_passives: f64,
}

/// Raw row from theme query
#[derive(Debug, Clone, FromRow)]
struct ThemeRow {
    theme: String,
    sentiment: String,
    count: i32,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Check if signals feature is enabled in settings
pub async fn is_signals_enabled(pool: &DbPool) -> Result<bool, SignalsError> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = ?"
    )
    .bind(SIGNALS_ENABLED_KEY)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|v| v == "true").unwrap_or(false))
}

/// Get team attention signals for all departments
///
/// Returns teams sorted by attention_score descending, filtered to MIN_TEAM_SIZE.
pub async fn get_team_attention_signals(
    pool: &DbPool,
) -> Result<AttentionAreasSummary, SignalsError> {
    // Query to calculate all factors for each department
    // Uses CTEs to build up tenure, performance, and engagement metrics
    let rows = sqlx::query_as::<_, TeamAggregateRow>(
        r#"
        WITH active_employees AS (
            -- Base: Active employees with valid department
            SELECT
                e.id,
                e.department,
                e.hire_date
            FROM employees e
            WHERE e.status = 'active'
              AND e.department IS NOT NULL
              AND e.department != ''
        ),
        team_counts AS (
            -- Get headcount per department
            SELECT department, COUNT(*) as headcount
            FROM active_employees
            GROUP BY department
            HAVING COUNT(*) >= ?
        ),
        tenure_risk AS (
            -- Calculate tenure metrics per department
            SELECT
                ae.department,
                COALESCE(
                    100.0 * SUM(CASE
                        WHEN julianday('now') - julianday(ae.hire_date) < 365
                        THEN 1 ELSE 0
                    END) / NULLIF(COUNT(*), 0),
                    0
                ) as pct_under_1yr,
                COALESCE(
                    100.0 * SUM(CASE
                        WHEN julianday('now') - julianday(ae.hire_date) BETWEEN 1095 AND 1825
                        THEN 1 ELSE 0
                    END) / NULLIF(COUNT(*), 0),
                    0
                ) as pct_3_to_5yr
            FROM active_employees ae
            WHERE ae.hire_date IS NOT NULL
            GROUP BY ae.department
        ),
        latest_ratings AS (
            -- Get latest rating per employee
            SELECT
                pr.employee_id,
                pr.overall_rating,
                pr.review_cycle_id,
                ROW_NUMBER() OVER (
                    PARTITION BY pr.employee_id
                    ORDER BY rc.end_date DESC
                ) as rn
            FROM performance_ratings pr
            JOIN review_cycles rc ON pr.review_cycle_id = rc.id
        ),
        previous_ratings AS (
            -- Get second-latest rating per employee for trend comparison
            SELECT
                pr.employee_id,
                pr.overall_rating as prev_rating,
                ROW_NUMBER() OVER (
                    PARTITION BY pr.employee_id
                    ORDER BY rc.end_date DESC
                ) as rn
            FROM performance_ratings pr
            JOIN review_cycles rc ON pr.review_cycle_id = rc.id
        ),
        performance_risk AS (
            -- Calculate performance metrics per department
            SELECT
                ae.department,
                COALESCE(
                    100.0 * SUM(CASE
                        WHEN lr.overall_rating IS NOT NULL
                         AND pvr.prev_rating IS NOT NULL
                         AND lr.overall_rating < pvr.prev_rating
                        THEN 1 ELSE 0
                    END) / NULLIF(SUM(CASE WHEN lr.overall_rating IS NOT NULL THEN 1 ELSE 0 END), 0),
                    0
                ) as pct_declining,
                COALESCE(
                    100.0 * SUM(CASE
                        WHEN lr.overall_rating IS NOT NULL
                         AND lr.overall_rating < 3.0
                        THEN 1 ELSE 0
                    END) / NULLIF(SUM(CASE WHEN lr.overall_rating IS NOT NULL THEN 1 ELSE 0 END), 0),
                    0
                ) as pct_needs_improvement
            FROM active_employees ae
            LEFT JOIN latest_ratings lr ON ae.id = lr.employee_id AND lr.rn = 1
            LEFT JOIN previous_ratings pvr ON ae.id = pvr.employee_id AND pvr.rn = 2
            GROUP BY ae.department
        ),
        latest_enps AS (
            -- Get latest eNPS per employee
            SELECT
                en.employee_id,
                en.score,
                ROW_NUMBER() OVER (
                    PARTITION BY en.employee_id
                    ORDER BY en.survey_date DESC
                ) as rn
            FROM enps_responses en
        ),
        engagement_risk AS (
            -- Calculate engagement metrics per department (based on eNPS)
            SELECT
                ae.department,
                COALESCE(
                    100.0 * SUM(CASE
                        WHEN le.score IS NOT NULL AND le.score <= 6
                        THEN 1 ELSE 0
                    END) / NULLIF(SUM(CASE WHEN le.score IS NOT NULL THEN 1 ELSE 0 END), 0),
                    0
                ) as pct_detractors,
                COALESCE(
                    100.0 * SUM(CASE
                        WHEN le.score IS NOT NULL AND le.score BETWEEN 7 AND 8
                        THEN 1 ELSE 0
                    END) / NULLIF(SUM(CASE WHEN le.score IS NOT NULL THEN 1 ELSE 0 END), 0),
                    0
                ) as pct_passives
            FROM active_employees ae
            LEFT JOIN latest_enps le ON ae.id = le.employee_id AND le.rn = 1
            GROUP BY ae.department
        )
        -- Final aggregation joining all factors
        SELECT
            tc.department,
            tc.headcount,
            COALESCE(tr.pct_under_1yr, 0) as pct_under_1yr,
            COALESCE(tr.pct_3_to_5yr, 0) as pct_3_to_5yr,
            COALESCE(pr.pct_declining, 0) as pct_declining,
            COALESCE(pr.pct_needs_improvement, 0) as pct_needs_improvement,
            COALESCE(er.pct_detractors, 0) as pct_detractors,
            COALESCE(er.pct_passives, 0) as pct_passives
        FROM team_counts tc
        LEFT JOIN tenure_risk tr ON tc.department = tr.department
        LEFT JOIN performance_risk pr ON tc.department = pr.department
        LEFT JOIN engagement_risk er ON tc.department = er.department
        ORDER BY tc.department
        "#,
    )
    .bind(MIN_TEAM_SIZE)
    .fetch_all(pool)
    .await?;

    // Convert rows to TeamAttentionSignal with calculated scores
    let mut teams: Vec<TeamAttentionSignal> = Vec::with_capacity(rows.len());

    for row in rows {
        let tenure_factor = TenureFactor::calculate(row.pct_under_1yr, row.pct_3_to_5yr);
        let performance_factor =
            PerformanceFactor::calculate(row.pct_declining, row.pct_needs_improvement);
        let engagement_factor =
            EngagementFactor::calculate(row.pct_detractors, row.pct_passives);

        let attention_score =
            TeamAttentionSignal::calculate_score(&tenure_factor, &performance_factor, &engagement_factor);

        let attention_level = AttentionLevel::from_score(attention_score);

        // Get common themes for this team
        let common_themes = get_common_themes_for_team(pool, &row.department)
            .await
            .unwrap_or_default();

        teams.push(TeamAttentionSignal {
            team: row.department,
            headcount: row.headcount,
            attention_score,
            attention_level,
            tenure_factor,
            performance_factor,
            engagement_factor,
            common_themes,
        });
    }

    // Sort by attention_score descending
    teams.sort_by(|a, b| {
        b.attention_score
            .partial_cmp(&a.attention_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(AttentionAreasSummary {
        teams,
        disclaimer: DISCLAIMER.to_string(),
        computed_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Get common themes for a specific team from review highlights
///
/// Returns top 3 themes with their dominant sentiment.
pub async fn get_common_themes_for_team(
    pool: &DbPool,
    department: &str,
) -> Result<Vec<ThemeOccurrence>, SignalsError> {
    // Query themes from review_highlights for employees in this department
    // Limited to last 18 months
    let rows = sqlx::query_as::<_, ThemeRow>(
        r#"
        WITH department_employees AS (
            SELECT id FROM employees
            WHERE department = ? AND status = 'active'
        ),
        theme_data AS (
            -- Parse themes JSON array and count occurrences
            SELECT
                json_each.value as theme,
                rh.overall_sentiment as sentiment
            FROM review_highlights rh
            JOIN department_employees de ON rh.employee_id = de.id
            JOIN json_each(rh.themes)
            WHERE datetime(rh.created_at) >= datetime('now', '-18 months')
        ),
        theme_counts AS (
            SELECT
                theme,
                COUNT(*) as count,
                -- Get dominant sentiment by most common
                (
                    SELECT sentiment
                    FROM theme_data td2
                    WHERE td2.theme = theme_data.theme
                    GROUP BY sentiment
                    ORDER BY COUNT(*) DESC
                    LIMIT 1
                ) as dominant_sentiment
            FROM theme_data
            GROUP BY theme
        )
        SELECT
            theme,
            COALESCE(dominant_sentiment, 'neutral') as sentiment,
            CAST(count AS INTEGER) as count
        FROM theme_counts
        ORDER BY count DESC
        LIMIT 3
        "#,
    )
    .bind(department)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ThemeOccurrence {
            theme: r.theme,
            sentiment: r.sentiment,
            count: r.count,
        })
        .collect())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- AttentionLevel Tests --------------------

    #[test]
    fn test_attention_level_from_score_high() {
        assert_eq!(AttentionLevel::from_score(70.0), AttentionLevel::High);
        assert_eq!(AttentionLevel::from_score(85.0), AttentionLevel::High);
        assert_eq!(AttentionLevel::from_score(100.0), AttentionLevel::High);
    }

    #[test]
    fn test_attention_level_from_score_moderate() {
        assert_eq!(AttentionLevel::from_score(50.0), AttentionLevel::Moderate);
        assert_eq!(AttentionLevel::from_score(60.0), AttentionLevel::Moderate);
        assert_eq!(AttentionLevel::from_score(69.0), AttentionLevel::Moderate);
    }

    #[test]
    fn test_attention_level_from_score_monitor() {
        assert_eq!(AttentionLevel::from_score(30.0), AttentionLevel::Monitor);
        assert_eq!(AttentionLevel::from_score(40.0), AttentionLevel::Monitor);
        assert_eq!(AttentionLevel::from_score(49.0), AttentionLevel::Monitor);
    }

    #[test]
    fn test_attention_level_from_score_low() {
        assert_eq!(AttentionLevel::from_score(0.0), AttentionLevel::Low);
        assert_eq!(AttentionLevel::from_score(15.0), AttentionLevel::Low);
        assert_eq!(AttentionLevel::from_score(29.0), AttentionLevel::Low);
    }

    #[test]
    fn test_attention_level_boundary_values() {
        // Test exact boundaries
        assert_eq!(AttentionLevel::from_score(69.9), AttentionLevel::Moderate);
        assert_eq!(AttentionLevel::from_score(49.9), AttentionLevel::Monitor);
        assert_eq!(AttentionLevel::from_score(29.9), AttentionLevel::Low);
    }

    // -------------------- TenureFactor Tests --------------------

    #[test]
    fn test_tenure_factor_calculation() {
        let factor = TenureFactor::calculate(40.0, 30.0);
        // (40 * 0.6) + (30 * 0.4) = 24 + 12 = 36
        assert!((factor.score - 36.0).abs() < 0.01);
        assert!((factor.pct_under_1yr - 40.0).abs() < 0.01);
        assert!((factor.pct_3_to_5yr - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_tenure_factor_max_cap() {
        // Very high values should be capped at 100
        let factor = TenureFactor::calculate(100.0, 100.0);
        assert!(factor.score <= 100.0);
    }

    #[test]
    fn test_tenure_factor_zero_values() {
        let factor = TenureFactor::calculate(0.0, 0.0);
        assert!((factor.score - 0.0).abs() < 0.01);
    }

    // -------------------- PerformanceFactor Tests --------------------

    #[test]
    fn test_performance_factor_calculation() {
        let factor = PerformanceFactor::calculate(50.0, 20.0);
        // (50 * 0.7) + (20 * 0.3) = 35 + 6 = 41
        assert!((factor.score - 41.0).abs() < 0.01);
    }

    #[test]
    fn test_performance_factor_max_cap() {
        let factor = PerformanceFactor::calculate(100.0, 100.0);
        assert!(factor.score <= 100.0);
    }

    // -------------------- EngagementFactor Tests --------------------

    #[test]
    fn test_engagement_factor_calculation() {
        let factor = EngagementFactor::calculate(30.0, 40.0);
        // (30 * 0.8) + (40 * 0.2) = 24 + 8 = 32
        assert!((factor.score - 32.0).abs() < 0.01);
    }

    #[test]
    fn test_engagement_factor_max_cap() {
        let factor = EngagementFactor::calculate(100.0, 100.0);
        assert!(factor.score <= 100.0);
    }

    // -------------------- Composite Score Tests --------------------

    #[test]
    fn test_composite_score_calculation() {
        let tenure = TenureFactor::calculate(40.0, 30.0); // score = 36
        let performance = PerformanceFactor::calculate(50.0, 20.0); // score = 41
        let engagement = EngagementFactor::calculate(30.0, 40.0); // score = 32

        let score = TeamAttentionSignal::calculate_score(&tenure, &performance, &engagement);
        // (36 * 0.35) + (41 * 0.35) + (32 * 0.30) = 12.6 + 14.35 + 9.6 = 36.55
        assert!((score - 36.55).abs() < 0.1);
    }

    #[test]
    fn test_composite_score_max_values() {
        let tenure = TenureFactor::calculate(100.0, 100.0);
        let performance = PerformanceFactor::calculate(100.0, 100.0);
        let engagement = EngagementFactor::calculate(100.0, 100.0);

        let score = TeamAttentionSignal::calculate_score(&tenure, &performance, &engagement);
        // All factors at 100 should result in max score
        assert!(score <= 100.0);
    }

    #[test]
    fn test_composite_score_zero_values() {
        let tenure = TenureFactor::calculate(0.0, 0.0);
        let performance = PerformanceFactor::calculate(0.0, 0.0);
        let engagement = EngagementFactor::calculate(0.0, 0.0);

        let score = TeamAttentionSignal::calculate_score(&tenure, &performance, &engagement);
        assert!((score - 0.0).abs() < 0.01);
    }

    // -------------------- Weight Validation Tests --------------------

    #[test]
    fn test_weights_sum_to_one() {
        let total = TENURE_WEIGHT + PERFORMANCE_WEIGHT + ENGAGEMENT_WEIGHT;
        assert!((total - 1.0).abs() < 0.001, "Weights must sum to 1.0");
    }

    #[test]
    fn test_min_team_size_is_reasonable() {
        assert!(
            MIN_TEAM_SIZE >= 5,
            "MIN_TEAM_SIZE should be at least 5 for privacy"
        );
    }

    // -------------------- Serialization Tests --------------------

    #[test]
    fn test_attention_level_serialization() {
        let level = AttentionLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"high\"");

        let level = AttentionLevel::Moderate;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"moderate\"");
    }

    #[test]
    fn test_team_attention_signal_serialization() {
        let signal = TeamAttentionSignal {
            team: "Engineering".to_string(),
            headcount: 25,
            attention_score: 65.0,
            attention_level: AttentionLevel::Moderate,
            tenure_factor: TenureFactor::calculate(30.0, 20.0),
            performance_factor: PerformanceFactor::calculate(40.0, 10.0),
            engagement_factor: EngagementFactor::calculate(25.0, 35.0),
            common_themes: vec![],
        };

        let json = serde_json::to_string(&signal).unwrap();
        assert!(json.contains("\"team\":\"Engineering\""));
        assert!(json.contains("\"attention_level\":\"moderate\""));
    }
}
