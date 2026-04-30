//! HR persona definitions, token budget management, system-prompt assembly,
//! and the top-level `build_chat_context` / `get_system_prompt_for_message`
//! entry points used by the chat command path.

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::db::DbPool;
use crate::memory;

use super::aggregates::{
    build_org_aggregates, format_org_aggregates, rating_label, OrgAggregates,
};
use super::query::{classify_query, extract_mentions, QueryMentions, QueryType};
use super::retrieval::{
    build_employee_list, find_employees_by_theme, find_recent_terminations,
    find_relevant_employees, get_company_context, ChatContext, CompanyContext, EmployeeContext,
    EmployeeSummary,
};
use super::ContextError;

// ============================================================================
// Token Budget Constants
// ============================================================================
// Claude Sonnet 4 has 200K context window. We allocate conservatively:
// - System prompt (persona + company + employees): 20K tokens
// - Conversation history: 150K tokens
// - Output reserved: 4K tokens
// - Safety buffer: 26K tokens

/// Approximate characters per token (conservative estimate for English text)
const CHARS_PER_TOKEN: usize = 4;

/// Maximum tokens for the entire system prompt (persona + company + employees + memory)
const MAX_SYSTEM_PROMPT_TOKENS: usize = 20_000;

/// Maximum tokens for conversation history
const MAX_CONVERSATION_TOKENS: usize = 150_000;

/// Tokens reserved for Claude's response output
#[allow(dead_code)]
const OUTPUT_TOKENS_RESERVED: usize = 4_096;

/// Maximum tokens for employee context section (part of system prompt budget)
const MAX_EMPLOYEE_CONTEXT_TOKENS: usize = 4_000;

/// Maximum number of employees to include in context
#[allow(dead_code)]
const MAX_EMPLOYEES_IN_CONTEXT: usize = 10;

// ============================================================================
// HR Personas (V2.1.3)
// ============================================================================

/// HR persona for customizing Claude's communication style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub id: &'static str,
    pub name: &'static str,
    pub style: &'static str,
    pub best_for: &'static str,
    pub preamble: &'static str,
    pub communication_style: &'static str,
    pub sample_response: &'static str,
}

/// Available HR personas - each offers a different communication style
pub const PERSONAS: [Persona; 5] = [
    Persona {
        id: "alex",
        name: "Alex",
        style: "Warm, practical",
        best_for: "General HR leadership",
        preamble: "You are Alex, an experienced VP of People Operations helping {user_display} at {company_name}, a company based in {company_state}.\n\nYour role is to be a trusted HR thought partner—someone who's seen these situations before and can offer practical, actionable guidance.",
        communication_style: "- Be warm but professional, like a trusted colleague\n- Lead with practical answers, then explain the reasoning\n- Acknowledge when situations are genuinely difficult\n- Offer specific language or scripts when helpful\n- Flag when legal review is needed, but don't over-hedge on routine matters",
        sample_response: "I've seen this situation many times. Let's start with a clear, honest conversation about expectations and give them a path forward.",
    },
    Persona {
        id: "jordan",
        name: "Jordan",
        style: "Formal, compliance-focused",
        best_for: "Regulated industries",
        preamble: "You are Jordan, a meticulous HR Director with deep expertise in employment law and compliance, advising {user_display} at {company_name}, based in {company_state}.\n\nYour role is to ensure every HR action is legally defensible, well-documented, and follows best practices for risk management.",
        communication_style: "- Prioritize compliance and documentation requirements\n- Reference specific policies, laws, or regulations when applicable\n- Recommend clear audit trails for all decisions\n- Use formal, precise language\n- When in doubt, recommend consulting legal counsel",
        sample_response: "Before proceeding, let's ensure we have documentation. Per your company's PIP policy, here are the required steps to maintain compliance...",
    },
    Persona {
        id: "sam",
        name: "Sam",
        style: "Startup-friendly, direct",
        best_for: "Early-stage, lean HR",
        preamble: "You are Sam, a pragmatic People Ops leader who's built HR from scratch at multiple startups, now advising {user_display} at {company_name}, based in {company_state}.\n\nYour role is to help move fast without breaking things—practical solutions that work for lean teams.",
        communication_style: "- Be direct and concise—no corporate fluff\n- Prioritize speed and pragmatism over perfection\n- Suggest scrappy, MVP approaches when appropriate\n- Acknowledge that perfect documentation isn't always possible\n- Focus on what matters most right now",
        sample_response: "Here's the 80/20: Have a direct conversation this week. Set clear expectations. Give them 30 days. If no improvement, move on.",
    },
    Persona {
        id: "morgan",
        name: "Morgan",
        style: "Data-driven, analytical",
        best_for: "Metrics-focused users",
        preamble: "You are Morgan, a People Analytics leader who brings data rigor to HR decisions, advising {user_display} at {company_name}, based in {company_state}.\n\nYour role is to ensure decisions are evidence-based, measurable, and tied to business outcomes.",
        communication_style: "- Lead with data and metrics when available\n- Suggest ways to measure outcomes and impact\n- Reference benchmarks and industry standards\n- Ask clarifying questions to understand the full picture\n- Recommend tracking mechanisms for future decisions",
        sample_response: "Let's look at the data: What's their performance trajectory? How does their output compare to peers? What does their 360 feedback show?",
    },
    Persona {
        id: "taylor",
        name: "Taylor",
        style: "Employee-advocate, empathetic",
        best_for: "People-first cultures",
        preamble: "You are Taylor, a compassionate HR leader who puts employee wellbeing at the center of every decision, advising {user_display} at {company_name}, based in {company_state}.\n\nYour role is to find solutions that honor both business needs and human dignity.",
        communication_style: "- Lead with empathy and understanding\n- Consider the employee's perspective and circumstances\n- Suggest supportive approaches before punitive ones\n- Acknowledge the emotional weight of difficult decisions\n- Look for win-win solutions when possible",
        sample_response: "This is a difficult situation for everyone involved. Before we discuss performance, let's understand: what support does this person need? What might be contributing to their struggles?",
    },
];

