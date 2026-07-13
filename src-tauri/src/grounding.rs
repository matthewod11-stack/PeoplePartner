//! Shared citation-validation core (FHR-105, People Map decision 5).
//!
//! Validates the citation IDs asserted by generated content against the
//! canonical set of grounding items actually supplied at context-assembly
//! time, splitting them into valid and phantom. Extracted from the recruiting
//! grounding validator (FHR-78); the H-9 hallucination *score penalty* and
//! confidence adjustment deliberately stay recruiting-side — briefs have no
//! scores, so this module knows nothing about them. Pure: no LLM, no I/O.
//!
//! Consumers: `recruiting::scoring::grounding` (layers the H-9 penalty on
//! top) and People Map's prep-brief validator (FHR-108).

use std::collections::HashSet;

/// Outcome of validating one ordered list of cited IDs against the canonical set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitCitations {
    /// IDs present in the canonical set — source order and duplicates preserved.
    pub valid_ids: Vec<String>,
    /// IDs absent from the canonical set (phantoms) — source order and duplicates preserved.
    pub phantom_ids: Vec<String>,
}

/// Split `cited` into IDs that exist in `canonical` and phantoms that don't.
/// Order and duplicates are preserved on both sides: the recruiting
/// differential oracle (FHR-80) diffs violation order against the TS golden
/// set, and duplicate valid citations are legitimate repeat references.
pub fn split_citations(cited: &[String], canonical: &HashSet<String>) -> SplitCitations {
    let mut valid_ids = Vec::with_capacity(cited.len());
    let mut phantom_ids = Vec::new();
    for id in cited {
        if canonical.contains(id) {
            valid_ids.push(id.clone());
        } else {
            phantom_ids.push(id.clone());
        }
    }
    SplitCitations {
        valid_ids,
        phantom_ids,
    }
}

/// Generated content that asserts citations against a canonical grounding set.
pub trait CitationCarrying {
    /// Every citation ID the content asserts, in stable source order.
    fn cited_ids(&self) -> Vec<String>;
}

/// All phantom citations asserted by `content`, in stable source order.
pub fn phantom_citations<T: CitationCarrying + ?Sized>(
    content: &T,
    canonical: &HashSet<String>,
) -> Vec<String> {
    split_citations(&content.cited_ids(), canonical).phantom_ids
}

/// True when `content` cites nothing outside the canonical set — the
/// "0 phantom citations" acceptance shape shared by recruiting narratives
/// and prep briefs.
pub fn is_fully_grounded<T: CitationCarrying + ?Sized>(
    content: &T,
    canonical: &HashSet<String>,
) -> bool {
    phantom_citations(content, canonical).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }
    fn cited(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn splits_valid_from_phantom_preserving_order() {
        let r = split_citations(&cited(&["a", "fake-1", "b", "fake-2"]), &canon(&["a", "b"]));
        assert_eq!(r.valid_ids, cited(&["a", "b"]));
        assert_eq!(r.phantom_ids, cited(&["fake-1", "fake-2"]));
    }

    #[test]
    fn preserves_duplicates_on_both_sides() {
        let r = split_citations(&cited(&["a", "a", "fake", "fake"]), &canon(&["a"]));
        assert_eq!(r.valid_ids, cited(&["a", "a"]));
        assert_eq!(r.phantom_ids, cited(&["fake", "fake"]));
    }

    #[test]
    fn empty_canonical_set_makes_everything_phantom() {
        let r = split_citations(&cited(&["a"]), &canon(&[]));
        assert!(r.valid_ids.is_empty());
        assert_eq!(r.phantom_ids, cited(&["a"]));
    }

    #[test]
    fn empty_cited_list_is_trivially_clean() {
        let r = split_citations(&[], &canon(&["a"]));
        assert!(r.valid_ids.is_empty());
        assert!(r.phantom_ids.is_empty());
    }

    struct FakeContent(Vec<String>);
    impl CitationCarrying for FakeContent {
        fn cited_ids(&self) -> Vec<String> {
            self.0.clone()
        }
    }

    #[test]
    fn trait_helpers_report_phantoms_and_groundedness() {
        let clean = FakeContent(cited(&["a", "b"]));
        let dirty = FakeContent(cited(&["a", "fake"]));
        let canonical = canon(&["a", "b"]);
        assert!(is_fully_grounded(&clean, &canonical));
        assert!(phantom_citations(&clean, &canonical).is_empty());
        assert!(!is_fully_grounded(&dirty, &canonical));
        assert_eq!(phantom_citations(&dirty, &canonical), cited(&["fake"]));
    }
}
