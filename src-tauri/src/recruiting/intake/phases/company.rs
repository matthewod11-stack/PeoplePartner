//! Phase 2 — Company intel. Port of company-intel.ts. Smell-fix: the crawl
//! runs in `company_url_input.parse` (it has the URL) and lands in
//! `pending_company_intel`; `company_analysis` displays it read-only and
//! promotes it on confirm. No mutable closure / no `__INTEL__` string hack.

use async_trait::async_trait;
use crate::recruiting::intake::context::{has_company_data, IntakeContext, IntakeContextUpdates};
use crate::recruiting::intake::deps::{IntakeDeps, IntakeError};
use crate::recruiting::intake::node::{IntakeNode, NodeId, ParsedResponse, Phase};
use crate::recruiting::intake::schemas::{CompanyIntel, CompetitorMap, Message, MessageRole};

const COMPANY_INTEL_SYSTEM: &str = r#"You are a company analyst. Based on the user's input, construct or update the company intelligence. Return a JSON object with:
- name: company name
- techStack: array of technologies
- teamSize: estimated team size
- fundingStage: funding stage if known
- productCategory: what they build
- cultureSignals: array of culture indicators
- pitch: one-sentence value prop
- competitors: array of competitor names

If information is not available, omit the field. Be thorough in extracting details."#;

const COMPETITOR_SYSTEM: &str = r#"Parse the user's competitor/company preferences. Return a JSON object with:
- targetCompanies: array of companies to source from
- avoidCompanies: array of companies to avoid
- competitorReason: object mapping company name to reason (e.g., "Stripe": "similar fintech infra")"#;

const CONFIRM_PHRASES: &[&str] = &["yes", "looks good", "correct", "confirmed", "proceed", "lgtm", "looks right", "accurate"];
const CONFIRM_OR_SKIP_PHRASES: &[&str] = &["looks good", "proceed", "skip", "none", "no", "n/a"];

fn matches_any(resp: &str, phrases: &[&str]) -> bool {
    let n = resp.trim().to_lowercase();
    phrases.iter().any(|p| n.contains(p)) || n == "y"
}