/// Get persona by ID, defaulting to Alex if not found
pub fn get_persona(id: Option<&str>) -> &'static Persona {
    let id = id.unwrap_or("alex");
    PERSONAS
        .iter()
        .find(|p| p.id == id)
        .unwrap_or(&PERSONAS[0]) // Default to Alex
}

// ============================================================================
// Token Budget Types
// ============================================================================

/// Token budget allocation per query type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    pub employee_context: usize,
    pub theme_context: usize,
    pub memory_context: usize,
    pub total_context: usize,
}

impl TokenBudget {
    pub fn for_query_type(query_type: QueryType) -> Self {
        match query_type {
            QueryType::Aggregate => TokenBudget {
                employee_context: 0,
                theme_context: 500,
                memory_context: 500,
                total_context: 1_000,
            },
            QueryType::List => TokenBudget {
                employee_context: 2_000,
                theme_context: 0,
                memory_context: 500,
                total_context: 2_500,
            },
            QueryType::Individual => TokenBudget {
                employee_context: 4_000,
                theme_context: 0,
                memory_context: 1_000,
                total_context: 5_000,
            },
            QueryType::Comparison => TokenBudget {
                employee_context: 3_000,
                theme_context: 0,
                memory_context: 500,
                total_context: 3_500,
            },
            QueryType::Attrition => TokenBudget {
                employee_context: 2_000,
                theme_context: 0,
                memory_context: 500,
                total_context: 2_500,
            },
            QueryType::General => TokenBudget {
                employee_context: 2_000,
                theme_context: 0,
                memory_context: 1_000,
                total_context: 3_000,
            },
        }
    }
}

/// Actual token usage tracked during context retrieval
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub employee_tokens: usize,
    pub memory_tokens: usize,
    pub aggregates_tokens: usize,
    pub total_tokens: usize,
}

/// Comprehensive retrieval metrics for observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMetrics {
    pub query_type: QueryType,
    pub employees_found: usize,
    pub employees_included: usize,
    pub memories_found: usize,
    pub memories_included: usize,
    pub aggregates_included: bool,
    pub token_budget: TokenBudget,
    pub token_usage: TokenUsage,
    pub retrieval_time_ms: u64,
}

impl Default for RetrievalMetrics {
    fn default() -> Self {
        RetrievalMetrics {
            query_type: QueryType::General,
            employees_found: 0,
            employees_included: 0,
            memories_found: 0,
            memories_included: 0,
            aggregates_included: false,
            token_budget: TokenBudget::for_query_type(QueryType::General),
            token_usage: TokenUsage::default(),
            retrieval_time_ms: 0,
        }
    }
}

/// Result of get_system_prompt_for_message (V2.1.4)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptResult {
    pub system_prompt: String,
    pub employee_ids_used: Vec<String>,
    pub aggregates: Option<OrgAggregates>,
    pub query_type: QueryType,
    pub metrics: RetrievalMetrics,
}

// ============================================================================
// Excerpting Helpers (V2.2.2a)
// ============================================================================

/// Default maximum sentences to include in excerpts
#[allow(dead_code)]
const DEFAULT_MAX_SENTENCES: usize = 3;

/// Minimum sentences to preserve (even under tight budgets)
const MIN_SENTENCES: usize = 1;

/// Maximum sentences for career summaries at full budget
const FULL_BUDGET_SUMMARY_SENTENCES: usize = 5;

/// Maximum sentences for career summaries at reduced budget
const REDUCED_BUDGET_SUMMARY_SENTENCES: usize = 2;

/// Token threshold below which we consider budget "reduced"
const REDUCED_BUDGET_THRESHOLD: usize = 800;

