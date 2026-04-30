//! Query intent classification + mention extraction.
//!
//! Pure logic — no DB access. Consumed by retrieval routing and the system
//! prompt builder.

use serde::{Deserialize, Serialize};

// ============================================================================
// Query Classification Types
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
// Query Mention Types
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
// Query Classification
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

    #[test]
    fn test_query_type_serialization() {
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
    fn test_classify_aggregate_queries() {
        let mentions = extract_mentions("How many employees do we have?");
        assert_eq!(classify_query("How many employees do we have?", &mentions), QueryType::Aggregate);

        let mentions = extract_mentions("What's our eNPS?");
        assert_eq!(classify_query("What's our eNPS?", &mentions), QueryType::Aggregate);

        let mentions = extract_mentions("What's the average performance rating?");
        assert_eq!(classify_query("What's the average performance rating?", &mentions), QueryType::Aggregate);

        let mentions = extract_mentions("What's our total headcount?");
        assert_eq!(classify_query("What's our total headcount?", &mentions), QueryType::Aggregate);
    }

    #[test]
    fn test_classify_list_queries() {
        let mentions = extract_mentions("Who's in Engineering?");
        assert_eq!(classify_query("Who's in Engineering?", &mentions), QueryType::List);

        let mentions = extract_mentions("Show me everyone in Sales");
        assert_eq!(classify_query("Show me everyone in Sales", &mentions), QueryType::List);

        let mentions = extract_mentions("List all employees in Marketing");
        assert_eq!(classify_query("List all employees in Marketing", &mentions), QueryType::List);
    }

    #[test]
    fn test_classify_individual_queries() {
        let mentions = extract_mentions("Tell me about Sarah Chen");
        assert_eq!(classify_query("Tell me about Sarah Chen", &mentions), QueryType::Individual);

        let mentions = extract_mentions("What's John's rating?");
        assert_eq!(classify_query("What's John's rating?", &mentions), QueryType::Individual);

        let mentions = extract_mentions("Is Marcus struggling?");
        assert_eq!(classify_query("Is Marcus struggling?", &mentions), QueryType::Individual);
    }

    #[test]
    fn test_classify_comparison_queries() {
        let mentions = extract_mentions("Who are our top performers?");
        assert_eq!(classify_query("Who are our top performers?", &mentions), QueryType::Comparison);

        let mentions = extract_mentions("Who's underperforming?");
        assert_eq!(classify_query("Who's underperforming?", &mentions), QueryType::Comparison);

        let mentions = extract_mentions("Show me the star employees");
        assert_eq!(classify_query("Show me the star employees", &mentions), QueryType::Comparison);

        let mentions = extract_mentions("Who needs improvement?");
        assert_eq!(classify_query("Who needs improvement?", &mentions), QueryType::Comparison);
    }

    #[test]
    fn test_classify_attrition_queries() {
        let mentions = extract_mentions("Who left this year?");
        assert_eq!(classify_query("Who left this year?", &mentions), QueryType::Attrition);

        let mentions = extract_mentions("What's our turnover rate?");
        assert_eq!(classify_query("What's our turnover rate?", &mentions), QueryType::Attrition);

        let mentions = extract_mentions("Show me recent departures");
        assert_eq!(classify_query("Show me recent departures", &mentions), QueryType::Attrition);

        let mentions = extract_mentions("Who's been terminated?");
        assert_eq!(classify_query("Who's been terminated?", &mentions), QueryType::Attrition);
    }

    #[test]
    fn test_classify_status_check_queries() {
        let mentions = extract_mentions("How's the Engineering team doing?");
        assert_eq!(classify_query("How's the Engineering team doing?", &mentions), QueryType::Aggregate);

        let mentions = extract_mentions("How is the Sales department doing?");
        assert_eq!(classify_query("How is the Sales department doing?", &mentions), QueryType::Aggregate);
    }

    #[test]
    fn test_classify_general_fallback() {
        let mentions = extract_mentions("Tell me something interesting");
        assert_eq!(classify_query("Tell me something interesting", &mentions), QueryType::General);

        let mentions = extract_mentions("Hello, can you help me?");
        assert_eq!(classify_query("Hello, can you help me?", &mentions), QueryType::General);
    }

    #[test]
    fn test_classify_priority_individual_over_aggregate() {
        let mentions = extract_mentions("Tell me about Sarah's performance");
        assert_eq!(classify_query("Tell me about Sarah's performance", &mentions), QueryType::Individual);
    }

    #[test]
    fn test_classify_priority_comparison_over_list() {
        let mentions = extract_mentions("Who are the top performers in Engineering?");
        assert_eq!(classify_query("Who are the top performers in Engineering?", &mentions), QueryType::Comparison);
    }

    #[test]
    fn test_classify_priority_attrition_over_list() {
        let mentions = extract_mentions("Who left the Engineering team?");
        assert_eq!(classify_query("Who left the Engineering team?", &mentions), QueryType::Attrition);
    }

    #[test]
    fn test_classify_aggregate_with_name_and_wants_aggregate() {
        let mentions = extract_mentions("What's our company eNPS?");
        assert!(mentions.wants_aggregate);
        assert_eq!(classify_query("What's our company eNPS?", &mentions), QueryType::Aggregate);
    }

    #[test]
    fn test_is_attrition_query_keywords() {
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
        assert!(!is_attrition_query("who's in engineering?"));
        assert!(!is_attrition_query("what's our enps score?"));
        assert!(!is_attrition_query("tell me about sarah chen"));
        assert!(!is_attrition_query("how many employees do we have?"));
    }

    #[test]
    fn test_is_list_query_keywords() {
        let mentions = QueryMentions::default();

        assert!(is_list_query("who's in engineering?", &mentions));
        assert!(is_list_query("show me the sales team", &mentions));
        assert!(is_list_query("list all employees in marketing", &mentions));
        assert!(is_list_query("everyone in operations", &mentions));
    }

    #[test]
    fn test_is_list_query_with_department() {
        let mut mentions = QueryMentions::default();
        mentions.departments.push("Engineering".to_string());

        assert!(is_list_query("who is on the engineering team?", &mentions));
        assert!(is_list_query("show me engineering", &mentions));
    }

    #[test]
    fn test_is_list_query_negative() {
        let mentions = QueryMentions::default();

        assert!(!is_list_query("what's our enps?", &mentions));
        assert!(!is_list_query("how many employees?", &mentions));
    }

    #[test]
    fn test_is_aggregate_query_keywords() {
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
        assert!(!is_aggregate_query("tell me about sarah"));
        assert!(!is_aggregate_query("who's in engineering?"));
        assert!(!is_aggregate_query("who left this year?"));
    }

    #[test]
    fn test_is_status_check_patterns() {
        assert!(is_status_check("how's the engineering team doing?"));
        assert!(is_status_check("how is the sales department?"));
        assert!(is_status_check("how are the new hires doing?"));
        assert!(is_status_check("how's our marketing team doing?"));
        assert!(is_status_check("how is our retention doing overall?"));
    }

    #[test]
    fn test_is_status_check_negative() {
        assert!(!is_status_check("who's in engineering?"));
        assert!(!is_status_check("tell me about sarah"));
        assert!(!is_status_check("what's our enps?"));
        assert!(!is_status_check("show me the sales team"));
    }

    #[test]
    fn test_classify_empty_query() {
        let mentions = extract_mentions("");
        assert_eq!(classify_query("", &mentions), QueryType::General);
    }

    #[test]
    fn test_classify_single_word_query() {
        let mentions = extract_mentions("help");
        assert_eq!(classify_query("help", &mentions), QueryType::General);

        let mentions = extract_mentions("turnover");
        assert_eq!(classify_query("turnover", &mentions), QueryType::Attrition);
    }

    #[test]
    fn test_classify_case_insensitive() {
        let mentions = extract_mentions("how many employees do we have?");
        assert_eq!(
            classify_query("how many employees do we have?", &mentions),
            QueryType::Aggregate
        );

        let mentions = extract_mentions("What's our total headcount?");
        assert_eq!(
            classify_query("What's our total headcount?", &mentions),
            QueryType::Aggregate
        );

        let mentions = extract_mentions("who left the company?");
        assert_eq!(
            classify_query("who left the company?", &mentions),
            QueryType::Attrition
        );
    }

    #[test]
    fn test_classify_with_punctuation() {
        let mentions = extract_mentions("Who left??? Tell me!");
        assert_eq!(classify_query("Who left??? Tell me!", &mentions), QueryType::Attrition);
    }

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
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"collaboration".to_string()));
        assert_eq!(mentions.theme_target, ThemeTarget::Strengths);
    }

    #[test]
    fn test_failing_query_teamwork() {
        let query = "Show me people with teamwork feedback";
        let mentions = extract_mentions(query);
        assert!(mentions.is_theme_query);
        assert!(mentions.requested_themes.contains(&"collaboration".to_string()));
    }

    #[test]
    fn test_classify_failing_queries() {
        let query1 = "Employees strong in collaboration";
        let mentions1 = extract_mentions(query1);
        let type1 = classify_query(query1, &mentions1);

        let query2 = "Show me people with teamwork feedback";
        let mentions2 = extract_mentions(query2);
        let type2 = classify_query(query2, &mentions2);

        assert_eq!(type1, QueryType::Comparison, "Query1 should be Comparison");
        assert_eq!(type2, QueryType::Comparison, "Query2 should be Comparison");
    }
}
