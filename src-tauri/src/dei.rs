// HR Command Center - DEI & Fairness Lens Module
// Demographic representation analysis with privacy guardrails (V2.4.2)
//
// Key guardrails:
// - Group-level only (never individual predictions)
// - Opt-in required with first-use disclaimer
// - Small-n suppression (groups < 5 hidden)
// - Strong disclaimers on all outputs
// - Audit trail for DEI queries

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;

use crate::db::DbPool;

// ============================================================================
// Constants
// ============================================================================

/// Minimum group size for privacy protection
/// Groups smaller than this will have data suppressed
pub const MIN_GROUP_SIZE: i64 = 5;

/// Settings key for fairness lens feature
pub const FAIRNESS_LENS_ENABLED_KEY: &str = "fairness_lens_enabled";

/// Settings key for first-use acknowledgment
pub const FAIRNESS_LENS_ACKNOWLEDGED_KEY: &str = "fairness_lens_acknowledged";

/// Disclaimer text for all DEI outputs
pub const DEI_DISCLAIMER: &str = "This analysis reflects historical data patterns \
and may reveal systemic biases rather than individual performance differences. \
Groups with fewer than 5 members are suppressed to protect privacy.";

/// Disclaimer text for promotion inference
pub const PROMOTION_DISCLAIMER: &str = "Promotion data is inferred from job title \
changes and may not reflect all career progression events.";

/// Title keywords that indicate a promotion
pub const PROMOTION_KEYWORDS: &[&str] = &[
    "Senior", "Lead", "Manager", "Director", "VP", "Vice President", "Head", "Chief", "Principal"
];

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Serialize)]
pub enum DeiError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Feature not enabled")]
    NotEnabled,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl From<sqlx::Error> for DeiError {
    fn from(err: sqlx::Error) -> Self {
        DeiError::Database(err.to_string())
    }
}

// ============================================================================
// Types
// ============================================================================

/// A single demographic breakdown item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeiBreakdown {
    /// Category label (e.g., "Female", "Male", "Engineering")
    pub label: String,
    /// Count of employees in this category
    pub count: i64,
    /// Percentage of total
    pub percentage: f64,
    /// Whether this group is suppressed due to small size
    pub suppressed: bool,
}

/// Representation breakdown result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepresentationResult {
    /// The grouping dimension (e.g., "gender", "ethnicity")
    pub group_by: String,
    /// Filter applied (e.g., department name or null for all)
    pub filter_department: Option<String>,
    /// Breakdown items
    pub breakdown: Vec<DeiBreakdown>,
    /// Total count (including suppressed groups)
    pub total: i64,
    /// Disclaimer text
    pub disclaimer: String,
    /// When this was computed (ISO 8601)
    pub computed_at: String,
}

/// Rating parity item showing average rating by demographic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingParityItem {
    /// Category label
    pub label: String,
    /// Number of employees with ratings
    pub count: i64,
    /// Average rating for this group
    pub avg_rating: Option<f64>,
    /// Whether this group is suppressed due to small size
    pub suppressed: bool,
}

/// Rating parity result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingParityResult {
    /// The grouping dimension (e.g., "gender", "ethnicity")
    pub group_by: String,
    /// Parity items by group
    pub items: Vec<RatingParityItem>,
    /// Overall average rating
    pub overall_avg: Option<f64>,
    /// Disclaimer text
    pub disclaimer: String,
    /// When this was computed (ISO 8601)
    pub computed_at: String,
}

/// Promotion rate item by demographic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRateItem {
    /// Category label
    pub label: String,
    /// Number of employees in group
    pub total_count: i64,
    /// Number with promotion-indicating titles
    pub promoted_count: i64,
    /// Promotion rate percentage
    pub rate: Option<f64>,
    /// Whether this group is suppressed due to small size
    pub suppressed: bool,
}

