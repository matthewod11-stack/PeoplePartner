//! `IntakeContext` accumulator + merge semantics. Ported from
//! `~/Projects/Sourcerer/packages/intake/src/intake-context.ts`.
//! Merge rule: scalar fields overwrite when the update is `Some`; array
//! fields concatenate. `pending_*` scratch fields hold computed artifacts
//! (crawled intel, generated config) so `prompt()` stays read-only.

use serde::{Deserialize, Serialize};
use super::schemas::{
    CompanyIntel, CompetitorMap, CompositeProfile, Message, ProfileAnalysis,
    RoleParameters, SearchConfig, TalentProfile,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeContext {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub role_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub role_parameters: Option<RoleParameters>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub company_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub company_intel: Option<CompanyIntel>,
    #[serde(default)]
    pub team_profiles: Vec<ProfileAnalysis>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub composite_profile: Option<CompositeProfile>,
    #[serde(default)]
    pub anti_patterns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub competitor_map: Option<CompetitorMap>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub talent_profile: Option<TalentProfile>,
    #[serde(default)]
    pub similarity_seeds: Vec<String>,
    #[serde(default)]
    pub conversation_history: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pending_company_intel: Option<CompanyIntel>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pending_search_config: Option<SearchConfig>,
}

/// All-optional partial returned by `IntakeNode::parse` and applied by
/// `merge_context_updates`. Mirrors `Partial<IntakeContext>` in TS.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IntakeContextUpdates {
    pub role_description: Option<String>,
    pub role_parameters: Option<RoleParameters>,
    pub company_url: Option<String>,
    pub company_intel: Option<CompanyIntel>,
    pub team_profiles: Option<Vec<ProfileAnalysis>>,
    pub composite_profile: Option<CompositeProfile>,
    pub anti_patterns: Option<Vec<String>>,
    pub competitor_map: Option<CompetitorMap>,
    pub talent_profile: Option<TalentProfile>,
    pub similarity_seeds: Option<Vec<String>>,
    pub pending_company_intel: Option<CompanyIntel>,
    pub pending_search_config: Option<SearchConfig>,
}

/// Scalars overwrite when `Some`; arrays concat.
pub fn merge_context_updates(mut ctx: IntakeContext, u: IntakeContextUpdates) -> IntakeContext {
    if let Some(v) = u.role_description { ctx.role_description = Some(v); }
    if let Some(v) = u.role_parameters { ctx.role_parameters = Some(v); }
    if let Some(v) = u.company_url { ctx.company_url = Some(v); }
    if let Some(v) = u.company_intel { ctx.company_intel = Some(v); }
    if let Some(v) = u.composite_profile { ctx.composite_profile = Some(v); }
    if let Some(v) = u.competitor_map { ctx.competitor_map = Some(v); }
    if let Some(v) = u.talent_profile { ctx.talent_profile = Some(v); }
    if let Some(v) = u.pending_company_intel { ctx.pending_company_intel = Some(v); }
    if let Some(v) = u.pending_search_config { ctx.pending_search_config = Some(v); }
    if let Some(mut v) = u.team_profiles { ctx.team_profiles.append(&mut v); }
    if let Some(mut v) = u.anti_patterns { ctx.anti_patterns.append(&mut v); }
    if let Some(mut v) = u.similarity_seeds { ctx.similarity_seeds.append(&mut v); }
    ctx
}

pub fn append_message(mut ctx: IntakeContext, message: Message) -> IntakeContext {
    ctx.conversation_history.push(message);
    ctx
}

pub fn has_role_data(ctx: &IntakeContext) -> bool { ctx.role_parameters.is_some() }
pub fn has_company_data(ctx: &IntakeContext) -> bool { ctx.company_intel.is_some() }
pub fn has_team_profiles(ctx: &IntakeContext) -> bool { !ctx.team_profiles.is_empty() }
pub fn has_competitor_map(ctx: &IntakeContext) -> bool { ctx.competitor_map.is_some() }

pub fn serialize_context(ctx: &IntakeContext) -> String {
    serde_json::to_string_pretty(ctx).expect("IntakeContext is always serializable")
}

pub fn deserialize_context(json: &str) -> Result<IntakeContext, super::deps::IntakeError> {
    serde_json::from_str(json).map_err(|e| super::deps::IntakeError::Deserialize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::schemas::{MessageRole, Message};

    fn msg(content: &str) -> Message {
        Message { role: MessageRole::User, content: content.into() }
    }

    #[test]
    fn merge_overwrites_scalars() {
        let ctx = IntakeContext { role_description: Some("old".into()), ..Default::default() };
        let updates = IntakeContextUpdates { role_description: Some("new".into()), ..Default::default() };
        let merged = merge_context_updates(ctx, updates);
        assert_eq!(merged.role_description.as_deref(), Some("new"));
    }

    #[test]
    fn merge_concats_arrays() {
        let ctx = IntakeContext { similarity_seeds: vec!["a".into()], ..Default::default() };
        let updates = IntakeContextUpdates {
            similarity_seeds: Some(vec!["b".into(), "c".into()]), ..Default::default()
        };
        let merged = merge_context_updates(ctx, updates);
        assert_eq!(merged.similarity_seeds, vec!["a", "b", "c"]);
    }

    #[test]
    fn merge_none_update_leaves_field_untouched() {
        let ctx = IntakeContext { company_url: Some("x".into()), ..Default::default() };
        let merged = merge_context_updates(ctx, IntakeContextUpdates::default());
        assert_eq!(merged.company_url.as_deref(), Some("x"));
    }

    #[test]
    fn append_message_grows_history() {
        let ctx = IntakeContext::default();
        let ctx = append_message(ctx, msg("hi"));
        assert_eq!(ctx.conversation_history.len(), 1);
    }

    #[test]
    fn deserialize_round_trips() {
        let ctx = IntakeContext { role_description: Some("r".into()), ..Default::default() };
        let json = serialize_context(&ctx);
        let back = deserialize_context(&json).expect("valid");
        assert_eq!(back.role_description, ctx.role_description);
    }

    #[test]
    fn deserialize_rejects_garbage() {
        assert!(deserialize_context("not json").is_err());
    }
}
