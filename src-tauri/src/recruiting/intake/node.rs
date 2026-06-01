//! Node identity (`NodeId`), the `IntakeNode` trait, and the parse/step
//! result types. Ported from the `ConversationNode` interface in
//! `~/Projects/Sourcerer/packages/core/src/intake.ts`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::context::{IntakeContext, IntakeContextUpdates};
use super::deps::{IntakeDeps, IntakeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeId {
    RoleJdInput, RoleParseConfirm, RoleRefine,
    CompanyUrlInput, CompanyAnalysis, CompanyConfirm,
    TeamInput, TeamAnalysis, AntiPatterns,
    ConfigGenerate, ConfigReview,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase { Role, Company, SuccessProfile, Strategy }

/// What a node's `parse` returns.
#[derive(Debug, Clone, Default)]
pub struct ParsedResponse {
    pub structured: serde_json::Value,
    pub context_updates: IntakeContextUpdates,
    pub follow_up_needed: bool,
    pub follow_up_reason: Option<String>,
}

/// Result of one `submit_response` step.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub parsed: ParsedResponse,
    pub next: NodeId,
    pub done: bool,
}

#[async_trait]
pub trait IntakeNode: Send + Sync {
    fn id(&self) -> NodeId;
    fn phase(&self) -> Phase;
    async fn prompt(&self, ctx: &IntakeContext) -> String;
    async fn parse(&self, response: &str, ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError>;
    fn next(&self, parsed: &ParsedResponse, ctx: &IntakeContext) -> NodeId;
    fn skip_if(&self, _ctx: &IntakeContext) -> bool { false }
}
