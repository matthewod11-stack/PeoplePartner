//! Anti-filters — apply the subset of `SearchConfig.anti_filters` that can be
//! evaluated on raw discovery data (URL host, author, title — not full page
//! text, which doesn't exist yet).
//!
//! # Evaluated kinds
//! - `exclude_company` — case-insensitive match against URL host, author
//!   field, and a word-boundary title check.
//!
//! # Skipped (deferred to scoring)
//! Kinds that need extracted signals — `exclude_seniority`,
//! `min_experience_years`, `max_experience_years`, `exclude_signal` — are
//! returned in the second element of the tuple so `DiscoveryStats` can be
//! honest about what was skipped.

use crate::recruiting::intake::schemas::AntiFilter;
use crate::recruiting::search::candidate::RawCandidate;

/// Apply evaluable anti-filters to `candidates`.
///
/// Returns `(kept, skipped_kinds)` where `skipped_kinds` is the deduplicated
/// list of filter kind names that could not be evaluated at discovery time.
pub fn apply_anti_filters(
    candidates: Vec<RawCandidate>,
    filters: &[AntiFilter],
) -> (Vec<RawCandidate>, Vec<String>) {
    // Separate evaluable from skip-list filters
    let mut skipped_kinds: Vec<String> = Vec::new();
    let mut evaluable: Vec<&AntiFilter> = Vec::new();

    for f in filters {
        match f.kind.as_str() {
            "exclude_company" => evaluable.push(f),
            // Deferred — need signals from scoring phase
            "exclude_seniority"
            | "min_experience_years"
            | "max_experience_years"
            | "exclude_signal" => {
                if !skipped_kinds.contains(&f.kind) {
                    skipped_kinds.push(f.kind.clone());
                }
            }
            // Unknown kinds also go on the skip list
            other => {
                if !skipped_kinds.contains(&other.to_string()) {
                    skipped_kinds.push(other.to_string());
                }
            }
        }
    }

    if evaluable.is_empty() {
        return (candidates, skipped_kinds);
    }

    let kept = candidates
        .into_iter()
        .filter(|c| !is_excluded(c, &evaluable))
        .collect();

    (kept, skipped_kinds)
}

/// Returns `true` when `candidate` matches any evaluable anti-filter.
fn is_excluded(candidate: &RawCandidate, filters: &[&AntiFilter]) -> bool {
    for filter in filters {
        if filter.kind.as_str() == "exclude_company"
            && matches_exclude_company(candidate, &filter.value)
        {
            return true;
        }
    }
    false
}

/// Case-insensitive company exclusion.
///
/// Matches against:
/// 1. URL host — `acme.com` host contains `acme`
/// 2. `author` field — direct contains check
/// 3. `title` — word-boundary check: value must appear as a whole word
///    (preceded/followed by non-alphanumeric or string boundary)
///
/// Deliberately does NOT scan the full `text` field — full text can mention
/// a company in passing without the candidate working there.
fn matches_exclude_company(candidate: &RawCandidate, value: &str) -> bool {
    let value_lower = value.to_lowercase();

    // 1. URL host
    if let Some(host) = extract_host(&candidate.url) {
        if host.to_lowercase().contains(&value_lower) {
            return true;
        }
    }

    // 2. Author
    if let Some(author) = &candidate.author {
        if author.to_lowercase().contains(&value_lower) {
            return true;
        }
    }

    // 3. Title — word-boundary check
    if let Some(title) = &candidate.title {
        if title_contains_word(&title.to_lowercase(), &value_lower) {
            return true;
        }
    }

    false
}

/// Extract the hostname from a URL (without port).
fn extract_host(url: &str) -> Option<&str> {
    // Strip scheme
    let without_scheme = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else {
        url
    };

    // Take up to the first '/' or end of string
    let host_and_maybe_port = without_scheme.split('/').next()?;
    // Strip port if present
    let host = host_and_maybe_port.split(':').next()?;
    Some(host)
}

