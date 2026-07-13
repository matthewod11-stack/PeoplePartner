//! Grounding-context assembly for prep briefs (FHR-107, People Map T6).
//!
//! Given an employee id, assembles every grounding item a brief may cite —
//! allowlisted record fields, the career summary, and review narratives —
//! assigning stable citation IDs at assembly time. The canonical set handed
//! to the citation validator is exactly this assembled set (spec In-scope 1).
//!
//! Field allowlist (T1 build-time decision): name, title, department,
//! manager, hire date, work state, status. Demographic and termination
//! columns never enter a brief — assembly reads only the allowlisted fields,
//! and a test pins that their values never surface. Assembly order is
//! deterministic, so the same record yields the same citation IDs on every
//! assembly (thread anchors survive a regenerate).
//!
//! Document chunks are not employee-linked in the schema (documents are
//! folder-indexed); per-employee document grounding arrives with the brief
//! generator's context integration (T7), not here.

use std::collections::HashSet;

use crate::db::DbPool;

/// Fewest narrative items that can anchor any thread at all (T1 empirical
/// floor). Below this the brief is facts-only with an explicit note —
/// fewer/none beats filler (decision 7).
pub const THREAD_ANCHOR_MINIMUM: usize = 2;

/// Thread budget for a given narrative-item count: <2 → none (thin record),
/// 2–3 → one, 4+ → up to three (T1 build-time decision).
pub fn max_threads_for(narrative_items: usize) -> usize {
    match narrative_items {
        0..=1 => 0,
        2..=3 => 1,
        _ => 3,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundingKind {
    /// Structured record field — citable as a fact, not a thread anchor.
    RecordField,
    /// Cross-review career summary narrative.
    CareerSummary,
    /// One narrative field of one performance review.
    ReviewNarrative,
}

/// One citable unit of grounding context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundingItem {
    /// Stable citation id, `C1`.. in assembly order.
    pub citation_id: String,
    pub kind: GroundingKind,
    /// Human-readable source label, e.g. `Strengths (review dated 2025-03-22)`.
    pub label: String,
    pub content: String,
}

impl GroundingItem {
    /// Narrative items anchor threads; structured record fields do not
    /// (tenure/role facts still appear in the Facts section).
    pub fn is_narrative(&self) -> bool {
        self.kind != GroundingKind::RecordField
    }
}

/// Everything the brief generator needs about one employee's grounding
/// surface. `items` is the canonical citation set — nothing else may be cited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundingContext {
    pub employee_id: String,
    pub employee_name: String,
    pub job_title: Option<String>,
    pub department: Option<String>,
    pub items: Vec<GroundingItem>,
}

impl GroundingContext {
    /// The canonical set for citation validation: exactly the assembled items.
    pub fn canonical_ids(&self) -> HashSet<String> {
        self.items.iter().map(|i| i.citation_id.clone()).collect()
    }

    pub fn narrative_item_count(&self) -> usize {
        self.items.iter().filter(|i| i.is_narrative()).count()
    }

    /// Thread budget for this record (0 on a thin record).
    pub fn max_threads(&self) -> usize {
        max_threads_for(self.narrative_item_count())
    }

