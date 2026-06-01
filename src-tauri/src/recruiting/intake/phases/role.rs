//! Phase 1 — Role context. Port of
//! `~/Projects/Sourcerer/packages/intake/src/phases/role-context.ts`.

use async_trait::async_trait;
use crate::recruiting::intake::context::{IntakeContext, IntakeContextUpdates};
use crate::recruiting::intake::deps::{IntakeDeps, IntakeError};
use crate::recruiting::intake::node::{IntakeNode, NodeId, ParsedResponse, Phase};
use crate::recruiting::intake::schemas::{Message, MessageRole, RoleParameters};

const ROLE_PARSE_SYSTEM: &str = r#"You are a recruiter's assistant. Parse the provided role description or job posting into structured parameters. Return a JSON object with:
- title: job title
- level: seniority level (junior, mid, senior, staff, principal, lead, director)
- scope: what this person will own/do (one sentence)
- location: location if mentioned
- remotePolicy: one of "remote", "hybrid", "in_person", "negotiable" if mentioned
- compensationRange: {min, max, currency} if mentioned
- mustHaveSkills: array of required skills
- niceToHaveSkills: array of preferred skills
- teamSize: team size if mentioned
- reportingTo: who this person reports to if mentioned

If information is not available, omit the field. Be thorough in extracting skills."#;

const ROLE_REFINE_SYSTEM: &str = r#"You are a recruiter's assistant. The user wants to refine the role parameters. Apply their changes to the existing parameters and return the complete updated JSON object with all fields:
- title, level, scope, location, remotePolicy, compensationRange, mustHaveSkills, niceToHaveSkills, teamSize, reportingTo"#;

const CONFIRM_PHRASES: &[&str] = &["yes", "looks good", "correct", "confirmed", "proceed", "lgtm", "looks right"];

fn is_confirmed(resp: &str) -> bool {
    let n = resp.trim().to_lowercase();
    CONFIRM_PHRASES.iter().any(|p| n.contains(p)) || n == "y"
}

fn user_msg(content: &str) -> Message { Message { role: MessageRole::User, content: content.into() } }
fn system_msg(content: String) -> Message { Message { role: MessageRole::System, content } }

pub struct RoleJdInput;
#[async_trait]
impl IntakeNode for RoleJdInput {
    fn id(&self) -> NodeId { NodeId::RoleJdInput }
    fn phase(&self) -> Phase { Phase::Role }
    async fn prompt(&self, ctx: &IntakeContext) -> String {
        if let Some(desc) = &ctx.role_description {
            format!(
                "I see you've already provided some role information. Would you like to add more detail, or shall we proceed with what we have?\n\nCurrent info: {}...",
                &desc[..desc.len().min(200)]
            )
        } else {
            "Let's start by understanding the role you're hiring for.\n\nYou can:\n1. Paste a job description (JD)\n2. Describe the role in your own words\n3. Share a link to the job posting\n\nWhat do you have?".into()
        }
    }
    async fn parse(&self, response: &str, _ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError> {
        let messages = vec![system_msg(ROLE_PARSE_SYSTEM.to_string()), user_msg(response)];
        let v = deps.provider.structured_output(messages, "RoleParameters").await?;
        let role: RoleParameters = serde_json::from_value(v)
            .map_err(|e| IntakeError::Deserialize(e.to_string()))?;
        Ok(ParsedResponse {
            context_updates: IntakeContextUpdates {
                role_description: Some(response.to_string()),
                role_parameters: Some(role),
                ..Default::default()
            },
            ..Default::default()
        })
    }
    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::RoleParseConfirm }
}

