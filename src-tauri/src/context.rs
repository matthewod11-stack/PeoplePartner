// HR Command Center - Context Builder Module
// Builds contextual system prompts for Claude with company and employee data
//
// Key responsibilities:
// 1. Extract employee mentions from user queries
// 2. Retrieve relevant employees with performance/eNPS data
// 3. Build system prompts with the "Alex" HR persona
// 4. Manage context size to stay within token limits

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

use crate::db::DbPool;
use crate::highlights;
use crate::memory;

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

/// Maximum characters for employee context (derived from token budget)
const MAX_EMPLOYEE_CONTEXT_CHARS: usize = MAX_EMPLOYEE_CONTEXT_TOKENS * CHARS_PER_TOKEN;

/// Maximum number of employees to include in context
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
// Query Classification Types (Phase 2.7)
// ============================================================================

/// Query classification result for adaptive context retrieval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryType {
    /// Stats questions: "How many...", "What's our...", "Overall..."
    Aggregate,
    /// Roster questions: "Who's in...", "Show me...", "List all..."
    List,
    /// Named employee questions: "Tell me about Sarah", "What's John's rating?"
    Individual,
    /// Ranking questions: "Top performers", "Who's struggling", "Best in Sales"
    Comparison,
    /// Turnover questions: "Who left", "Attrition rate", "Recent departures"
    Attrition,
    /// Can't determine — use fallback behavior
    General,
}

// ============================================================================
// Organization Aggregate Types (Phase 2.7)
// ============================================================================

/// Organization-wide aggregate statistics
/// Computed from full database for every query (~2K chars when formatted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgAggregates {
    // Headcount
    pub total_employees: i64,
    pub active_count: i64,
    pub terminated_count: i64,
    pub on_leave_count: i64,

    // By department (sorted by count descending)
    pub by_department: Vec<DepartmentCount>,

    // Performance (active employees only, most recent rating per employee)
    pub avg_rating: Option<f64>,
    pub rating_distribution: RatingDistribution,
    pub employees_with_no_rating: i64,

    // Engagement (reuses existing EnpsAggregate)
    pub enps: EnpsAggregate,

    // Attrition (YTD)
    pub attrition: AttritionStats,
}

/// Department headcount with percentage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentCount {
    pub name: String,
    pub count: i64,
    pub percentage: f64,
}

/// Performance rating distribution buckets
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RatingDistribution {
    /// Rating >= 4.5
    pub exceptional: i64,
    /// Rating 3.5 - 4.49
    pub exceeds: i64,
    /// Rating 2.5 - 3.49
    pub meets: i64,
    /// Rating < 2.5
    pub needs_improvement: i64,
}

/// Year-to-date attrition statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttritionStats {
    pub terminations_ytd: i64,
    pub voluntary: i64,
    pub involuntary: i64,
    pub avg_tenure_months: Option<f64>,
    pub turnover_rate_annualized: Option<f64>,
}

// ============================================================================
// Answer Verification Types (V2.1.4)
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

/// Result of get_system_prompt_for_message (V2.1.4)
/// Extended to include verification context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptResult {
    /// The system prompt to send to Claude
    pub system_prompt: String,
    /// Employee IDs included in context (for audit logging)
    pub employee_ids_used: Vec<String>,
    /// Organization aggregates (for verification)
    pub aggregates: Option<OrgAggregates>,
    /// Query classification (for verification)
    pub query_type: QueryType,
    /// Retrieval metrics for observability (V2.2.2)
    pub metrics: RetrievalMetrics,
}

// ============================================================================
// Token Budget & Retrieval Metrics (V2.2.2)
// ============================================================================

/// Token budget allocation per query type
/// Defines how many tokens to allocate for each context section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Token budget for employee context (profiles or summaries)
    pub employee_context: usize,
    /// Token budget for theme context (future: V2.2.2b)
    pub theme_context: usize,
    /// Token budget for memory context (past conversations)
    pub memory_context: usize,
    /// Combined total budget
    pub total_context: usize,
}

impl TokenBudget {
    /// Get the token budget configuration for a query type
    pub fn for_query_type(query_type: QueryType) -> Self {
        match query_type {
            QueryType::Aggregate => TokenBudget {
                employee_context: 0,      // Aggregates don't need individual employees
                theme_context: 500,       // Room for theme analysis
                memory_context: 500,
                total_context: 1_000,
            },
            QueryType::List => TokenBudget {
                employee_context: 2_000,  // Lightweight summaries
                theme_context: 0,
                memory_context: 500,
                total_context: 2_500,
            },
            QueryType::Individual => TokenBudget {
                employee_context: 4_000,  // Full profiles
                theme_context: 0,
                memory_context: 1_000,    // More memory for context
                total_context: 5_000,
            },
            QueryType::Comparison => TokenBudget {
                employee_context: 3_000,  // Multiple full profiles
                theme_context: 0,
                memory_context: 500,
                total_context: 3_500,
            },
            QueryType::Attrition => TokenBudget {
                employee_context: 2_000,  // Termination details
                theme_context: 0,
                memory_context: 500,
                total_context: 2_500,
            },
            QueryType::General => TokenBudget {
                employee_context: 2_000,  // Balanced
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
    /// Tokens used by employee context
    pub employee_tokens: usize,
    /// Tokens used by memory context
    pub memory_tokens: usize,
    /// Tokens used by organization aggregates
    pub aggregates_tokens: usize,
    /// Total tokens used (sum of all sections)
    pub total_tokens: usize,
}

/// Comprehensive retrieval metrics for observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMetrics {
    /// Query type classification
    pub query_type: QueryType,
    /// Number of employees matched by query
    pub employees_found: usize,
    /// Number of employees included in context
    pub employees_included: usize,
    /// Number of memories matched by query
    pub memories_found: usize,
    /// Number of memories included in context
    pub memories_included: usize,
    /// Whether organization aggregates were included
    pub aggregates_included: bool,
    /// Token budget allocation for this query type
    pub token_budget: TokenBudget,
    /// Actual token usage
    pub token_usage: TokenUsage,
    /// Total retrieval time in milliseconds
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

// ============================================================================
// Error Types
// ============================================================================

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

// ============================================================================
// Employee Context Types
// ============================================================================

/// Employee with performance and eNPS data for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeContext {
    pub id: String,
    pub full_name: String,
    pub email: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub hire_date: Option<String>,
    pub work_state: Option<String>,
    pub status: String,
    pub manager_name: Option<String>,

    // Performance data
    pub latest_rating: Option<f64>,
    pub latest_rating_cycle: Option<String>,
    pub rating_trend: Option<String>, // "improving", "stable", "declining"
    pub all_ratings: Vec<RatingInfo>,

    // eNPS data
    pub latest_enps: Option<i32>,
    pub latest_enps_date: Option<String>,
    pub enps_trend: Option<String>,
    pub all_enps: Vec<EnpsInfo>,

    // V2.2.1: Extracted highlights from performance reviews
    pub career_summary: Option<String>,
    pub key_strengths: Vec<String>,
    pub development_areas: Vec<String>,
    pub recent_highlights: Vec<CycleHighlight>,
}

/// Extracted highlight data for a single review cycle (V2.2.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleHighlight {
    pub cycle_name: String,
    pub strengths: Vec<String>,
    pub opportunities: Vec<String>,
    pub themes: Vec<String>,
    pub sentiment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingInfo {
    pub cycle_name: String,
    pub overall_rating: f64,
    pub rating_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnpsInfo {
    pub score: i32,
    pub survey_name: Option<String>,
    pub survey_date: String,
    pub feedback: Option<String>,
}

/// Company context for system prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyContext {
    pub name: String,
    pub state: String,
    pub industry: Option<String>,
    pub employee_count: i64,
    pub department_count: i64,
}

/// Lightweight employee summary for list queries (~70 chars each)
/// Used when showing rosters instead of full profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeSummary {
    pub id: String,
    pub full_name: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub status: String,
    pub hire_date: Option<String>,
}

/// Full context for building system prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatContext {
    pub company: Option<CompanyContext>,
    pub aggregates: Option<OrgAggregates>,          // Phase 2.7: org-wide stats
    pub query_type: QueryType,                      // Phase 2.7: classification result
    pub employees: Vec<EmployeeContext>,            // Full profiles (for Individual/Comparison)
    pub employee_summaries: Vec<EmployeeSummary>,   // Brief roster (for List queries)
    pub employee_ids_used: Vec<String>,
    pub memory_summaries: Vec<String>,
    pub metrics: RetrievalMetrics,                  // V2.2.2: retrieval observability
}

// ============================================================================
// Query Analysis
// ============================================================================

/// Direction for tenure-based queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenureDirection {
    /// "who's been here longest", "most senior"
    Longest,
    /// "newest employees", "recent hires", "just started"
    Newest,
    /// "upcoming anniversaries", "work anniversary"
    Anniversary,
}

/// Target for theme-based queries (V2.2.2b)
/// Determines which field to search in review highlights
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum ThemeTarget {
    /// Search all theme-related fields (themes, strengths, opportunities)
    #[default]
    Any,
    /// "excels at", "strong in", "praised for" → search strengths
    Strengths,
    /// "needs help", "struggles with", "concerns" → search opportunities
    Opportunities,
}

/// Extracted mentions from a user query
#[derive(Debug, Clone, Default)]
pub struct QueryMentions {
    /// Potential employee names found in query
    pub names: Vec<String>,
    /// Department names found in query
    pub departments: Vec<String>,
    /// Keywords suggesting aggregate queries (team, all, everyone, etc.)
    pub is_aggregate_query: bool,
    /// Keywords suggesting performance-related queries
    pub is_performance_query: bool,
    /// Keywords suggesting eNPS-related queries
    pub is_enps_query: bool,
    /// Keywords suggesting tenure-related queries
    pub is_tenure_query: bool,
    /// Keywords suggesting top performer queries
    pub is_top_performer_query: bool,
    /// Keywords suggesting underperformer queries
    pub is_underperformer_query: bool,
    /// Specific tenure direction (longest vs newest vs anniversary)
    pub tenure_direction: Option<TenureDirection>,
    /// Whether query wants aggregate stats rather than individual employees
    pub wants_aggregate: bool,
    /// V2.2.2b: Keywords suggesting theme-based queries
    pub is_theme_query: bool,
    /// V2.2.2b: Specific themes requested (e.g., "leadership", "communication")
    pub requested_themes: Vec<String>,
    /// V2.2.2b: Target field for theme search (strengths vs opportunities vs any)
    pub theme_target: ThemeTarget,
}