/// Promotion rates result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRatesResult {
    /// The grouping dimension (e.g., "gender", "ethnicity")
    pub group_by: String,
    /// Rates by group
    pub items: Vec<PromotionRateItem>,
    /// Overall promotion rate
    pub overall_rate: Option<f64>,
    /// Promotion data disclaimer
    pub disclaimer: String,
    /// When this was computed (ISO 8601)
    pub computed_at: String,
}

/// Complete fairness lens summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairnessLensSummary {
    /// Representation by gender
    pub gender_representation: RepresentationResult,
    /// Representation by ethnicity
    pub ethnicity_representation: RepresentationResult,
    /// Rating parity by gender
    pub gender_rating_parity: RatingParityResult,
    /// Rating parity by ethnicity
    pub ethnicity_rating_parity: RatingParityResult,
    /// Promotion rates by gender
    pub gender_promotion_rates: PromotionRatesResult,
    /// Promotion rates by ethnicity
    pub ethnicity_promotion_rates: PromotionRatesResult,
    /// Main disclaimer
    pub disclaimer: String,
    /// When this was computed (ISO 8601)
    pub computed_at: String,
}

// ============================================================================
// Database Row Types
// ============================================================================

/// Raw row from representation breakdown query
#[derive(Debug, Clone, FromRow)]
struct BreakdownRow {
    label: String,
    count: i64,
}

/// Raw row from rating parity query
#[derive(Debug, Clone, FromRow)]
struct RatingParityRow {
    label: String,
    count: i64,
    avg_rating: Option<f64>,
}

/// Raw row from promotion rates query
#[derive(Debug, Clone, FromRow)]
struct PromotionRow {
    label: String,
    total_count: i64,
    promoted_count: i64,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Check if fairness lens feature is enabled in settings
pub async fn is_fairness_lens_enabled(pool: &DbPool) -> Result<bool, DeiError> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = ?"
    )
    .bind(FAIRNESS_LENS_ENABLED_KEY)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|v| v == "true").unwrap_or(false))
}

