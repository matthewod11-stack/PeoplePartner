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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdvEntry {
    name: String,
    signals: ExtractedSignals,
    config: ScoringConfig,
    ts: TsTotalTier,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsTotalTier {
    total: f64,
    tier: u8,
}

fn load_fixture() -> Fixture {
    let raw = include_str!("fixtures/golden-ts-reference.json");
    serde_json::from_str(raw).expect("golden-ts-reference.json parses")
}

#[test]
fn golden_set_matches_ts_reference() {
    let fx = load_fixture();
    assert_eq!(fx.golden.len(), 15, "golden set must be 15 candidates");
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
            assert!((rust.confidence - ts.confidence).abs() < TOL, "{} {}: confidence", g.id, ts.dimension);
        }

        // Tier: exact match vs TS AND vs the human-authored expectedTier.
        let tier = u8::from(assign_tier(out.total, &fx.meta.config.tier_thresholds));
        assert_eq!(tier, g.ts.tier, "{}: tier {} vs ts {}", g.id, tier, g.ts.tier);
        assert_eq!(tier, g.expected_tier, "{}: tier {} vs expectedTier {}", g.id, tier, g.expected_tier);

        // Evidence present (used by the narrative grounding test in Task 5).
        assert!(!g.evidence.is_empty(), "{}: evidence", g.id);
    }
}