/// Extract potential employee names and departments from a query
/// Uses simple heuristics - looks for capitalized words that could be names
pub fn extract_mentions(query: &str) -> QueryMentions {
    let mut mentions = QueryMentions::default();

    // Common HR-related keywords that indicate aggregate queries
    let aggregate_keywords = [
        "team", "all", "everyone", "department", "org", "organization",
        "headcount", "turnover", "attrition", "company-wide", "across",
    ];

    let performance_keywords = [
        "performance", "rating", "review", "performer",
        "pip", "improvement plan", "developing", "exceeds", "exceptional",
    ];

    let enps_keywords = [
        "enps", "nps", "promoter", "engagement", "satisfaction", "survey",
        "detractor", "passive", "morale",
    ];

    // Tenure query keywords - phrases for direction detection
    let tenure_longest_keywords = [
        "been here longest", "longest tenure", "most senior", "longest serving",
        "been here the longest", "here longest", "oldest employee", "most tenured",
    ];
    let tenure_newest_keywords = [
        "newest", "recent hire", "recently hired", "just started", "new employee",
        "just joined", "newest hire", "most recent hire", "started recently",
    ];
    let tenure_anniversary_keywords = [
        "anniversary", "work anniversary", "tenure milestone", "years of service",
    ];
    let tenure_general_keywords = [
        "tenure", "how long", "been here", "started", "hire date", "joined",
    ];

    // Top performer keywords (distinct from general performance)
    let top_performer_keywords = [
        "top performer", "best performer", "high performer", "star employee",
        "exceptional performer", "highest rated", "best rated", "top rated",
        "strongest performer", "a-player", "highest performer",
    ];

    // Underperformer keywords (distinct from general performance)
    let underperformer_keywords = [
        "underperform", "low performer", "struggling", "needs improvement",
        "below expectations", "poor performer", "weakest", "lowest rated",
        "performance issue", "performance problem", "not performing",
    ];

    // Aggregate stat keywords (wants calculation, not individuals)
    let wants_aggregate_keywords = [
        "our enps", "company enps", "overall enps", "average enps",
        "how many", "total", "count", "percentage", "average rating",
        "overall rating", "company-wide", "across the company",
    ];

    let query_lower = query.to_lowercase();

    // Check for aggregate query indicators
    mentions.is_aggregate_query = aggregate_keywords
        .iter()
        .any(|kw| query_lower.contains(kw));

    mentions.is_performance_query = performance_keywords
        .iter()
        .any(|kw| query_lower.contains(kw));

    mentions.is_enps_query = enps_keywords
        .iter()
        .any(|kw| query_lower.contains(kw));

    // Check for tenure-related queries and direction
    if tenure_longest_keywords.iter().any(|kw| query_lower.contains(kw)) {
        mentions.is_tenure_query = true;
        mentions.tenure_direction = Some(TenureDirection::Longest);
    } else if tenure_newest_keywords.iter().any(|kw| query_lower.contains(kw)) {
        mentions.is_tenure_query = true;
        mentions.tenure_direction = Some(TenureDirection::Newest);
    } else if tenure_anniversary_keywords.iter().any(|kw| query_lower.contains(kw)) {
        mentions.is_tenure_query = true;
        mentions.tenure_direction = Some(TenureDirection::Anniversary);
    } else if tenure_general_keywords.iter().any(|kw| query_lower.contains(kw)) {
        mentions.is_tenure_query = true;
        // No specific direction - could be asking about a specific person's tenure
    }

    // Check for top performer queries
    mentions.is_top_performer_query = top_performer_keywords
        .iter()
        .any(|kw| query_lower.contains(kw));

    // Check for underperformer queries
    mentions.is_underperformer_query = underperformer_keywords
        .iter()
        .any(|kw| query_lower.contains(kw));

    // Check if query wants aggregate stats (not individual employees)
    mentions.wants_aggregate = wants_aggregate_keywords
        .iter()
        .any(|kw| query_lower.contains(kw));

    // Extract potential names (capitalized words, 2+ chars, not at sentence start)
    // This is a simple heuristic - more sophisticated NER could be added later
    let words: Vec<&str> = query.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        // Strip possessives before other cleaning (Sarah's → Sarah)
        let mut working_word = *word;
        if working_word.ends_with("'s") || working_word.ends_with("'s") {
            working_word = &working_word[..working_word.len() - 2];
        } else if working_word.ends_with("s'") {
            working_word = &working_word[..working_word.len() - 2];
        }
        // Now clean remaining punctuation
        let clean_word = working_word.trim_matches(|c: char| !c.is_alphanumeric());

        // Skip if too short or all lowercase
        if clean_word.len() < 2 {
            continue;
        }

        let first_char = clean_word.chars().next().unwrap_or(' ');
        if !first_char.is_uppercase() {
            continue;
        }

        // Skip common non-name capitalized words
        let skip_words = [
            // Common question/sentence starters
            "I", "The", "What", "Who", "How", "When", "Where", "Why",
            "Can", "Could", "Would", "Should", "Is", "Are", "Was", "Were",
            "Tell", "Show", "List", "Give", "Help", "Please", "Hello",
            // HR acronyms and terms
            "HR", "HR's", "PIP", "Q1", "Q2", "Q3", "Q4", "FY", "YTD",
            // Common HR nouns (not person names)
            "Employees", "Employee", "People", "Team", "Teams", "Staff",
            "Manager", "Managers", "Worker", "Workers", "Member", "Members",
            "Performer", "Performers", "Hire", "Hires", "Candidate", "Candidates",
            // Days and months
            "Monday", "Tuesday", "Wednesday", "Thursday", "Friday",
            "January", "February", "March", "April", "May", "June",
            "July", "August", "September", "October", "November", "December",
            // Department names (should not be treated as person names)
            "Engineering", "Marketing", "Sales", "Finance", "Operations",
            "Product", "Design", "Legal", "IT", "Research", "Development",
            "Executive", "Support", "Success",
        ];

        if skip_words.contains(&clean_word) {
            continue;
        }

        // Check if this might be a name (followed by another capitalized word = full name)
        if i + 1 < words.len() {
            let next_word = words[i + 1].trim_matches(|c: char| !c.is_alphanumeric());
            let next_first = next_word.chars().next().unwrap_or(' ');

            if next_first.is_uppercase() && !skip_words.contains(&next_word) {
                // Likely a full name
                mentions.names.push(format!("{} {}", clean_word, next_word));
            }
        }

        // Also add single names for partial matching
        if clean_word.len() >= 3 && !skip_words.contains(&clean_word) {
            mentions.names.push(clean_word.to_string());
        }
    }

    // Deduplicate names
    mentions.names.sort();
    mentions.names.dedup();

    // Extract department mentions (common department names)
    // Must match at word boundaries to avoid false positives (e.g., "wITh" matching "IT")
    let department_names = [
        "Engineering", "Marketing", "Sales", "Finance", "HR", "Human Resources",
        "Operations", "Product", "Design", "Legal", "Customer Support",
        "Customer Success", "IT", "Research", "Development", "R&D",
    ];

    let query_lower = query.to_lowercase();
    for dept in department_names {
        if matches_word_boundary(&query_lower, &dept.to_lowercase()) {
            mentions.departments.push(dept.to_string());
        }
    }

    // V2.2.2b: Theme-based query detection
    let lower = query.to_lowercase();

    // Map of query terms to canonical theme names
    let theme_map: &[(&str, &str)] = &[
        // Direct theme matches
        ("leadership", "leadership"),
        ("technical growth", "technical-growth"),
        ("technical-growth", "technical-growth"),
        ("communication", "communication"),
        ("collaboration", "collaboration"),
        ("execution", "execution"),
        ("learning", "learning"),
        ("innovation", "innovation"),
        ("mentoring", "mentoring"),
        ("problem solving", "problem-solving"),
        ("problem-solving", "problem-solving"),
        ("customer focus", "customer-focus"),
        ("customer-focus", "customer-focus"),
        // Semantic variants
        ("people skills", "communication"),
        ("interpersonal", "communication"),
        ("soft skills", "communication"),
        ("teamwork", "collaboration"),
        ("team player", "collaboration"),
        ("technical skills", "technical-growth"),
        ("coding", "technical-growth"),
        ("engineering skills", "technical-growth"),
        ("creative", "innovation"),
        ("creativity", "innovation"),
        ("coaching", "mentoring"),
        ("teaching", "mentoring"),
        ("analytical", "problem-solving"),
        ("client focus", "customer-focus"),
        ("customer service", "customer-focus"),
        ("delivery", "execution"),
        ("results", "execution"),
        ("growth mindset", "learning"),
        ("self-improvement", "learning"),
    ];

    // Detect themes in query
    for (term, theme) in theme_map {
        if lower.contains(term) {
            if !mentions.requested_themes.contains(&theme.to_string()) {
                mentions.requested_themes.push(theme.to_string());
            }
        }
    }

    // If themes found, mark as theme query
    if !mentions.requested_themes.is_empty() {
        mentions.is_theme_query = true;
    }

    // Detect theme target (strengths vs opportunities)
    let opportunity_phrases = [
        "needs help", "struggles with", "concerns about", "concerns with",
        "needs improvement", "development area", "working on", "improve",
        "weak in", "challenge with", "difficulty with", "issue with",
    ];
    let strength_phrases = [
        "excels at", "strong in", "praised for", "recognized for",
        "good at", "great at", "excellent", "skilled in", "talented",
    ];

    for phrase in opportunity_phrases {
        if lower.contains(phrase) {
            mentions.theme_target = ThemeTarget::Opportunities;
            break;
        }
    }
    // Strength phrases override if both match (explicit positive intent)
    for phrase in strength_phrases {
        if lower.contains(phrase) {
            mentions.theme_target = ThemeTarget::Strengths;
            break;
        }
    }

    mentions
}

// ============================================================================
// Query Classification (Phase 2.7)
// ============================================================================

/// Classify a query to determine the appropriate context retrieval strategy.
/// Uses priority-based logic to handle ambiguous queries.
///
/// Priority order:
/// 1. Individual - explicit names always win
/// 2. Comparison - ranking/filtering queries
/// 3. Attrition - turnover-specific queries
/// 4. List - roster requests
/// 5. Aggregate - stats/counts/status checks
/// 6. General - fallback
pub fn classify_query(message: &str, mentions: &QueryMentions) -> QueryType {
    let lower = message.to_lowercase();

    // Priority 1: Individual (explicit names always win, unless aggregate query)
    if !mentions.names.is_empty() && !mentions.wants_aggregate {
        return QueryType::Individual;
    }

    // Priority 2: Comparison (ranking/filtering)
    if mentions.is_top_performer_query || mentions.is_underperformer_query {
        return QueryType::Comparison;
    }

    // Priority 3: Attrition (turnover-specific)
    if is_attrition_query(&lower) {
        return QueryType::Attrition;
    }

    // Priority 3.5: Theme-based queries (V2.2.2b)
    // "who has leadership feedback?", "communication issues in Engineering"
    if mentions.is_theme_query {
        return QueryType::Comparison; // Reuse Comparison for employee filtering by theme
    }

    // Priority 4: List (roster requests)
    if is_list_query(&lower, mentions) {
        return QueryType::List;
    }

    // Priority 5: Aggregate (stats/counts or status checks)
    if mentions.wants_aggregate || is_aggregate_query(&lower) || is_status_check(&lower) {
        return QueryType::Aggregate;
    }

    // Fallback
    QueryType::General
}

/// Check if a term appears at word boundaries in the text
/// Returns true if the term is surrounded by non-alphanumeric chars or string start/end
/// This prevents false positives like "wITh" matching "IT"
fn matches_word_boundary(text: &str, term: &str) -> bool {
    let mut search_start = 0;
    while let Some(pos) = text[search_start..].find(term) {
        let abs_pos = search_start + pos;
        let term_end = abs_pos + term.len();

        // Check character before match (or start of string)
        let valid_start = abs_pos == 0
            || !text[..abs_pos]
                .chars()
                .last()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);

        // Check character after match (or end of string)
        let valid_end = term_end >= text.len()
            || !text[term_end..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);

        if valid_start && valid_end {
            return true;
        }

        // Continue searching from next position
        search_start = abs_pos + 1;
        if search_start >= text.len() {
            break;
        }
    }
    false
}

/// Check if query is attrition/turnover focused
fn is_attrition_query(lower: &str) -> bool {
    let attrition_keywords = [
        "attrition",
        "turnover",
        "who left",
        "who's left",
        "departed",
        "terminated",
        "resignation",
        "quit",
        "recent departures",
        "offboarding",
        "left the company",
        "left this year",
        "voluntary departure",
        "involuntary termination",
    ];

    attrition_keywords.iter().any(|kw| lower.contains(kw))
}

/// Check if query is a list/roster request
fn is_list_query(lower: &str, mentions: &QueryMentions) -> bool {
    let list_keywords = [
        "who's in",
        "who is in",
        "show me",
        "list all",
        "list the",
        "all employees",
        "everyone in",
        "people in",
        "members of",
        "the team in",
        "employees in",
    ];

    // Direct list keyword match
    if list_keywords.iter().any(|kw| lower.contains(kw)) {
        return true;
    }

    // Department mentioned without aggregate keywords = likely wants roster
    if !mentions.departments.is_empty()
        && !mentions.wants_aggregate
        && !mentions.is_top_performer_query
        && !mentions.is_underperformer_query
    {
        // Check for roster-style phrasing
        let roster_patterns = ["who", "show", "list", "tell me about the"];
        if roster_patterns.iter().any(|p| lower.contains(p)) {
            return true;
        }
    }

    false
}

/// Check if query wants aggregate stats (broader than wants_aggregate flag)
fn is_aggregate_query(lower: &str) -> bool {
    let aggregate_keywords = [
        "how many",
        "what's our",
        "what is our",
        "total number",
        "count of",
        "average",
        "overall",
        "company-wide",
        "org-wide",
        "percentage",
        "rate",
        "headcount",
        "breakdown",
        "distribution",
        "summary",
        "statistics",
        "metrics",
    ];

    aggregate_keywords.iter().any(|kw| lower.contains(kw))
}

/// Check if query is a status check (e.g., "How's X doing?")
/// These are aggregate-style questions even without explicit aggregate keywords
fn is_status_check(lower: &str) -> bool {
    let status_patterns = [
        "how's the",
        "how is the",
        "how are the",
        "how's our",
        "how is our",
        "doing overall",
        "team doing",
        "department doing",
    ];

    status_patterns.iter().any(|p| lower.contains(p))
}

// ============================================================================
// Context Retrieval
// ============================================================================

/// Internal struct for employee query result
#[derive(Debug, FromRow)]
struct EmployeeRow {
    id: String,
    email: String,
    full_name: String,
    department: Option<String>,
    job_title: Option<String>,
    hire_date: Option<String>,
    work_state: Option<String>,
    status: String,
    manager_id: Option<String>,
}

/// Internal struct for rating query result
#[derive(Debug, FromRow)]
struct RatingRow {
    overall_rating: f64,
    cycle_name: String,
    rating_date: Option<String>,
}

/// Internal struct for eNPS query result
#[derive(Debug, FromRow)]
struct EnpsRow {
    score: i32,
    survey_name: Option<String>,
    survey_date: String,
    feedback_text: Option<String>,
}

