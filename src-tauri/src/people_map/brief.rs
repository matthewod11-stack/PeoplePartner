//! Prep-brief generator (FHR-108, People Map T7).
//!
//! Composes the brief prompt from an assembled grounding context, sends it
//! through the audited chat seam (Standard redaction + an audit row per
//! attempt come from the seam, #112), parses the JSON response, and enforces
//! grounding: citations outside the canonical set are dropped along with the
//! fact or thread that asserted them (T7 build-time decision — drop, don't
//! silently retry; the UI owns the regenerate affordance). A brief left with
//! no real facts is an error, never a hallucinated render.
//!
//! The prompt template lives in `prompts/prep_brief.md` with its comfort-test
//! checklist header; the header is stripped before the text goes anywhere.

use async_trait::async_trait;

use super::context::{assemble_grounding_context, ContextError, GroundingContext, GroundingKind};
use super::schema::PrepBrief;
use crate::audit::{EgressAudit, EgressSource};
use crate::chat::{ChatError, ChatMessage};
use crate::db::DbPool;
use crate::grounding::phantom_citations;

/// Rendered verbatim when a record can't anchor threads (decision 7 —
/// fewer/none beats filler).
pub const THIN_RECORD_NOTE: &str = "This record is too thin to anchor conversation threads — \
     add performance-review narratives or import documents to enrich future briefs.";

const TEMPLATE: &str = include_str!("prompts/prep_brief.md");

#[derive(Debug, thiserror::Error)]
pub enum BriefError {
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error("chat seam error: {0}")]
    Chat(#[from] ChatError),
    #[error("brief response was not valid JSON: {0}")]
    Parse(String),
    #[error(
        "brief cited nothing from the record ({phantom_count} phantom citations) — regenerate"
    )]
    NotGrounded { phantom_count: usize },
}

/// Transport seam: production goes through the chat seam; tests capture the
/// composed prompt. The trial path (T8) implements this over the proxy.
#[async_trait]
pub trait BriefTransport: Send + Sync {
    async fn complete(
        &self,
        pool: &DbPool,
        audit: EgressAudit,
        messages: Vec<ChatMessage>,
        system_prompt: String,
    ) -> Result<String, ChatError>;
}

/// Production transport: the user's active BYOK provider via
/// `chat::send_message` — Standard redaction and the per-attempt audit row
/// happen inside the seam (#112).
pub struct SeamTransport;

#[async_trait]
impl BriefTransport for SeamTransport {
    async fn complete(
        &self,
        pool: &DbPool,
        audit: EgressAudit,
        messages: Vec<ChatMessage>,
        system_prompt: String,
    ) -> Result<String, ChatError> {
        let active = crate::chat::resolve_active_provider(pool)
            .await
            .map_err(|e| ChatError::RequestError(e.to_string()))?;
        let response = crate::chat::send_message(
            pool,
            audit,
            messages,
            Some(system_prompt),
            &active.provider_id,
            active.model_id.as_deref(),
        )
        .await?;
        Ok(response.content)
    }
}

fn kind_label(kind: GroundingKind) -> &'static str {
    match kind {
        GroundingKind::RecordField => "Record",
        GroundingKind::CareerSummary => "Summary",
        GroundingKind::ReviewNarrative => "Review",
    }
}

/// Template body with the comfort-test checklist header stripped — the
/// checklist is for template editors, not the model.
fn template_body() -> &'static str {
    match TEMPLATE.split_once("-->") {
        Some((_, body)) => body.trim_start(),
        None => TEMPLATE,
    }
}