/// Extract the first N sentences from text using Unicode sentence boundaries.
pub fn excerpt_to_sentences(text: &str, max_sentences: usize) -> String {
    if max_sentences == 0 {
        return String::new();
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let sentences: Vec<&str> = trimmed.unicode_sentences().collect();

    if sentences.len() <= max_sentences {
        return trimmed.to_string();
    }

    let excerpt: String = sentences[..max_sentences].concat();
    let mut result = excerpt.trim_end().to_string();

    if !result.ends_with('.') && !result.ends_with('!') && !result.ends_with('?') {
        result.push_str("...");
    } else {
        result.push_str("..");
    }

    result
}

/// Calculate the number of sentences to include based on available token budget.
pub fn calculate_excerpt_limits(token_budget: usize) -> (usize, usize) {
    if token_budget >= REDUCED_BUDGET_THRESHOLD {
        (FULL_BUDGET_SUMMARY_SENTENCES, 3)
    } else if token_budget >= REDUCED_BUDGET_THRESHOLD / 2 {
        (REDUCED_BUDGET_SUMMARY_SENTENCES, 2)
    } else {
        (MIN_SENTENCES, 1)
    }
}

/// Calculate per-employee token budget based on total budget and employee count.
pub fn calculate_per_employee_budget(total_budget: usize, employee_count: usize) -> usize {
    if employee_count == 0 {
        return total_budget;
    }

    const MIN_PER_EMPLOYEE: usize = 200;
    let calculated = total_budget / employee_count;
    calculated.max(MIN_PER_EMPLOYEE)
}

// ============================================================================
// Context Formatting
// ============================================================================

/// Format employee context for inclusion in system prompt.
pub fn format_employee_context(employees: &[EmployeeContext]) -> String {
    format_employee_context_with_budget(employees, None)
}

/// Format employee context with explicit token budget for dynamic excerpting.
pub fn format_employee_context_with_budget(
    employees: &[EmployeeContext],
    total_token_budget: Option<usize>,
) -> String {
    if employees.is_empty() {
        return String::new();
    }

    let budget = total_token_budget.unwrap_or(MAX_EMPLOYEE_CONTEXT_TOKENS);
    let per_employee_budget = calculate_per_employee_budget(budget, employees.len());

    let mut output = String::new();
    let mut total_chars = 0;
    let max_chars = budget * CHARS_PER_TOKEN;

    for emp in employees {
        let emp_text = format_single_employee_with_budget(emp, Some(per_employee_budget));

        if total_chars + emp_text.len() > max_chars {
            output.push_str("\n[Additional employees omitted due to context limit]");
            break;
        }

        output.push_str(&emp_text);
        output.push_str("\n---\n");
        total_chars += emp_text.len() + 5;
    }

    output
}

/// Format employee summaries for list queries (~70 chars each)
pub fn format_employee_summaries(summaries: &[EmployeeSummary], total_count: Option<i64>) -> String {
    if summaries.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();

    if let Some(total) = total_count {
        if summaries.len() < total as usize {
            lines.push(format!(
                "EMPLOYEES (showing {} of {}):",
                summaries.len(),
                total
            ));
        } else {
            lines.push(format!("EMPLOYEES ({}):", summaries.len()));
        }
    } else {
        lines.push(format!("EMPLOYEES ({}):", summaries.len()));
    }

    for emp in summaries {
        let title = emp.job_title.as_deref().unwrap_or("No title");
        let dept = emp.department.as_deref().unwrap_or("Unassigned");
        let hire = emp
            .hire_date
            .as_deref()
            .map(|d| format!(", hired {}", d))
            .unwrap_or_default();

        lines.push(format!(
            "• {} — {}, {} ({}{hire})",
            emp.full_name, title, dept, emp.status
        ));
    }

    lines.join("\n")
}

/// Format a single employee's context (backward-compatible wrapper)
fn format_single_employee(emp: &EmployeeContext) -> String {
    format_single_employee_with_budget(emp, None)
}

/// Format a single employee's context with optional token budget for excerpting.
fn format_single_employee_with_budget(emp: &EmployeeContext, token_budget: Option<usize>) -> String {
    let mut lines = Vec::new();

    let (summary_sentences, highlight_cycles) = token_budget
        .map(calculate_excerpt_limits)
        .unwrap_or((FULL_BUDGET_SUMMARY_SENTENCES, 3));

    // Basic info
    lines.push(format!("**{}** ({})", emp.full_name, emp.status));

    if let Some(ref title) = emp.job_title {
        if let Some(ref dept) = emp.department {
            lines.push(format!("  {} — {}", title, dept));
        } else {
            lines.push(format!("  {}", title));
        }
    }

    if let Some(ref manager) = emp.manager_name {
        lines.push(format!("  Reports to: {}", manager));
    }

    if let Some(ref state) = emp.work_state {
        lines.push(format!("  Work location: {}", state));
    }

    if let Some(ref hire_date) = emp.hire_date {
        lines.push(format!("  Hire date: {}", hire_date));
    }

    // Performance info
    if !emp.all_ratings.is_empty() {
        lines.push("  Performance:".to_string());
        for rating in emp.all_ratings.iter().take(3) {
            let label = rating_label(rating.overall_rating);
            lines.push(format!("    - {} {}: {:.1} ({})",
                rating.cycle_name,
                rating.rating_date.as_deref().unwrap_or(""),
                rating.overall_rating,
                label
            ));
        }
        if let Some(ref trend) = emp.rating_trend {
            lines.push(format!("    Trend: {}", trend));
        }
    }

    // eNPS info
    if !emp.all_enps.is_empty() {
        lines.push("  eNPS:".to_string());
        for enps in emp.all_enps.iter().take(3) {
            let category = enps_category(enps.score);
            let survey = enps.survey_name.as_deref().unwrap_or("Survey");
            lines.push(format!("    - {} ({}): {} ({})",
                survey,
                enps.survey_date,
                enps.score,
                category
            ));
            if let Some(ref feedback) = enps.feedback {
                let truncated = if feedback.len() > 100 {
                    format!("{}...", &feedback[..100])
                } else {
                    feedback.clone()
                };
                lines.push(format!("      \"{}\"\n", truncated));
            }
        }
        if let Some(ref trend) = emp.enps_trend {
            lines.push(format!("    Trend: {}", trend));
        }
    }

    // V2.2.1: Career summary and highlights (extracted from reviews)
    if let Some(ref narrative) = emp.career_summary {
        lines.push("  Career Summary:".to_string());
        let excerpted = excerpt_to_sentences(narrative, summary_sentences);
        lines.push(format!("    {}", excerpted));
    }

    if !emp.key_strengths.is_empty() || !emp.development_areas.is_empty() {
        if !emp.key_strengths.is_empty() {
            lines.push(format!("  Key Strengths: {}", emp.key_strengths.join(", ")));
        }
        if !emp.development_areas.is_empty() {
            lines.push(format!("  Development Areas: {}", emp.development_areas.join(", ")));
        }
    }

    if !emp.recent_highlights.is_empty() {
        lines.push("  Recent Review Highlights:".to_string());
        for h in emp.recent_highlights.iter().take(highlight_cycles) {
            let sentiment_emoji = match h.sentiment.as_str() {
                "positive" => "↑",
                "negative" => "↓",
                "mixed" => "↔",
                _ => "•",
            };
            lines.push(format!("    {} {} ({})", sentiment_emoji, h.cycle_name, h.sentiment));
            if !h.strengths.is_empty() {
                lines.push(format!("      Strengths: {}", h.strengths.join(", ")));
            }
            if !h.opportunities.is_empty() {
                lines.push(format!("      Growth areas: {}", h.opportunities.join(", ")));
            }
            if !h.themes.is_empty() {
                lines.push(format!("      Themes: {}", h.themes.join(", ")));
            }
        }
    }

    lines.join("\n")
}

/// Get eNPS category
fn enps_category(score: i32) -> &'static str {
    if score >= 9 {
        "Promoter"
    } else if score >= 7 {
        "Passive"
    } else {
        "Detractor"
    }
}