    /// True when the record can't anchor threads: facts-only brief + note.
    pub fn is_thin(&self) -> bool {
        self.narrative_item_count() < THREAD_ANCHOR_MINIMUM
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("employee lookup failed: {0}")]
    Employee(#[from] crate::employees::EmployeeError),
    #[error("review lookup failed: {0}")]
    Review(#[from] crate::performance_reviews::ReviewError),
}

struct ItemBuilder {
    items: Vec<GroundingItem>,
}

impl ItemBuilder {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn push(&mut self, kind: GroundingKind, label: &str, content: &str) {
        let content = content.trim();
        if content.is_empty() {
            return;
        }
        self.items.push(GroundingItem {
            citation_id: format!("C{}", self.items.len() + 1),
            kind,
            label: label.to_string(),
            content: content.to_string(),
        });
    }

    fn push_opt(&mut self, kind: GroundingKind, label: &str, content: Option<&str>) {
        if let Some(content) = content {
            self.push(kind, label, content);
        }
    }
}

/// Assemble the grounding context for one employee. Deterministic: record
/// fields in fixed order, then the career summary, then review narratives
/// (reviews newest-cycle-first, fields in schema order).
pub async fn assemble_grounding_context(
    pool: &DbPool,
    employee_id: &str,
) -> Result<GroundingContext, ContextError> {
    let employee = crate::employees::get_employee(pool, employee_id).await?;
    let mut b = ItemBuilder::new();

    // 1. Allowlisted record fields, fixed order.
    b.push(GroundingKind::RecordField, "Full name", &employee.full_name);
    b.push_opt(
        GroundingKind::RecordField,
        "Job title",
        employee.job_title.as_deref(),
    );
    b.push_opt(
        GroundingKind::RecordField,
        "Department",
        employee.department.as_deref(),
    );
    if let Some(manager_id) = employee.manager_id.as_deref() {
        // A dangling manager reference must not fail the brief — skip it.
        if let Ok(manager) = crate::employees::get_employee(pool, manager_id).await {
            b.push(GroundingKind::RecordField, "Manager", &manager.full_name);
        }
    }
    b.push_opt(
        GroundingKind::RecordField,
        "Hire date",
        employee.hire_date.as_deref(),
    );
    b.push_opt(
        GroundingKind::RecordField,
        "Work state",
        employee.work_state.as_deref(),
    );
    b.push(
        GroundingKind::RecordField,
        "Employment status",
        &employee.status,
    );

    // 2. Career summary (cross-review, when one has been built).
    if let Some(summary) = crate::highlights::get_summary_or_none(pool, employee_id).await {
        b.push_opt(
            GroundingKind::CareerSummary,
            "Career summary",
            summary.career_narrative.as_deref(),
        );
        if !summary.notable_accomplishments.is_empty() {
            b.push(
                GroundingKind::CareerSummary,
                "Notable accomplishments (summary)",
                &summary.notable_accomplishments.join("; "),
            );
        }
    }

    // 3. Review narratives, newest cycle first, fields in schema order.
    let reviews = crate::performance_reviews::get_reviews_for_employee(pool, employee_id).await?;
    for review in &reviews {
        let dated = review.review_date.as_deref().unwrap_or("undated");
        let fields: [(&str, Option<&str>); 6] = [
            ("Strengths", review.strengths.as_deref()),
            (
                "Areas for improvement",
                review.areas_for_improvement.as_deref(),
            ),
            ("Accomplishments", review.accomplishments.as_deref()),
            ("Goals for next period", review.goals_next_period.as_deref()),
            ("Manager comments", review.manager_comments.as_deref()),
            ("Self-assessment", review.self_assessment.as_deref()),
        ];
        for (name, content) in fields {
            b.push_opt(
                GroundingKind::ReviewNarrative,
                &format!("{name} (review dated {dated})"),
                content,
            );
        }
    }

    Ok(GroundingContext {
        employee_id: employee.id,
        employee_name: employee.full_name,
        job_title: employee.job_title,
        department: employee.department,
        items: b.items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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

    async fn insert_employee(pool: &DbPool, id: &str, name: &str, manager_id: Option<&str>) {
        sqlx::query(
            "INSERT INTO employees (id, email, full_name, department, job_title, manager_id,
                                    hire_date, work_state, status)
             VALUES (?, ?, ?, 'Engineering', 'Software Engineer', ?, '2023-06-27', 'California', 'active')",
        )
        .bind(id)
        .bind(format!("{id}@example.com"))
        .bind(name)
        .bind(manager_id)
        .execute(pool)
        .await
        .expect("insert employee");
    }

    async fn insert_bare_employee(pool: &DbPool, id: &str, name: &str) {
        sqlx::query("INSERT INTO employees (id, email, full_name) VALUES (?, ?, ?)")
            .bind(id)
            .bind(format!("{id}@example.com"))
            .bind(name)
            .execute(pool)
            .await
            .expect("insert bare employee");
    }

    async fn insert_cycle(pool: &DbPool, id: &str, start: &str) {
        sqlx::query(
            "INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date)
             VALUES (?, ?, 'annual', ?, ?)",
        )
        .bind(id)
        .bind(format!("Cycle {id}"))
        .bind(start)
        .bind(start)
        .execute(pool)
        .await
        .expect("insert cycle");
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_review(
        pool: &DbPool,
        id: &str,
        employee_id: &str,
        cycle_id: &str,
        strengths: Option<&str>,
        areas: Option<&str>,
        accomplishments: Option<&str>,
        manager_comments: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO performance_reviews (id, employee_id, review_cycle_id, strengths,
                 areas_for_improvement, accomplishments, manager_comments, review_date)
             VALUES (?, ?, ?, ?, ?, ?, ?, '2025-03-22')",
        )
        .bind(id)
        .bind(employee_id)
        .bind(cycle_id)
        .bind(strengths)
        .bind(areas)
        .bind(accomplishments)
        .bind(manager_comments)
        .execute(pool)
        .await
        .expect("insert review");
    }

    #[test]
    fn thread_budget_boundaries() {
        assert_eq!(max_threads_for(0), 0);
        assert_eq!(max_threads_for(1), 0);
        assert_eq!(max_threads_for(2), 1);
        assert_eq!(max_threads_for(3), 1);
        assert_eq!(max_threads_for(4), 3);
        assert_eq!(max_threads_for(12), 3);
    }

    #[tokio::test]
    async fn empty_record_is_thin_with_record_fields_only() {
        let pool = test_pool().await;
        insert_bare_employee(&pool, "emp-1", "Ada Example").await;

        let ctx = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("assemble");
        assert_eq!(ctx.narrative_item_count(), 0);
        assert!(ctx.is_thin());
        assert_eq!(ctx.max_threads(), 0);
        // Bare record still yields citable facts: name + default status.
        assert!(ctx
            .items
            .iter()
            .all(|i| i.kind == GroundingKind::RecordField));
        assert_eq!(ctx.items[0].label, "Full name");
        assert_eq!(ctx.items[0].citation_id, "C1");
    }

    #[tokio::test]
    async fn typical_record_assembles_in_deterministic_order() {
        let pool = test_pool().await;
        insert_employee(&pool, "mgr-1", "Grace Manager", None).await;
        insert_employee(&pool, "emp-1", "Ada Example", Some("mgr-1")).await;
        insert_cycle(&pool, "cy-2025", "2025-01-01").await;
        insert_cycle(&pool, "cy-2024", "2024-01-01").await;
        insert_review(
            &pool,
            "rev-2025",
            "emp-1",
            "cy-2025",
            Some("Reliable performer."),
            Some("Could document more."),
            Some("Led the gateway work."),
            Some("Valued team member."),
        )
        .await;
        insert_review(
            &pool,
            "rev-2024",
            "emp-1",
            "cy-2024",
            Some("Quick learner."),
            None,
            None,
            None,
        )
        .await;

        let ctx = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("assemble");

        // 7 record fields + 4 narrative fields (2025 review) + 1 (2024 review).
        assert_eq!(ctx.items.len(), 12);
        assert_eq!(ctx.narrative_item_count(), 5);
        assert!(!ctx.is_thin());
        assert_eq!(ctx.max_threads(), 3);

        // Record fields in fixed order, manager resolved to a name.
        let labels: Vec<&str> = ctx.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            &labels[..7],
            &[
                "Full name",
                "Job title",
                "Department",
                "Manager",
                "Hire date",
                "Work state",
                "Employment status"
            ]
        );
        assert_eq!(ctx.items[3].content, "Grace Manager");

        // Newest cycle's narratives come first; citation ids are sequential.
        assert_eq!(ctx.items[7].label, "Strengths (review dated 2025-03-22)");
        assert_eq!(ctx.items[7].content, "Reliable performer.");
        assert_eq!(ctx.items[11].content, "Quick learner.");
        for (idx, item) in ctx.items.iter().enumerate() {
            assert_eq!(item.citation_id, format!("C{}", idx + 1));
        }
    }

    #[tokio::test]
    async fn single_narrative_field_is_still_thin() {
        let pool = test_pool().await;
        insert_bare_employee(&pool, "emp-1", "Ada Example").await;
        insert_cycle(&pool, "cy-1", "2025-01-01").await;
        insert_review(
            &pool,
            "rev-1",
            "emp-1",
            "cy-1",
            Some("Shows promise."),
            None,
            None,
            None,
        )
        .await;

        let ctx = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("assemble");
        assert_eq!(ctx.narrative_item_count(), 1);
        assert!(ctx.is_thin());
        assert_eq!(ctx.max_threads(), 0);
    }

    #[tokio::test]
    async fn citation_ids_stable_across_two_assemblies() {
        let pool = test_pool().await;
        insert_employee(&pool, "emp-1", "Ada Example", None).await;
        insert_cycle(&pool, "cy-1", "2025-01-01").await;
        insert_review(
            &pool,
            "rev-1",
            "emp-1",
            "cy-1",
            Some("Reliable."),
            Some("Docs."),
            Some("Shipped."),
            Some("Solid."),
        )
        .await;

        let first = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("first");
        let second = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("second");
        assert_eq!(first, second, "same record must assemble identically");
    }

    #[tokio::test]
    async fn canonical_set_is_exactly_the_assembled_items() {
        let pool = test_pool().await;
        insert_employee(&pool, "emp-1", "Ada Example", None).await;

        let ctx = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("assemble");
        let canonical = ctx.canonical_ids();
        assert_eq!(canonical.len(), ctx.items.len(), "ids are unique");
        for item in &ctx.items {
            assert!(canonical.contains(&item.citation_id));
        }
    }

    #[tokio::test]
    async fn allowlist_excludes_demographic_and_termination_fields() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO employees (id, email, full_name, department, job_title, hire_date,
                                    work_state, status, date_of_birth, gender, ethnicity,
                                    termination_date, termination_reason)
             VALUES ('emp-1', 'e@example.com', 'Ada Example', 'Engineering', 'Engineer',
                     '2020-01-01', 'California', 'terminated', '1990-05-05', 'female',
                     'Asian', '2026-01-15', 'Position eliminated in restructure')",
        )
        .execute(&pool)
        .await
        .expect("insert employee with sensitive fields");

        let ctx = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("assemble");
        let all_text: String = ctx
            .items
            .iter()
            .map(|i| format!("{} {}", i.label, i.content))
            .collect::<Vec<_>>()
            .join(" ");
        for forbidden in ["1990-05-05", "female", "Asian", "2026-01-15", "restructure"] {
            assert!(
                !all_text.contains(forbidden),
                "allowlisted assembly leaked {forbidden:?}"
            );
        }
        // Status itself is allowlisted — being terminated is a record fact...
        assert!(all_text.contains("terminated"));
        // ...but the reason and date never surface.
    }

    #[tokio::test]
    async fn unknown_employee_errors() {
        let pool = test_pool().await;
        let err = assemble_grounding_context(&pool, "missing").await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn dangling_manager_reference_is_skipped_not_fatal() {
        let pool = test_pool().await;
        // FK enforcement would reject a dangling manager_id on insert, so
        // create a real manager, point at them, then delete the manager row
        // with FKs relaxed to simulate drifted data.
        insert_employee(&pool, "mgr-1", "Grace Manager", None).await;
        insert_employee(&pool, "emp-1", "Ada Example", Some("mgr-1")).await;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .expect("relax fks");
        sqlx::query("DELETE FROM employees WHERE id = 'mgr-1'")
            .execute(&pool)
            .await
            .expect("delete manager");

        let ctx = assemble_grounding_context(&pool, "emp-1")
            .await
            .expect("assemble");
        assert!(ctx.items.iter().all(|i| i.label != "Manager"));
    }
}