/// Find employees matching the extracted mentions
/// Routes to specialized retrieval functions based on query type (primary intent)
/// If selected_employee_id is provided, that employee is always included first
pub async fn find_relevant_employees(
    pool: &DbPool,
    mentions: &QueryMentions,
    limit: usize,
    selected_employee_id: Option<&str>,
) -> Result<Vec<EmployeeContext>, ContextError> {
    // If a specific employee is selected, always include them first
    let (selected_employee, remaining_limit) = if let Some(id) = selected_employee_id {
        match get_employee_context(pool, id).await {
            Ok(emp) => (Some(emp), limit.saturating_sub(1)),
            Err(_) => (None, limit), // ID not found, continue without
        }
    } else {
        (None, limit)
    };
    // Helper to prepend selected employee and filter duplicates
    let finalize_results = |mut employees: Vec<EmployeeContext>| {
        if let Some(ref selected) = selected_employee {
            // Remove selected employee if already in list (avoid duplicates)
            employees.retain(|e| e.id != selected.id);
            // Prepend selected employee
            let mut result = vec![selected.clone()];
            result.extend(employees);
            result
        } else {
            employees
        }
    };

    // Priority 1: Underperformer queries (most specific)
    if mentions.is_underperformer_query {
        let employees = find_underperformers(pool, remaining_limit).await?;
        return Ok(finalize_results(employees));
    }

    // Priority 2: Top performer queries
    if mentions.is_top_performer_query {
        let employees = find_top_performers(pool, remaining_limit).await?;
        return Ok(finalize_results(employees));
    }

    // Priority 3: Tenure queries with direction
    if mentions.is_tenure_query {
        let employees = match mentions.tenure_direction {
            Some(TenureDirection::Longest) => find_longest_tenure(pool, remaining_limit).await?,
            Some(TenureDirection::Newest) => find_newest_employees(pool, remaining_limit).await?,
            Some(TenureDirection::Anniversary) => find_upcoming_anniversaries(pool, remaining_limit).await?,
            None => find_longest_tenure(pool, remaining_limit).await?, // Default to longest if direction unclear
        };
        return Ok(finalize_results(employees));
    }

    // Priority 4: Name-based search (explicit employee mentions)
    let mut employee_ids: Vec<String> = Vec::new();

    // Get selected employee info for smart filtering
    let selected_id = selected_employee.as_ref().map(|e| e.id.as_str());
    let selected_name_lower = selected_employee
        .as_ref()
        .map(|e| e.full_name.to_lowercase());

    for name in &mentions.names {
        // If an employee is selected AND their name matches this query name,
        // skip searching for other employees with the same name.
        // This prevents "Tell me about Amanda" from returning all Amandas
        // when the user has already selected a specific Amanda.
        if let Some(ref sel_name) = selected_name_lower {
            let name_lower = name.to_lowercase();
            if sel_name.contains(&name_lower) || name_lower.contains(sel_name.split_whitespace().next().unwrap_or("")) {
                // Selected employee's name matches this query name — skip other matches
                continue;
            }
        }

        let pattern = format!("%{}%", name);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM employees WHERE full_name LIKE ? LIMIT 5"
        )
        .bind(&pattern)
        .fetch_all(pool)
        .await?;

        for (id,) in rows {
            if !employee_ids.contains(&id) && Some(id.as_str()) != selected_id {
                employee_ids.push(id);
            }
        }
    }

    // Priority 5: Department-based search
    for dept in &mentions.departments {
        let pattern = format!("%{}%", dept);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM employees WHERE department LIKE ? AND status = 'active' LIMIT 10"
        )
        .bind(&pattern)
        .fetch_all(pool)
        .await?;

        for (id,) in rows {
            if !employee_ids.contains(&id) && Some(id.as_str()) != selected_id {
                employee_ids.push(id);
            }
        }
    }

    // Priority 6: Aggregate query fallback (random sample)
    if employee_ids.is_empty() && mentions.is_aggregate_query {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM employees WHERE status = 'active' ORDER BY RANDOM() LIMIT ?"
        )
        .bind(remaining_limit as i64)
        .fetch_all(pool)
        .await?;

        for (id,) in rows {
            if Some(id.as_str()) != selected_id {
                employee_ids.push(id);
            }
        }
    }

    // Limit results
    employee_ids.truncate(remaining_limit);

    // Fetch full employee context for each ID
    let mut employees = Vec::new();
    for id in employee_ids {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }

    Ok(finalize_results(employees))
}

/// Get full context for a single employee including performance and eNPS
pub async fn get_employee_context(
    pool: &DbPool,
    employee_id: &str,
) -> Result<EmployeeContext, ContextError> {
    // Get employee basic info
    let emp: EmployeeRow = sqlx::query_as(
        "SELECT id, email, full_name, department, job_title, hire_date, work_state, status, manager_id FROM employees WHERE id = ?"
    )
    .bind(employee_id)
    .fetch_one(pool)
    .await?;

    // Get manager name if exists
    let manager_name: Option<String> = if let Some(ref manager_id) = emp.manager_id {
        sqlx::query("SELECT full_name FROM employees WHERE id = ?")
            .bind(manager_id)
            .fetch_optional(pool)
            .await?
            .map(|row| row.get("full_name"))
    } else {
        None
    };

    // Get performance ratings with cycle names
    let ratings: Vec<RatingRow> = sqlx::query_as(
        r#"
        SELECT pr.overall_rating, rc.name as cycle_name, pr.rating_date
        FROM performance_ratings pr
        JOIN review_cycles rc ON pr.review_cycle_id = rc.id
        WHERE pr.employee_id = ?
        ORDER BY rc.start_date DESC
        "#
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;

    // Get eNPS responses
    let enps_responses: Vec<EnpsRow> = sqlx::query_as(
        "SELECT score, survey_name, survey_date, feedback_text FROM enps_responses WHERE employee_id = ? ORDER BY survey_date DESC"
    )
    .bind(employee_id)
    .fetch_all(pool)
    .await?;

    // Calculate rating trend
    let rating_trend = calculate_trend(&ratings.iter().map(|r| r.overall_rating).collect::<Vec<_>>());

    // Calculate eNPS trend
    let enps_trend = calculate_trend(
        &enps_responses.iter().map(|e| e.score as f64).collect::<Vec<_>>()
    );

    // Build rating info list
    let all_ratings: Vec<RatingInfo> = ratings
        .iter()
        .map(|r| RatingInfo {
            cycle_name: r.cycle_name.clone(),
            overall_rating: r.overall_rating,
            rating_date: r.rating_date.clone(),
        })
        .collect();

    // Build eNPS info list
    let all_enps: Vec<EnpsInfo> = enps_responses
        .iter()
        .map(|e| EnpsInfo {
            score: e.score,
            survey_name: e.survey_name.clone(),
            survey_date: e.survey_date.clone(),
            feedback: e.feedback_text.clone(),
        })
        .collect();

    // V2.2.1: Get extracted highlights and summary (graceful degradation)
    let raw_highlights = highlights::get_highlights_or_empty(pool, employee_id).await;
    let summary = highlights::get_summary_or_none(pool, employee_id).await;

    // Build cycle name lookup from review_cycles
    let cycle_names: std::collections::HashMap<String, String> = if !raw_highlights.is_empty() {
        let cycle_ids: Vec<String> = raw_highlights.iter().map(|h| h.review_cycle_id.clone()).collect();
        let placeholders = cycle_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("SELECT id, name FROM review_cycles WHERE id IN ({})", placeholders);

        let mut query_builder = sqlx::query(&query);
        for id in &cycle_ids {
            query_builder = query_builder.bind(id);
        }

        query_builder
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|row| (row.get::<String, _>("id"), row.get::<String, _>("name")))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Build CycleHighlight list from raw highlights
    let recent_highlights: Vec<CycleHighlight> = raw_highlights
        .into_iter()
        .take(3) // Limit to 3 most recent cycles for context
        .map(|h| CycleHighlight {
            cycle_name: cycle_names
                .get(&h.review_cycle_id)
                .cloned()
                .unwrap_or_else(|| "Review".to_string()),
            strengths: h.strengths,
            opportunities: h.opportunities,
            themes: h.themes,
            sentiment: h.overall_sentiment,
        })
        .collect();

    // Extract summary data
    let career_summary = summary.as_ref().and_then(|s| s.career_narrative.clone());
    let key_strengths = summary.as_ref().map(|s| s.key_strengths.clone()).unwrap_or_default();
    let development_areas = summary.as_ref().map(|s| s.development_areas.clone()).unwrap_or_default();

    Ok(EmployeeContext {
        id: emp.id,
        full_name: emp.full_name,
        email: emp.email,
        department: emp.department,
        job_title: emp.job_title,
        hire_date: emp.hire_date,
        work_state: emp.work_state,
        status: emp.status,
        manager_name,
        latest_rating: ratings.first().map(|r| r.overall_rating),
        latest_rating_cycle: ratings.first().map(|r| r.cycle_name.clone()),
        rating_trend,
        all_ratings,
        latest_enps: enps_responses.first().map(|e| e.score),
        latest_enps_date: enps_responses.first().map(|e| e.survey_date.clone()),
        enps_trend,
        all_enps,
        // V2.2.1: Highlights data
        career_summary,
        key_strengths,
        development_areas,
        recent_highlights,
    })
}

/// Calculate trend from a series of values (most recent first)
fn calculate_trend(values: &[f64]) -> Option<String> {
    if values.len() < 2 {
        return None;
    }

    let recent = values[0];
    let older = values[values.len() - 1];
    let diff = recent - older;

    // Use a small threshold to avoid noise
    if diff > 0.3 {
        Some("improving".to_string())
    } else if diff < -0.3 {
        Some("declining".to_string())
    } else {
        Some("stable".to_string())
    }
}

/// Get company context
pub async fn get_company_context(pool: &DbPool) -> Result<Option<CompanyContext>, ContextError> {
    let company: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT name, state, industry FROM company WHERE id = 'default'"
    )
    .fetch_optional(pool)
    .await?;

    let Some((name, state, industry)) = company else {
        return Ok(None);
    };

    // Get employee and department counts
    let employee_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM employees WHERE status = 'active'")
        .fetch_one(pool)
        .await?
        .get("count");

    let department_count: i64 = sqlx::query(
        "SELECT COUNT(DISTINCT department) as count FROM employees WHERE department IS NOT NULL AND status = 'active'"
    )
    .fetch_one(pool)
    .await?
    .get("count");

    Ok(Some(CompanyContext {
        name,
        state,
        industry,
        employee_count,
        department_count,
    }))
}

// ============================================================================
// Specialized Retrieval Functions
// ============================================================================

/// Aggregate eNPS calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnpsAggregate {
    /// eNPS score (-100 to +100)
    pub score: i32,
    /// Number of promoters (score >= 9)
    pub promoters: i64,
    /// Number of passives (score 7-8)
    pub passives: i64,
    /// Number of detractors (score <= 6)
    pub detractors: i64,
    /// Total survey responses
    pub total_responses: i64,
    /// Response rate vs active employees
    pub response_rate: f64,
}

/// Find employees with longest tenure (sorted by hire_date ASC)
pub async fn find_longest_tenure(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'active' AND hire_date IS NOT NULL ORDER BY hire_date ASC LIMIT ?"
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut employees = Vec::new();
    for (id,) in rows {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }
    Ok(employees)
}

/// Find newest employees (sorted by hire_date DESC)
pub async fn find_newest_employees(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'active' AND hire_date IS NOT NULL ORDER BY hire_date DESC LIMIT ?"
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut employees = Vec::new();
    for (id,) in rows {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }
    Ok(employees)
}

/// Find employees hired within the last N days (for new hires digest)
pub async fn find_recent_hires(
    pool: &DbPool,
    days: i64,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'active' AND hire_date IS NOT NULL AND hire_date >= date('now', ? || ' days') ORDER BY hire_date DESC LIMIT ?"
    )
    .bind(-days)  // Negative to go back in time
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut employees = Vec::new();
    for (id,) in rows {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }
    Ok(employees)
}

/// Find underperforming employees (rating < 2.5 in recent cycles)
pub async fn find_underperformers(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    // Find employees with at least one rating below 2.5, prioritizing those with multiple low ratings
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT e.id
        FROM employees e
        JOIN performance_ratings pr ON e.id = pr.employee_id
        WHERE e.status = 'active' AND pr.overall_rating < 2.5
        GROUP BY e.id
        ORDER BY COUNT(*) DESC, MIN(pr.overall_rating) ASC
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut employees = Vec::new();
    for (id,) in rows {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }
    Ok(employees)
}

/// Find top performers (rating >= 4.5 in recent cycles)
pub async fn find_top_performers(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    // Find employees with high ratings, prioritizing consistent excellence
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT e.id
        FROM employees e
        JOIN performance_ratings pr ON e.id = pr.employee_id
        WHERE e.status = 'active' AND pr.overall_rating >= 4.5
        GROUP BY e.id
        ORDER BY COUNT(*) DESC, MAX(pr.overall_rating) DESC
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut employees = Vec::new();
    for (id,) in rows {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }
    Ok(employees)
}

/// V2.2.2b: Find employees by theme from extracted review highlights
/// Searches themes, strengths, or opportunities based on ThemeTarget
pub async fn find_employees_by_theme(
    pool: &DbPool,
    themes: &[String],
    department: Option<&str>,
    target: ThemeTarget,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    if themes.is_empty() {
        return Ok(vec![]);
    }

    // Build dynamic WHERE clause for theme matching
    // Theme tags are stored in the `themes` column as JSON arrays like '["leadership", "mentoring"]'
    // Note: `strengths` and `opportunities` columns contain textual descriptions, not theme tags,
    // so we always search the `themes` column. ThemeTarget is metadata for context interpretation.
    let mut theme_conditions = Vec::new();
    for theme in themes {
        let pattern = format!("%\"{}%", theme); // Matches "theme" in JSON array
        // Always search the themes column - that's where theme tags are stored
        theme_conditions.push(format!("rh.themes LIKE '{}'", pattern));
    }

    // Combine theme conditions with OR (match any requested theme)
    let theme_where = theme_conditions.join(" OR ");

    // Build department filter
    let dept_filter = if department.is_some() {
        "AND e.department = ?"
    } else {
        ""
    };

    let query = format!(
        r#"
        SELECT e.id, COUNT(*) as match_count
        FROM employees e
        JOIN review_highlights rh ON e.id = rh.employee_id
        WHERE e.status = 'active'
          AND ({})
          {}
        GROUP BY e.id
        ORDER BY match_count DESC
        LIMIT ?
        "#,
        theme_where, dept_filter
    );

    // Execute query with appropriate bindings
    let rows: Vec<(String, i64)> = if let Some(dept) = department {
        sqlx::query_as(&query)
            .bind(dept)
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as(&query)
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
    };

    // Fetch full employee context for each match
    let mut employees = Vec::new();
    for (id, _match_count) in &rows {
        if let Ok(emp) = get_employee_context(pool, id).await {
            employees.push(emp);
        }
    }

    Ok(employees)
}

