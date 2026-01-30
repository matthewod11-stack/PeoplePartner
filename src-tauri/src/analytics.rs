// HR Command Center - Analytics Module
// V2.3.2: Provides structured chart generation from natural language queries
//
// Architecture:
// 1. User asks "Show me headcount by department"
// 2. Keyword detection identifies chart request
// 3. Claude emits AnalyticsRequest JSON
// 4. Template matcher selects whitelisted SQL
// 5. Execute SQL, return ChartData
// 6. Frontend renders with Recharts

use serde::{Deserialize, Serialize};

// =============================================================================
// Chart Intent - What the user wants to visualize
// =============================================================================

/// Intent types that map to whitelisted SQL templates.
/// Each intent corresponds to a specific analytical question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartIntent {
    /// Headcount breakdown by a grouping dimension
    HeadcountBy,
    /// Performance rating distribution
    RatingDistribution,
    /// eNPS score breakdown (Promoters/Passives/Detractors)
    EnpsBreakdown,
    /// Attrition/turnover analysis
    AttritionAnalysis,
    /// Tenure distribution buckets
    TenureDistribution,
}

impl ChartIntent {
    /// Returns a human-readable description of the intent
    pub fn description(&self) -> &'static str {
        match self {
            ChartIntent::HeadcountBy => "Employee headcount breakdown",
            ChartIntent::RatingDistribution => "Performance rating distribution",
            ChartIntent::EnpsBreakdown => "Employee Net Promoter Score breakdown",
            ChartIntent::AttritionAnalysis => "Attrition and turnover analysis",
            ChartIntent::TenureDistribution => "Employee tenure distribution",
        }
    }
}

// =============================================================================
// Group By - How to segment the data
// =============================================================================

/// Grouping dimension for chart data.
/// Determines how data points are categorized on the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// Group by department (Engineering, Sales, etc.)
    Department,
    /// Group by employment status (active, terminated, leave)
    Status,
    /// Group by gender
    Gender,
    /// Group by ethnicity
    Ethnicity,
    /// Group by work state (CA, NY, TX, etc.)
    WorkState,
    /// Group by tenure bucket (<1yr, 1-3yr, 3-5yr, 5+yr)
    TenureBucket,
    /// Group by rating bucket (Exceptional, Exceeds, Meets, Needs Improvement)
    RatingBucket,
    /// Group by calendar quarter (for time series)
    Quarter,
}

impl GroupBy {
    /// Returns a human-readable label for this grouping
    pub fn label(&self) -> &'static str {
        match self {
            GroupBy::Department => "Department",
            GroupBy::Status => "Status",
            GroupBy::Gender => "Gender",
            GroupBy::Ethnicity => "Ethnicity",
            GroupBy::WorkState => "Work State",
            GroupBy::TenureBucket => "Tenure",
            GroupBy::RatingBucket => "Rating",
            GroupBy::Quarter => "Quarter",
        }
    }
}

// =============================================================================
// Chart Type - Visual representation
// =============================================================================

/// Chart type for rendering.
/// The template system recommends chart types, but can be overridden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    /// Vertical bar chart - good for comparisons
    Bar,
    /// Pie chart - good for proportions
    Pie,
    /// Line chart - good for time series
    Line,
    /// Horizontal bar chart - good for long labels
    HorizontalBar,
}

// =============================================================================
// Filters - Constraints on the data
// =============================================================================

/// Filter constraints for analytics queries.
/// All filters are optional and combined with AND logic.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChartFilters {
    /// Filter to specific departments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departments: Option<Vec<String>>,

    /// Filter to specific statuses (active, terminated, leave)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statuses: Option<Vec<String>>,

    /// Filter to date range start (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_from: Option<String>,

    /// Filter to date range end (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_to: Option<String>,

    /// Filter to specific gender
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,

    /// Filter to specific ethnicity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethnicity: Option<String>,
}

impl ChartFilters {
    /// Returns true if no filters are applied
    pub fn is_empty(&self) -> bool {
        self.departments.is_none()
            && self.statuses.is_none()
            && self.date_from.is_none()
            && self.date_to.is_none()
            && self.gender.is_none()
            && self.ethnicity.is_none()
    }

    /// Returns a human-readable description of applied filters
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        if let Some(depts) = &self.departments {
            if depts.len() == 1 {
                parts.push(format!("{} department", depts[0]));
            } else {
                parts.push(format!("{} departments", depts.len()));
            }
        }