// ============================================================================
// Token Estimation Utilities
// ============================================================================

/// Estimate token count from text length (conservative: ~4 chars per token)
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() + CHARS_PER_TOKEN - 1) / CHARS_PER_TOKEN
}

/// Convert a token budget to approximate character budget
#[allow(dead_code)]
pub fn tokens_to_chars(tokens: usize) -> usize {
    tokens * CHARS_PER_TOKEN
}

/// Get the maximum conversation token budget
pub fn get_max_conversation_tokens() -> usize {
    MAX_CONVERSATION_TOKENS
}

/// Compute a conversation token budget adapted to a model's context window.
pub fn compute_conversation_budget(context_window: usize) -> usize {
    (context_window * 3) / 4
}

/// Get the maximum system prompt token budget
#[allow(dead_code)]
pub fn get_max_system_prompt_tokens() -> usize {
    MAX_SYSTEM_PROMPT_TOKENS
}

// ============================================================================
// System Prompt Building
// ============================================================================

/// Build the complete system prompt for Claude
pub fn build_system_prompt(
    company: Option<&CompanyContext>,
    aggregates: Option<&OrgAggregates>,
    employee_context: &str,
    document_context: &str,
    memory_summaries: &[String],
    user_name: Option<&str>,
    persona_id: Option<&str>,
) -> String {
    let persona = get_persona(persona_id);
    let company_name = company.map(|c| c.name.as_str()).unwrap_or("your company");
    let company_state = company.map(|c| c.state.as_str()).unwrap_or("your state");
    let user_display = user_name.unwrap_or("the HR team");

    let preamble = persona
        .preamble
        .replace("{user_display}", user_display)
        .replace("{company_name}", company_name)
        .replace("{company_state}", company_state);

    let company_info = if let Some(c) = company {
        format!(
            "{} is based in {} with {} active employees across {} departments.",
            c.name, c.state, c.employee_count, c.department_count
        )
    } else {
        "Company profile not yet configured.".to_string()
    };

    let org_data = if let Some(agg) = aggregates {
        format_org_aggregates(agg, company.map(|c| c.name.as_str()))
    } else {
        "Organization data not available.".to_string()
    };

    let memories = if memory_summaries.is_empty() {
        "No relevant past conversations.".to_string()
    } else {
        memory_summaries.join("\n\n")
    };

    let employee_section = if employee_context.is_empty() {
        String::new()
    } else {
        format!("\nRELEVANT EMPLOYEES:\n{}", employee_context)
    };

    let document_section = if document_context.is_empty() {
        String::new()
    } else {
        format!("\nRELEVANT DOCUMENTS:\n{}", document_context)
    };

    format!(
r#"{preamble}

COMMUNICATION STYLE:
{communication_style}

COMPANY CONTEXT:
{company_info}

{org_data}

CONTEXT AWARENESS:
- {company_name} is in {company_state}, so consider state-specific employment law
- When federal and state law differ, flag it clearly
- Reference specific employees by name when their data is relevant
- Build on previous conversations when you remember relevant context
- Use the ORGANIZATION DATA above to answer aggregate questions accurately
- When answering from company documents, cite the source naturally (e.g., "According to your Employee Handbook...")
- If document content conflicts with general knowledge, prefer the company's documented policy

BOUNDARIES:
- This is guidance, not legal advice—the user acknowledged this during setup
- For anything involving potential litigation, recommend legal counsel
- You don't have access to confidential investigation details
- Compensation data is not available (V1)
{employee_section}
{document_section}

RELEVANT PAST CONVERSATIONS:
{memories}

Answer questions as {persona_name} would—{persona_style}."#,
        preamble = preamble,
        communication_style = persona.communication_style,
        company_name = company_name,
        company_state = company_state,
        company_info = company_info,
        org_data = org_data,
        employee_section = employee_section,
        document_section = document_section,
        memories = memories,
        persona_name = persona.name,
        persona_style = persona.style.to_lowercase(),
    )
}

// ============================================================================
// Main Context Building Function
// ============================================================================

/// Maximum employees for list queries (lightweight summaries)
const MAX_LIST_EMPLOYEES: usize = 30;
/// Maximum employees for comparison queries (full profiles)
const MAX_COMPARISON_EMPLOYEES: usize = 8;
/// Maximum employees for individual queries
const MAX_INDIVIDUAL_EMPLOYEES: usize = 3;
/// Maximum employees for attrition queries
const MAX_ATTRITION_EMPLOYEES: usize = 10;
/// Maximum employees for general fallback queries
const MAX_GENERAL_EMPLOYEES: usize = 5;