/// Find employees with upcoming work anniversaries (within next 30 days)
pub async fn find_upcoming_anniversaries(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    // Find employees whose hire_date anniversary falls within next 30 days
    // Uses SQLite date functions to compare month/day
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT id FROM employees
        WHERE status = 'active'
        AND hire_date IS NOT NULL
        AND (
            (strftime('%m-%d', hire_date) >= strftime('%m-%d', 'now')
             AND strftime('%m-%d', hire_date) <= strftime('%m-%d', 'now', '+30 days'))
            OR
            (strftime('%m-%d', 'now', '+30 days') < strftime('%m-%d', 'now')
             AND (strftime('%m-%d', hire_date) >= strftime('%m-%d', 'now')
                  OR strftime('%m-%d', hire_date) <= strftime('%m-%d', 'now', '+30 days')))
        )
        ORDER BY strftime('%m-%d', hire_date)
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut employees = Vec::new();
    for (id,) in rows {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }
    Ok(employees)
}

/// Find recently terminated employees for attrition queries
/// Returns full EmployeeContext with termination details
pub async fn find_recent_terminations(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeContext>, ContextError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM employees WHERE status = 'terminated' ORDER BY termination_date DESC LIMIT ?"
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut employees = Vec::new();
    for (id,) in rows {
        if let Ok(emp) = get_employee_context(pool, &id).await {
            employees.push(emp);
        }
    }
    Ok(employees)
}

/// Build a lightweight employee list for roster queries
/// Returns EmployeeSummary (name, dept, title, status, hire date) without full perf data
pub async fn build_employee_list(
    pool: &DbPool,
    mentions: &QueryMentions,
    limit: usize,
) -> Result<Vec<EmployeeSummary>, ContextError> {
    // Build query based on department filter
    let rows = if !mentions.departments.is_empty() {
        let dept = &mentions.departments[0];
        let pattern = format!("%{}%", dept);
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
            r#"
            SELECT id, full_name, department, job_title, status, hire_date
            FROM employees
            WHERE department LIKE ? AND status = 'active'
            ORDER BY full_name
            LIMIT ?
            "#
        )
        .bind(&pattern)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?
    } else {
        // No department filter - return active employees
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
            r#"
            SELECT id, full_name, department, job_title, status, hire_date
            FROM employees
            WHERE status = 'active'
            ORDER BY full_name
            LIMIT ?
            "#
        )
        .bind(limit as i64)
        .fetch_all(pool)
        .await?
    };

    let summaries: Vec<EmployeeSummary> = rows
        .into_iter()
        .map(|(id, full_name, department, job_title, status, hire_date)| EmployeeSummary {
            id,
            full_name,
            department,
            job_title,
            status,
            hire_date,
        })
        .collect();

    Ok(summaries)
}

/// Build a list of terminated employees for attrition list queries
pub async fn build_termination_list(
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<EmployeeSummary>, ContextError> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String, Option<String>)>(
        r#"
        SELECT id, full_name, department, job_title, status, hire_date
        FROM employees
        WHERE status = 'terminated'
        ORDER BY termination_date DESC
        LIMIT ?
        "#
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let summaries: Vec<EmployeeSummary> = rows
        .into_iter()
        .map(|(id, full_name, department, job_title, status, hire_date)| EmployeeSummary {
            id,
            full_name,
            department,
            job_title,
            status,
            hire_date,
        })
        .collect();

    Ok(summaries)
}

/// Calculate aggregate eNPS score for the organization
pub async fn calculate_aggregate_enps(pool: &DbPool) -> Result<EnpsAggregate, ContextError> {
    // Get the most recent survey response per employee to avoid double-counting
    let stats: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        WITH latest_responses AS (
            SELECT employee_id, score, survey_date,
                   ROW_NUMBER() OVER (PARTITION BY employee_id ORDER BY survey_date DESC) as rn
            FROM enps_responses
        )
        SELECT
            COUNT(*) as total,
            SUM(CASE WHEN score >= 9 THEN 1 ELSE 0 END) as promoters,
            SUM(CASE WHEN score >= 7 AND score <= 8 THEN 1 ELSE 0 END) as passives,
            SUM(CASE WHEN score <= 6 THEN 1 ELSE 0 END) as detractors
        FROM latest_responses
        WHERE rn = 1
        "#
    )
    .fetch_one(pool)
    .await?;

    let (total, promoters, passives, detractors) = stats;

    // Get active employee count for response rate
    let active_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM employees WHERE status = 'active'")
        .fetch_one(pool)
        .await?
        .get("count");

    let score = if total > 0 {
        ((promoters - detractors) * 100 / total) as i32
    } else {
        0
    };

    let response_rate = if active_count > 0 {
        (total as f64 / active_count as f64) * 100.0
    } else {
        0.0
    };

    Ok(EnpsAggregate {
        score,
        promoters,
        passives,
        detractors,
        total_responses: total,
        response_rate,
    })
}

/// Format aggregate eNPS for inclusion in context
pub fn format_aggregate_enps(enps: &EnpsAggregate) -> String {
    format!(
        "Company eNPS: {} (Promoters: {}, Passives: {}, Detractors: {}) — {} responses ({:.0}% response rate)",
        enps.score, enps.promoters, enps.passives, enps.detractors,
        enps.total_responses, enps.response_rate
    )
}

// ============================================================================
// Organization Aggregates (Phase 2.7)
// ============================================================================

/// Build organization-wide aggregates from the full database
/// These are computed for every query to give Claude accurate org-level context
pub async fn build_org_aggregates(pool: &DbPool) -> Result<OrgAggregates, ContextError> {
    // 1. Headcount by status
    let headcount = fetch_headcount_by_status(pool).await?;

    // 2. Headcount by department
    let by_department = fetch_headcount_by_department(pool, headcount.active_count).await?;

    // 3. Performance distribution (most recent rating per active employee)
    let (avg_rating, rating_distribution, employees_with_no_rating) =
        fetch_performance_distribution(pool, headcount.active_count).await?;

    // 4. eNPS (reuse existing function)
    let enps = calculate_aggregate_enps(pool).await?;

    // 5. Attrition YTD
    let attrition = fetch_attrition_stats(pool, headcount.active_count).await?;

    Ok(OrgAggregates {
        total_employees: headcount.total,
        active_count: headcount.active_count,
        terminated_count: headcount.terminated_count,
        on_leave_count: headcount.on_leave_count,
        by_department,
        avg_rating,
        rating_distribution,
        employees_with_no_rating,
        enps,
        attrition,
    })
}

/// Internal struct for headcount query result
struct HeadcountResult {
    total: i64,
    active_count: i64,
    terminated_count: i64,
    on_leave_count: i64,
}

/// Fetch headcount by status
async fn fetch_headcount_by_status(pool: &DbPool) -> Result<HeadcountResult, ContextError> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) as total,
            SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END) as active,
            SUM(CASE WHEN status = 'terminated' THEN 1 ELSE 0 END) as terminated,
            SUM(CASE WHEN status = 'leave' THEN 1 ELSE 0 END) as on_leave
        FROM employees
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(HeadcountResult {
        total: row.get::<i64, _>("total"),
        active_count: row.get::<i64, _>("active"),
        terminated_count: row.get::<i64, _>("terminated"),
        on_leave_count: row.get::<i64, _>("on_leave"),
    })
}

/// Fetch headcount by department (active employees only)
async fn fetch_headcount_by_department(
    pool: &DbPool,
    total_active: i64,
) -> Result<Vec<DepartmentCount>, ContextError> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(department, 'Unassigned') as department,
            COUNT(*) as count
        FROM employees
        WHERE status = 'active'
        GROUP BY department
        ORDER BY count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let departments: Vec<DepartmentCount> = rows
        .iter()
        .map(|row| {
            let name: String = row.get("department");
            let count: i64 = row.get("count");
            let percentage = if total_active > 0 {
                (count as f64 / total_active as f64) * 100.0
            } else {
                0.0
            };
            DepartmentCount {
                name,
                count,
                percentage,
            }
        })
        .collect();

    Ok(departments)
}

/// Fetch performance rating distribution (most recent rating per active employee)
async fn fetch_performance_distribution(
    pool: &DbPool,
    total_active: i64,
) -> Result<(Option<f64>, RatingDistribution, i64), ContextError> {
    // Get most recent rating per active employee
    let row = sqlx::query(
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
        SELECT
            AVG(overall_rating) as avg_rating,
            SUM(CASE WHEN overall_rating >= 4.5 THEN 1 ELSE 0 END) as exceptional,
            SUM(CASE WHEN overall_rating >= 3.5 AND overall_rating < 4.5 THEN 1 ELSE 0 END) as exceeds,
            SUM(CASE WHEN overall_rating >= 2.5 AND overall_rating < 3.5 THEN 1 ELSE 0 END) as meets,
            SUM(CASE WHEN overall_rating < 2.5 THEN 1 ELSE 0 END) as needs_improvement,
            COUNT(*) as rated_count
        FROM latest_ratings
        WHERE rn = 1
        "#,
    )
    .fetch_one(pool)
    .await?;

    let avg_rating: Option<f64> = row.get("avg_rating");
    let rated_count: i64 = row.get("rated_count");
    let employees_with_no_rating = total_active - rated_count;

    let distribution = RatingDistribution {
        exceptional: row.get("exceptional"),
        exceeds: row.get("exceeds"),
        meets: row.get("meets"),
        needs_improvement: row.get("needs_improvement"),
    };

    Ok((avg_rating, distribution, employees_with_no_rating))
}

/// Fetch attrition stats for YTD
async fn fetch_attrition_stats(
    pool: &DbPool,
    current_active: i64,
) -> Result<AttritionStats, ContextError> {
    // Get YTD termination stats
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) as terminations,
            SUM(CASE WHEN termination_reason = 'voluntary' THEN 1 ELSE 0 END) as voluntary,
            SUM(CASE WHEN termination_reason = 'involuntary' THEN 1 ELSE 0 END) as involuntary,
            AVG(
                CAST((julianday(termination_date) - julianday(hire_date)) / 30.0 AS REAL)
            ) as avg_tenure_months
        FROM employees
        WHERE status = 'terminated'
          AND termination_date >= date('now', 'start of year')
        "#,
    )
    .fetch_one(pool)
    .await?;

    let terminations_ytd: i64 = row.get("terminations");
    let voluntary: i64 = row.get("voluntary");
    let involuntary: i64 = row.get("involuntary");
    let avg_tenure_months: Option<f64> = row.get("avg_tenure_months");

    // Calculate annualized turnover rate
    // Formula: (terminations / avg headcount) * (12 / months elapsed) * 100
    let turnover_rate_annualized = calculate_turnover_rate(pool, terminations_ytd, current_active).await?;

    Ok(AttritionStats {
        terminations_ytd,
        voluntary,
        involuntary,
        avg_tenure_months,
        turnover_rate_annualized,
    })
}

/// Calculate annualized turnover rate
async fn calculate_turnover_rate(
    pool: &DbPool,
    terminations_ytd: i64,
    current_active: i64,
) -> Result<Option<f64>, ContextError> {
    if terminations_ytd == 0 {
        return Ok(Some(0.0));
    }

    // Get months elapsed this year
    let row = sqlx::query(
        r#"
        SELECT
            (julianday('now') - julianday(date('now', 'start of year'))) / 30.0 as months_elapsed
        "#,
    )
    .fetch_one(pool)
    .await?;

    let months_elapsed: f64 = row.get("months_elapsed");

    if months_elapsed <= 0.0 {
        return Ok(None);
    }

    // Approximate average headcount = current active + half of terminations
    let avg_headcount = current_active as f64 + (terminations_ytd as f64 / 2.0);

    if avg_headcount <= 0.0 {
        return Ok(None);
    }

    // Annualized rate = (terminations / avg headcount) * (12 / months elapsed) * 100
    let rate = (terminations_ytd as f64 / avg_headcount) * (12.0 / months_elapsed) * 100.0;

    Ok(Some(rate))
}

