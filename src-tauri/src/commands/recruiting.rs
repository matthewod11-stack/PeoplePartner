//! Tauri command bridge for the `recruiting` module.

use crate::db::Database;
use crate::keyring::{self, KeyringError};
use crate::recruiting::adapters::exa::{self, ExaError, ExaSearchResponse};
use crate::recruiting::cost::{CostModel, CostReport, CostTracker};
use crate::recruiting::identity::{
    arbiter::{IdentityArbiter, NoopArbiter},
    resolver::resolve,
    types::{ResolveStats, ResolvedCandidate},
};
use crate::recruiting::search::{
    enrich::{enrich_candidates, exa_enrichment_priority, should_run_exa_enrichment, ContentEnricher, EnrichConfig, EnrichError, EnrichmentOutcome, ExaContentEnricher},
    executor::DiscoveryStats,
    source::{CandidateSource, ExaCandidateSource, SearchError},
    DiscoveryPipeline,
};
use crate::recruiting::intake::schemas::SearchConfig;
use crate::recruiting::{self, RecruitingSearch, EXA_PROVIDER_ID};
use serde::Serialize;

/// Create a new recruiting search. Returns the generated row ID.
#[tauri::command]
pub(crate) async fn recruiting_create_search(
    state: tauri::State<'_, Database>,
    query: String,
    seed_employee_id: Option<String>,
) -> Result<String, String> {
    recruiting::create_search(&state.pool, &query, seed_employee_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// List all recruiting searches, newest first.
#[tauri::command]
pub(crate) async fn recruiting_list_searches(
    state: tauri::State<'_, Database>,
) -> Result<Vec<RecruitingSearch>, String> {
    recruiting::list_searches(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Discriminated error type for `recruiting_search_exa`. The frontend
/// pattern-matches on `kind`:
///   - `MissingKey`  → render the "Recruiting needs your Exa API key" banner.
///   - `InvalidKey`  → same banner; suffix that Exa rejected the stored key.
///   - `RateLimit`   → soft toast with the Exa-supplied message.
///   - `Network`     → soft toast ("couldn't reach Exa").
///   - `ExaApi`      → surface status + body inline (debugging-friendly).
///   - `Internal`    → unexpected path: Keychain read failure, response
///     parse failure, etc.
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum RecruitingSearchError {
    MissingKey,
    InvalidKey,
    RateLimit { message: String },
    Network { message: String },
    ExaApi { status: u16, body: String },
    Internal { message: String },
}

impl From<ExaError> for RecruitingSearchError {
    fn from(err: ExaError) -> Self {
        match err {
            ExaError::Network(e) => RecruitingSearchError::Network {
                message: e.to_string(),
            },
            ExaError::InvalidKey => RecruitingSearchError::InvalidKey,
            ExaError::RateLimit { message } => RecruitingSearchError::RateLimit { message },
            ExaError::Api { status, body } => RecruitingSearchError::ExaApi { status, body },
            ExaError::InvalidResponse(message) => RecruitingSearchError::Internal { message },
        }
    }
}

// ============================================================================
// Exa API key management — Recruiting-namespaced wrappers
// ============================================================================
//
// These deliberately bypass `commands::api_keys::*_provider_api_key` because
// those commands gate on the LLM-provider registry (`providers::get_provider`)
// — Exa is a data source, not an LLM, so it isn't (and shouldn't be) registered
// there. The gate is a real safety net for the LLM-key path (catches typos
// like "anthropic-key"), so punching a hole in it would be wrong; better to
// keep the LLM and data-source key surfaces independent.
//
// Storage is shared with the LLM keys (same Keychain service, account
// `exa_api_key` per `keychain_account_for_provider("exa")`), so a key written
// here is readable by `recruiting_search_exa` and vice-versa.

/// Check whether an Exa API key is stored in the Keychain.
#[tauri::command]
pub(crate) fn recruiting_has_exa_key() -> Result<bool, String> {
    Ok(keyring::has_provider_api_key(EXA_PROVIDER_ID))
}

/// Store the Exa API key in the Keychain. Format validation lives client-side
/// (UUID regex in `ExaKeyInput.tsx`); the Rust side is intentionally permissive
/// to keep the recruiting command surface small. If Exa changes its key format,
/// nothing here needs to change.
#[tauri::command]
pub(crate) fn recruiting_store_exa_key(api_key: String) -> Result<(), String> {
    keyring::store_provider_api_key(EXA_PROVIDER_ID, &api_key).map_err(|e| e.to_string())
}

/// Delete the Exa API key from the Keychain. Idempotent — returns Ok even
/// when no key was stored (matches `keyring::delete_provider_api_key`).
#[tauri::command]
pub(crate) fn recruiting_delete_exa_key() -> Result<(), String> {
    keyring::delete_provider_api_key(EXA_PROVIDER_ID).map_err(|e| e.to_string())
}

/// Execute an Exa search using the user's BYOK key from Keychain.
///
/// Returns `MissingKey` when no key is stored — the frontend uses this as
/// the signal to render the "add your Exa key in Settings" banner instead
/// of treating it as an error.
#[tauri::command]
pub(crate) async fn recruiting_search_exa(
    query: String,
) -> Result<ExaSearchResponse, RecruitingSearchError> {
    let api_key = match keyring::get_provider_api_key(EXA_PROVIDER_ID) {
        Ok(key) => key,
        Err(KeyringError::NotFound) => return Err(RecruitingSearchError::MissingKey),
        Err(other) => {
            return Err(RecruitingSearchError::Internal {
                message: other.to_string(),
            })
        }
    };

    exa::search(&query, &api_key).await.map_err(Into::into)
}

// ============================================================================
// Intake conversation commands (FHR-86 S4.2) — state-passing boundary
// ============================================================================

use crate::recruiting::intake::deps::{IntakeDeps, IntakeError};
use crate::recruiting::intake::prod::production_intake_deps;
use crate::recruiting::intake::runner::{
    create_intake_engine, extract_intake_result, restore_intake_engine,
};
use crate::recruiting::intake::schemas::TalentProfile;
use crate::recruiting::intake::seed::{build_seeded_context, extract_seed};
use crate::settings;

/// One turn of the intake conversation. `state` is the opaque serialized
/// `ConversationState` the frontend round-trips; `prompt` is `None` when done.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeTurn {
    pub prompt: Option<String>,
    pub state: String,
    pub done: bool,
}

/// The extracted intake result: the runnable `SearchConfig` plus artifacts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeResultDto {
    pub search_config: SearchConfig,
    pub talent_profile: TalentProfile,
    pub similarity_seeds: Vec<String>,
}

/// Discriminated error for the intake commands. The frontend matches on `kind`.
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum IntakeCommandError {
    MissingLlmKey,
    MissingExaKey,
    Provider { message: String },
    Research { message: String },
    Internal { message: String },
}

impl From<IntakeError> for IntakeCommandError {
    fn from(e: IntakeError) -> Self {
        match e {
            IntakeError::Provider(m) => IntakeCommandError::Provider { message: m },
            IntakeError::Research(m) => IntakeCommandError::Research { message: m },
            other => IntakeCommandError::Internal { message: other.to_string() },
        }
    }
}

/// Assemble production intake deps from settings (LLM provider + model) and the
/// Keychain (Exa key). Returns typed missing-key errors so the UI can route the
/// user to Settings. This is the one piece of new wireable logic.
async fn deps_from_env(pool: &crate::db::DbPool) -> Result<IntakeDeps, IntakeCommandError> {
    let provider_id = settings::get_setting(pool, "active_provider")
        .await
        .map_err(|e| IntakeCommandError::Internal { message: e.to_string() })?
        .unwrap_or_else(|| "anthropic".to_string());

    if !keyring::has_provider_api_key(&provider_id) {
        return Err(IntakeCommandError::MissingLlmKey);
    }

    let model_key = format!("active_model_{}", provider_id);
    let model_id = settings::get_setting(pool, &model_key)
        .await
        .map_err(|e| IntakeCommandError::Internal { message: e.to_string() })?;

    let exa_key = match keyring::get_provider_api_key(EXA_PROVIDER_ID) {
        Ok(k) => k,
        Err(KeyringError::NotFound) => return Err(IntakeCommandError::MissingExaKey),
        Err(other) => return Err(IntakeCommandError::Internal { message: other.to_string() }),
    };

    Ok(production_intake_deps(&provider_id, model_id, exa_key))
}

/// Seed input for `recruiting_intake_start_from_seed`. Exactly one of
/// `file_path` (read + parsed via the document pipeline) or `seed_text` (pasted).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeSeedInput {
    pub file_path: Option<String>,
    pub seed_text: Option<String>,
}

/// Resolve the seed input to raw text. Reuses the document pipeline's
/// `parse_file` (md/txt/pdf/docx) for files; pasted text passes through.
fn resolve_seed_text(input: &IntakeSeedInput) -> Result<String, IntakeCommandError> {
    match (&input.file_path, &input.seed_text) {
        (Some(path), None) => {
            let chunks = crate::documents::parse_file(std::path::Path::new(path))
                .map_err(|e| IntakeCommandError::Internal { message: format!("Couldn't read {path}: {e}") })?;
            let text = chunks.iter().map(|c| c.content.as_str()).collect::<Vec<_>>().join("\n\n");
            if text.trim().is_empty() {
                return Err(IntakeCommandError::Internal { message: format!("{path} had no readable text") });
            }
            Ok(text)
        }
        (None, Some(t)) if !t.trim().is_empty() => Ok(t.clone()),
        (Some(_), Some(_)) => Err(IntakeCommandError::Internal {
            message: "provide only one of filePath or seedText, not both".into(),
        }),
        _ => Err(IntakeCommandError::Internal {
            message: "provide exactly one non-empty of filePath or seedText".into(),
        }),
    }
}

/// Start an intake conversation seeded from a JD/transcript. Parses + redacts
/// the input, runs one structured extraction, pre-seeds the context, and returns
/// the first prompt (which lands on a confirm turn for whatever was extracted).
#[tauri::command]
pub(crate) async fn recruiting_intake_start_from_seed(
    state: tauri::State<'_, Database>,
    seed: IntakeSeedInput,
) -> Result<IntakeTurn, IntakeCommandError> {
    let deps = deps_from_env(&state.pool).await?;          // preflight LLM/Exa keys
    let text = resolve_seed_text(&seed)?;
    let redacted = crate::pii::scan_and_redact(&text).redacted_text;  // financial-scope (names/emails = FHR-90)
    let extraction = extract_seed(&redacted, &deps).await?;
    let ctx = build_seeded_context(extraction, redacted, &deps.clock.now_iso8601());
    let mut engine = create_intake_engine(&deps, Some(ctx));
    let prompt = engine.get_prompt().await;
    Ok(IntakeTurn { done: engine.is_done(), state: engine.serialize_state(), prompt })
}

/// Start an intake conversation. Returns the first prompt + opaque state blob.
#[tauri::command]
pub(crate) async fn recruiting_intake_start(
    state: tauri::State<'_, Database>,
) -> Result<IntakeTurn, IntakeCommandError> {
    // Fail fast on missing LLM/Exa keys before creating any conversation state.
    let deps = deps_from_env(&state.pool).await?;
    let mut engine = create_intake_engine(&deps, None);
    let prompt = engine.get_prompt().await;
    Ok(IntakeTurn { done: engine.is_done(), state: engine.serialize_state(), prompt })
}

/// Submit one user response. Restores the engine from `conversation_state`,
/// steps once, returns the next prompt + new state. On a `submit_response`
/// error the frontend keeps its prior (unchanged) state blob and can safely
/// retry the same turn — the failed step never re-serializes. (A malformed
/// `conversation_state` instead fails at restore and maps to `Internal`,
/// which is not retryable with the same blob.)
#[tauri::command]
pub(crate) async fn recruiting_intake_step(
    state: tauri::State<'_, Database>,
    conversation_state: String,
    response: String,
) -> Result<IntakeTurn, IntakeCommandError> {
    let deps = deps_from_env(&state.pool).await?;
    let mut engine = restore_intake_engine(&conversation_state)?;
    engine.submit_response(&response, &deps).await?;
    let prompt = engine.get_prompt().await;
    Ok(IntakeTurn { done: engine.is_done(), state: engine.serialize_state(), prompt })
}

/// Extract the final result (runnable `SearchConfig` + artifacts) once done.
#[tauri::command]
pub(crate) async fn recruiting_intake_extract(
    state: tauri::State<'_, Database>,
    conversation_state: String,
) -> Result<IntakeResultDto, IntakeCommandError> {
    let deps = deps_from_env(&state.pool).await?;
    let engine = restore_intake_engine(&conversation_state)?;
    let result = extract_intake_result(engine.context(), &deps).await?;
    Ok(IntakeResultDto {
        search_config: result.search_config,
        talent_profile: result.talent_profile,
        similarity_seeds: result.similarity_seeds,
    })
}

// ============================================================================
// recruiting_run_search — full pipeline command (FHR-73 S1.1 Task 7)
// ============================================================================

/// Full pipeline result returned by `recruiting_run_search`.
///
/// All fields are camelCase for the TypeScript DTO.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSearchResult {
    /// Final resolved candidates (in-memory only — ruled decision #1).
    pub candidates: Vec<ResolvedCandidate>,
    /// Discovery funnel counters from `DiscoveryPipeline::run`.
    pub discovery_stats: DiscoveryStats,
    /// Identity-resolution aggregate counts.
    pub resolve_stats: ResolveStats,
    /// Enrichment outcome (enriched / skipped / failures / stopped_early).
    pub enrichment_outcome: EnrichmentOutcome,
    /// Accumulated cost report for the whole run (discovery + enrichment share
    /// one budget — ruled: one CostTracker across both phases).
    pub cost_report: CostReport,
}