/// Check whether `text` contains `word` as a whole word (word-boundary match).
///
/// "Word boundary" here means the character before and after `word` (if
/// present) is non-alphanumeric.
fn title_contains_word(text: &str, word: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = text[start..].find(word) {
        let abs_pos = start + pos;
        let abs_end = abs_pos + word.len();

        let before_ok = abs_pos == 0
            || !text[..abs_pos]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        let after_ok = abs_end == text.len()
            || !text[abs_end..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);

        if before_ok && after_ok {
            return true;
        }
        // Advance past the first char of this match, staying on a UTF-8 char
        // boundary — `abs_pos + 1` could land inside a multibyte char and
        // panic on the next `text[start..]` slice (e.g. "caféé".find("é")).
        start = abs_pos
            + text[abs_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::search::candidate::{CandidateProvenance, DiscoveryVia, RawCandidate};

    fn anti(kind: &str, value: &str) -> AntiFilter {
        AntiFilter {
            kind: kind.into(),
            value: value.into(),
            reason: "test".into(),
        }
    }

    fn cand_with(
        url: &str,
        author: Option<&str>,
        title: Option<&str>,
        _body: Option<&str>,
    ) -> RawCandidate {
        RawCandidate {
            name: "Test".into(),
            url: url.into(),
            exa_id: url.into(),
            title: title.map(|s| s.into()),
            author: author.map(|s| s.into()),
            published_date: None,
            score: None,
            text: _body.map(|s| s.into()),
            identifiers: vec![],
            source_adapter: "exa".into(),
            provenance: CandidateProvenance {
                via: DiscoveryVia::TierQuery,
                query_text: Some("q".into()),
                seed_url: None,
                tier_priority: Some(1),
            },
        }
    }

    #[test]
    fn exclude_company_matches_host_author_title_not_body() {
        let cands = vec![
            // host matches "acme"
            cand_with(
                "https://acme.com/jane",
                Some("Jane"),
                Some("Eng at Acme"),
                Some("works elsewhere"),
            ),
            // body mentions acme but host/author/title do not — must be kept
            cand_with(
                "https://other.com/bob",
                Some("Bob"),
                Some("Eng"),
                Some("mentions acme in passing"),
            ),
        ];
        let (kept, skipped) = apply_anti_filters(cands, &[anti("exclude_company", "acme")]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].url, "https://other.com/bob");
        assert!(skipped.is_empty());
    }

    #[test]
    fn exclude_company_matches_author_field() {
        let cands = vec![
            cand_with("https://x.com/a", Some("Jane at Acme Corp"), None, None),
            cand_with("https://x.com/b", Some("Bob"), None, None),
        ];
        let (kept, _) = apply_anti_filters(cands, &[anti("exclude_company", "acme")]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].url, "https://x.com/b");
    }

    #[test]
    fn exclude_company_title_word_boundary() {
        let cands = vec![
            // "Acme" as a whole word in title — filtered
            cand_with("https://x.com/a", None, Some("Engineer at Acme"), None),
            // "Acme" embedded in a longer word — NOT filtered
            cand_with("https://x.com/b", None, Some("AcmeCorp Engineer"), None),
        ];
        let (kept, _) = apply_anti_filters(cands, &[anti("exclude_company", "acme")]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].url, "https://x.com/b");
    }

    #[test]
    fn exclude_company_multibyte_value_no_panic_and_matches() {
        // "Société" is multibyte (é = 2 bytes). The non-matching candidate
        // exercises the inner `start +=` advance loop that previously could
        // slice inside a multibyte char and panic.
        let cands = vec![
            cand_with("https://x.com/a", None, Some("Engineer at Société Générale"), None),
            // A title that contains "é" repeatedly but NOT the whole word
            // "Société" — forces the search loop to advance past multibyte chars.
            cand_with("https://x.com/b", None, Some("caféé crémerie résumé"), None),
        ];
        let (kept, _) = apply_anti_filters(cands, &[anti("exclude_company", "Société")]);
        assert_eq!(kept.len(), 1, "the Société candidate must be filtered out");
        assert_eq!(kept[0].url, "https://x.com/b");
    }

    #[test]
    fn unsupported_kind_passes_through_and_is_flagged() {
        let (kept, skipped) = apply_anti_filters(
            vec![cand_with("https://x.com/a", None, None, None)],
            &[anti("exclude_seniority", "junior")],
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(skipped, vec!["exclude_seniority".to_string()]);
    }

    #[test]
    fn unknown_kind_also_flagged() {
        let (kept, skipped) = apply_anti_filters(
            vec![cand_with("https://x.com/a", None, None, None)],
            &[anti("exclude_foo", "bar")],
        );
        assert_eq!(kept.len(), 1);
        assert!(skipped.contains(&"exclude_foo".to_string()));
    }

    #[test]
    fn skipped_kinds_deduped() {
        // Two filters of the same unsupported kind — should only appear once
        let (_, skipped) = apply_anti_filters(
            vec![cand_with("https://x.com/a", None, None, None)],
            &[
                anti("exclude_seniority", "junior"),
                anti("exclude_seniority", "entry"),
            ],
        );
        assert_eq!(
            skipped.iter().filter(|s| *s == "exclude_seniority").count(),
            1
        );
    }

    #[test]
    fn no_filters_passes_all_candidates_through() {
        let cands = vec![
            cand_with("https://a.com/x", None, None, None),
            cand_with("https://b.com/y", None, None, None),
        ];
        let (kept, skipped) = apply_anti_filters(cands, &[]);
        assert_eq!(kept.len(), 2);
        assert!(skipped.is_empty());
    }

    #[test]
    fn case_insensitive_host_match() {
        let cands = vec![
            cand_with("https://ACME.com/person", None, None, None),
            cand_with("https://other.com/person", None, None, None),
        ];
        let (kept, _) = apply_anti_filters(cands, &[anti("exclude_company", "Acme")]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].url, "https://other.com/person");
    }

    #[test]
    fn multiple_evaluable_filters_any_match_excludes() {
        let cands = vec![
            cand_with("https://acme.com/a", None, None, None),
            cand_with("https://beta.com/b", None, Some("Software Eng at Beta Corp"), None),
            cand_with("https://safe.com/c", None, None, None),
        ];
        let (kept, _) = apply_anti_filters(
            cands,
            &[
                anti("exclude_company", "acme"),
                anti("exclude_company", "beta"),
            ],
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].url, "https://safe.com/c");
    }
}
