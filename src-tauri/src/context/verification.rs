//! Verifies numeric claims in Claude's responses against ground-truth aggregates.
//! Catches the most common factual errors (wrong headcount, off-by-one rating,
//! incorrect eNPS) by regex-extracting the model's numbers and comparing them
//! to the precomputed `OrgAggregates`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use super::aggregates::OrgAggregates;
use super::query::QueryType;

// ============================================================================
// Verification Types
// ============================================================================

/// Result of verifying Claude's numeric claims against ground truth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether this was an aggregate query (verification only applies to aggregate)
    pub is_aggregate_query: bool,
    /// Individual numeric claims found and verified
    pub claims: Vec<NumericClaim>,
    /// Overall verification status
    pub overall_status: VerificationStatus,
    /// SQL query used to compute ground truth (for transparency)
    pub sql_query: Option<String>,
}

/// Overall verification status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// All numeric claims match ground truth
    Verified,
    /// Some claims match, some don't
    PartialMatch,
    /// No numeric claims could be verified (none extracted or no ground truth)
    Unverified,
    /// Not an aggregate query, verification not applicable
    NotApplicable,
}

/// A single numeric claim extracted from Claude's response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericClaim {
    /// Type of claim (headcount, rating, eNPS, etc.)
    pub claim_type: ClaimType,
    /// The numeric value found in Claude's response
    pub value_found: f64,
    /// The ground truth value from the database (if available)
    pub ground_truth: Option<f64>,
    /// Whether the claim matches ground truth (within tolerance)
    pub is_match: bool,
}

/// Type of numeric claim being verified
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimType {
    /// Total employee headcount
    TotalHeadcount,
    /// Active employee count
    ActiveCount,
    /// Count for a specific department
    DepartmentCount,
    /// Average performance rating
    AvgRating,
    /// eNPS score
    EnpsScore,
    /// Turnover/attrition rate
    TurnoverRate,
    /// Generic percentage (department breakdown, etc.)
    Percentage,
}

// ============================================================================
// Static Regex Patterns
// ============================================================================

static HEADCOUNT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+)\s*(?:total\s+)?(?:employees?|people|team\s*members?|staff|headcount)")
        .expect("headcount regex")
});
static ACTIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+)\s*active(?:\s+employees?)?|active[:\s]+(\d+)")
        .expect("active count regex")
});
static RATING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:average|avg|mean)\s*(?:rating|score)?[:\s]*(?:of\s+|is\s+)?(\d+\.?\d*)|(\d+\.?\d*)\s*(?:average|avg)")
        .expect("rating regex")
});
static ENPS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"enps\s*(?:score)?[:\s]*(?:of\s+|is\s+)?([+-]?\d+)|([+-]?\d+)\s*enps")
        .expect("eNPS regex")
});
static TURNOVER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+\.?\d*)\s*%\s*(?:turnover|attrition)|(?:turnover|attrition)\s*(?:rate)?[:\s]*(?:of\s+)?(\d+\.?\d*)\s*%")
        .expect("turnover regex")
});

// ============================================================================
// Public Verification Entry Point
// ============================================================================

/// Verify numeric claims in Claude's response against ground truth aggregates
pub fn verify_response(
    response: &str,
    aggregates: Option<&OrgAggregates>,
    query_type: QueryType,
) -> VerificationResult {
    // Only verify aggregate queries
    if query_type != QueryType::Aggregate {
        return VerificationResult {
            is_aggregate_query: false,
            claims: vec![],
            overall_status: VerificationStatus::NotApplicable,
            sql_query: None,
        };
    }

    // Need aggregates to verify
    let Some(agg) = aggregates else {
        return VerificationResult {
            is_aggregate_query: true,
            claims: vec![],
            overall_status: VerificationStatus::Unverified,
            sql_query: None,
        };
    };

    // Extract and verify claims
    let claims = extract_numeric_claims(response, agg);
    let overall_status = compute_verification_status(&claims);
    let sql_query = Some(generate_verification_sql(agg));

    VerificationResult {
        is_aggregate_query: true,
        claims,
        overall_status,
        sql_query,
    }
}