/// Format organization aggregates for inclusion in system prompt
/// Produces a compact (~1.5-2K chars) summary of org-wide stats
pub fn format_org_aggregates(agg: &OrgAggregates, company_name: Option<&str>) -> String {
    let mut lines = Vec::new();

    // Header
    lines.push("ORGANIZATION DATA:".to_string());
    lines.push(String::new());

    // Workforce summary
    if let Some(name) = company_name {
        lines.push(format!("COMPANY: {}", name));
    }
    lines.push(format!(
        "WORKFORCE: {} employees",
        agg.total_employees
    ));
    lines.push(format!(
        "• Active: {} | Terminated: {} | On Leave: {}",
        agg.active_count, agg.terminated_count, agg.on_leave_count
    ));
    lines.push(String::new());

    // Departments (compact format for space efficiency)
    if !agg.by_department.is_empty() {
        lines.push("DEPARTMENTS:".to_string());
        let dept_strs: Vec<String> = agg
            .by_department
            .iter()
            .take(8) // Limit to 8 departments to save space
            .map(|d| format!("{}: {} ({:.0}%)", d.name, d.count, d.percentage))
            .collect();
        // Group 3 departments per line for compactness
        for chunk in dept_strs.chunks(3) {
            lines.push(format!("• {}", chunk.join(" • ")));
        }
        lines.push(String::new());
    }

    // Performance
    lines.push(format!(
        "PERFORMANCE ({} active employees):",
        agg.active_count
    ));
    if let Some(avg) = agg.avg_rating {
        let label = rating_label(avg);
        lines.push(format!("• Avg rating: {:.1} ({})", avg, label));
    } else {
        lines.push("• No performance data available".to_string());
    }
    let dist = &agg.rating_distribution;
    if dist.exceptional > 0 || dist.exceeds > 0 || dist.meets > 0 || dist.needs_improvement > 0 {
        lines.push(format!(
            "• Distribution: Exceptional: {} | Exceeds: {} | Meets: {} | Needs Improvement: {}",
            dist.exceptional, dist.exceeds, dist.meets, dist.needs_improvement
        ));
    }
    if agg.employees_with_no_rating > 0 {
        lines.push(format!(
            "• Employees with no rating: {}",
            agg.employees_with_no_rating
        ));
    }
    lines.push(String::new());

    // Engagement (eNPS)
    lines.push("ENGAGEMENT:".to_string());
    let sign = if agg.enps.score >= 0 { "+" } else { "" };
    lines.push(format!(
        "• eNPS: {}{} (Promoters: {}, Passives: {}, Detractors: {})",
        sign, agg.enps.score, agg.enps.promoters, agg.enps.passives, agg.enps.detractors
    ));
    lines.push(format!(
        "• Response rate: {:.0}% ({} of {} active)",
        agg.enps.response_rate, agg.enps.total_responses, agg.active_count
    ));
    lines.push(String::new());

    // Attrition
    lines.push("ATTRITION (YTD):".to_string());
    if agg.attrition.terminations_ytd > 0 {
        lines.push(format!(
            "• Terminations: {} (Voluntary: {}, Involuntary: {})",
            agg.attrition.terminations_ytd,
            agg.attrition.voluntary,
            agg.attrition.involuntary
        ));
        if let Some(tenure) = agg.attrition.avg_tenure_months {
            let years = tenure / 12.0;
            lines.push(format!("• Avg tenure at exit: {:.1} years", years));
        }
        if let Some(rate) = agg.attrition.turnover_rate_annualized {
            lines.push(format!("• Turnover rate: {:.1}% annualized", rate));
        }
    } else {
        lines.push("• No terminations YTD".to_string());
    }

    lines.join("\n")
}

// ============================================================================
// Excerpting Helpers (V2.2.2a)
// ============================================================================

