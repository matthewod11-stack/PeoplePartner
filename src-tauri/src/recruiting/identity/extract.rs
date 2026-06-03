//! Identifier and name extraction from Exa search result URLs and page text.
//!
//! Ported from `packages/adapters/adapter-exa/src/parsers.ts`
//! (`extractIdentifiers`, `extractName`).
//!
//! Rules:
//!   - LinkedIn `/in/<slug>` URLs → `IdentifierType::Linkedin` (High)
//!   - GitHub `github.com/<username>` → `Github` (High), stop-worded usernames skipped
//!   - Twitter/X handles → `Twitter` (Medium), stop-worded handles skipped
//!   - Email addresses → `Email` (Medium)
//!   - The result URL itself, when NOT a social domain → `PersonalUrl` (Medium)
//!   - LinkedIn generic paths (`/feed/`, `/jobs/`, etc.) are stop-worded

use regex::Regex;
use std::sync::OnceLock;

use super::normalize::{
    normalize_github_username, normalize_linkedin_url, normalize_twitter_handle,
};
use super::types::{ConfidenceLevel, IdentifierType, ObservedIdentifier};

// ---------------------------------------------------------------------------
// Static compiled regexes
// ---------------------------------------------------------------------------

fn linkedin_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"linkedin\.com/in/([\w-]+)").unwrap())
}

fn github_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"github\.com/([\w-]+)").unwrap())
}

fn twitter_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:twitter|x)\.com/([\w]+)").unwrap())
}

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[\w.+\-]+@[\w.\-]+\.\w{2,}").unwrap())
}

// ---------------------------------------------------------------------------
// Stop-word lists (from TS parsers.ts)
// ---------------------------------------------------------------------------

const GITHUB_STOPWORDS: &[&str] = &[
    "features", "about", "pricing", "blog", "docs", "settings", "topics",
];

const TWITTER_STOPWORDS: &[&str] = &["home", "search", "explore", "settings", "i"];

/// LinkedIn paths that should NOT become identifiers.
/// These are generic feed/navigation URLs, not profile URLs.
const LINKEDIN_STOPWORDS: &[&str] = &[
    "feed", "jobs", "messaging", "notifications", "mynetwork", "learning",
    "search", "sales", "company", "school", "groups", "events",
];

const SOCIAL_DOMAINS: &[&str] = &["linkedin.com", "github.com", "twitter.com", "x.com"];

fn is_social_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    SOCIAL_DOMAINS.iter().any(|d| lower.contains(d))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Extract all identifiers from a search result's URL and optional page text.
///
/// `url`            — the result page URL (checked first for social profiles).
/// `text`           — optional additional page text to scan for identifiers.
/// `source_adapter` — adapter name to stamp on each identifier (e.g. `"exa"`).
///
/// Deduplicates by `(type, normalized_value)` before returning.
pub fn extract_identifiers(
    url: &str,
    text: Option<&str>,
    source_adapter: &str,
) -> Vec<ObservedIdentifier> {
    let combined = match text {
        Some(t) => format!("{} {}", url, t),
        None => url.to_string(),
    };

    let mut identifiers: Vec<ObservedIdentifier> = Vec::new();

    // --- LinkedIn ---
    for cap in linkedin_re().captures_iter(&combined) {
        let slug_raw = &cap[1];
        let slug = slug_raw.to_lowercase();
        // Skip stop-worded slugs
        if LINKEDIN_STOPWORDS.contains(&slug.as_str()) {
            continue;
        }
        let raw_url = format!("https://linkedin.com/in/{}", slug_raw);
        let value = normalize_linkedin_url(&raw_url);
        identifiers.push(ObservedIdentifier {
            kind: IdentifierType::Linkedin,
            value,
            raw_value: raw_url,
            source_adapter: source_adapter.to_string(),
            confidence: ConfidenceLevel::High,
        });
    }

    // --- GitHub ---
    for cap in github_re().captures_iter(&combined) {
        let username = cap[1].to_lowercase();
        if GITHUB_STOPWORDS.contains(&username.as_str()) {
            continue;
        }
        let raw_at = format!("@{}", username);
        let value = normalize_github_username(&username);
        identifiers.push(ObservedIdentifier {
            kind: IdentifierType::Github,
            value,
            raw_value: raw_at,
            source_adapter: source_adapter.to_string(),
            confidence: ConfidenceLevel::High,
        });
    }

    // --- Twitter / X ---
    for cap in twitter_re().captures_iter(&combined) {
        let handle = cap[1].to_lowercase();
        if TWITTER_STOPWORDS.contains(&handle.as_str()) {
            continue;
        }
        let raw_at = format!("@{}", handle);
        let value = normalize_twitter_handle(&handle);
        identifiers.push(ObservedIdentifier {
            kind: IdentifierType::Twitter,
            value,
            raw_value: raw_at,
            source_adapter: source_adapter.to_string(),
            confidence: ConfidenceLevel::Medium,
        });
    }

    // --- Email ---
    for cap in email_re().captures_iter(&combined) {
        let raw = cap[0].to_lowercase();
        identifiers.push(ObservedIdentifier {
            kind: IdentifierType::Email,
            value: raw.clone(),
            raw_value: raw,
            source_adapter: source_adapter.to_string(),
            confidence: ConfidenceLevel::Medium,
        });
    }

    // --- Personal URL (result URL itself, if not a social platform) ---
    if !is_social_url(url) {
        identifiers.push(ObservedIdentifier {
            kind: IdentifierType::PersonalUrl,
            value: url.to_string(),
            raw_value: url.to_string(),
            source_adapter: source_adapter.to_string(),
            confidence: ConfidenceLevel::Medium,
        });
    }

    // --- Deduplicate by (type, value) ---
    let mut seen = std::collections::HashSet::new();
    identifiers.retain(|id| {
        let key = format!("{:?}:{}", id.kind, id.value);
        seen.insert(key)
    });

    identifiers
}