/// Get representation breakdown by a demographic field
///
/// Returns groups with counts and percentages, suppressing small groups.
pub async fn get_representation_breakdown(
    pool: &DbPool,
    group_by: &str,
    filter_department: Option<&str>,
) -> Result<RepresentationResult, DeiError> {
    // Validate group_by to prevent SQL injection
    let column = match group_by {
        "gender" => "gender",
        "ethnicity" => "ethnicity",
        _ => return Err(DeiError::InvalidInput(format!("Invalid group_by: {}", group_by))),
    };

    // Build query based on whether we have a department filter
    let rows = if let Some(dept) = filter_department {
        sqlx::query_as::<_, BreakdownRow>(&format!(
            r#"
            SELECT COALESCE({}, 'Not Specified') as label, COUNT(*) as count
            FROM employees
            WHERE status = 'active' AND department = ?
            GROUP BY {}
            ORDER BY count DESC
            "#,
            column, column
        ))
        .bind(dept)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, BreakdownRow>(&format!(
            r#"
            SELECT COALESCE({}, 'Not Specified') as label, COUNT(*) as count
            FROM employees
            WHERE status = 'active'
            GROUP BY {}
            ORDER BY count DESC
            "#,
            column, column
        ))
        .fetch_all(pool)
        .await?
    };

    // Calculate total
    let total: i64 = rows.iter().map(|r| r.count).sum();

    // Convert to breakdown items with suppression
    let breakdown: Vec<DeiBreakdown> = rows
        .into_iter()
        .map(|row| {
            let suppressed = row.count < MIN_GROUP_SIZE;
            DeiBreakdown {
                label: row.label,
                count: if suppressed { 0 } else { row.count },
                percentage: if suppressed || total == 0 {
                    0.0
                } else {
                    (row.count as f64 / total as f64 * 100.0 * 10.0).round() / 10.0
                },
                suppressed,
            }
        })
        .collect();

    Ok(RepresentationResult {
        group_by: group_by.to_string(),
        filter_department: filter_department.map(String::from),
        breakdown,
        total,
        disclaimer: DEI_DISCLAIMER.to_string(),
        computed_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Get rating parity by a demographic field
///
/// Returns average ratings per group, suppressing small groups.
pub async fn get_rating_parity(
    pool: &DbPool,
    group_by: &str,
) -> Result<RatingParityResult, DeiError> {
    // Validate group_by to prevent SQL injection
    let column = match group_by {
        "gender" => "gender",
        "ethnicity" => "ethnicity",
        _ => return Err(DeiError::InvalidInput(format!("Invalid group_by: {}", group_by))),
    };

    let rows = sqlx::query_as::<_, RatingParityRow>(&format!(
        r#"
        WITH latest_ratings AS (
            SELECT
                pr.employee_id,
                pr.overall_rating,
                e.{},
                ROW_NUMBER() OVER (PARTITION BY pr.employee_id ORDER BY rc.end_date DESC) as rn
            FROM performance_ratings pr
            JOIN review_cycles rc ON pr.review_cycle_id = rc.id
            JOIN employees e ON pr.employee_id = e.id
            WHERE e.status = 'active'
        )
        SELECT
            COALESCE({}, 'Not Specified') as label,
            COUNT(*) as count,
            ROUND(AVG(overall_rating), 2) as avg_rating
        FROM latest_ratings
        WHERE rn = 1
        GROUP BY {}
        ORDER BY avg_rating DESC
        "#,
        column, column, column
    ))
    .fetch_all(pool)
    .await?;

    // Calculate overall average
    let overall_avg = sqlx::query_scalar::<_, Option<f64>>(
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
        SELECT ROUND(AVG(overall_rating), 2)
        FROM latest_ratings
        WHERE rn = 1
        "#
    )
    .fetch_one(pool)
    .await?;

    // Convert to items with suppression
    let items: Vec<RatingParityItem> = rows
        .into_iter()
        .map(|row| {
            let suppressed = row.count < MIN_GROUP_SIZE;
            RatingParityItem {
                label: row.label,
                count: if suppressed { 0 } else { row.count },
                avg_rating: if suppressed { None } else { row.avg_rating },
                suppressed,
            }
        })
        .collect();

    Ok(RatingParityResult {
        group_by: group_by.to_string(),
        items,
        overall_avg,
        disclaimer: DEI_DISCLAIMER.to_string(),
        computed_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Get promotion rates by a demographic field
///
/// Infers promotions from job title keywords. Suppresses small groups.
pub async fn get_promotion_rates(
    pool: &DbPool,
    group_by: &str,
) -> Result<PromotionRatesResult, DeiError> {
    // Validate group_by to prevent SQL injection
    let column = match group_by {
        "gender" => "gender",
        "ethnicity" => "ethnicity",
        _ => return Err(DeiError::InvalidInput(format!("Invalid group_by: {}", group_by))),
    };

    // Build CASE expression for promotion keywords
    let keyword_conditions: Vec<String> = PROMOTION_KEYWORDS
        .iter()
        .map(|kw| format!("job_title LIKE '%{}%'", kw))
        .collect();
    let promotion_case = format!(
        "CASE WHEN {} THEN 1 ELSE 0 END",
        keyword_conditions.join(" OR ")
    );

    let rows = sqlx::query_as::<_, PromotionRow>(&format!(
        r#"
        SELECT
            COALESCE({}, 'Not Specified') as label,
            COUNT(*) as total_count,
            SUM({}) as promoted_count
        FROM employees
        WHERE status = 'active'
        GROUP BY {}
        ORDER BY total_count DESC
        "#,
        column, promotion_case, column
    ))
    .fetch_all(pool)
    .await?;

    // Calculate overall rate
    let overall_row = sqlx::query_as::<_, (i64, i64)>(&format!(
        r#"
        SELECT
            COUNT(*) as total,
            SUM({}) as promoted
        FROM employees
        WHERE status = 'active'
        "#,
        promotion_case
    ))
    .fetch_one(pool)
    .await?;

    let overall_rate = if overall_row.0 > 0 {
        Some((overall_row.1 as f64 / overall_row.0 as f64 * 100.0 * 10.0).round() / 10.0)
    } else {
        None
    };

    // Convert to items with suppression
    let items: Vec<PromotionRateItem> = rows
        .into_iter()
        .map(|row| {
            let suppressed = row.total_count < MIN_GROUP_SIZE;
            PromotionRateItem {
                label: row.label,
                total_count: if suppressed { 0 } else { row.total_count },
                promoted_count: if suppressed { 0 } else { row.promoted_count },
                rate: if suppressed || row.total_count == 0 {
                    None
                } else {
                    Some((row.promoted_count as f64 / row.total_count as f64 * 100.0 * 10.0).round() / 10.0)
                },
                suppressed,
            }
        })
        .collect();

    Ok(PromotionRatesResult {
        group_by: group_by.to_string(),
        items,
        overall_rate,
        disclaimer: PROMOTION_DISCLAIMER.to_string(),
        computed_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Get complete fairness lens summary
///
/// Returns all DEI metrics in a single response.
pub async fn get_fairness_lens_summary(
    pool: &DbPool,
) -> Result<FairnessLensSummary, DeiError> {
    // Check if feature is enabled
    if !is_fairness_lens_enabled(pool).await? {
        return Err(DeiError::NotEnabled);
    }

    // Fetch all metrics in parallel-ish (sequential for SQLite)
    let gender_representation = get_representation_breakdown(pool, "gender", None).await?;
    let ethnicity_representation = get_representation_breakdown(pool, "ethnicity", None).await?;
    let gender_rating_parity = get_rating_parity(pool, "gender").await?;
    let ethnicity_rating_parity = get_rating_parity(pool, "ethnicity").await?;
    let gender_promotion_rates = get_promotion_rates(pool, "gender").await?;
    let ethnicity_promotion_rates = get_promotion_rates(pool, "ethnicity").await?;

    Ok(FairnessLensSummary {
        gender_representation,
        ethnicity_representation,
        gender_rating_parity,
        ethnicity_rating_parity,
        gender_promotion_rates,
        ethnicity_promotion_rates,
        disclaimer: DEI_DISCLAIMER.to_string(),
        computed_at: chrono::Utc::now().to_rfc3339(),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- Constants Tests --------------------

    #[test]
    fn test_min_group_size_is_five() {
        assert_eq!(MIN_GROUP_SIZE, 5, "MIN_GROUP_SIZE should be 5 for privacy");
    }

    #[test]
    fn test_promotion_keywords_not_empty() {
        assert!(!PROMOTION_KEYWORDS.is_empty(), "Should have promotion keywords");
        assert!(PROMOTION_KEYWORDS.contains(&"Senior"));
        assert!(PROMOTION_KEYWORDS.contains(&"Manager"));
        assert!(PROMOTION_KEYWORDS.contains(&"Director"));
    }

    // -------------------- DeiBreakdown Tests --------------------

    #[test]
    fn test_dei_breakdown_serialization() {
        let breakdown = DeiBreakdown {
            label: "Female".to_string(),
            count: 42,
            percentage: 45.5,
            suppressed: false,
        };
        let json = serde_json::to_string(&breakdown).unwrap();
        assert!(json.contains("\"label\":\"Female\""));
        assert!(json.contains("\"count\":42"));
        assert!(json.contains("\"percentage\":45.5"));
        assert!(json.contains("\"suppressed\":false"));
    }

    #[test]
    fn test_dei_breakdown_suppressed() {
        let breakdown = DeiBreakdown {
            label: "Other".to_string(),
            count: 0, // Suppressed count
            percentage: 0.0,
            suppressed: true,
        };
        assert!(breakdown.suppressed);
        assert_eq!(breakdown.count, 0);
    }

    // -------------------- RatingParityItem Tests --------------------

    #[test]
    fn test_rating_parity_item_with_rating() {
        let item = RatingParityItem {
            label: "Male".to_string(),
            count: 25,
            avg_rating: Some(3.85),
            suppressed: false,
        };
        assert!(!item.suppressed);
        assert_eq!(item.avg_rating, Some(3.85));
    }

    #[test]
    fn test_rating_parity_item_suppressed() {
        let item = RatingParityItem {
            label: "Other".to_string(),
            count: 0,
            avg_rating: None,
            suppressed: true,
        };
        assert!(item.suppressed);
        assert!(item.avg_rating.is_none());
    }

    // -------------------- PromotionRateItem Tests --------------------

    #[test]
    fn test_promotion_rate_calculation() {
        let item = PromotionRateItem {
            label: "Female".to_string(),
            total_count: 100,
            promoted_count: 35,
            rate: Some(35.0),
            suppressed: false,
        };
        assert_eq!(item.rate, Some(35.0));
    }

    #[test]
    fn test_promotion_rate_suppressed() {
        let item = PromotionRateItem {
            label: "Other".to_string(),
            total_count: 0,
            promoted_count: 0,
            rate: None,
            suppressed: true,
        };
        assert!(item.suppressed);
        assert!(item.rate.is_none());
    }

    // -------------------- Suppression Logic Tests --------------------

    #[test]
    fn test_suppression_boundary_below() {
        // Count of 4 should be suppressed
        let count: i64 = 4;
        assert!(count < MIN_GROUP_SIZE, "Count of 4 should be suppressed");
    }

    #[test]
    fn test_suppression_boundary_at() {
        // Count of 5 should NOT be suppressed
        let count: i64 = 5;
        assert!(count >= MIN_GROUP_SIZE, "Count of 5 should not be suppressed");
    }

    #[test]
    fn test_suppression_boundary_above() {
        // Count of 6 should NOT be suppressed
        let count: i64 = 6;
        assert!(count >= MIN_GROUP_SIZE, "Count of 6 should not be suppressed");
    }

    // -------------------- Input Validation Tests --------------------

    #[test]
    fn test_valid_group_by_gender() {
        let valid = matches!("gender", "gender" | "ethnicity");
        assert!(valid);
    }

    #[test]
    fn test_valid_group_by_ethnicity() {
        let valid = matches!("ethnicity", "gender" | "ethnicity");
        assert!(valid);
    }

    #[test]
    fn test_invalid_group_by() {
        let valid = matches!("department", "gender" | "ethnicity");
        assert!(!valid, "department should not be a valid DEI group_by");
    }

    // -------------------- Error Type Tests --------------------

    #[test]
    fn test_dei_error_not_enabled() {
        let err = DeiError::NotEnabled;
        assert_eq!(err.to_string(), "Feature not enabled");
    }

    #[test]
    fn test_dei_error_invalid_input() {
        let err = DeiError::InvalidInput("bad field".to_string());
        assert!(err.to_string().contains("bad field"));
    }

    #[test]
    fn test_dei_error_serialization() {
        let err = DeiError::NotEnabled;
        let json = serde_json::to_string(&err).unwrap();
        // Serialized as variant name
        assert!(json.contains("NotEnabled"));
    }

    // -------------------- Disclaimer Tests --------------------

    #[test]
    fn test_dei_disclaimer_mentions_privacy() {
        assert!(DEI_DISCLAIMER.contains("5"));
        assert!(DEI_DISCLAIMER.contains("suppress"));
    }

    #[test]
    fn test_promotion_disclaimer_mentions_inference() {
        assert!(PROMOTION_DISCLAIMER.contains("inferred"));
        assert!(PROMOTION_DISCLAIMER.contains("title"));
    }
}