        if let Some(statuses) = &self.statuses {
            parts.push(statuses.join(", "));
        }

        if let Some(from) = &self.date_from {
            if let Some(to) = &self.date_to {
                parts.push(format!("{} to {}", from, to));
            } else {
                parts.push(format!("from {}", from));
            }
        } else if let Some(to) = &self.date_to {
            parts.push(format!("until {}", to));
        }

        if let Some(gender) = &self.gender {
            parts.push(format!("{} employees", gender));
        }

        if let Some(ethnicity) = &self.ethnicity {
            parts.push(ethnicity.clone());
        }

        if parts.is_empty() {
            "None".to_string()
        } else {
            parts.join(", ")
        }
    }
}

// =============================================================================
// Analytics Request - What Claude emits
// =============================================================================

/// The analytics request that Claude emits in response to chart queries.
/// This is parsed from Claude's response and used to select SQL templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRequest {
    /// What the user wants to visualize
    pub intent: ChartIntent,

    /// How to group the data
    pub group_by: GroupBy,

    /// Optional filters to apply
    #[serde(default)]
    pub filters: ChartFilters,

    /// Suggested chart type (Claude can recommend, but we may override)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_chart: Option<ChartType>,

    /// Natural language description of what's being shown
    pub description: String,
}

// =============================================================================
// Chart Data - What we return to frontend
// =============================================================================

/// A single data point in a chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    /// Label for this data point (e.g., "Engineering", "Q1 2025")
    pub label: String,

    /// Primary value (count, percentage, etc.)
    pub value: f64,

    /// Optional percentage (pre-calculated for convenience)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
}

/// Complete chart data returned to frontend for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    /// The chart type to render
    pub chart_type: ChartType,

    /// Data points for the chart
    pub data: Vec<ChartDataPoint>,

    /// Chart title
    pub title: String,

    /// Human-readable filter description
    pub filters_applied: String,

    /// Total count (for percentage calculations in frontend)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,

    /// X-axis label (for bar/line charts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_label: Option<String>,

    /// Y-axis label (for bar/line charts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_label: Option<String>,
}

// =============================================================================
// Chart Result - Outcome of analytics execution
// =============================================================================

/// Result of attempting to generate a chart.
/// Enables graceful fallback when chart generation fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChartResult {
    /// Successfully generated chart data
    Success {
        data: ChartData,
    },

    /// Chart not possible, include text fallback
    Fallback {
        reason: String,
        text_response: String,
    },

    /// Request was not identified as a chart query
    NotChartQuery,
}

// =============================================================================
// Response Parsing
// =============================================================================

/// Markers used to identify analytics request in Claude's response
const ANALYTICS_START_MARKER: &str = "<analytics_request>";
const ANALYTICS_END_MARKER: &str = "</analytics_request>";

/// Extracts an AnalyticsRequest from Claude's response text.
/// Returns None if no valid request is found.
pub fn extract_analytics_request(response: &str) -> Option<AnalyticsRequest> {
    let start = response.find(ANALYTICS_START_MARKER)?;
    let end = response.find(ANALYTICS_END_MARKER)?;

    if end <= start {
        return None;
    }

    let json_start = start + ANALYTICS_START_MARKER.len();
    let json_str = response[json_start..end].trim();

    serde_json::from_str(json_str).ok()
}

/// Removes the analytics request block from response text.
/// Returns the text without the JSON block for display.
pub fn strip_analytics_block(response: &str) -> String {
    let Some(start) = response.find(ANALYTICS_START_MARKER) else {
        return response.to_string();
    };

    let Some(end) = response.find(ANALYTICS_END_MARKER) else {
        return response.to_string();
    };

    if end <= start {
        return response.to_string();
    }

    let before = &response[..start];
    let after = &response[end + ANALYTICS_END_MARKER.len()..];

    format!("{}{}", before.trim_end(), after.trim_start())
}

// =============================================================================
// Chart Keyword Detection
// =============================================================================