/// Heuristically determine a candidate's display name from available signals.
///
/// Priority (matches TS `extractName`):
/// 1. `author` — if present, non-empty, and < 60 chars.
/// 2. `title` — if it looks like a personal page title (short, starts with
///    capital, no unusual characters, < 50 chars).
/// 3. LinkedIn slug from URL → Title Case.
/// 4. GitHub username from URL.
/// 5. URL-derived slug (last path component, dashes → spaces).
/// 6. `title` as-is, or `"Unknown"`.
pub fn extract_name(author: Option<&str>, title: Option<&str>, url: &str) -> String {
    // 1. Explicit author
    if let Some(a) = author {
        let a = a.trim();
        if !a.is_empty() && a.len() < 60 {
            return a.to_string();
        }
    }

    // 2. Title heuristic — looks like a personal name (short, starts capital, safe chars)
    if let Some(t) = title {
        let t = t.trim();
        if t.len() < 50 {
            // Matches TS: /^[A-Z][\w\s.'-]+$/ — `\w` is alphanumeric (+`_`),
            // `\s` is any whitespace, plus the literal `.`, `'`, `-` chars.
            let looks_personal = t.starts_with(|c: char| c.is_ascii_uppercase())
                && t
                    .chars()
                    .all(|c| c.is_alphanumeric() || c.is_whitespace() || ".'- ".contains(c));
            if looks_personal {
                return t.to_string();
            }
        }
    }

    // 3. LinkedIn slug → Title Case
    if let Some(cap) = linkedin_re().captures(url) {
        let slug = &cap[1];
        return slug
            .split('-')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().to_string() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
    }

    // 4. GitHub username
    if let Some(cap) = github_re().captures(url) {
        return cap[1].to_string();
    }

    // 5. URL slug — last non-empty path component, dashes → spaces
    if let Some(last) = url
        .split('/')
        .filter(|s| !s.is_empty() && !s.contains('.'))
        .last()
    {
        if last.len() > 2 && last.contains('-') {
            return last.replace('-', " ");
        }
    }

    // 6. Fallback: title or "Unknown"
    title.unwrap_or("Unknown").to_string()
}

#[cfg(test)]
mod extract_tests {
    use super::*;

    #[test]
    fn extract_identifiers_from_url_and_text() {
        let ids = extract_identifiers(
            "https://www.linkedin.com/in/jane-doe-99",
            Some("Reach me at jane@acme.com or github.com/jane-doe"),
            "exa",
        );
        let kinds: Vec<_> = ids.iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&IdentifierType::Linkedin));
        assert!(kinds.contains(&IdentifierType::Email));
        assert!(kinds.contains(&IdentifierType::Github));
        assert!(
            ids.iter()
                .any(|i| i.kind == IdentifierType::Linkedin && i.value == "jane-doe-99"),
            "LinkedIn value should be the normalized slug; got: {:?}",
            ids.iter()
                .filter(|i| i.kind == IdentifierType::Linkedin)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn extract_identifiers_ignores_stopword_urls() {
        let ids = extract_identifiers("https://www.linkedin.com/feed/", None, "exa");
        assert!(
            ids.iter().all(|i| i.kind != IdentifierType::Linkedin),
            "feed/ is a stop-word path and must not produce a LinkedIn identifier"
        );
    }

    #[test]
    fn extract_name_prefers_author_then_title_then_slug() {
        assert_eq!(
            extract_name(
                Some("Jane Doe"),
                Some("Jane Doe — Engineer"),
                "https://x.com/jane-doe"
            ),
            "Jane Doe"
        );
        // No author/title → URL slug
        assert_eq!(
            extract_name(None, None, "https://example.com/john-smith"),
            "john smith"
        );
    }
}