pub struct RoleParseConfirm;
#[async_trait]
impl IntakeNode for RoleParseConfirm {
    fn id(&self) -> NodeId { NodeId::RoleParseConfirm }
    fn phase(&self) -> Phase { Phase::Role }
    async fn prompt(&self, ctx: &IntakeContext) -> String {
        let params = match &ctx.role_parameters {
            Some(p) => p,
            None => return "I wasn't able to extract role parameters. Could you describe the role again?".into(),
        };

        let mut lines = vec![
            "Here's what I extracted:\n".to_string(),
            format!("**Title:** {}", params.title),
            format!("**Level:** {}", params.level),
            format!("**Scope:** {}", params.scope),
        ];

        if let Some(loc) = &params.location { lines.push(format!("**Location:** {}", loc)); }
        if let Some(rp) = &params.remote_policy { lines.push(format!("**Remote policy:** {:?}", rp)); }
        if let Some(comp) = &params.compensation_range {
            let min_str = comp.min.map(|v| format!("{} {}", comp.currency, v)).unwrap_or_else(|| "?".into());
            let max_str = comp.max.map(|v| format!("{} {}", comp.currency, v)).unwrap_or_else(|| "?".into());
            lines.push(format!("**Compensation:** {} - {}", min_str, max_str));
        }
        if !params.must_have_skills.is_empty() {
            lines.push(format!("**Must-have skills:** {}", params.must_have_skills.join(", ")));
        }
        if !params.nice_to_have_skills.is_empty() {
            lines.push(format!("**Nice-to-have skills:** {}", params.nice_to_have_skills.join(", ")));
        }
        if let Some(ts) = &params.team_size { lines.push(format!("**Team size:** {}", ts)); }
        if let Some(rt) = &params.reporting_to { lines.push(format!("**Reports to:** {}", rt)); }

        lines.push("\nDoes this look right? You can say \"yes\" to proceed, or tell me what to change.".into());

        lines.join("\n")
    }
    async fn parse(&self, response: &str, _ctx: &IntakeContext, _deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError> {
        let confirmed = is_confirmed(response);
        Ok(ParsedResponse {
            structured: serde_json::json!({ "confirmed": confirmed }),
            follow_up_needed: !confirmed,
            ..Default::default()
        })
    }
    fn next(&self, parsed: &ParsedResponse, _c: &IntakeContext) -> NodeId {
        if parsed.structured["confirmed"] == serde_json::Value::Bool(true) {
            NodeId::CompanyUrlInput
        } else {
            NodeId::RoleRefine
        }
    }
}

pub struct RoleRefine;
#[async_trait]
impl IntakeNode for RoleRefine {
    fn id(&self) -> NodeId { NodeId::RoleRefine }
    fn phase(&self) -> Phase { Phase::Role }
    async fn prompt(&self, _ctx: &IntakeContext) -> String {
        "What would you like to change? I'll update the role parameters.".into()
    }
    async fn parse(&self, response: &str, ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError> {
        let current = ctx.role_parameters.as_ref()
            .ok_or_else(|| IntakeError::Provider("no role params to refine".into()))?;
        let system = format!("{ROLE_REFINE_SYSTEM}\n\nCurrent parameters:\n{}",
            serde_json::to_string_pretty(current).unwrap());
        let messages = vec![system_msg(system), user_msg(response)];
        let v = deps.provider.structured_output(messages, "RoleParameters").await?;
        let role: RoleParameters = serde_json::from_value(v)
            .map_err(|e| IntakeError::Deserialize(e.to_string()))?;
        Ok(ParsedResponse {
            context_updates: IntakeContextUpdates { role_parameters: Some(role), ..Default::default() },
            ..Default::default()
        })
    }
    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::RoleParseConfirm }
}

pub fn create_role_context_nodes() -> Vec<Box<dyn IntakeNode>> {
    vec![Box::new(RoleJdInput), Box::new(RoleParseConfirm), Box::new(RoleRefine)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::context::IntakeContext;
    use crate::recruiting::intake::deps::testing::{FakeProvider, FakeResearch, fake_deps};
    use crate::recruiting::intake::node::{IntakeNode, NodeId};

    fn role_json() -> serde_json::Value {
        serde_json::json!({"title":"Staff Eng","level":"staff","scope":"infra",
            "mustHaveSkills":["rust"],"niceToHaveSkills":[]})
    }

    #[tokio::test]
    async fn jd_input_parses_role_then_goes_to_confirm() {
        let node = RoleJdInput;
        let deps = fake_deps(FakeProvider::new(vec![role_json()]), FakeResearch::default());
        let ctx = IntakeContext::default();
        let parsed = node.parse("Senior infra role", &ctx, &deps).await.unwrap();
        assert!(parsed.context_updates.role_parameters.is_some());
        assert_eq!(node.next(&parsed, &ctx), NodeId::RoleParseConfirm);
    }

    #[tokio::test]
    async fn confirm_yes_advances_to_company() {
        let node = RoleParseConfirm;
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext::default();
        let parsed = node.parse("yes, looks good", &ctx, &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &ctx), NodeId::CompanyUrlInput);
    }

    #[tokio::test]
    async fn confirm_change_goes_to_refine() {
        let node = RoleParseConfirm;
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext::default();
        let parsed = node.parse("change the title to Principal", &ctx, &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &ctx), NodeId::RoleRefine);
    }

    #[tokio::test]
    async fn refine_loops_back_to_confirm() {
        let node = RoleRefine;
        let deps = fake_deps(FakeProvider::new(vec![role_json()]), FakeResearch::default());
        let ctx = IntakeContext { role_parameters: Some(
            serde_json::from_value(role_json()).unwrap()), ..Default::default() };
        let parsed = node.parse("make it principal", &ctx, &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &ctx), NodeId::RoleParseConfirm);
    }
}