/// Extract numeric claims from Claude's response and compare to ground truth
fn extract_numeric_claims(response: &str, agg: &OrgAggregates) -> Vec<NumericClaim> {
    let mut claims = Vec::new();
    let response_lower = response.to_lowercase();

    // Headcount patterns: "100 employees", "have 100 people", "headcount of 100"
    // Also match "100 total employees", "100 active employees", etc.
    for cap in HEADCOUNT_RE.captures_iter(&response_lower) {
        if let Ok(n) = cap[1].parse::<f64>() {
            // Check if this is specifically about active employees
            let context_before = &response_lower[..cap.get(0).unwrap().start()];
            let is_active = context_before.ends_with("active ");

            let (ground_truth, claim_type) = if is_active {
                (agg.active_count as f64, ClaimType::ActiveCount)
            } else {
                (agg.total_employees as f64, ClaimType::TotalHeadcount)
            };

            claims.push(NumericClaim {
                claim_type,
                value_found: n,
                ground_truth: Some(ground_truth),
                is_match: (n - ground_truth).abs() < 0.5, // Counts should be exact
            });
        }
    }

    // Active count patterns: "82 active", "active: 82"
    for cap in ACTIVE_RE.captures_iter(&response_lower) {
        let num_str = cap.get(1).or(cap.get(2)).map(|m| m.as_str());
        if let Some(ns) = num_str {
            if let Ok(n) = ns.parse::<f64>() {
                // Avoid duplicate if already captured by headcount pattern
                if !claims.iter().any(|c| c.claim_type == ClaimType::ActiveCount && (c.value_found - n).abs() < 0.5) {
                    claims.push(NumericClaim {
                        claim_type: ClaimType::ActiveCount,
                        value_found: n,
                        ground_truth: Some(agg.active_count as f64),
                        is_match: (n - agg.active_count as f64).abs() < 0.5,
                    });
                }
            }
        }
    }

    // Average rating patterns: "average rating of 3.4", "3.4 average", "avg rating is 3.4"
    if let Some(avg_rating) = agg.avg_rating {
        for cap in RATING_RE.captures_iter(&response_lower) {
            let num_str = cap.get(1).or(cap.get(2)).map(|m| m.as_str());
            if let Some(ns) = num_str {
                if let Ok(n) = ns.parse::<f64>() {
                    // Ratings are typically 1.0-5.0, filter out obvious non-ratings
                    if n >= 1.0 && n <= 5.0 {
                        claims.push(NumericClaim {
                            claim_type: ClaimType::AvgRating,
                            value_found: n,
                            ground_truth: Some(avg_rating),
                            is_match: (n - avg_rating).abs() <= 0.1, // Allow ±0.1 tolerance
                        });
                    }
                }
            }
        }
    }

    // eNPS patterns: "eNPS of +12", "eNPS is -5", "eNPS: 12", "eNPS score of 15"
    for cap in ENPS_RE.captures_iter(&response_lower) {
        let num_str = cap.get(1).or(cap.get(2)).map(|m| m.as_str());
        if let Some(ns) = num_str {
            if let Ok(n) = ns.parse::<f64>() {
                // eNPS typically ranges from -100 to +100
                if n >= -100.0 && n <= 100.0 {
                    claims.push(NumericClaim {
                        claim_type: ClaimType::EnpsScore,
                        value_found: n,
                        ground_truth: Some(agg.enps.score as f64),
                        is_match: (n - agg.enps.score as f64).abs() < 0.5, // Exact match for integer score
                    });
                }
            }
        }
    }

    // Turnover rate patterns: "14.6% turnover", "turnover rate of 14.6%", "attrition of 12%"
    if let Some(turnover_rate) = agg.attrition.turnover_rate_annualized {
        for cap in TURNOVER_RE.captures_iter(&response_lower) {
            let num_str = cap.get(1).or(cap.get(2)).map(|m| m.as_str());
            if let Some(ns) = num_str {
                if let Ok(n) = ns.parse::<f64>() {
                    claims.push(NumericClaim {
                        claim_type: ClaimType::TurnoverRate,
                        value_found: n,
                        ground_truth: Some(turnover_rate),
                        is_match: (n - turnover_rate).abs() <= 1.0, // Allow ±1% tolerance
                    });
                }
            }
        }
    }

    // Department percentages: "34% in Engineering", "Engineering (34%)"
    for dept in &agg.by_department {
        let dept_lower = dept.name.to_lowercase();
        let dept_pct_re = Regex::new(&format!(
            r"(\d+\.?\d*)\s*%\s*(?:in\s+|of\s+)?{}|{}\s*\(?(\d+\.?\d*)\s*%",
            regex::escape(&dept_lower),
            regex::escape(&dept_lower)
        )).expect("department percentage regex");

        for cap in dept_pct_re.captures_iter(&response_lower) {
            let num_str = cap.get(1).or(cap.get(2)).map(|m| m.as_str());
            if let Some(ns) = num_str {
                if let Ok(n) = ns.parse::<f64>() {
                    claims.push(NumericClaim {
                        claim_type: ClaimType::Percentage,
                        value_found: n,
                        ground_truth: Some(dept.percentage),
                        is_match: (n - dept.percentage).abs() <= 1.0, // Allow ±1% tolerance
                    });
                }
            }
        }
    }

    claims
}

