//! Serde data types for the intake engine. Field names are snake_case in Rust,
//! camelCase on the wire (`#[serde(rename_all = "camelCase")]`) to match the
//! Sourcerer TS shape and the future UI. Ported from
//! `~/Projects/Sourcerer/packages/core/src/intake.ts`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole { System, User, Assistant }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleParameters {
    pub title: String,
    pub level: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub remote_policy: Option<RemotePolicy>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub compensation_range: Option<CompensationRange>,
    #[serde(default)]
    pub must_have_skills: Vec<String>,
    #[serde(default)]
    pub nice_to_have_skills: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub team_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reporting_to: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemotePolicy { Remote, Hybrid, InPerson, Negotiable }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompensationRange {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max: Option<f64>,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyIntel {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub tech_stack: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub team_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub funding_stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub product_category: Option<String>,
    #[serde(default)]
    pub culture_signals: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pitch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub competitors: Option<Vec<String>>,
    pub analyzed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerStep {
    pub company: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileInput {
    GithubUrl { url: String },
    LinkedinUrl { url: String },
    PastedText { text: String },
    NameCompany { name: String, company: String },
    PersonalUrl { url: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAnalysis {
    pub input_type: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(default)]
    pub career_trajectory: Vec<CareerStep>,
    #[serde(default)]
    pub skill_signatures: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub seniority_level: Option<String>,
    #[serde(default)]
    pub culture_signals: Vec<String>,
    #[serde(default)]
    pub urls: Vec<String>,
    pub analyzed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeProfile {
    #[serde(default)]
    pub career_trajectories: Vec<Vec<CareerStep>>,
    #[serde(default)]
    pub skill_signatures: Vec<String>,
    #[serde(default)]
    pub seniority_calibration: String,
    #[serde(default)]
    pub culture_signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetitorMap {
    #[serde(default)]
    pub target_companies: Vec<String>,
    #[serde(default)]
    pub avoid_companies: Vec<String>,
    #[serde(default)]
    pub competitor_reason: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawledContent {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub title: Option<String>,
    pub text: String,
    pub crawled_at: String,
    pub adapter: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarResult {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub title: Option<String>,
    pub similarity: f32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TalentProfile {
    pub role: RoleParameters,
    pub company: CompanyIntel,
    pub success_patterns: CompositeProfile,
    #[serde(default)]
    pub anti_patterns: Vec<String>,
    pub competitor_map: CompetitorMap,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_companies: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub include_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exclude_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_results: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchQueryTier {
    pub priority: u8,
    pub queries: Vec<SearchQuery>,
}

pub type ScoringWeights = std::collections::HashMap<String, f64>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierThresholds {
    pub tier1_min_score: u32,
    pub tier2_min_score: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentPriority {
    pub adapter: String,
    pub required: bool,
    pub run_condition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntiFilter {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchConfig {
    pub role_name: String,
    pub tiers: Vec<SearchQueryTier>,
    pub scoring_weights: ScoringWeights,
    pub tier_thresholds: TierThresholds,
    pub enrichment_priority: Vec<EnrichmentPriority>,
    pub anti_filters: Vec<AntiFilter>,
    #[serde(default)]
    pub similarity_seeds: Vec<String>,
    pub max_candidates: u32,
    pub max_cost_usd: f64,
    pub created_at: String,
    pub version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_parameters_round_trips_camelcase() {
        let json = r#"{"title":"Staff Engineer","level":"staff","scope":"owns infra",
            "mustHaveSkills":["rust"],"niceToHaveSkills":[]}"#;
        let role: RoleParameters = serde_json::from_str(json).expect("deserialize");
        assert_eq!(role.title, "Staff Engineer");
        assert_eq!(role.must_have_skills, vec!["rust".to_string()]);
        let back = serde_json::to_value(&role).unwrap();
        assert!(back.get("mustHaveSkills").is_some());
        assert!(back.get("must_have_skills").is_none());
    }

    #[test]
    fn profile_input_is_tagged_by_type() {
        let p = ProfileInput::GithubUrl { url: "https://github.com/x".into() };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["type"], "github_url");
        assert_eq!(v["url"], "https://github.com/x");
    }
}