/// Compose (system_prompt, user_message) for one grounding context.
pub fn compose_brief_prompt(ctx: &GroundingContext) -> (String, String) {
    let role_line = match (ctx.job_title.as_deref(), ctx.department.as_deref()) {
        (Some(title), Some(dept)) => format!("{title}, {dept}"),
        (Some(title), None) => title.to_string(),
        (None, Some(dept)) => dept.to_string(),
        (None, None) => "role on file".to_string(),
    };
    let thin_instruction = if ctx.is_thin() {
        format!(
            "This record IS thin ({} narrative source(s)). Output the facts, an empty \
             threads array, and set thinRecordNote to exactly: \"{THIN_RECORD_NOTE}\"",
            ctx.narrative_item_count()
        )
    } else {
        format!(
            "If the context cannot anchor real threads, set threads to [] and \
             thinRecordNote to exactly: \"{THIN_RECORD_NOTE}\". Otherwise thinRecordNote \
             must be null. Never pad with generic questions."
        )
    };

    let system = template_body()
        .replace("{{employee_name}}", &ctx.employee_name)
        .replace("{{employee_id}}", &ctx.employee_id)
        .replace("{{role_line}}", &role_line)
        .replace("{{max_threads}}", &ctx.max_threads().to_string())
        .replace("{{thin_record_instruction}}", &thin_instruction);

    let mut user = format!(
        "Grounding context for {} — the ONLY citable items:\n",
        ctx.employee_name
    );
    for item in &ctx.items {
        user.push_str(&format!(
            "[{}] ({} — {}): {}\n",
            item.citation_id,
            kind_label(item.kind),
            item.label,
            item.content
        ));
    }
    user.push_str("\nProduce the JSON brief now.");
    (system, user)
}

/// Parse the model's response into a `PrepBrief`. Tolerates markdown fences
/// and stray prose by slicing from the first `{` to the last `}`.
pub fn parse_brief_response(text: &str) -> Result<PrepBrief, BriefError> {
    let start = text.find('{');
    let end = text.rfind('}');
    let json = match (start, end) {
        (Some(s), Some(e)) if s < e => &text[s..=e],
        _ => return Err(BriefError::Parse("no JSON object found".into())),
    };
    serde_json::from_str(json).map_err(|e| BriefError::Parse(e.to_string()))
}

/// Enforce grounding and the thread budget on a parsed brief.
///
/// T7 build-time decision (architecture-review carry-over): phantom handling
/// on this path is **drop, don't retry** — a fact or thread citing outside
/// the canonical set is removed; if nothing grounded remains the caller gets
/// `NotGrounded` and the user regenerates deliberately. No silent second LLM
/// call (predictable cost, and trial messages are metered).
pub fn validate_brief(
    mut brief: PrepBrief,
    ctx: &GroundingContext,
) -> Result<PrepBrief, BriefError> {
    // The model never gets to relabel who the brief is about.
    brief.employee_id = ctx.employee_id.clone();

    let canonical = ctx.canonical_ids();
    let phantom_count = phantom_citations(&brief, &canonical).len();
    brief.facts.retain(|f| canonical.contains(&f.citation_id));
    brief
        .threads
        .retain(|t| canonical.contains(&t.anchor_citation_id));

    if brief.facts.is_empty() {
        return Err(BriefError::NotGrounded { phantom_count });
    }

    brief.threads.truncate(ctx.max_threads());
    if ctx.is_thin() {
        brief.threads.clear();
    }
    if brief.threads.is_empty() && brief.thin_record_note.is_none() {
        brief.thin_record_note = Some(THIN_RECORD_NOTE.to_string());
    }
    if !brief.threads.is_empty() {
        brief.thin_record_note = None;
    }

    Ok(brief)
}

