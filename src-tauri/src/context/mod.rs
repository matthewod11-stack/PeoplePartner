//! Context Builder Module — split into submodules per #40c.
//!
//! Builds contextual system prompts for Claude with company and employee data.
//! Split is structural only — public surface preserved via `pub use`
//! re-exports below; external callers (`commands/*`, `chat.rs`, etc.)
//! reference `crate::context::*` unchanged.
//!
//! - `query` — intent classification + mention extraction
//! - `retrieval` — DB fetch functions (employee/company context, batch + per-row)
//! - `aggregates` — `OrgAggregates`, formatting, headcount/perf/eNPS/attrition
//! - `verification` — numeric-claim verification of model responses
//! - `prompt` — Alex personas, token budget, employee/system prompt formatting

mod aggregates;
mod prompt;
mod query;
mod retrieval;
mod verification;

use serde::Serialize;
use thiserror::Error;

// ---- Module-level error type (used by every submodule) ----

#[derive(Error, Debug, Serialize)]
pub enum ContextError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Context building error: {0}")]
    BuildError(String),
}

impl From<sqlx::Error> for ContextError {
    fn from(err: sqlx::Error) -> Self {
        ContextError::Database(err.to_string())
    }
}

// ---- Public surface preserved from the pre-split file ----

pub use aggregates::{
    build_org_aggregates, calculate_aggregate_enps, format_aggregate_enps, format_org_aggregates,
    AttritionStats, DepartmentCount, EnpsAggregate, OrgAggregates, RatingDistribution,
};
pub use prompt::{
    build_chat_context, build_system_prompt, calculate_excerpt_limits,
    calculate_per_employee_budget, compute_conversation_budget, estimate_tokens,
    excerpt_to_sentences, format_employee_context, format_employee_context_with_budget,
    format_employee_summaries, get_max_conversation_tokens, get_max_system_prompt_tokens,
    get_persona, get_system_prompt_for_message, tokens_to_chars, Persona, RetrievalMetrics,
    SystemPromptResult, TokenBudget, TokenUsage, PERSONAS,
};
pub use query::{
    classify_query, extract_mentions, QueryMentions, QueryType, TenureDirection, ThemeTarget,
};
pub use retrieval::{
    build_employee_list, build_termination_list, find_employees_by_theme, find_longest_tenure,
    find_newest_employees, find_recent_hires, find_recent_terminations, find_relevant_employees,
    find_top_performers, find_underperformers, find_upcoming_anniversaries, get_company_context,
    get_employee_context, get_employee_contexts, ChatContext, CompanyContext, CycleHighlight,
    EmployeeContext, EmployeeSummary, EnpsInfo, RatingInfo,
};
pub use verification::{
    verify_response, ClaimType, NumericClaim, VerificationResult, VerificationStatus,
};
