//! FHR-80 (Recruit S2.4) — differential eval oracle. Diffs the Rust scorer
//! against the vendored TS reference (`fixtures/golden-ts-reference.json`,
//! captured from Sourcerer's `calculateScore`/`assignTier`) over the 15-candidate
//! golden set + adversarial vectors. The MS2 gate: port fidelity proven.

use serde::Deserialize;

use super::schemas::ExtractedSignals;
use super::score::{assign_tier, score_candidate, ScoringConfig};

const TOL: f64 = 1e-6;

#[derive(Debug, Deserialize)]
struct Fixture {
    meta: Meta,
    golden: Vec<GoldenEntry>,
    adversarial: Vec<AdvEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    #[allow(dead_code)]
    captured_at: String,
    #[allow(dead_code)]
    sourcerer_commit: String,
    config: ScoringConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenEntry {
    id: String,
    name: String,
    evidence: Vec<super::evidence::EvidenceItem>,
    expected_signals: ExtractedSignals,
    expected_tier: u8,
    ts: TsScore,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsScore {
    total: f64,
    breakdown: Vec<TsComp>,
    tier: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsComp {
    dimension: String,
    raw: f64,
    weight: f64,
    weighted: f64,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdvEntry {
    name: String,
    signals: ExtractedSignals,
    config: ScoringConfig,
    ts: TsTotalTier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsTotalTier {
    total: f64,
    tier: u8,
}

// ── Real-LLM narrative grounding (the "no phantom IDs" acceptance check).
// #[ignore]d: hits the live LLM, needs an Anthropic key in the keyring, costs
// tokens, and is non-deterministic — NOT a CI gate. Run manually:
//   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored narrative_grounding
use std::sync::Arc;

use super::narrative::{generate_narrative, scan_phantom_citations, NarrativeOptions};
use crate::recruiting::identity::types::{PersonIdentity, ResolvedCandidate};
use crate::recruiting::intake::deps::IntakeProvider;
use crate::recruiting::intake::prod::AppIntakeProvider;
use crate::recruiting::intake::schemas::{
    CompanyIntel, CompetitorMap, CompositeProfile, RoleParameters, TalentProfile,
};

fn load_fixture() -> Fixture {
    let raw = include_str!("fixtures/golden-ts-reference.json");
    serde_json::from_str(raw).expect("golden-ts-reference.json parses")
}

#[test]
fn golden_set_matches_ts_reference() {
    let fx = load_fixture();
    assert_eq!(fx.golden.len(), 15, "golden set: expected 15, got {}", fx.golden.len());
    for g in &fx.golden {
        let out = score_candidate(&g.expected_signals, &fx.meta.config);

        // Total within tight tolerance.
        assert!(
            (out.total - g.ts.total).abs() < TOL,
            "{}: total {} vs ts {}",
            g.id, out.total, g.ts.total
        );

        // Per-component diff (catches a raw↔weight swap that nets the same total).
        assert_eq!(out.breakdown.len(), g.ts.breakdown.len(), "{}: breakdown len", g.id);
        for (rust, ts) in out.breakdown.iter().zip(&g.ts.breakdown) {
            assert_eq!(rust.dimension, ts.dimension, "{}: dimension order", g.id);
            assert!((rust.raw - ts.raw).abs() < TOL, "{} {}: raw {} vs {}", g.id, ts.dimension, rust.raw, ts.raw);
            assert!((rust.weight - ts.weight).abs() < TOL, "{} {}: weight {} vs {}", g.id, ts.dimension, rust.weight, ts.weight);
            assert!((rust.weighted - ts.weighted).abs() < TOL, "{} {}: weighted {} vs {}", g.id, ts.dimension, rust.weighted, ts.weighted);
            assert!((rust.confidence - ts.confidence).abs() < TOL, "{} {}: confidence {} vs {}", g.id, ts.dimension, rust.confidence, ts.confidence);
        }

        // Tier: exact match vs TS AND vs the human-authored expectedTier.
        let tier = u8::from(assign_tier(out.total, &fx.meta.config.tier_thresholds));
        assert_eq!(tier, g.ts.tier, "{}: tier {} vs ts {}", g.id, tier, g.ts.tier);
        assert_eq!(tier, g.expected_tier, "{}: tier {} vs expectedTier {}", g.id, tier, g.expected_tier);

        // Evidence present (used by the narrative grounding test in Task 5).
        assert!(!g.evidence.is_empty(), "{}: evidence", g.id);
    }
}

#[test]
fn adversarial_vectors_match_ts_reference() {
    let fx = load_fixture();
    assert!(
        fx.adversarial.len() >= 11,
        "expected the full adversarial set, got {}",
        fx.adversarial.len()
    );
    // Names that MUST be present — proves the paths the golden set never exercises
    // (penalties, per-dimension confidence gating, clamping, exact tier boundaries) are covered.
    let names: std::collections::HashSet<&str> =
        fx.adversarial.iter().map(|a| a.name.as_str()).collect();
    for required in [
        "redflag-low", "redflag-medium", "redflag-high", "redflag-mixed-sum",
        "confidence-zero", "confidence-partial", "confidence-mixed",
        "clamp-low", "clamp-high", "boundary-tier1", "boundary-tier2",
    ] {
        assert!(names.contains(required), "missing adversarial vector: {required}");
    }

    for a in &fx.adversarial {
        let out = score_candidate(&a.signals, &a.config);
        assert!(
            (out.total - a.ts.total).abs() < TOL,
            "{}: total {} vs ts {}",
            a.name, out.total, a.ts.total
        );
        let tier = u8::from(assign_tier(out.total, &a.config.tier_thresholds));
        assert_eq!(tier, a.ts.tier, "{}: tier {} vs ts {}", a.name, tier, a.ts.tier);
    }
}

fn eval_talent_profile() -> TalentProfile {
    // Mirrors the golden set's role; defaults for the rest — the grounding check
    // turns on evidence citations, not profile fidelity, and a sparse profile is
    // a stricter wander test for the model.
    TalentProfile {
        role: RoleParameters::default(),
        company: CompanyIntel::default(),
        success_patterns: CompositeProfile::default(),
        anti_patterns: vec![],
        competitor_map: CompetitorMap::default(),
        created_at: "2026-05-01T00:00:00.000Z".into(),
    }
}

fn resolved_from(id: &str, name: &str) -> ResolvedCandidate {
    ResolvedCandidate {
        id: id.into(),
        identity: PersonIdentity {
            canonical_id: id.into(),
            observed_identifiers: vec![],
            merged_from: None,
            merge_confidence: 1.0,
            low_confidence_merge: false,
        },
        name: name.into(),
        sources: std::collections::BTreeMap::new(),
        page_text: None,
    }
}

#[tokio::test]
#[ignore = "hits the real LLM; needs anthropic key in keyring; run manually for FHR-80 acceptance"]
async fn narrative_grounding_cites_only_real_evidence() {
    if !crate::keyring::has_provider_api_key("anthropic") {
        eprintln!("SKIP narrative_grounding: no anthropic key in keyring");
        return;
    }
    let fx = load_fixture();
    let provider: Arc<dyn IntakeProvider> = Arc::new(AppIntakeProvider::new("anthropic", None));
    let profile = eval_talent_profile();

    let mut failures: Vec<String> = Vec::new();
    for g in &fx.golden {
        let candidate = resolved_from(&g.id, &g.name);
        let real_ids: Vec<String> = g.evidence.iter().map(|e| e.id.clone()).collect();
        let score = score_candidate(&g.expected_signals, &fx.meta.config);
        let result = generate_narrative(
            &candidate,
            &profile,
            &g.expected_signals,
            &score,
            &g.evidence,
            provider.clone(),
            &NarrativeOptions::default(),
        )
        .await
        .unwrap_or_else(|e| panic!("{}: narrative generation failed: {e:?}", g.id));

        let phantoms = scan_phantom_citations(&result.narrative, &real_ids);
        if !phantoms.is_empty() {
            failures.push(format!(
                "{}: phantom citations {:?}\n  narrative: {}",
                g.id, phantoms, result.narrative
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "phantom-citation failures:\n{}",
        failures.join("\n")
    );
}