/// Keywords that indicate user wants a chart/visualization
const CHART_KEYWORDS: &[&str] = &[
    // Explicit chart requests
    "show chart",
    "show me a chart",
    "show a chart",
    "create a chart",
    "make a chart",
    "generate a chart",
    "in chart form",
    "as a chart",
    "chart form",
    // Visualization terms
    "visualize",
    "visualization",
    "visual breakdown",
    "visual representation",
    // Chart types
    "pie chart",
    "bar chart",
    "line chart",
    "bar graph",
    "pie graph",
    "graph",
    "plot",
    "diagram",
    // Distribution/breakdown with "show"
    "show me headcount",
    "show headcount",
    "show breakdown",
    "show distribution",
    "show me breakdown",
    "show me distribution",
    // Analytics patterns
    "chart of",
    "breakdown chart",
    "distribution chart",
    "headcount by",
    "breakdown by",
    "employees by",
    // eNPS specific
    "enps chart",
    "enps breakdown",
    "enps distribution",
];

/// Checks if a query is requesting a chart/visualization.
pub fn is_chart_query(query: &str) -> bool {
    let query_lower = query.to_lowercase();
    CHART_KEYWORDS.iter().any(|kw| query_lower.contains(kw))
}

/// Returns the chart keywords found in a query.
pub fn find_chart_keywords(query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    CHART_KEYWORDS
        .iter()
        .filter(|kw| query_lower.contains(*kw))
        .map(|s| s.to_string())
        .collect()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_keyword_detection() {
        // Explicit chart requests
        assert!(is_chart_query("Show me a chart of headcount"));
        assert!(is_chart_query("Visualize our department breakdown"));
        assert!(is_chart_query("Create a pie chart of gender"));
        assert!(is_chart_query("GRAPH the turnover rate"));
        // New patterns
        assert!(is_chart_query("show me headcount by department"));
        assert!(is_chart_query("headcount by department"));
        assert!(is_chart_query("employees by status"));
        assert!(is_chart_query("pie chart"));
        // User-reported queries that should work
        assert!(is_chart_query("show me our team enps in chart form"));
        assert!(is_chart_query("show me our eNPS breakdown"));
        assert!(is_chart_query("make it a bar graph"));
        // Negative cases
        assert!(!is_chart_query("How many employees do we have?"));
        assert!(!is_chart_query("Tell me about the engineering team"));
        assert!(!is_chart_query("What's the headcount?")); // No "by" pattern
    }

    #[test]
    fn test_extract_analytics_request() {
        let response = r#"Here's what I found:

<analytics_request>
{
  "intent": "headcount_by",
  "group_by": "department",
  "filters": {},
  "description": "Employee headcount by department"
}
</analytics_request>

The engineering team is the largest."#;

        let request = extract_analytics_request(response).unwrap();
        assert_eq!(request.intent, ChartIntent::HeadcountBy);
        assert_eq!(request.group_by, GroupBy::Department);
        assert!(request.filters.is_empty());
    }

    #[test]
    fn test_extract_analytics_request_not_found() {
        let response = "Here's what I found about the engineering team.";
        assert!(extract_analytics_request(response).is_none());
    }

    #[test]
    fn test_strip_analytics_block() {
        let response = r#"Here's the breakdown:

<analytics_request>
{"intent": "headcount_by", "group_by": "department", "filters": {}, "description": "test"}
</analytics_request>

Let me know if you need more details."#;

        let stripped = strip_analytics_block(response);
        assert!(!stripped.contains("<analytics_request>"));
        assert!(!stripped.contains("</analytics_request>"));
        assert!(stripped.contains("Here's the breakdown:"));
        assert!(stripped.contains("Let me know if you need more details."));
    }

    #[test]
    fn test_filters_describe() {
        let empty = ChartFilters::default();
        assert_eq!(empty.describe(), "None");

        let with_dept = ChartFilters {
            departments: Some(vec!["Engineering".to_string()]),
            ..Default::default()
        };
        assert_eq!(with_dept.describe(), "Engineering department");

        let with_status = ChartFilters {
            statuses: Some(vec!["active".to_string()]),
            ..Default::default()
        };
        assert_eq!(with_status.describe(), "active");
    }

    #[test]
    fn test_chart_type_serialization() {
        let pie = ChartType::Pie;
        let json = serde_json::to_string(&pie).unwrap();
        assert_eq!(json, "\"pie\"");

        let parsed: ChartType = serde_json::from_str("\"bar\"").unwrap();
        assert_eq!(parsed, ChartType::Bar);
    }
}