/// Build complete context for a chat message using query-adaptive retrieval
pub async fn build_chat_context(
    pool: &DbPool,
    user_message: &str,
    selected_employee_id: Option<&str>,
) -> Result<ChatContext, ContextError> {
    let start_time = std::time::Instant::now();

    let mentions = extract_mentions(user_message);
    let query_type = classify_query(user_message, &mentions);

    let token_budget = TokenBudget::for_query_type(query_type);

    let company = get_company_context(pool).await?;

    let aggregates = match build_org_aggregates(pool).await {
        Ok(agg) => Some(agg),
        Err(e) => {
            log::warn!("Failed to build org aggregates: {}", e);
            None
        }
    };

    let (employees, employee_summaries) = match query_type {
        QueryType::Aggregate => (vec![], vec![]),
        QueryType::List => {
            let summaries = build_employee_list(pool, &mentions, MAX_LIST_EMPLOYEES).await?;
            (vec![], summaries)
        }
        QueryType::Individual => {
            let employees = find_relevant_employees(
                pool,
                &mentions,
                MAX_INDIVIDUAL_EMPLOYEES,
                selected_employee_id,
            )
            .await?;
            (employees, vec![])
        }
        QueryType::Comparison => {
            if mentions.is_theme_query && !mentions.requested_themes.is_empty() {
                let dept = mentions.departments.first().map(|s| s.as_str());
                let employees = find_employees_by_theme(
                    pool,
                    &mentions.requested_themes,
                    dept,
                    mentions.theme_target,
                    MAX_COMPARISON_EMPLOYEES,
                )
                .await?;
                (employees, vec![])
            } else {
                let employees = find_relevant_employees(
                    pool,
                    &mentions,
                    MAX_COMPARISON_EMPLOYEES,
                    selected_employee_id,
                )
                .await?;
                (employees, vec![])
            }
        }
        QueryType::Attrition => {
            let employees = find_recent_terminations(pool, MAX_ATTRITION_EMPLOYEES).await?;
            (employees, vec![])
        }
        QueryType::General => {
            let employees = find_relevant_employees(
                pool,
                &mentions,
                MAX_GENERAL_EMPLOYEES,
                selected_employee_id,
            )
            .await?;
            (employees, vec![])
        }
    };

    let mut employee_ids_used: Vec<String> = employees.iter().map(|e| e.id.clone()).collect();
    employee_ids_used.extend(employee_summaries.iter().map(|e| e.id.clone()));

    let memory_summaries: Vec<String> = match memory::find_relevant_memories(
        pool,
        user_message,
        memory::DEFAULT_MEMORY_LIMIT,
    )
    .await
    {
        Ok(memories) => memories.into_iter().map(|m| m.summary).collect(),
        Err(e) => {
            log::warn!("Failed to retrieve memories: {}", e);
            Vec::new()
        }
    };

    let employees_included = employees.len() + employee_summaries.len();
    let memories_included = memory_summaries.len();

    let employee_tokens = if !employees.is_empty() {
        employees.len() * 500 / CHARS_PER_TOKEN
    } else {
        employee_summaries.len() * 70 / CHARS_PER_TOKEN
    };

    let memory_tokens = memory_summaries
        .iter()
        .map(|m| m.len() / CHARS_PER_TOKEN)
        .sum();

    let aggregates_tokens = if aggregates.is_some() { 500 } else { 0 };

    let token_usage = TokenUsage {
        employee_tokens,
        memory_tokens,
        aggregates_tokens,
        total_tokens: employee_tokens + memory_tokens + aggregates_tokens,
    };

    let retrieval_time_ms = start_time.elapsed().as_millis() as u64;
    let metrics = RetrievalMetrics {
        query_type,
        employees_found: employees_included,
        employees_included,
        memories_found: memories_included,
        memories_included,
        aggregates_included: aggregates.is_some(),
        token_budget,
        token_usage,
        retrieval_time_ms,
    };

    let document_chunks: Vec<crate::documents::RetrievedChunk> =
        match crate::documents::search_documents(pool, user_message).await {
            Ok(chunks) => chunks,
            Err(e) => {
                log::warn!("Failed to search documents: {}", e);
                Vec::new()
            }
        };

    Ok(ChatContext {
        company,
        aggregates,
        query_type,
        employees,
        employee_summaries,
        employee_ids_used,
        memory_summaries,
        document_chunks,
        metrics,
    })
}