// ---------------------------------------------------------------------------
// From<SearchError> → RecruitingSearchError
// ---------------------------------------------------------------------------

impl From<SearchError> for RecruitingSearchError {
    fn from(err: SearchError) -> Self {
        match err {
            SearchError::Source(exa_err) => exa_err.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// From<EnrichError> → RecruitingSearchError
// ---------------------------------------------------------------------------

impl From<EnrichError> for RecruitingSearchError {
    fn from(err: EnrichError) -> Self {
        match err {
            EnrichError::Exa(exa_err) => exa_err.into(),
            EnrichError::Empty => RecruitingSearchError::Internal {
                message: "enrichment called with empty URL batch".into(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// run_search_inner — pure (injectable seams for tests)
// ---------------------------------------------------------------------------

/// Core pipeline: discovery → identity resolution → optional enrichment.
///
/// Generic over `S: CandidateSource` so tests can inject `FakeCandidateSource`
/// (a `#[cfg(test)]` type) without needing a running Exa server.
/// The enricher and arbiter are also injectable for the same reason.
///
/// One `CostTracker` is created here and threaded through both discovery (as
/// `&mut dyn CostGate`) and enrichment (as `&mut CostTracker`), so
/// `max_cost_usd` is a shared budget across the whole run.
pub(crate) async fn run_search_inner<S: CandidateSource>(
    cfg: &SearchConfig,
    source: S,
    enricher: &dyn ContentEnricher,
    arbiter: &dyn IdentityArbiter,
) -> Result<RunSearchResult, RecruitingSearchError> {
    // --- Shared budget tracker ---
    let mut cost = CostTracker::new(CostModel::default(), cfg.max_cost_usd);

    // --- Phase 1: discovery ---
    let (raw, discovery_stats) =
        DiscoveryPipeline::run(source, cfg, &mut cost)
            .await
            .map_err(RecruitingSearchError::from)?;

    // --- Phase 2: identity resolution ---
    let resolved = resolve(raw, arbiter)
        .await
        .map_err(|e| RecruitingSearchError::Internal { message: e.to_string() })?;

    let mut candidates = resolved.candidates;
    let resolve_stats = resolved.stats;

    // --- Phase 3: optional enrichment ---
    let exa_prio = exa_enrichment_priority(cfg);
    let enrichment_outcome = if should_run_exa_enrichment(exa_prio, &candidates) {
        enrich_candidates(
            &mut candidates,
            enricher,
            &mut cost,
            &EnrichConfig::default(),
        )
        .await
    } else {
        EnrichmentOutcome::default()
    };

    // stopped_early = either phase stopped the run early.
    let stopped_early = discovery_stats.stopped_early || enrichment_outcome.stopped_early;
    let stop_reason = if stopped_early {
        Some("budget exhausted".to_owned())
    } else {
        None
    };
    let cost_report = cost.report(stopped_early, stop_reason);

    Ok(RunSearchResult {
        candidates,
        discovery_stats,
        resolve_stats,
        enrichment_outcome,
        cost_report,
    })
}

/// Execute the full Sourcerer discovery pipeline and return the in-memory result.
///
/// Fetches the Exa key from Keychain (same as `recruiting_search_exa`), builds
/// production source + enricher, and calls `run_search_inner`.
#[tauri::command]
pub(crate) async fn recruiting_run_search(
    search_config: SearchConfig,
) -> Result<RunSearchResult, RecruitingSearchError> {
    let api_key = match keyring::get_provider_api_key(EXA_PROVIDER_ID) {
        Ok(key) => key,
        Err(KeyringError::NotFound) => return Err(RecruitingSearchError::MissingKey),
        Err(other) => {
            return Err(RecruitingSearchError::Internal {
                message: other.to_string(),
            })
        }
    };

    // source takes the clone; enricher moves the original — both own a copy.
    let source = ExaCandidateSource::new(api_key.clone());
    let enricher = ExaContentEnricher { exa_api_key: api_key };
    let arbiter = NoopArbiter;

    run_search_inner(&search_config, source, &enricher, &arbiter).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::deps::IntakeError;

    #[test]
    fn intake_error_maps_to_command_error_kinds() {
        let p: IntakeCommandError = IntakeError::Provider("boom".into()).into();
        assert!(matches!(p, IntakeCommandError::Provider { .. }));
        let r: IntakeCommandError = IntakeError::Research("net".into()).into();
        assert!(matches!(r, IntakeCommandError::Research { .. }));
        let i: IntakeCommandError = IntakeError::SubmitAfterDone.into();
        assert!(matches!(i, IntakeCommandError::Internal { .. }));
    }

    #[test]
    fn intake_turn_serializes_camel_case() {
        let t = IntakeTurn { prompt: Some("hi".into()), state: "{}".into(), done: false };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["prompt"], "hi");
        assert_eq!(v["done"], false);
        assert_eq!(v["state"], "{}");
    }

    #[test]
    fn resolve_seed_text_accepts_nonempty_paste() {
        let input = IntakeSeedInput { file_path: None, seed_text: Some("a JD".into()) };
        assert_eq!(resolve_seed_text(&input).unwrap(), "a JD");
    }

    #[test]
    fn resolve_seed_text_rejects_both_empty() {
        let input = IntakeSeedInput { file_path: None, seed_text: None };
        assert!(matches!(resolve_seed_text(&input), Err(IntakeCommandError::Internal { .. })));
    }

    #[test]
    fn resolve_seed_text_rejects_both_set() {
        let input = IntakeSeedInput { file_path: Some("/x".into()), seed_text: Some("y".into()) };
        let Err(IntakeCommandError::Internal { message }) = resolve_seed_text(&input) else { panic!("expected Internal error") };
        assert!(message.contains("both"), "both-set error should mention both: {message}");
    }

    #[test]
    fn resolve_seed_text_rejects_blank_paste() {
        let input = IntakeSeedInput { file_path: None, seed_text: Some("   ".into()) };
        assert!(matches!(resolve_seed_text(&input), Err(IntakeCommandError::Internal { .. })));
    }

    // -------------------------------------------------------------------------
    // run_search_inner tests (Task 7)
    // -------------------------------------------------------------------------

    use crate::recruiting::adapters::exa::ExaHit;
    use crate::recruiting::identity::arbiter::NoopArbiter as TestNoopArbiter;
    use crate::recruiting::intake::schemas::{SearchQuery, SearchQueryTier, TierThresholds};
    use crate::recruiting::search::enrich::FakeEnricher;
    use crate::recruiting::search::source::FakeCandidateSource;

    fn sample_search_config() -> SearchConfig {
        SearchConfig {
            role_name: "test-role".into(),
            tiers: vec![SearchQueryTier {
                priority: 1,
                queries: vec![SearchQuery {
                    text: "senior rust engineer".into(),
                    target_companies: None,
                    include_domains: None,
                    exclude_domains: None,
                    max_results: None,
                }],
            }],
            scoring_weights: Default::default(),
            tier_thresholds: TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![],
            anti_filters: vec![],
            similarity_seeds: vec![],
            max_candidates: 100,
            max_cost_usd: 100.0,
            created_at: "2026-06-03T00:00:00Z".into(),
            version: 1,
        }
    }

    fn hit(url: &str) -> ExaHit {
        ExaHit {
            id: url.into(),
            url: url.into(),
            title: None,
            score: Some(0.8),
            published_date: None,
            author: None,
            text: None,
            highlights: None,
            summary: None,
        }
    }

    #[tokio::test]
    async fn run_search_inner_end_to_end_with_fakes() {
        let cfg = sample_search_config();

        let mut source = FakeCandidateSource::empty();
        source.search_results.insert(
            "senior rust engineer".into(),
            vec![
                hit("https://linkedin.com/in/alice-smith"),
                hit("https://github.com/bob"),
            ],
        );

        let enricher = FakeEnricher::echo();
        let res = run_search_inner(&cfg, source, &enricher, &TestNoopArbiter)
            .await
            .unwrap();

        assert!(
            !res.candidates.is_empty(),
            "pipeline must return at least one candidate"
        );
        assert_eq!(
            res.discovery_stats.query_failures, 0,
            "no query failures with fake source"
        );
        // resolve_stats must reflect the resolved candidate list, not be left zeroed.
        assert_eq!(
            res.resolve_stats.output_count,
            res.candidates.len(),
            "resolve_stats.output_count must match the resolved candidate count"
        );
        // The fake echo enricher fills page_text for every URL-bearing candidate,
        // so at least one must be enriched (proves the enrichment field is populated).
        assert!(
            res.enrichment_outcome.enriched > 0,
            "echo enricher must enrich at least one candidate; got {}",
            res.enrichment_outcome.enriched
        );
        // The CostTracker records calls — enricher called for each candidate → call_count >= 1.
        assert!(
            res.cost_report.snapshot.call_count >= 1,
            "cost tracker must record at least one call; got {}",
            res.cost_report.snapshot.call_count
        );
    }

    #[test]
    fn search_error_maps_to_invalid_key() {
        use crate::recruiting::adapters::exa::ExaError;
        let mapped: RecruitingSearchError = SearchError::Source(ExaError::InvalidKey).into();
        assert!(
            matches!(mapped, RecruitingSearchError::InvalidKey),
            "SearchError::Source(InvalidKey) must map to RecruitingSearchError::InvalidKey"
        );
    }

    #[test]
    fn search_error_rate_limit_maps_correctly() {
        use crate::recruiting::adapters::exa::ExaError;
        let mapped: RecruitingSearchError =
            SearchError::Source(ExaError::RateLimit { message: "slow down".into() }).into();
        assert!(matches!(mapped, RecruitingSearchError::RateLimit { .. }));
    }

    #[test]
    fn enrich_error_maps_to_invalid_key() {
        use crate::recruiting::adapters::exa::ExaError;
        let mapped: RecruitingSearchError = EnrichError::Exa(ExaError::InvalidKey).into();
        assert!(
            matches!(mapped, RecruitingSearchError::InvalidKey),
            "EnrichError::Exa(InvalidKey) must map to RecruitingSearchError::InvalidKey"
        );
    }

    #[test]
    fn enrich_error_empty_maps_to_internal() {
        let mapped: RecruitingSearchError = EnrichError::Empty.into();
        assert!(matches!(mapped, RecruitingSearchError::Internal { .. }));
    }

    #[test]
    fn run_search_result_serializes_camel_case() {
        use crate::recruiting::cost::{CostModel, CostTracker};
        let tracker = CostTracker::new(CostModel::default(), 10.0);
        let result = RunSearchResult {
            candidates: vec![],
            discovery_stats: DiscoveryStats::default(),
            resolve_stats: ResolveStats::default(),
            enrichment_outcome: EnrichmentOutcome::default(),
            cost_report: tracker.report(false, None),
        };
        let v = serde_json::to_value(&result).unwrap();
        assert!(v.get("candidates").is_some(), "candidates field must exist");
        assert!(v.get("discoveryStats").is_some(), "discoveryStats must be camelCase");
        assert!(v.get("resolveStats").is_some(), "resolveStats must be camelCase");
        assert!(v.get("enrichmentOutcome").is_some(), "enrichmentOutcome must be camelCase");
        assert!(v.get("costReport").is_some(), "costReport must be camelCase");
    }
}