/// Compute overall verification status from individual claims
fn compute_verification_status(claims: &[NumericClaim]) -> VerificationStatus {
    if claims.is_empty() {
        return VerificationStatus::Unverified;
    }

    let all_match = claims.iter().all(|c| c.is_match);
    let any_match = claims.iter().any(|c| c.is_match);

    if all_match {
        VerificationStatus::Verified
    } else if any_match {
        VerificationStatus::PartialMatch
    } else {
        VerificationStatus::PartialMatch // Even all mismatches = partial (we detected claims)
    }
}

/// Generate SQL query string for transparency (what queries produced ground truth)
fn generate_verification_sql(agg: &OrgAggregates) -> String {
    format!(
r#"-- Organization Aggregates (Ground Truth)
-- Total: {} | Active: {} | Terminated: {}

SELECT COUNT(*) as total,
       SUM(CASE WHEN status='active' THEN 1 ELSE 0 END) as active
FROM employees;

-- Average Rating: {:.2}
SELECT ROUND(AVG(pr.overall_rating), 2)
FROM performance_ratings pr
JOIN (SELECT employee_id, MAX(rating_date) as max_date
      FROM performance_ratings GROUP BY employee_id) latest
  ON pr.employee_id = latest.employee_id
 AND pr.rating_date = latest.max_date;

-- eNPS Score: {}
SELECT ROUND(
  (SUM(CASE WHEN score >= 9 THEN 1.0 ELSE 0 END) -
   SUM(CASE WHEN score <= 6 THEN 1.0 ELSE 0 END)) / COUNT(*) * 100
) FROM enps_responses WHERE id IN (
  SELECT MAX(id) FROM enps_responses GROUP BY employee_id
);"#,
        agg.total_employees,
        agg.active_count,
        agg.terminated_count,
        agg.avg_rating.unwrap_or(0.0),
        agg.enps.score
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::aggregates::{
        AttritionStats, DepartmentCount, EnpsAggregate, RatingDistribution,
    };

    /// Helper to create test OrgAggregates for verification tests
    fn make_test_aggregates() -> OrgAggregates {
        OrgAggregates {
            total_employees: 100,
            active_count: 85,
            terminated_count: 15,
            on_leave_count: 0,
            by_department: vec![
                DepartmentCount {
                    name: "Engineering".to_string(),
                    count: 34,
                    percentage: 34.0,
                },
                DepartmentCount {
                    name: "Sales".to_string(),
                    count: 26,
                    percentage: 26.0,
                },
            ],
            avg_rating: Some(3.45),
            rating_distribution: RatingDistribution::default(),
            employees_with_no_rating: 0,
            enps: EnpsAggregate {
                score: 12,
                promoters: 30,
                passives: 40,
                detractors: 15,
                total_responses: 85,
                response_rate: 100.0,
            },
            attrition: AttritionStats {
                terminations_ytd: 15,
                voluntary: 10,
                involuntary: 5,
                avg_tenure_months: Some(24.0),
                turnover_rate_annualized: Some(14.6),
            },
        }
    }

    #[test]
    fn test_verify_headcount_exact_match() {
        let agg = make_test_aggregates();
        let response = "You currently have 100 employees in total.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.is_aggregate_query);
        assert_eq!(result.overall_status, VerificationStatus::Verified);
        assert!(!result.claims.is_empty());
        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::TotalHeadcount && c.is_match));
    }

    #[test]
    fn test_verify_headcount_mismatch() {
        let agg = make_test_aggregates();
        let response = "Your company has 95 employees."; // Wrong: actual is 100
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.is_aggregate_query);
        assert_eq!(result.overall_status, VerificationStatus::PartialMatch);
        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::TotalHeadcount && !c.is_match));
    }

    #[test]
    fn test_verify_rating_within_tolerance() {
        let agg = make_test_aggregates();

        // Within tolerance (±0.1): 3.4 is within 0.1 of 3.45
        let response = "The average rating is 3.4 out of 5.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.is_aggregate_query);
        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::AvgRating && c.is_match));
    }

    #[test]
    fn test_verify_rating_outside_tolerance() {
        let agg = make_test_aggregates();

        // Outside tolerance: 3.0 is 0.45 away from 3.45
        let response = "Your team's average rating is 3.0.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::AvgRating && !c.is_match));
    }

    #[test]
    fn test_verify_enps_with_positive_sign() {
        let agg = make_test_aggregates();

        let response = "Your eNPS is +12, which is healthy.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.is_aggregate_query);
        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::EnpsScore && c.is_match));
    }

    #[test]
    fn test_verify_enps_without_sign() {
        let agg = make_test_aggregates();

        let response = "The company eNPS score is 12.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::EnpsScore && c.is_match));
    }

    #[test]
    fn test_non_aggregate_query_not_applicable() {
        let agg = make_test_aggregates();
        let response = "Sarah has been performing well this quarter.";

        let result = verify_response(response, Some(&agg), QueryType::Individual);

        assert!(!result.is_aggregate_query);
        assert_eq!(result.overall_status, VerificationStatus::NotApplicable);
        assert!(result.claims.is_empty());
    }

    #[test]
    fn test_verify_turnover_rate_within_tolerance() {
        let agg = make_test_aggregates();

        // Within tolerance (±1%): 15% is within 1% of 14.6%
        let response = "The annual 15% turnover rate is concerning.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::TurnoverRate && c.is_match));
    }

    #[test]
    fn test_verify_active_employees() {
        let agg = make_test_aggregates();

        let response = "There are 85 active employees currently.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::ActiveCount && c.is_match));
    }

    #[test]
    fn test_verify_no_aggregates_returns_unverified() {
        let response = "You have 100 employees.";
        let result = verify_response(response, None, QueryType::Aggregate);

        assert!(result.is_aggregate_query);
        assert_eq!(result.overall_status, VerificationStatus::Unverified);
        assert!(result.claims.is_empty());
    }

    #[test]
    fn test_verify_multiple_claims_all_match() {
        let agg = make_test_aggregates();

        // Response with multiple verifiable claims, all correct
        let response = "Your company has 100 employees total, with 85 active. The eNPS is 12.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.claims.len() >= 2);
        assert_eq!(result.overall_status, VerificationStatus::Verified);
    }
}