/// Get the system prompt for a chat message
pub async fn get_system_prompt_for_message(
    pool: &DbPool,
    user_message: &str,
    selected_employee_id: Option<&str>,
) -> Result<SystemPromptResult, ContextError> {
    let context = build_chat_context(pool, user_message, selected_employee_id).await?;

    let user_name = crate::settings::get_setting(pool, "user_name")
        .await
        .ok()
        .flatten();

    let persona_id = crate::settings::get_setting(pool, "persona")
        .await
        .ok()
        .flatten();

    let employee_context = if !context.employees.is_empty() {
        format_employee_context(&context.employees)
    } else if !context.employee_summaries.is_empty() {
        let total_count = context.aggregates.as_ref().map(|a| a.total_employees);
        format_employee_summaries(&context.employee_summaries, total_count)
    } else {
        String::new()
    };

    let document_context = crate::documents::format_document_context(&context.document_chunks);

    let system_prompt = build_system_prompt(
        context.company.as_ref(),
        context.aggregates.as_ref(),
        &employee_context,
        &document_context,
        &context.memory_summaries,
        user_name.as_deref(),
        persona_id.as_deref(),
    );

    Ok(SystemPromptResult {
        system_prompt,
        employee_ids_used: context.employee_ids_used,
        aggregates: context.aggregates,
        query_type: context.query_type,
        metrics: context.metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::retrieval::{CycleHighlight, EnpsInfo, RatingInfo};

    #[test]
    fn test_enps_category() {
        assert_eq!(enps_category(10), "Promoter");
        assert_eq!(enps_category(9), "Promoter");
        assert_eq!(enps_category(8), "Passive");
        assert_eq!(enps_category(7), "Passive");
        assert_eq!(enps_category(6), "Detractor");
        assert_eq!(enps_category(0), "Detractor");
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short_text() {
        assert_eq!(estimate_tokens("Hello"), 2);
    }

    #[test]
    fn test_estimate_tokens_exact_multiple() {
        assert_eq!(estimate_tokens("12345678"), 2);
    }

    #[test]
    fn test_estimate_tokens_rounds_up() {
        assert_eq!(estimate_tokens("123456789"), 3);
    }

    #[test]
    fn test_estimate_tokens_longer_text() {
        let text = "a".repeat(100);
        assert_eq!(estimate_tokens(&text), 25);
    }

    #[test]
    fn test_tokens_to_chars() {
        assert_eq!(tokens_to_chars(100), 400);
        assert_eq!(tokens_to_chars(0), 0);
        assert_eq!(tokens_to_chars(1), 4);
    }

    #[test]
    fn test_get_max_conversation_tokens() {
        assert_eq!(get_max_conversation_tokens(), 150_000);
    }

    #[test]
    fn test_format_employee_summaries_empty() {
        let summaries: Vec<EmployeeSummary> = vec![];
        let result = format_employee_summaries(&summaries, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_employee_summaries_single() {
        let summaries = vec![EmployeeSummary {
            id: "1".to_string(),
            full_name: "Sarah Chen".to_string(),
            department: Some("Marketing".to_string()),
            job_title: Some("Marketing Manager".to_string()),
            status: "active".to_string(),
            hire_date: Some("2020-03-15".to_string()),
        }];

        let result = format_employee_summaries(&summaries, None);

        assert!(result.contains("EMPLOYEES (1):"));
        assert!(result.contains("Sarah Chen"));
        assert!(result.contains("Marketing Manager"));
        assert!(result.contains("Marketing"));
        assert!(result.contains("active"));
        assert!(result.contains("hired 2020-03-15"));
    }

    #[test]
    fn test_format_employee_summaries_multiple() {
        let summaries = vec![
            EmployeeSummary {
                id: "1".to_string(),
                full_name: "Sarah Chen".to_string(),
                department: Some("Marketing".to_string()),
                job_title: Some("Marketing Manager".to_string()),
                status: "active".to_string(),
                hire_date: Some("2020-03-15".to_string()),
            },
            EmployeeSummary {
                id: "2".to_string(),
                full_name: "John Smith".to_string(),
                department: Some("Engineering".to_string()),
                job_title: Some("Senior Engineer".to_string()),
                status: "active".to_string(),
                hire_date: Some("2019-01-10".to_string()),
            },
        ];

        let result = format_employee_summaries(&summaries, None);

        assert!(result.contains("EMPLOYEES (2):"));
        assert!(result.contains("Sarah Chen"));
        assert!(result.contains("John Smith"));
    }

    #[test]
    fn test_format_employee_summaries_with_total_count() {
        let summaries = vec![EmployeeSummary {
            id: "1".to_string(),
            full_name: "Sarah Chen".to_string(),
            department: Some("Marketing".to_string()),
            job_title: Some("Marketing Manager".to_string()),
            status: "active".to_string(),
            hire_date: None,
        }];

        let result = format_employee_summaries(&summaries, Some(28));

        assert!(result.contains("EMPLOYEES (showing 1 of 28):"));
    }

    #[test]
    fn test_format_employee_summaries_total_equals_shown() {
        let summaries = vec![
            EmployeeSummary {
                id: "1".to_string(),
                full_name: "Sarah Chen".to_string(),
                department: Some("Marketing".to_string()),
                job_title: Some("Manager".to_string()),
                status: "active".to_string(),
                hire_date: None,
            },
            EmployeeSummary {
                id: "2".to_string(),
                full_name: "John Smith".to_string(),
                department: Some("Engineering".to_string()),
                job_title: Some("Engineer".to_string()),
                status: "active".to_string(),
                hire_date: None,
            },
        ];

        let result = format_employee_summaries(&summaries, Some(2));

        assert!(result.contains("EMPLOYEES (2):"));
        assert!(!result.contains("showing"));
    }

    #[test]
    fn test_format_employee_summaries_missing_fields() {
        let summaries = vec![EmployeeSummary {
            id: "1".to_string(),
            full_name: "New Hire".to_string(),
            department: None,
            job_title: None,
            status: "active".to_string(),
            hire_date: None,
        }];

        let result = format_employee_summaries(&summaries, None);

        assert!(result.contains("New Hire"));
        assert!(result.contains("No title"));
        assert!(result.contains("Unassigned"));
        assert!(!result.contains("hired"));
    }

    #[test]
    fn test_employee_summary_size_budget() {
        let summaries: Vec<EmployeeSummary> = (0..30)
            .map(|i| EmployeeSummary {
                id: format!("{}", i),
                full_name: format!("Employee Name {}", i),
                department: Some("Engineering".to_string()),
                job_title: Some("Software Engineer".to_string()),
                status: "active".to_string(),
                hire_date: Some("2023-01-01".to_string()),
            })
            .collect();

        let result = format_employee_summaries(&summaries, Some(100));

        assert!(
            result.len() < 3000,
            "Summary list too large: {} chars",
            result.len()
        );
    }

    #[test]
    fn test_get_persona_default() {
        let persona = get_persona(None);
        assert_eq!(persona.id, "alex");
        assert_eq!(persona.name, "Alex");
    }

    #[test]
    fn test_get_persona_by_id() {
        let jordan = get_persona(Some("jordan"));
        assert_eq!(jordan.id, "jordan");
        assert_eq!(jordan.name, "Jordan");
        assert!(jordan.style.contains("compliance"));

        let sam = get_persona(Some("sam"));
        assert_eq!(sam.id, "sam");
        assert!(sam.style.contains("direct"));

        let morgan = get_persona(Some("morgan"));
        assert_eq!(morgan.id, "morgan");
        assert!(morgan.style.contains("analytical"));

        let taylor = get_persona(Some("taylor"));
        assert_eq!(taylor.id, "taylor");
        assert!(taylor.style.contains("empathetic"));
    }

    #[test]
    fn test_get_persona_invalid_fallback() {
        let persona = get_persona(Some("invalid_persona"));
        assert_eq!(persona.id, "alex");
    }

    #[test]
    fn test_persona_preamble_has_placeholders() {
        for persona in PERSONAS.iter() {
            assert!(
                persona.preamble.contains("{user_display}"),
                "{} preamble missing {{user_display}}",
                persona.name
            );
            assert!(
                persona.preamble.contains("{company_name}"),
                "{} preamble missing {{company_name}}",
                persona.name
            );
            assert!(
                persona.preamble.contains("{company_state}"),
                "{} preamble missing {{company_state}}",
                persona.name
            );
        }
    }

    fn make_test_employee_with_highlights() -> EmployeeContext {
        EmployeeContext {
            id: "emp-1".to_string(),
            full_name: "Sarah Chen".to_string(),
            email: "sarah@company.com".to_string(),
            department: Some("Engineering".to_string()),
            job_title: Some("Senior Engineer".to_string()),
            hire_date: Some("2020-01-15".to_string()),
            work_state: Some("California".to_string()),
            status: "Active".to_string(),
            manager_name: Some("John Doe".to_string()),
            latest_rating: Some(4.2),
            latest_rating_cycle: Some("2024 H2".to_string()),
            rating_trend: Some("improving".to_string()),
            all_ratings: vec![
                RatingInfo {
                    cycle_name: "2024 H2".to_string(),
                    overall_rating: 4.2,
                    rating_date: Some("2024-12-01".to_string()),
                },
            ],
            latest_enps: Some(9),
            latest_enps_date: Some("2024-11-01".to_string()),
            enps_trend: Some("stable".to_string()),
            all_enps: vec![],
            career_summary: Some("Sarah is a high-performing engineer with strong technical leadership skills.".to_string()),
            key_strengths: vec!["Technical leadership".to_string(), "Problem solving".to_string(), "Mentoring".to_string()],
            development_areas: vec!["Public speaking".to_string(), "Documentation".to_string()],
            recent_highlights: vec![
                CycleHighlight {
                    cycle_name: "2024 H2".to_string(),
                    strengths: vec!["Led v2 migration".to_string(), "Improved test coverage".to_string()],
                    opportunities: vec!["Cross-team communication".to_string()],
                    themes: vec!["leadership".to_string(), "technical-growth".to_string()],
                    sentiment: "positive".to_string(),
                },
                CycleHighlight {
                    cycle_name: "2024 H1".to_string(),
                    strengths: vec!["Delivered key feature".to_string()],
                    opportunities: vec!["Meeting deadlines".to_string()],
                    themes: vec!["execution".to_string()],
                    sentiment: "mixed".to_string(),
                },
            ],
        }
    }

    #[test]
    fn test_format_employee_includes_career_summary() {
        let emp = make_test_employee_with_highlights();
        let formatted = format_single_employee(&emp);

        assert!(formatted.contains("Career Summary:"));
        assert!(formatted.contains("high-performing engineer"));
    }

    #[test]
    fn test_format_employee_includes_key_strengths() {
        let emp = make_test_employee_with_highlights();
        let formatted = format_single_employee(&emp);

        assert!(formatted.contains("Key Strengths:"));
        assert!(formatted.contains("Technical leadership"));
        assert!(formatted.contains("Problem solving"));
    }

    #[test]
    fn test_format_employee_includes_development_areas() {
        let emp = make_test_employee_with_highlights();
        let formatted = format_single_employee(&emp);

        assert!(formatted.contains("Development Areas:"));
        assert!(formatted.contains("Public speaking"));
    }

    #[test]
    fn test_format_employee_includes_recent_highlights() {
        let emp = make_test_employee_with_highlights();
        let formatted = format_single_employee(&emp);

        assert!(formatted.contains("Recent Review Highlights:"));
        assert!(formatted.contains("2024 H2"));
        assert!(formatted.contains("2024 H1"));
        assert!(formatted.contains("Led v2 migration"));
        assert!(formatted.contains("leadership"));
    }

    #[test]
    fn test_format_employee_sentiment_indicators() {
        let emp = make_test_employee_with_highlights();
        let formatted = format_single_employee(&emp);

        assert!(formatted.contains("↑ 2024 H2 (positive)"));
        assert!(formatted.contains("↔ 2024 H1 (mixed)"));
    }

    #[test]
    fn test_format_employee_without_highlights_still_works() {
        let emp = EmployeeContext {
            id: "emp-2".to_string(),
            full_name: "New Employee".to_string(),
            email: "new@company.com".to_string(),
            department: Some("Sales".to_string()),
            job_title: Some("Sales Rep".to_string()),
            hire_date: None,
            work_state: None,
            status: "Active".to_string(),
            manager_name: None,
            latest_rating: None,
            latest_rating_cycle: None,
            rating_trend: None,
            all_ratings: vec![],
            latest_enps: None,
            latest_enps_date: None,
            enps_trend: None,
            all_enps: vec![],
            career_summary: None,
            key_strengths: vec![],
            development_areas: vec![],
            recent_highlights: vec![],
        };

        let formatted = format_single_employee(&emp);

        assert!(formatted.contains("New Employee"));
        assert!(formatted.contains("Active"));
        assert!(!formatted.contains("Career Summary:"));
        assert!(!formatted.contains("Key Strengths:"));
        assert!(!formatted.contains("Recent Review Highlights:"));
    }

    #[test]
    fn test_token_budget_for_aggregate_query() {
        let budget = TokenBudget::for_query_type(QueryType::Aggregate);
        assert_eq!(budget.employee_context, 0);
        assert_eq!(budget.theme_context, 500);
        assert_eq!(budget.memory_context, 500);
        assert_eq!(budget.total_context, 1_000);
    }

    #[test]
    fn test_token_budget_for_individual_query() {
        let budget = TokenBudget::for_query_type(QueryType::Individual);
        assert_eq!(budget.employee_context, 4_000);
        assert_eq!(budget.theme_context, 0);
        assert_eq!(budget.memory_context, 1_000);
        assert_eq!(budget.total_context, 5_000);
    }

    #[test]
    fn test_token_budget_for_list_query() {
        let budget = TokenBudget::for_query_type(QueryType::List);
        assert_eq!(budget.employee_context, 2_000);
        assert_eq!(budget.total_context, 2_500);
    }

    #[test]
    fn test_token_budget_for_comparison_query() {
        let budget = TokenBudget::for_query_type(QueryType::Comparison);
        assert_eq!(budget.employee_context, 3_000);
        assert_eq!(budget.total_context, 3_500);
    }

    #[test]
    fn test_token_budget_for_attrition_query() {
        let budget = TokenBudget::for_query_type(QueryType::Attrition);
        assert_eq!(budget.employee_context, 2_000);
        assert_eq!(budget.total_context, 2_500);
    }

    #[test]
    fn test_token_budget_for_general_query() {
        let budget = TokenBudget::for_query_type(QueryType::General);
        assert_eq!(budget.employee_context, 2_000);
        assert_eq!(budget.memory_context, 1_000);
        assert_eq!(budget.total_context, 3_000);
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.employee_tokens, 0);
        assert_eq!(usage.memory_tokens, 0);
        assert_eq!(usage.aggregates_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_retrieval_metrics_default() {
        let metrics = RetrievalMetrics::default();
        assert_eq!(metrics.query_type, QueryType::General);
        assert_eq!(metrics.employees_found, 0);
        assert_eq!(metrics.employees_included, 0);
        assert_eq!(metrics.memories_found, 0);
        assert_eq!(metrics.memories_included, 0);
        assert!(!metrics.aggregates_included);
        assert_eq!(metrics.retrieval_time_ms, 0);
    }

    #[test]
    fn test_excerpt_to_sentences_empty() {
        assert_eq!(excerpt_to_sentences("", 3), "");
        assert_eq!(excerpt_to_sentences("  ", 3), "");
    }

    #[test]
    fn test_excerpt_to_sentences_zero_max() {
        assert_eq!(excerpt_to_sentences("Hello world. This is a test.", 0), "");
    }

    #[test]
    fn test_excerpt_to_sentences_single_sentence() {
        let text = "This is a single sentence.";
        assert_eq!(excerpt_to_sentences(text, 3), "This is a single sentence.");
    }

    #[test]
    fn test_excerpt_to_sentences_exact_match() {
        let text = "First sentence. Second sentence. Third sentence.";
        assert_eq!(excerpt_to_sentences(text, 3), text);
    }

    #[test]
    fn test_excerpt_to_sentences_truncation() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence.";
        let result = excerpt_to_sentences(text, 2);
        assert!(result.starts_with("First sentence. Second sentence."));
        assert!(result.ends_with(".."));
    }

    #[test]
    fn test_excerpt_to_sentences_unicode() {
        let text = "Hello! How are you? I'm fine. Thanks for asking!";
        let result = excerpt_to_sentences(text, 2);
        assert!(result.starts_with("Hello! How are you?"));
        assert!(result.ends_with(".."));
    }

    #[test]
    fn test_excerpt_to_sentences_preserves_whitespace() {
        let text = "  First sentence.   Second sentence.  ";
        let result = excerpt_to_sentences(text, 1);
        assert!(result.starts_with("First sentence."));
    }

    #[test]
    fn test_calculate_excerpt_limits_full_budget() {
        let (summary, cycles) = calculate_excerpt_limits(1000);
        assert_eq!(summary, 5);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_calculate_excerpt_limits_reduced_budget() {
        let (summary, cycles) = calculate_excerpt_limits(500);
        assert_eq!(summary, 2);
        assert_eq!(cycles, 2);
    }

    #[test]
    fn test_calculate_excerpt_limits_tight_budget() {
        let (summary, cycles) = calculate_excerpt_limits(200);
        assert_eq!(summary, 1);
        assert_eq!(cycles, 1);
    }

    #[test]
    fn test_calculate_per_employee_budget_single() {
        assert_eq!(calculate_per_employee_budget(4000, 1), 4000);
    }

    #[test]
    fn test_calculate_per_employee_budget_multiple() {
        assert_eq!(calculate_per_employee_budget(4000, 4), 1000);
    }

    #[test]
    fn test_calculate_per_employee_budget_minimum_floor() {
        assert_eq!(calculate_per_employee_budget(1000, 10), 200);
        assert_eq!(calculate_per_employee_budget(500, 10), 200);
    }

    #[test]
    fn test_calculate_per_employee_budget_zero_employees() {
        assert_eq!(calculate_per_employee_budget(4000, 0), 4000);
    }
}