/// Default maximum sentences to include in excerpts
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
/// Returns the original text if it contains fewer than max_sentences.
pub fn excerpt_to_sentences(text: &str, max_sentences: usize) -> String {
    if max_sentences == 0 {
        return String::new();
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Use Unicode sentence boundaries for accurate splitting
    let sentences: Vec<&str> = trimmed.unicode_sentences().collect();

    if sentences.len() <= max_sentences {
        // Return original text if already within limit
        return trimmed.to_string();
    }

    // Join the first N sentences
    let excerpt: String = sentences[..max_sentences].concat();
    let mut result = excerpt.trim_end().to_string();

    // Add ellipsis to indicate truncation
    if !result.ends_with('.') && !result.ends_with('!') && !result.ends_with('?') {
        result.push_str("...");
    } else {
        result.push_str("..");
    }

    result
}

/// Calculate the number of sentences to include based on available token budget.
/// Returns (summary_sentences, highlight_cycles) tuple.
pub fn calculate_excerpt_limits(token_budget: usize) -> (usize, usize) {
    if token_budget >= REDUCED_BUDGET_THRESHOLD {
        // Full budget: 5 summary sentences, 3 highlight cycles
        (FULL_BUDGET_SUMMARY_SENTENCES, 3)
    } else if token_budget >= REDUCED_BUDGET_THRESHOLD / 2 {
        // Reduced budget: 2 summary sentences, 2 highlight cycles
        (REDUCED_BUDGET_SUMMARY_SENTENCES, 2)
    } else {
        // Tight budget: 1 summary sentence, 1 highlight cycle
        (MIN_SENTENCES, 1)
    }
}

/// Calculate per-employee token budget based on total budget and employee count.
/// Distributes budget evenly with a minimum floor per employee.
pub fn calculate_per_employee_budget(total_budget: usize, employee_count: usize) -> usize {
    if employee_count == 0 {
        return total_budget;
    }

    // Minimum budget per employee (enough for basic info + 1-2 sentences)
    const MIN_PER_EMPLOYEE: usize = 200;

    let calculated = total_budget / employee_count;
    calculated.max(MIN_PER_EMPLOYEE)
}

// ============================================================================
// Context Formatting
// ============================================================================

/// Format employee context for inclusion in system prompt.
/// Uses token budget to control excerpting of long content.
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

    // Calculate per-employee budget based on total and count
    let budget = total_token_budget.unwrap_or(MAX_EMPLOYEE_CONTEXT_TOKENS);
    let per_employee_budget = calculate_per_employee_budget(budget, employees.len());

    let mut output = String::new();
    let mut total_chars = 0;
    let max_chars = budget * CHARS_PER_TOKEN;

    for emp in employees {
        let emp_text = format_single_employee_with_budget(emp, Some(per_employee_budget));

        // Check if adding this employee would exceed the limit
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
/// Used for roster displays where full performance data isn't needed
pub fn format_employee_summaries(summaries: &[EmployeeSummary], total_count: Option<i64>) -> String {
    if summaries.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();

    // Show count context if available
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
/// When a budget is provided, long content like career summaries and highlights
/// will be truncated to fit within the budget.
fn format_single_employee_with_budget(emp: &EmployeeContext, token_budget: Option<usize>) -> String {
    let mut lines = Vec::new();

    // Calculate excerpt limits based on budget
    let (summary_sentences, highlight_cycles) = token_budget
        .map(calculate_excerpt_limits)
        .unwrap_or((FULL_BUDGET_SUMMARY_SENTENCES, 3));

    // Basic info (always included in full)
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
                // Truncate long feedback
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
    // V2.2.2a: Apply dynamic excerpting based on token budget
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

    // Recent review highlights (themes, strengths per cycle)
    // V2.2.2a: Limit cycles based on token budget
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

/// Get human-readable rating label
fn rating_label(rating: f64) -> &'static str {
    if rating >= 4.5 {
        "Exceptional"
    } else if rating >= 3.5 {
        "Exceeds Expectations"
    } else if rating >= 2.5 {
        "Meets Expectations"
    } else if rating >= 1.5 {
        "Developing"
    } else {
        "Unsatisfactory"
    }
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
/// This is a rough approximation; actual tokenization varies by content.
pub fn estimate_tokens(text: &str) -> usize {
    // Round up to be conservative
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

/// Get the maximum system prompt token budget
#[allow(dead_code)]
pub fn get_max_system_prompt_tokens() -> usize {
    MAX_SYSTEM_PROMPT_TOKENS
}

// ============================================================================
// System Prompt Building
// ============================================================================

/// Build the complete system prompt for Claude (Phase 2.7 - includes org aggregates)
/// V2.1.3: Added persona_id parameter to support persona switching
pub fn build_system_prompt(
    company: Option<&CompanyContext>,
    aggregates: Option<&OrgAggregates>,
    employee_context: &str,
    memory_summaries: &[String],
    user_name: Option<&str>,
    persona_id: Option<&str>,
) -> String {
    let persona = get_persona(persona_id);
    let company_name = company.map(|c| c.name.as_str()).unwrap_or("your company");
    let company_state = company.map(|c| c.state.as_str()).unwrap_or("your state");
    let user_display = user_name.unwrap_or("the HR team");

    // Build persona preamble with variable substitution
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

    // Format org-wide aggregates (Phase 2.7)
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

    // Build employee section (may be empty for Aggregate queries)
    let employee_section = if employee_context.is_empty() {
        String::new()
    } else {
        format!("\nRELEVANT EMPLOYEES:\n{}", employee_context)
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

BOUNDARIES:
- This is guidance, not legal advice—the user acknowledged this during setup
- For anything involving potential litigation, recommend legal counsel
- You don't have access to confidential investigation details
- Compensation data is not available (V1)
{employee_section}

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

/// Build complete context for a chat message using query-adaptive retrieval (Phase 2.7)
///
/// This function:
/// 1. Classifies the query type (Aggregate, List, Individual, Comparison, Attrition, General)
/// 2. Always computes organization-wide aggregates for accurate stats
/// 3. Routes to appropriate employee retrieval based on query type
/// 4. If selected_employee_id is provided, that employee is always prioritized
/// 5. Tracks retrieval metrics for observability (V2.2.2)
pub async fn build_chat_context(
    pool: &DbPool,
    user_message: &str,
    selected_employee_id: Option<&str>,
) -> Result<ChatContext, ContextError> {
    // V2.2.2: Start timing for retrieval metrics
    let start_time = std::time::Instant::now();

    // Step 1: Extract mentions and classify query
    let mentions = extract_mentions(user_message);
    let query_type = classify_query(user_message, &mentions);

    // V2.2.2: Get token budget for this query type
    let token_budget = TokenBudget::for_query_type(query_type);

    // Step 2: Get company context
    let company = get_company_context(pool).await?;

    // Step 3: Always compute organization aggregates (cheap SQL, enables accurate stats)
    let aggregates = match build_org_aggregates(pool).await {
        Ok(agg) => Some(agg),
        Err(e) => {
            eprintln!("Warning: Failed to build org aggregates: {}", e);
            None
        }
    };

    // Step 4: Query-adaptive employee retrieval
    let (employees, employee_summaries) = match query_type {
        QueryType::Aggregate => {
            // Aggregate queries don't need individual employee data
            // The aggregates provide all necessary stats
            (vec![], vec![])
        }
        QueryType::List => {
            // List queries get lightweight summaries (no full perf data)
            let summaries = build_employee_list(pool, &mentions, MAX_LIST_EMPLOYEES).await?;
            (vec![], summaries)
        }
        QueryType::Individual => {
            // Individual queries get full profiles for named employees
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
            // V2.2.2b: Theme-based queries use specialized retrieval
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
                // Standard comparison: top/bottom performers
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
            // Attrition queries get recent terminations with full context
            let employees = find_recent_terminations(pool, MAX_ATTRITION_EMPLOYEES).await?;
            (employees, vec![])
        }
        QueryType::General => {
            // General fallback: sample of relevant employees
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

    // Collect employee IDs for audit logging
    let mut employee_ids_used: Vec<String> = employees.iter().map(|e| e.id.clone()).collect();
    employee_ids_used.extend(employee_summaries.iter().map(|e| e.id.clone()));

    // Step 5: Find relevant past conversation memories (resilient - don't fail if lookup errors)
    let memory_summaries: Vec<String> = match memory::find_relevant_memories(
        pool,
        user_message,
        memory::DEFAULT_MEMORY_LIMIT,
    )
    .await
    {
        Ok(memories) => memories.into_iter().map(|m| m.summary).collect(),
        Err(e) => {
            eprintln!("Warning: Failed to retrieve memories: {}", e);
            Vec::new()
        }
    };

    // V2.2.2: Calculate token usage for each section
    let employees_included = employees.len() + employee_summaries.len();
    let memories_included = memory_summaries.len();

    // Estimate tokens for each section (using chars/4 approximation)
    let employee_tokens = if !employees.is_empty() {
        // Full profiles: estimate based on formatted content
        employees.len() * 500 / CHARS_PER_TOKEN // ~500 chars per full profile
    } else {
        // Summaries: much smaller
        employee_summaries.len() * 70 / CHARS_PER_TOKEN // ~70 chars per summary
    };

    let memory_tokens = memory_summaries
        .iter()
        .map(|m| m.len() / CHARS_PER_TOKEN)
        .sum();

    let aggregates_tokens = if aggregates.is_some() { 500 } else { 0 }; // ~2K chars formatted

    let token_usage = TokenUsage {
        employee_tokens,
        memory_tokens,
        aggregates_tokens,
        total_tokens: employee_tokens + memory_tokens + aggregates_tokens,
    };

    // V2.2.2: Build retrieval metrics
    let retrieval_time_ms = start_time.elapsed().as_millis() as u64;
    let metrics = RetrievalMetrics {
        query_type,
        employees_found: employees_included, // Currently same as included; future: track pre-limit count
        employees_included,
        memories_found: memories_included, // Currently same as included
        memories_included,
        aggregates_included: aggregates.is_some(),
        token_budget,
        token_usage,
        retrieval_time_ms,
    };

    Ok(ChatContext {
        company,
        aggregates,
        query_type,
        employees,
        employee_summaries,
        employee_ids_used,
        memory_summaries,
        metrics,
    })
}

/// Get the system prompt for a chat message
/// If selected_employee_id is provided, that employee is always included first
///
/// V2.1.4: Now returns SystemPromptResult with aggregates and query_type for verification
/// V2.2.2: Now includes retrieval metrics for observability
pub async fn get_system_prompt_for_message(
    pool: &DbPool,
    user_message: &str,
    selected_employee_id: Option<&str>,
) -> Result<SystemPromptResult, ContextError> {
    let context = build_chat_context(pool, user_message, selected_employee_id).await?;

    // Fetch user_name from settings (if set)
    let user_name = crate::settings::get_setting(pool, "user_name")
        .await
        .ok()
        .flatten();

    // Fetch persona preference from settings (V2.1.3)
    let persona_id = crate::settings::get_setting(pool, "persona")
        .await
        .ok()
        .flatten();

    // Build employee context: full profiles or summaries depending on query type
    let employee_context = if !context.employees.is_empty() {
        format_employee_context(&context.employees)
    } else if !context.employee_summaries.is_empty() {
        // For list queries, get total count from aggregates for context
        let total_count = context.aggregates.as_ref().map(|a| a.total_employees);
        format_employee_summaries(&context.employee_summaries, total_count)
    } else {
        String::new() // Aggregate queries don't need employee details
    };

    let system_prompt = build_system_prompt(
        context.company.as_ref(),
        context.aggregates.as_ref(),
        &employee_context,
        &context.memory_summaries,
        user_name.as_deref(),
        persona_id.as_deref(),
    );

    Ok(SystemPromptResult {
        system_prompt,
        employee_ids_used: context.employee_ids_used,
        aggregates: context.aggregates,
        query_type: context.query_type,
        metrics: context.metrics, // V2.2.2: Include retrieval metrics
    })
}

// ============================================================================
// Answer Verification Functions (V2.1.4)
// ============================================================================

use regex::Regex;

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
    let headcount_re = Regex::new(r"(\d+)\s*(?:total\s+)?(?:employees?|people|team\s*members?|staff|headcount)").unwrap();
    for cap in headcount_re.captures_iter(&response_lower) {
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
    let active_re = Regex::new(r"(\d+)\s*active(?:\s+employees?)?|active[:\s]+(\d+)").unwrap();
    for cap in active_re.captures_iter(&response_lower) {
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
        let rating_re = Regex::new(r"(?:average|avg|mean)\s*(?:rating|score)?[:\s]*(?:of\s+|is\s+)?(\d+\.?\d*)|(\d+\.?\d*)\s*(?:average|avg)").unwrap();
        for cap in rating_re.captures_iter(&response_lower) {
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
    let enps_re = Regex::new(r"enps\s*(?:score)?[:\s]*(?:of\s+|is\s+)?([+-]?\d+)|([+-]?\d+)\s*enps").unwrap();
    for cap in enps_re.captures_iter(&response_lower) {
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
        let turnover_re = Regex::new(r"(\d+\.?\d*)\s*%\s*(?:turnover|attrition)|(?:turnover|attrition)\s*(?:rate)?[:\s]*(?:of\s+)?(\d+\.?\d*)\s*%").unwrap();
        for cap in turnover_re.captures_iter(&response_lower) {
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
        )).unwrap();

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

    #[test]
    fn test_extract_mentions_names() {
        let query = "What's Sarah Chen's performance history?";
        let mentions = extract_mentions(query);
        assert!(mentions.names.iter().any(|n| n.contains("Sarah")));
    }

    #[test]
    fn test_extract_mentions_department() {
        let query = "How is the Engineering team doing?";
        let mentions = extract_mentions(query);
        assert!(mentions.departments.contains(&"Engineering".to_string()));
        assert!(mentions.is_aggregate_query);
    }

    #[test]
    fn test_extract_mentions_department_word_boundary() {
        // Bug fix: "IT" should NOT match when it's part of another word like "with"
        let query = "Show me people with teamwork feedback";
        let mentions = extract_mentions(query);
        assert!(
            !mentions.departments.contains(&"IT".to_string()),
            "Should not detect 'IT' in 'with' - departments found: {:?}",
            mentions.departments
        );

        // But actual IT department mentions should work
        let query2 = "How is IT doing?";
        let mentions2 = extract_mentions(query2);
        assert!(mentions2.departments.contains(&"IT".to_string()));

        // IT at start of string
        let query3 = "IT team needs help";
        let mentions3 = extract_mentions(query3);
        assert!(mentions3.departments.contains(&"IT".to_string()));

        // IT at end of string
        let query4 = "show me IT";
        let mentions4 = extract_mentions(query4);
        assert!(mentions4.departments.contains(&"IT".to_string()));
    }

    #[test]
    fn test_matches_word_boundary() {
        // Basic word boundary cases
        assert!(matches_word_boundary("hello world", "hello"));
        assert!(matches_word_boundary("hello world", "world"));
        assert!(matches_word_boundary("hello", "hello")); // exact match

        // Should NOT match substrings
        assert!(!matches_word_boundary("within", "it")); // "it" inside "within"
        assert!(!matches_word_boundary("with", "it")); // "it" at end of "with"
        assert!(!matches_word_boundary("item", "it")); // "it" at start of "item"

        // Should match with punctuation boundaries
        assert!(matches_word_boundary("hello, it works", "it"));
        assert!(matches_word_boundary("it's working", "it")); // apostrophe is not alphanumeric
        assert!(matches_word_boundary("(it)", "it"));

        // Case sensitivity (our function expects lowercase input)
        assert!(matches_word_boundary("the it team", "it"));
        assert!(!matches_word_boundary("the item", "it"));
    }

    #[test]
    fn test_extract_mentions_performance() {
        let query = "Who are our top performers?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_performance_query);
    }

    #[test]
    fn test_extract_mentions_enps() {
        let query = "What's our current eNPS score?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_enps_query);
    }

    #[test]
    fn test_rating_label() {
        assert_eq!(rating_label(4.8), "Exceptional");
        assert_eq!(rating_label(3.7), "Exceeds Expectations");
        assert_eq!(rating_label(3.0), "Meets Expectations");
        assert_eq!(rating_label(2.2), "Developing");
        assert_eq!(rating_label(1.2), "Unsatisfactory");
    }

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
    fn test_calculate_trend() {
        // Improving (most recent is higher)
        assert_eq!(calculate_trend(&[4.0, 3.5, 3.0]), Some("improving".to_string()));
        // Declining (most recent is lower)
        assert_eq!(calculate_trend(&[3.0, 3.5, 4.0]), Some("declining".to_string()));
        // Stable
        assert_eq!(calculate_trend(&[3.5, 3.4, 3.5]), Some("stable".to_string()));
        // Not enough data
        assert_eq!(calculate_trend(&[3.5]), None);
    }

    // =========================================================================
    // New tests for Phase 2.3.2 — Enhanced query extraction
    // =========================================================================

    #[test]
    fn test_extract_tenure_longest() {
        let query = "Who's been here the longest?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_tenure_query);
        assert_eq!(mentions.tenure_direction, Some(TenureDirection::Longest));
    }

    #[test]
    fn test_extract_tenure_newest() {
        let query = "Who are our newest hires?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_tenure_query);
        assert_eq!(mentions.tenure_direction, Some(TenureDirection::Newest));
    }

    #[test]
    fn test_extract_tenure_anniversary() {
        let query = "Who has a work anniversary coming up?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_tenure_query);
        assert_eq!(mentions.tenure_direction, Some(TenureDirection::Anniversary));
    }

    #[test]
    fn test_extract_underperformer() {
        let query = "Who's underperforming on the team?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_underperformer_query);
    }

    #[test]
    fn test_extract_underperformer_struggling() {
        let query = "Which employees are struggling?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_underperformer_query);
    }

    #[test]
    fn test_extract_top_performer() {
        let query = "Who are our top performers?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_top_performer_query);
    }

    #[test]
    fn test_extract_top_performer_star() {
        let query = "Who are the star employees in Engineering?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_top_performer_query);
        assert!(mentions.departments.contains(&"Engineering".to_string()));
    }

    #[test]
    fn test_extract_aggregate_enps() {
        let query = "What's our company eNPS?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_enps_query);
        assert!(mentions.wants_aggregate);
    }

    #[test]
    fn test_extract_possessive_name() {
        let query = "What's Sarah's performance history?";
        let mentions = extract_mentions(query);
        assert!(mentions.names.iter().any(|n| n == "Sarah"));
    }

    #[test]
    fn test_extract_possessive_full_name() {
        let query = "Tell me about Marcus Johnson's reviews";
        let mentions = extract_mentions(query);
        // Should find "Marcus" after stripping possessive from "Johnson's"
        assert!(mentions.names.iter().any(|n| n.contains("Marcus")));
    }

    #[test]
    fn test_extract_how_many() {
        let query = "How many employees do we have?";
        let mentions = extract_mentions(query);
        assert!(mentions.wants_aggregate);
    }

    // ========================================
    // Token Estimation Tests
    // ========================================

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short_text() {
        // "Hello" = 5 chars = ceil(5/4) = 2 tokens
        assert_eq!(estimate_tokens("Hello"), 2);
    }

    #[test]
    fn test_estimate_tokens_exact_multiple() {
        // 8 chars = 8/4 = 2 tokens
        assert_eq!(estimate_tokens("12345678"), 2);
    }

    #[test]
    fn test_estimate_tokens_rounds_up() {
        // 9 chars = ceil(9/4) = 3 tokens (conservative)
        assert_eq!(estimate_tokens("123456789"), 3);
    }

    #[test]
    fn test_estimate_tokens_longer_text() {
        // 100 chars = 100/4 = 25 tokens
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
        // Should return the constant value
        assert_eq!(get_max_conversation_tokens(), 150_000);
    }

    // ========================================
    // Organization Aggregates Tests (Phase 2.7)
    // ========================================

    #[test]
    fn test_format_org_aggregates_basic() {
        let agg = OrgAggregates {
            total_employees: 100,
            active_count: 82,
            terminated_count: 12,
            on_leave_count: 6,
            by_department: vec![
                DepartmentCount { name: "Engineering".to_string(), count: 28, percentage: 34.1 },
                DepartmentCount { name: "Sales".to_string(), count: 18, percentage: 22.0 },
                DepartmentCount { name: "Marketing".to_string(), count: 12, percentage: 14.6 },
            ],
            avg_rating: Some(3.4),
            rating_distribution: RatingDistribution {
                exceptional: 8,
                exceeds: 32,
                meets: 38,
                needs_improvement: 4,
            },
            employees_with_no_rating: 12,
            enps: EnpsAggregate {
                score: 12,
                promoters: 34,
                passives: 28,
                detractors: 20,
                total_responses: 67,
                response_rate: 81.7,
            },
            attrition: AttritionStats {
                terminations_ytd: 12,
                voluntary: 8,
                involuntary: 4,
                avg_tenure_months: Some(27.6),
                turnover_rate_annualized: Some(14.6),
            },
        };

        let formatted = format_org_aggregates(&agg, Some("Acme Corp"));

        // Check key sections are present
        assert!(formatted.contains("ORGANIZATION DATA:"));
        assert!(formatted.contains("COMPANY: Acme Corp"));
        assert!(formatted.contains("WORKFORCE: 100 employees"));
        assert!(formatted.contains("Active: 82"));
        assert!(formatted.contains("Terminated: 12"));
        assert!(formatted.contains("On Leave: 6"));

        // Check departments
        assert!(formatted.contains("DEPARTMENTS:"));
        assert!(formatted.contains("Engineering: 28"));
        assert!(formatted.contains("Sales: 18"));

        // Check performance
        assert!(formatted.contains("PERFORMANCE (82 active employees):"));
        assert!(formatted.contains("Avg rating: 3.4 (Meets Expectations)"));
        assert!(formatted.contains("Exceptional: 8"));

        // Check engagement
        assert!(formatted.contains("ENGAGEMENT:"));
        assert!(formatted.contains("eNPS: +12"));
        assert!(formatted.contains("Promoters: 34"));

        // Check attrition
        assert!(formatted.contains("ATTRITION (YTD):"));
        assert!(formatted.contains("Terminations: 12"));
        assert!(formatted.contains("Voluntary: 8"));
        assert!(formatted.contains("Turnover rate: 14.6%"));
    }

    #[test]
    fn test_format_org_aggregates_empty_data() {
        let agg = OrgAggregates {
            total_employees: 0,
            active_count: 0,
            terminated_count: 0,
            on_leave_count: 0,
            by_department: vec![],
            avg_rating: None,
            rating_distribution: RatingDistribution::default(),
            employees_with_no_rating: 0,
            enps: EnpsAggregate {
                score: 0,
                promoters: 0,
                passives: 0,
                detractors: 0,
                total_responses: 0,
                response_rate: 0.0,
            },
            attrition: AttritionStats::default(),
        };

        let formatted = format_org_aggregates(&agg, None);

        // Should still produce valid output
        assert!(formatted.contains("ORGANIZATION DATA:"));
        assert!(formatted.contains("WORKFORCE: 0 employees"));
        assert!(formatted.contains("No performance data available"));
        assert!(formatted.contains("No terminations YTD"));
    }

    #[test]
    fn test_format_org_aggregates_negative_enps() {
        let agg = OrgAggregates {
            total_employees: 50,
            active_count: 45,
            terminated_count: 5,
            on_leave_count: 0,
            by_department: vec![],
            avg_rating: Some(2.8),
            rating_distribution: RatingDistribution {
                exceptional: 2,
                exceeds: 10,
                meets: 25,
                needs_improvement: 8,
            },
            employees_with_no_rating: 0,
            enps: EnpsAggregate {
                score: -15,
                promoters: 10,
                passives: 15,
                detractors: 20,
                total_responses: 45,
                response_rate: 100.0,
            },
            attrition: AttritionStats::default(),
        };

        let formatted = format_org_aggregates(&agg, Some("Test Corp"));

        // Negative eNPS should not have + sign
        assert!(formatted.contains("eNPS: -15"));
        assert!(!formatted.contains("eNPS: +-15"));
    }

    #[test]
    fn test_rating_distribution_default() {
        let dist = RatingDistribution::default();
        assert_eq!(dist.exceptional, 0);
        assert_eq!(dist.exceeds, 0);
        assert_eq!(dist.meets, 0);
        assert_eq!(dist.needs_improvement, 0);
    }

    #[test]
    fn test_attrition_stats_default() {
        let stats = AttritionStats::default();
        assert_eq!(stats.terminations_ytd, 0);
        assert_eq!(stats.voluntary, 0);
        assert_eq!(stats.involuntary, 0);
        assert!(stats.avg_tenure_months.is_none());
        assert!(stats.turnover_rate_annualized.is_none());
    }

    #[test]
    fn test_query_type_serialization() {
        // Verify QueryType can be serialized/deserialized
        let types = vec![
            QueryType::Aggregate,
            QueryType::List,
            QueryType::Individual,
            QueryType::Comparison,
            QueryType::Attrition,
            QueryType::General,
        ];

        for qt in types {
            let serialized = serde_json::to_string(&qt).unwrap();
            let deserialized: QueryType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(qt, deserialized);
        }
    }

    #[test]
    fn test_format_org_aggregates_size_budget() {
        // Verify formatted output stays within reasonable size (~2K chars)
        let agg = OrgAggregates {
            total_employees: 500,
            active_count: 450,
            terminated_count: 40,
            on_leave_count: 10,
            by_department: vec![
                DepartmentCount { name: "Engineering".to_string(), count: 150, percentage: 33.3 },
                DepartmentCount { name: "Sales".to_string(), count: 100, percentage: 22.2 },
                DepartmentCount { name: "Marketing".to_string(), count: 60, percentage: 13.3 },
                DepartmentCount { name: "Operations".to_string(), count: 50, percentage: 11.1 },
                DepartmentCount { name: "Finance".to_string(), count: 40, percentage: 8.9 },
                DepartmentCount { name: "HR".to_string(), count: 30, percentage: 6.7 },
                DepartmentCount { name: "Legal".to_string(), count: 15, percentage: 3.3 },
                DepartmentCount { name: "Executive".to_string(), count: 5, percentage: 1.1 },
            ],
            avg_rating: Some(3.6),
            rating_distribution: RatingDistribution {
                exceptional: 45,
                exceeds: 180,
                meets: 200,
                needs_improvement: 25,
            },
            employees_with_no_rating: 50,
            enps: EnpsAggregate {
                score: 25,
                promoters: 180,
                passives: 150,
                detractors: 70,
                total_responses: 400,
                response_rate: 88.9,
            },
            attrition: AttritionStats {
                terminations_ytd: 40,
                voluntary: 30,
                involuntary: 10,
                avg_tenure_months: Some(36.0),
                turnover_rate_annualized: Some(8.5),
            },
        };

        let formatted = format_org_aggregates(&agg, Some("Large Enterprise Corp"));

        // Should stay under 2500 chars for reasonable context budget
        assert!(
            formatted.len() < 2500,
            "Formatted output too large: {} chars",
            formatted.len()
        );
    }

    // ========================================
    // Query Classification Tests (Phase 2.7.2)
    // ========================================

    #[test]
    fn test_classify_aggregate_queries() {
        // "How many employees?" → Aggregate
        let mentions = extract_mentions("How many employees do we have?");
        assert_eq!(classify_query("How many employees do we have?", &mentions), QueryType::Aggregate);

        // "What's our eNPS?" → Aggregate
        let mentions = extract_mentions("What's our eNPS?");
        assert_eq!(classify_query("What's our eNPS?", &mentions), QueryType::Aggregate);

        // "Average performance rating?" → Aggregate
        let mentions = extract_mentions("What's the average performance rating?");
        assert_eq!(classify_query("What's the average performance rating?", &mentions), QueryType::Aggregate);

        // "Company headcount" → Aggregate
        let mentions = extract_mentions("What's our total headcount?");
        assert_eq!(classify_query("What's our total headcount?", &mentions), QueryType::Aggregate);
    }

    #[test]
    fn test_classify_list_queries() {
        // "Who's in Engineering?" → List
        let mentions = extract_mentions("Who's in Engineering?");
        assert_eq!(classify_query("Who's in Engineering?", &mentions), QueryType::List);

        // "Show me everyone in Sales" → List
        let mentions = extract_mentions("Show me everyone in Sales");
        assert_eq!(classify_query("Show me everyone in Sales", &mentions), QueryType::List);

        // "List all employees in Marketing" → List
        let mentions = extract_mentions("List all employees in Marketing");
        assert_eq!(classify_query("List all employees in Marketing", &mentions), QueryType::List);
    }

    #[test]
    fn test_classify_individual_queries() {
        // "Tell me about Sarah Chen" → Individual
        let mentions = extract_mentions("Tell me about Sarah Chen");
        assert_eq!(classify_query("Tell me about Sarah Chen", &mentions), QueryType::Individual);

        // "What's John's rating?" → Individual
        let mentions = extract_mentions("What's John's rating?");
        assert_eq!(classify_query("What's John's rating?", &mentions), QueryType::Individual);

        // "Is Marcus struggling?" → Individual
        let mentions = extract_mentions("Is Marcus struggling?");
        assert_eq!(classify_query("Is Marcus struggling?", &mentions), QueryType::Individual);
    }

    #[test]
    fn test_classify_comparison_queries() {
        // "Who are our top performers?" → Comparison
        let mentions = extract_mentions("Who are our top performers?");
        assert_eq!(classify_query("Who are our top performers?", &mentions), QueryType::Comparison);

        // "Who's underperforming?" → Comparison
        let mentions = extract_mentions("Who's underperforming?");
        assert_eq!(classify_query("Who's underperforming?", &mentions), QueryType::Comparison);

        // "Show me the star employees" → Comparison
        let mentions = extract_mentions("Show me the star employees");
        assert_eq!(classify_query("Show me the star employees", &mentions), QueryType::Comparison);

        // "Who needs improvement?" → Comparison
        let mentions = extract_mentions("Who needs improvement?");
        assert_eq!(classify_query("Who needs improvement?", &mentions), QueryType::Comparison);
    }

    #[test]
    fn test_classify_attrition_queries() {
        // "Who left this year?" → Attrition
        let mentions = extract_mentions("Who left this year?");
        assert_eq!(classify_query("Who left this year?", &mentions), QueryType::Attrition);

        // "What's our turnover rate?" → Attrition (not Aggregate because turnover is attrition-specific)
        let mentions = extract_mentions("What's our turnover rate?");
        assert_eq!(classify_query("What's our turnover rate?", &mentions), QueryType::Attrition);

        // "Recent departures" → Attrition
        let mentions = extract_mentions("Show me recent departures");
        assert_eq!(classify_query("Show me recent departures", &mentions), QueryType::Attrition);

        // "Who's been terminated?" → Attrition
        let mentions = extract_mentions("Who's been terminated?");
        assert_eq!(classify_query("Who's been terminated?", &mentions), QueryType::Attrition);
    }

    #[test]
    fn test_classify_status_check_queries() {
        // "How's the Engineering team doing?" → Aggregate (status check)
        let mentions = extract_mentions("How's the Engineering team doing?");
        assert_eq!(classify_query("How's the Engineering team doing?", &mentions), QueryType::Aggregate);

        // "How is the Sales department doing?" → Aggregate
        let mentions = extract_mentions("How is the Sales department doing?");
        assert_eq!(classify_query("How is the Sales department doing?", &mentions), QueryType::Aggregate);
    }

    #[test]
    fn test_classify_general_fallback() {
        // Vague question with no clear intent → General
        let mentions = extract_mentions("Tell me something interesting");
        assert_eq!(classify_query("Tell me something interesting", &mentions), QueryType::General);

        // Simple greeting → General
        let mentions = extract_mentions("Hello, can you help me?");
        assert_eq!(classify_query("Hello, can you help me?", &mentions), QueryType::General);
    }

    #[test]
    fn test_classify_priority_individual_over_aggregate() {
        // Name + aggregate phrasing without wants_aggregate flag → Individual wins
        // "Tell me about Sarah's performance" has a name, should be Individual
        let mentions = extract_mentions("Tell me about Sarah's performance");
        assert_eq!(classify_query("Tell me about Sarah's performance", &mentions), QueryType::Individual);
    }

    #[test]
    fn test_classify_priority_comparison_over_list() {
        // "Top performers in Engineering" → Comparison (not List)
        let mentions = extract_mentions("Who are the top performers in Engineering?");
        assert_eq!(classify_query("Who are the top performers in Engineering?", &mentions), QueryType::Comparison);
    }

    #[test]
    fn test_classify_priority_attrition_over_list() {
        // "Who left the Engineering team?" → Attrition (not List)
        let mentions = extract_mentions("Who left the Engineering team?");
        assert_eq!(classify_query("Who left the Engineering team?", &mentions), QueryType::Attrition);
    }

    #[test]
    fn test_classify_aggregate_with_name_and_wants_aggregate() {
        // "What's our company eNPS?" with aggregate flag → Aggregate even if names detected
        let mentions = extract_mentions("What's our company eNPS?");
        // The wants_aggregate flag should be set, so it goes to Aggregate
        assert!(mentions.wants_aggregate);
        assert_eq!(classify_query("What's our company eNPS?", &mentions), QueryType::Aggregate);
    }

    // ========================================
    // Helper Function Tests (Phase 2.7.5)
    // ========================================

    #[test]
    fn test_is_attrition_query_keywords() {
        // Direct attrition keywords
        assert!(is_attrition_query("what's our attrition rate?"));
        assert!(is_attrition_query("show me the turnover data"));
        assert!(is_attrition_query("who left the company?"));
        assert!(is_attrition_query("who's left this year?"));
        assert!(is_attrition_query("recent departures please"));
        assert!(is_attrition_query("who was terminated?"));
        assert!(is_attrition_query("any resignations this quarter?"));
    }

    #[test]
    fn test_is_attrition_query_negative() {
        // Non-attrition queries should return false
        assert!(!is_attrition_query("who's in engineering?"));
        assert!(!is_attrition_query("what's our enps score?"));
        assert!(!is_attrition_query("tell me about sarah chen"));
        assert!(!is_attrition_query("how many employees do we have?"));
    }

    #[test]
    fn test_is_list_query_keywords() {
        let mentions = QueryMentions::default();

        // Direct list keywords
        assert!(is_list_query("who's in engineering?", &mentions));
        assert!(is_list_query("show me the sales team", &mentions));
        assert!(is_list_query("list all employees in marketing", &mentions));
        assert!(is_list_query("everyone in operations", &mentions));
    }

    #[test]
    fn test_is_list_query_with_department() {
        // Department mentioned + roster phrasing = list query
        let mut mentions = QueryMentions::default();
        mentions.departments.push("Engineering".to_string());

        assert!(is_list_query("who is on the engineering team?", &mentions));
        assert!(is_list_query("show me engineering", &mentions));
    }

    #[test]
    fn test_is_list_query_negative() {
        let mentions = QueryMentions::default();

        // Non-list queries
        assert!(!is_list_query("what's our enps?", &mentions));
        assert!(!is_list_query("how many employees?", &mentions));
    }

    #[test]
    fn test_is_aggregate_query_keywords() {
        // Aggregate stat keywords
        assert!(is_aggregate_query("how many employees do we have?"));
        assert!(is_aggregate_query("what's our total headcount?"));
        assert!(is_aggregate_query("what is our average rating?"));
        assert!(is_aggregate_query("show me the breakdown by department"));
        assert!(is_aggregate_query("what percentage are in engineering?"));
        assert!(is_aggregate_query("give me the summary"));
        assert!(is_aggregate_query("company-wide metrics please"));
    }

    #[test]
    fn test_is_aggregate_query_negative() {
        // Non-aggregate queries
        assert!(!is_aggregate_query("tell me about sarah"));
        assert!(!is_aggregate_query("who's in engineering?"));
        assert!(!is_aggregate_query("who left this year?"));
    }

    #[test]
    fn test_is_status_check_patterns() {
        // Status check patterns ("How's X doing?")
        assert!(is_status_check("how's the engineering team doing?"));
        assert!(is_status_check("how is the sales department?"));
        assert!(is_status_check("how are the new hires doing?"));
        assert!(is_status_check("how's our marketing team doing?"));
        assert!(is_status_check("how is our retention doing overall?"));
    }

    #[test]
    fn test_is_status_check_negative() {
        // Non-status queries
        assert!(!is_status_check("who's in engineering?"));
        assert!(!is_status_check("tell me about sarah"));
        assert!(!is_status_check("what's our enps?"));
        assert!(!is_status_check("show me the sales team"));
    }

    // ========================================
    // Employee Summary Formatting Tests (Phase 2.7.5)
    // ========================================

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

        // Showing 1 of 28 employees
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

        // Total equals shown count — should not say "showing x of y"
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

        // Should use defaults for missing fields
        assert!(result.contains("New Hire"));
        assert!(result.contains("No title"));
        assert!(result.contains("Unassigned"));
        assert!(!result.contains("hired")); // No hire date
    }

    // ========================================
    // Edge Case Tests (Phase 2.7.5)
    // ========================================

    #[test]
    fn test_classify_empty_query() {
        let mentions = extract_mentions("");
        assert_eq!(classify_query("", &mentions), QueryType::General);
    }

    #[test]
    fn test_classify_single_word_query() {
        // Single words should generally be General
        let mentions = extract_mentions("help");
        assert_eq!(classify_query("help", &mentions), QueryType::General);

        // Unless it's a clear keyword
        let mentions = extract_mentions("turnover");
        assert_eq!(classify_query("turnover", &mentions), QueryType::Attrition);
    }

    #[test]
    fn test_classify_case_insensitive() {
        // classify_query converts to lowercase internally, so keywords work regardless of case
        // Note: extract_mentions is case-sensitive for name detection (capitalized words = names)
        // so we test with lowercase to avoid name extraction interference

        // Lowercase aggregate query
        let mentions = extract_mentions("how many employees do we have?");
        assert_eq!(
            classify_query("how many employees do we have?", &mentions),
            QueryType::Aggregate
        );

        // Mixed case - title case shouldn't break aggregate detection
        let mentions = extract_mentions("What's our total headcount?");
        assert_eq!(
            classify_query("What's our total headcount?", &mentions),
            QueryType::Aggregate
        );

        // Lowercase attrition
        let mentions = extract_mentions("who left the company?");
        assert_eq!(
            classify_query("who left the company?", &mentions),
            QueryType::Attrition
        );
    }

    #[test]
    fn test_classify_with_punctuation() {
        // Punctuation shouldn't break classification
        let mentions = extract_mentions("Who left??? Tell me!");
        assert_eq!(classify_query("Who left??? Tell me!", &mentions), QueryType::Attrition);
    }

    #[test]
    fn test_employee_summary_size_budget() {
        // Each summary should be ~70 chars to stay within context budget
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

        // 30 summaries should stay well under 3000 chars
        assert!(
            result.len() < 3000,
            "Summary list too large: {} chars",
            result.len()
        );
    }

    // =========================================================================
    // Persona Tests (V2.1.3)
    // =========================================================================

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
        // Invalid ID should fall back to Alex
        let persona = get_persona(Some("invalid_persona"));
        assert_eq!(persona.id, "alex");
    }

    #[test]
    fn test_persona_preamble_has_placeholders() {
        // All personas should have the required placeholders for variable substitution
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

    // =========================================================================
    // V2.1.4 — Answer Verification Tests
    // =========================================================================

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

        // eNPS with positive sign
        let response = "Your eNPS is +12, which is healthy.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.is_aggregate_query);
        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::EnpsScore && c.is_match));
    }

    #[test]
    fn test_verify_enps_without_sign() {
        let agg = make_test_aggregates();

        // eNPS without sign
        let response = "The company eNPS score is 12.";
        let result = verify_response(response, Some(&agg), QueryType::Aggregate);

        assert!(result.claims.iter().any(|c| c.claim_type == ClaimType::EnpsScore && c.is_match));
    }

    #[test]
    fn test_non_aggregate_query_not_applicable() {
        let agg = make_test_aggregates();
        let response = "Sarah has been performing well this quarter.";

        // Individual query type should return NotApplicable
        let result = verify_response(response, Some(&agg), QueryType::Individual);

        assert!(!result.is_aggregate_query);
        assert_eq!(result.overall_status, VerificationStatus::NotApplicable);
        assert!(result.claims.is_empty());
    }

    #[test]
    fn test_verify_turnover_rate_within_tolerance() {
        let agg = make_test_aggregates();

        // Within tolerance (±1%): 15% is within 1% of 14.6%
        // Use format that matches the regex: "15% turnover" or "turnover of 15%"
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

    // ================================================================================
    // V2.2.1 Highlight Formatting Tests
    // ================================================================================

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
            // V2.2.1 highlights
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

        // Positive sentiment should show ↑
        assert!(formatted.contains("↑ 2024 H2 (positive)"));
        // Mixed sentiment should show ↔
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
            // No highlights data
            career_summary: None,
            key_strengths: vec![],
            development_areas: vec![],
            recent_highlights: vec![],
        };

        let formatted = format_single_employee(&emp);

        // Should still format basic info
        assert!(formatted.contains("New Employee"));
        assert!(formatted.contains("Active"));
        // Should NOT have highlight sections
        assert!(!formatted.contains("Career Summary:"));
        assert!(!formatted.contains("Key Strengths:"));
        assert!(!formatted.contains("Recent Review Highlights:"));
    }

    // =========================================================================
    // Token Budget & Metrics Tests (V2.2.2)
    // =========================================================================

    #[test]
    fn test_token_budget_for_aggregate_query() {
        let budget = TokenBudget::for_query_type(QueryType::Aggregate);
        assert_eq!(budget.employee_context, 0); // No individual employees needed
        assert_eq!(budget.theme_context, 500);
        assert_eq!(budget.memory_context, 500);
        assert_eq!(budget.total_context, 1_000);
    }

    #[test]
    fn test_token_budget_for_individual_query() {
        let budget = TokenBudget::for_query_type(QueryType::Individual);
        assert_eq!(budget.employee_context, 4_000); // Full profiles
        assert_eq!(budget.theme_context, 0);
        assert_eq!(budget.memory_context, 1_000);
        assert_eq!(budget.total_context, 5_000);
    }

    #[test]
    fn test_token_budget_for_list_query() {
        let budget = TokenBudget::for_query_type(QueryType::List);
        assert_eq!(budget.employee_context, 2_000); // Lightweight summaries
        assert_eq!(budget.total_context, 2_500);
    }

    #[test]
    fn test_token_budget_for_comparison_query() {
        let budget = TokenBudget::for_query_type(QueryType::Comparison);
        assert_eq!(budget.employee_context, 3_000); // Multiple full profiles
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

    // =========================================================================
    // V2.2.2a: Dynamic Excerpting Tests
    // =========================================================================

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
        // When text has exactly max_sentences, return it unchanged
        assert_eq!(excerpt_to_sentences(text, 3), text);
    }

    #[test]
    fn test_excerpt_to_sentences_truncation() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence.";
        let result = excerpt_to_sentences(text, 2);
        // Should have first 2 sentences plus ellipsis
        assert!(result.starts_with("First sentence. Second sentence."));
        assert!(result.ends_with(".."));
    }

    #[test]
    fn test_excerpt_to_sentences_unicode() {
        // Test with various punctuation types
        let text = "Hello! How are you? I'm fine. Thanks for asking!";
        let result = excerpt_to_sentences(text, 2);
        assert!(result.starts_with("Hello! How are you?"));
        assert!(result.ends_with(".."));
    }

    #[test]
    fn test_excerpt_to_sentences_preserves_whitespace() {
        let text = "  First sentence.   Second sentence.  ";
        // Should trim input and preserve internal structure
        let result = excerpt_to_sentences(text, 1);
        assert!(result.starts_with("First sentence."));
    }

    #[test]
    fn test_calculate_excerpt_limits_full_budget() {
        // Full budget (>= 800 tokens)
        let (summary, cycles) = calculate_excerpt_limits(1000);
        assert_eq!(summary, 5); // FULL_BUDGET_SUMMARY_SENTENCES
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_calculate_excerpt_limits_reduced_budget() {
        // Reduced budget (400-799 tokens)
        let (summary, cycles) = calculate_excerpt_limits(500);
        assert_eq!(summary, 2); // REDUCED_BUDGET_SUMMARY_SENTENCES
        assert_eq!(cycles, 2);
    }

    #[test]
    fn test_calculate_excerpt_limits_tight_budget() {
        // Tight budget (< 400 tokens)
        let (summary, cycles) = calculate_excerpt_limits(200);
        assert_eq!(summary, 1); // MIN_SENTENCES
        assert_eq!(cycles, 1);
    }

    #[test]
    fn test_calculate_per_employee_budget_single() {
        // Single employee gets full budget
        assert_eq!(calculate_per_employee_budget(4000, 1), 4000);
    }

    #[test]
    fn test_calculate_per_employee_budget_multiple() {
        // Multiple employees split evenly
        assert_eq!(calculate_per_employee_budget(4000, 4), 1000);
    }

    #[test]
    fn test_calculate_per_employee_budget_minimum_floor() {
        // Should not go below minimum (200)
        assert_eq!(calculate_per_employee_budget(1000, 10), 200);
        // 100 would be below floor, so should be 200
        assert_eq!(calculate_per_employee_budget(500, 10), 200);
    }

    #[test]
    fn test_calculate_per_employee_budget_zero_employees() {
        // Edge case: zero employees returns full budget
        assert_eq!(calculate_per_employee_budget(4000, 0), 4000);
    }

    // =========================================================================
    // V2.2.2b: Theme-Based Retrieval Tests
    // =========================================================================

    #[test]
    fn test_extract_mentions_theme_direct() {
        let query = "Who has leadership feedback?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"leadership".to_string()));
        assert_eq!(mentions.theme_target, ThemeTarget::Any);
    }

    #[test]
    fn test_extract_mentions_theme_opportunity() {
        let query = "Who needs help with communication?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"communication".to_string()));
        assert_eq!(mentions.theme_target, ThemeTarget::Opportunities);
    }

    #[test]
    fn test_extract_mentions_theme_strengths() {
        let query = "Employees who are strong in mentoring";
        let mentions = extract_mentions(query);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"mentoring".to_string()));
        assert_eq!(mentions.theme_target, ThemeTarget::Strengths);
    }

    #[test]
    fn test_extract_mentions_theme_with_department() {
        let query = "Leadership issues in Engineering";
        let mentions = extract_mentions(query);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"leadership".to_string()));
        assert!(mentions.departments.contains(&"Engineering".to_string()));
    }

    #[test]
    fn test_extract_mentions_theme_semantic() {
        // "people skills" should map to "communication"
        let query = "Who has issues with people skills?";
        let mentions = extract_mentions(query);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"communication".to_string()));
    }

    #[test]
    fn test_extract_mentions_multiple_themes() {
        let query = "Leadership and communication concerns";
        let mentions = extract_mentions(query);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"leadership".to_string()));
        assert!(mentions.requested_themes.contains(&"communication".to_string()));
    }

    #[test]
    fn test_classify_theme_query() {
        let query = "Who has leadership feedback?";
        let mentions = extract_mentions(query);
        let query_type = classify_query(query, &mentions);
        // Theme queries are classified as Comparison
        assert_eq!(query_type, QueryType::Comparison);
    }

    #[test]
    fn test_theme_target_default() {
        assert_eq!(ThemeTarget::default(), ThemeTarget::Any);
    }

    #[test]
    fn test_failing_query_collaboration() {
        let query = "Employees strong in collaboration";
        let mentions = extract_mentions(query);
        println!("Query: '{}'", query);
        println!("  is_theme_query: {}", mentions.is_theme_query);
        println!("  requested_themes: {:?}", mentions.requested_themes);
        println!("  theme_target: {:?}", mentions.theme_target);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"collaboration".to_string()));
        assert_eq!(mentions.theme_target, ThemeTarget::Strengths);
    }

    #[test]
    fn test_failing_query_teamwork() {
        let query = "Show me people with teamwork feedback";
        let mentions = extract_mentions(query);
        println!("Query: '{}'", query);
        println!("  is_theme_query: {}", mentions.is_theme_query);
        println!("  requested_themes: {:?}", mentions.requested_themes);
        println!("  theme_target: {:?}", mentions.theme_target);
        assert!(mentions.is_theme_query);
        // "teamwork" should map to "collaboration"
        assert!(mentions.requested_themes.contains(&"collaboration".to_string()));
    }

    #[test]
    fn test_classify_failing_queries() {
        // Test query classification for the failing queries
        let query1 = "Employees strong in collaboration";
        let mentions1 = extract_mentions(query1);
        let type1 = classify_query(query1, &mentions1);
        println!("Query1: '{}' -> {:?}", query1, type1);
        println!("  names: {:?}", mentions1.names);
        println!("  is_theme_query: {}", mentions1.is_theme_query);

        let query2 = "Show me people with teamwork feedback";
        let mentions2 = extract_mentions(query2);
        let type2 = classify_query(query2, &mentions2);
        println!("Query2: '{}' -> {:?}", query2, type2);
        println!("  names: {:?}", mentions2.names);
        println!("  is_theme_query: {}", mentions2.is_theme_query);

        // Both should be Comparison (theme queries)
        assert_eq!(type1, QueryType::Comparison, "Query1 should be Comparison");
        assert_eq!(type2, QueryType::Comparison, "Query2 should be Comparison");
    }
}