fn extract_url(resp: &str) -> String {
    resp.split_whitespace()
        .find(|t| t.starts_with("http://") || t.starts_with("https://"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| resp.trim().to_string())
}

fn user_msg(c: &str) -> Message { Message { role: MessageRole::User, content: c.into() } }
fn system_msg(c: String) -> Message { Message { role: MessageRole::System, content: c } }

pub struct CompanyUrlInput;

#[async_trait]
impl IntakeNode for CompanyUrlInput {
    fn id(&self) -> NodeId { NodeId::CompanyUrlInput }
    fn phase(&self) -> Phase { Phase::Company }
    fn skip_if(&self, ctx: &IntakeContext) -> bool { has_company_data(ctx) }

    async fn prompt(&self, ctx: &IntakeContext) -> String {
        if let Some(url) = &ctx.company_url {
            format!("I have the company URL: {}. Would you like me to analyze it, or provide a different URL?", url)
        } else {
            "Now let's learn about the company.\n\nPlease share the company's website URL. I'll analyze it to understand the tech stack, culture, and product.".into()
        }
    }

    async fn parse(&self, response: &str, _ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError>
    {
        let url = extract_url(response);
        // Smell-fix: crawl here (we have the URL), stash in pending_company_intel.
        let content = deps.research.crawl_url(&url).await?;
        let mut intel = deps.research.analyze_company(&content).await?;
        intel.url = url.clone();
        intel.analyzed_at = deps.clock.now_iso8601();
        Ok(ParsedResponse {
            context_updates: IntakeContextUpdates {
                company_url: Some(url),
                pending_company_intel: Some(intel),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::CompanyAnalysis }
}

pub struct CompanyAnalysis;

#[async_trait]
impl IntakeNode for CompanyAnalysis {
    fn id(&self) -> NodeId { NodeId::CompanyAnalysis }
    fn phase(&self) -> Phase { Phase::Company }

    async fn prompt(&self, ctx: &IntakeContext) -> String {
        let intel = match ctx.pending_company_intel.as_ref().or(ctx.company_intel.as_ref()) {
            Some(i) => i,
            None => return "I need a company URL to analyze. Could you provide one?".into(),
        };

        let mut lines = vec![
            format!("I've analyzed **{}**:\n", intel.name),
            format!("**Product:** {}", intel.product_category.as_deref().unwrap_or("Unknown")),
        ];

        if let Some(pitch) = &intel.pitch {
            lines.push(format!("**Pitch:** {}", pitch));
        }
        if !intel.tech_stack.is_empty() {
            lines.push(format!("**Tech stack:** {}", intel.tech_stack.join(", ")));
        }
        if let Some(ts) = &intel.team_size {
            lines.push(format!("**Team size:** {}", ts));
        }
        if let Some(fs) = &intel.funding_stage {
            lines.push(format!("**Funding:** {}", fs));
        }
        if !intel.culture_signals.is_empty() {
            lines.push(format!("**Culture:** {}", intel.culture_signals.join(", ")));
        }
        if let Some(competitors) = &intel.competitors {
            if !competitors.is_empty() {
                lines.push(format!("**Competitors:** {}", competitors.join(", ")));
            }
        }

        lines.push("\nDoes this look accurate? Any corrections or additions?".into());
        lines.join("\n")
    }

    async fn parse(&self, response: &str, ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError>
    {
        let confirmed = matches_any(response, CONFIRM_PHRASES);
        if confirmed {
            if let Some(intel) = ctx.pending_company_intel.clone().or_else(|| ctx.company_intel.clone()) {
                return Ok(ParsedResponse {
                    structured: serde_json::json!({ "confirmed": true }),
                    context_updates: IntakeContextUpdates { company_intel: Some(intel), ..Default::default() },
                    ..Default::default()
                });
            }
        }

        // Not confirmed (or nothing pending): LLM build/correct from the user's text.
        let system = match &ctx.company_intel {
            Some(existing) => format!("{COMPANY_INTEL_SYSTEM}\n\nExisting analysis:\n{}",
                serde_json::to_string_pretty(existing).unwrap_or_default()),
            None => COMPANY_INTEL_SYSTEM.to_string(),
        };
        let mut v = deps.provider.structured_output(
            vec![system_msg(system), user_msg(response)],
            "CompanyIntel",
        ).await?;

        // The LLM returns a partial intel (no url/analyzedAt). Inject them before deserializing.
        if let Some(obj) = v.as_object_mut() {
            obj.insert("url".into(), serde_json::json!(ctx.company_url.clone().unwrap_or_default()));
            obj.insert("analyzedAt".into(), serde_json::json!(deps.clock.now_iso8601()));
        }
        let intel: CompanyIntel = serde_json::from_value(v)
            .map_err(|e| IntakeError::Deserialize(e.to_string()))?;
        Ok(ParsedResponse {
            context_updates: IntakeContextUpdates { company_intel: Some(intel), ..Default::default() },
            ..Default::default()
        })
    }

    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::CompanyConfirm }
}

pub struct CompanyConfirm;

#[async_trait]
impl IntakeNode for CompanyConfirm {
    fn id(&self) -> NodeId { NodeId::CompanyConfirm }
    fn phase(&self) -> Phase { Phase::Company }

    async fn prompt(&self, ctx: &IntakeContext) -> String {
        let intel = match &ctx.company_intel {
            Some(i) => i,
            None => return "Let me know more about the company so I can build the intelligence profile.".into(),
        };

        let competitors = intel.competitors.clone().unwrap_or_default();

        if !competitors.is_empty() {
            format!(
                "I identified these competitors: {}.\n\nAre there other companies you'd specifically like to target for sourcing, or companies to avoid? (You can also say \"looks good\" to proceed.)",
                competitors.join(", ")
            )
        } else {
            r#"Which companies would you like to target for sourcing candidates? Also, are there companies to avoid? (e.g., "Target: Stripe, Coinbase. Avoid: OldCorp")"#.into()
        }
    }

    async fn parse(&self, response: &str, ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError>
    {
        if matches_any(response, CONFIRM_OR_SKIP_PHRASES) {
            let competitors = ctx.company_intel.as_ref()
                .and_then(|c| c.competitors.clone())
                .unwrap_or_default();
            let map = CompetitorMap {
                target_companies: competitors,
                avoid_companies: vec![],
                competitor_reason: Default::default(),
            };
            return Ok(ParsedResponse {
                structured: serde_json::json!({ "confirmed": true }),
                context_updates: IntakeContextUpdates { competitor_map: Some(map), ..Default::default() },
                ..Default::default()
            });
        }

        let existing = ctx.company_intel.as_ref()
            .and_then(|c| c.competitors.clone())
            .unwrap_or_default();
        let system = format!(
            "{COMPETITOR_SYSTEM}\n\nExisting competitors from analysis: {}",
            serde_json::to_string(&existing).unwrap_or_default()
        );
        let v = deps.provider.structured_output(
            vec![system_msg(system), user_msg(response)],
            "CompetitorMap",
        ).await?;
        let map: CompetitorMap = serde_json::from_value(v)
            .map_err(|e| IntakeError::Deserialize(e.to_string()))?;
        Ok(ParsedResponse {
            context_updates: IntakeContextUpdates { competitor_map: Some(map), ..Default::default() },
            ..Default::default()
        })
    }

    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::TeamInput }
}

pub fn create_company_intel_nodes() -> Vec<Box<dyn IntakeNode>> {
    vec![Box::new(CompanyUrlInput), Box::new(CompanyAnalysis), Box::new(CompanyConfirm)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::context::{has_company_data, IntakeContext};
    use crate::recruiting::intake::deps::testing::{FakeProvider, FakeResearch, fake_deps};
    use crate::recruiting::intake::node::{IntakeNode, NodeId};
    use crate::recruiting::intake::schemas::CompanyIntel;

    fn intel() -> CompanyIntel {
        CompanyIntel {
            name: "Acme".into(),
            url: "https://acme.com".into(),
            tech_stack: vec![],
            team_size: None,
            funding_stage: None,
            product_category: None,
            culture_signals: vec![],
            pitch: None,
            competitors: None,
            analyzed_at: "2026-06-01T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn url_input_extracts_url_and_crawls_into_pending() {
        let node = CompanyUrlInput;
        let research = FakeResearch { company: Some(intel()), ..Default::default() };
        let deps = fake_deps(FakeProvider::new(vec![]), research);
        let ctx = IntakeContext::default();
        let parsed = node.parse("check https://acme.com please", &ctx, &deps).await.unwrap();
        assert_eq!(parsed.context_updates.company_url.as_deref(), Some("https://acme.com"));
        assert!(parsed.context_updates.pending_company_intel.is_some());
        assert_eq!(node.next(&parsed, &ctx), NodeId::CompanyAnalysis);
    }

    #[test]
    fn url_input_skips_when_company_known() {
        let node = CompanyUrlInput;
        let ctx = IntakeContext { company_intel: Some(intel()), ..Default::default() };
        assert!(node.skip_if(&ctx));
        assert!(has_company_data(&ctx));
    }

    #[tokio::test]
    async fn analysis_promotes_pending_on_confirm_then_to_confirm_node() {
        let node = CompanyAnalysis;
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext { pending_company_intel: Some(intel()), ..Default::default() };
        let parsed = node.parse("yes accurate", &ctx, &deps).await.unwrap();
        assert!(parsed.context_updates.company_intel.is_some());
        assert_eq!(node.next(&parsed, &ctx), NodeId::CompanyConfirm);
    }

    #[tokio::test]
    async fn confirm_builds_competitor_map_and_goes_to_team() {
        let node = CompanyConfirm;
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext { company_intel: Some(intel()), ..Default::default() };
        let parsed = node.parse("looks good", &ctx, &deps).await.unwrap();
        assert!(parsed.context_updates.competitor_map.is_some());
        assert_eq!(node.next(&parsed, &ctx), NodeId::TeamInput);
    }
}