/// Generate a prep brief for one employee through the given transport.
/// Ephemeral by construction: nothing is persisted here — the audit row the
/// seam writes is the only durable trace (decision 9).
pub async fn generate_brief(
    pool: &DbPool,
    employee_id: &str,
    transport: &dyn BriefTransport,
) -> Result<PrepBrief, BriefError> {
    let ctx = assemble_grounding_context(pool, employee_id).await?;
    let (system_prompt, user_message) = compose_brief_prompt(&ctx);
    let audit = EgressAudit {
        source: EgressSource::PrepBrief,
        conversation_id: None,
        employee_ids: vec![ctx.employee_id.clone()],
        query_category: None,
    };
    let response = transport
        .complete(
            pool,
            audit,
            vec![ChatMessage {
                role: "user".into(),
                content: user_message,
            }],
            system_prompt,
        )
        .await?;
    let brief = parse_brief_response(&response)?;
    validate_brief(brief, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people_map::schema::{BriefFact, BriefThread};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::sync::Mutex;
    use std::time::Duration;

    async fn test_pool() -> DbPool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect :memory: pool");
        crate::db::run_migrations_for_tests(&pool)
            .await
            .expect("run migrations");
        pool
    }

    async fn seed_typical_employee(pool: &DbPool) {
        sqlx::query(
            "INSERT INTO employees (id, email, full_name, department, job_title, hire_date, work_state, status)
             VALUES ('emp-1', 'ada@example.com', 'Ada Example', 'Engineering', 'Software Engineer',
                     '2023-06-27', 'California', 'active')",
        )
        .execute(pool)
        .await
        .expect("insert employee");
        sqlx::query(
            "INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date)
             VALUES ('cy-1', 'Annual 2025', 'annual', '2025-01-01', '2025-12-31')",
        )
        .execute(pool)
        .await
        .expect("insert cycle");
        sqlx::query(
            "INSERT INTO performance_reviews (id, employee_id, review_cycle_id, strengths,
                 areas_for_improvement, accomplishments, manager_comments, review_date)
             VALUES ('rev-1', 'emp-1', 'cy-1', 'Reliable performer.', 'Could document more.',
                     'Led the gateway work with zero downtime.', 'Valued team member.', '2025-03-22')",
        )
        .execute(pool)
        .await
        .expect("insert review");
    }

    fn ctx_for(items_narrative: usize) -> GroundingContext {
        use crate::people_map::context::GroundingItem;
        let mut items = vec![GroundingItem {
            citation_id: "C1".into(),
            kind: GroundingKind::RecordField,
            label: "Full name".into(),
            content: "Ada Example".into(),
        }];
        for n in 0..items_narrative {
            items.push(GroundingItem {
                citation_id: format!("C{}", n + 2),
                kind: GroundingKind::ReviewNarrative,
                label: format!("Strengths (review dated 2025-0{}-01)", n + 1),
                content: format!("Narrative item {}.", n + 1),
            });
        }
        GroundingContext {
            employee_id: "emp-1".into(),
            employee_name: "Ada Example".into(),
            job_title: Some("Software Engineer".into()),
            department: Some("Engineering".into()),
            items,
        }
    }

    fn brief_with(facts: Vec<(&str, &str)>, threads: Vec<(&str, &str)>) -> PrepBrief {
        PrepBrief {
            employee_id: "model-claimed".into(),
            facts: facts
                .into_iter()
                .map(|(text, cid)| BriefFact {
                    text: text.into(),
                    citation_id: cid.into(),
                })
                .collect(),
            threads: threads
                .into_iter()
                .map(|(cid, q)| BriefThread {
                    anchor_citation_id: cid.into(),
                    anchor_fact: "anchor".into(),
                    question: q.into(),
                })
                .collect(),
            thin_record_note: None,
        }
    }

    struct CapturingTransport {
        captured: Mutex<Option<(String, Vec<String>, Vec<ChatMessage>, String)>>,
        response: String,
    }

    impl CapturingTransport {
        fn returning(response: &str) -> Self {
            Self {
                captured: Mutex::new(None),
                response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl BriefTransport for CapturingTransport {
        async fn complete(
            &self,
            _pool: &DbPool,
            audit: EgressAudit,
            messages: Vec<ChatMessage>,
            system_prompt: String,
        ) -> Result<String, ChatError> {
            *self.captured.lock().unwrap() = Some((
                audit.source.as_str().to_string(),
                audit.employee_ids.clone(),
                messages,
                system_prompt,
            ));
            if self.response == "ERR" {
                return Err(ChatError::ApiError("HTTP 500: boom".into()));
            }
            Ok(self.response.clone())
        }
    }

    fn valid_response_json() -> String {
        r#"{
            "employeeId": "emp-1",
            "facts": [
                {"text": "Software Engineer in Engineering.", "citationId": "C2"},
                {"text": "Led the gateway work with zero downtime.", "citationId": "C9"},
                {"text": "Entirely invented claim.", "citationId": "C99"}
            ],
            "threads": [
                {"anchorCitationId": "C9", "anchorFact": "Led the gateway work.", "question": "What made the cutover smooth?"}
            ],
            "thinRecordNote": null
        }"#
        .to_string()
    }

    // ------------------------------------------------------------------
    // Prompt composition
    // ------------------------------------------------------------------

    #[test]
    fn compose_includes_name_items_and_budget_and_strips_checklist() {
        let ctx = ctx_for(4);
        let (system, user) = compose_brief_prompt(&ctx);
        assert!(system.contains("Ada Example"));
        assert!(system.contains("Software Engineer, Engineering"));
        assert!(system.contains("At most 3 conversation openers"));
        assert!(system.contains("Hard rules"));
        assert!(
            !system.contains("TEMPLATE-CHANGE CHECKLIST"),
            "editor checklist must not reach the model"
        );
        assert!(user.contains("[C1] (Record — Full name): Ada Example"));
        assert!(user.contains("Narrative item 1."));
    }

    #[test]
    fn compose_thin_record_demands_empty_threads_and_note() {
        let ctx = ctx_for(1); // 1 narrative item → thin
        let (system, _user) = compose_brief_prompt(&ctx);
        assert!(system.contains("This record IS thin"));
        assert!(system.contains(THIN_RECORD_NOTE));
        assert!(system.contains("At most 0 conversation openers"));
    }

    // ------------------------------------------------------------------
    // Response parsing
    // ------------------------------------------------------------------

    #[test]
    fn parse_accepts_bare_and_fenced_json() {
        let bare = valid_response_json();
        assert!(parse_brief_response(&bare).is_ok());
        let fenced = format!("```json\n{bare}\n```");
        assert!(parse_brief_response(&fenced).is_ok());
        let prose = format!("Here is the brief:\n{bare}\nHope that helps!");
        assert!(parse_brief_response(&prose).is_ok());
    }

    #[test]
    fn parse_rejects_non_json() {
        assert!(matches!(
            parse_brief_response("I cannot produce a brief."),
            Err(BriefError::Parse(_))
        ));
    }

    // ------------------------------------------------------------------
    // Grounding enforcement (T7 decision: drop, don't retry)
    // ------------------------------------------------------------------

    #[test]
    fn phantom_fact_and_thread_are_dropped() {
        let ctx = ctx_for(4); // C1..C5 canonical
        let brief = brief_with(
            vec![("real", "C2"), ("fake", "C99")],
            vec![("C3", "ok?"), ("C98", "fake?")],
        );
        let out = validate_brief(brief, &ctx).expect("brief survives");
        assert_eq!(out.facts.len(), 1);
        assert_eq!(out.facts[0].citation_id, "C2");
        assert_eq!(out.threads.len(), 1);
        assert_eq!(out.threads[0].anchor_citation_id, "C3");
    }

    #[test]
    fn all_phantom_facts_is_not_grounded() {
        let ctx = ctx_for(4);
        let brief = brief_with(vec![("fake", "C99"), ("fake2", "C98")], vec![]);
        match validate_brief(brief, &ctx) {
            Err(BriefError::NotGrounded { phantom_count }) => assert_eq!(phantom_count, 2),
            other => panic!("expected NotGrounded, got {other:?}"),
        }
    }

    #[test]
    fn threads_truncated_to_budget() {
        let ctx = ctx_for(2); // budget 1
        let brief = brief_with(vec![("real", "C1")], vec![("C2", "one?"), ("C3", "two?")]);
        let out = validate_brief(brief, &ctx).expect("brief survives");
        assert_eq!(out.threads.len(), 1);
    }

    #[test]
    fn thin_record_forces_no_threads_and_a_note() {
        let ctx = ctx_for(1); // thin
        let brief = brief_with(vec![("real", "C1")], vec![("C2", "sneaky thread?")]);
        let out = validate_brief(brief, &ctx).expect("brief survives");
        assert!(out.threads.is_empty());
        assert_eq!(out.thin_record_note.as_deref(), Some(THIN_RECORD_NOTE));
    }

    #[test]
    fn note_cleared_when_threads_exist() {
        let ctx = ctx_for(4);
        let mut brief = brief_with(vec![("real", "C1")], vec![("C2", "ok?")]);
        brief.thin_record_note = Some("stale note".into());
        let out = validate_brief(brief, &ctx).expect("brief survives");
        assert!(out.thin_record_note.is_none());
    }

    #[test]
    fn employee_id_is_forced_from_context() {
        let ctx = ctx_for(4);
        let brief = brief_with(vec![("real", "C1")], vec![]);
        let out = validate_brief(brief, &ctx).expect("brief survives");
        assert_eq!(out.employee_id, "emp-1");
    }

    // ------------------------------------------------------------------
    // End-to-end through a capturing transport
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn generate_brief_pins_the_egress_seam() {
        let pool = test_pool().await;
        seed_typical_employee(&pool).await;
        let transport = CapturingTransport::returning(&valid_response_json());

        let brief = generate_brief(&pool, "emp-1", &transport)
            .await
            .expect("generate");

        let (source, employee_ids, messages, system) = self::capture(&transport);
        assert_eq!(source, "prep_brief");
        assert_eq!(employee_ids, vec!["emp-1".to_string()]);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("Ada Example"));
        assert!(messages[0].content.contains("Led the gateway work"));
        assert!(system.contains("Hard rules"));

        // The seeded record assembles C1..C10 (6 record fields + 4 review
        // narratives). C2 and C9 are canonical; C99 is a phantom and its
        // fact drops (T7 decision: drop, don't retry).
        let cited: Vec<&str> = brief.facts.iter().map(|f| f.citation_id.as_str()).collect();
        assert_eq!(cited, vec!["C2", "C9"]);
        assert_eq!(brief.threads.len(), 1);
        assert_eq!(brief.employee_id, "emp-1");
    }

    fn capture(t: &CapturingTransport) -> (String, Vec<String>, Vec<ChatMessage>, String) {
        t.captured
            .lock()
            .unwrap()
            .clone()
            .expect("transport captured the call")
    }

    #[tokio::test]
    async fn generate_brief_propagates_transport_error() {
        let pool = test_pool().await;
        seed_typical_employee(&pool).await;
        let transport = CapturingTransport::returning("ERR");

        match generate_brief(&pool, "emp-1", &transport).await {
            Err(BriefError::Chat(_)) => {}
            other => panic!("expected chat error, got {other:?}"),
        }
        // The audit row for the failed attempt is written inside the seam
        // (chat::send_message / the trial path) per #112 — pinned by the
        // seam's own integration tests and audit.rs's prep_brief round-trip.
    }

    // ------------------------------------------------------------------
    // Redaction pin: brief-shaped content under the Standard policy —
    // financial PII redacted, name and work history intact (decision 8).
    // ------------------------------------------------------------------

    #[test]
    fn standard_policy_redacts_financial_but_keeps_name_and_history() {
        let brief_shaped = "Grounding context for Ada Example — the ONLY citable items:\n\
             [C1] (Record — Full name): Ada Example\n\
             [C2] (Review — Strengths): Led payroll fix after SSN 123-45-6789 leaked.\n";
        let result =
            crate::pii::scan_and_redact_with(brief_shaped, crate::pii::RedactionPolicy::Standard);
        assert!(result.redacted_text.contains("[SSN_REDACTED]"));
        assert!(!result.redacted_text.contains("123-45-6789"));
        assert!(result.redacted_text.contains("Ada Example"));
        assert!(result.redacted_text.contains("Led payroll fix"));
    }
}
