//! Identifier normalizers — pure, deterministic, no network.
//!
//! Ported from `@sourcerer/core` `identity-resolver.ts` normalize* functions.
//! Key behaviors:
//!   - LinkedIn: strip protocol/www/query/trailing-slash, extract `/in/<slug>`, return slug only.
//!   - Email: lowercase; for gmail/googlemail strip dots from local + strip `+suffix`; map
//!     `googlemail.com` → `gmail.com`.  Non-gmail domains: lowercase only (no dot stripping).
//!   - GitHub: strip URL prefix/@ prefix, return lowercased username.
//!   - Twitter/X: strip URL prefix/@ prefix, return lowercased handle.

use super::types::IdentifierType;

/// Normalize a LinkedIn profile URL or slug to the bare profile slug (lowercase).
///
/// Examples:
/// - `"https://www.linkedin.com/in/Jane-Doe-123/"` → `"jane-doe-123"`
/// - `"linkedin.com/in/jane-doe-123"` → `"jane-doe-123"`
///
/// **Intentional divergence from the TS reference.** The TS
/// `normalizeLinkedInUrl` strips dashes from the slug (`"jane-doe-123"` →
/// `"janedoe123"`) and returns the full `linkedin.com/in/<slug>` path. This
/// Rust port instead returns the **dash-preserving slug** (`"jane-doe-123"`),
/// which becomes the merge key for the Task 4 resolver. This is a deliberate,
/// internally-consistent deviation: `/in/jane-doe-123` and `/in/janedoe123`
/// are genuinely *different* LinkedIn profiles, so preserving dashes avoids
/// false-positive merges and is the more correct identity key.
pub fn normalize_linkedin_url(raw: &str) -> String {
    let mut n = raw.to_lowercase();
    n = n.trim().to_string();
    // Strip protocol
    if let Some(rest) = n.strip_prefix("https://") {
        n = rest.to_string();
    } else if let Some(rest) = n.strip_prefix("http://") {
        n = rest.to_string();
    }
    // Strip www.
    if let Some(rest) = n.strip_prefix("www.") {
        n = rest.to_string();
    }
    // Strip query string
    if let Some(pos) = n.find('?') {
        n.truncate(pos);
    }
    // Strip trailing slashes
    while n.ends_with('/') {
        n.pop();
    }
    // Extract /in/<slug>
    if let Some(idx) = n.find("linkedin.com/in/") {
        let after = &n[idx + "linkedin.com/in/".len()..];
        // The slug is everything up to the next '/' (if any)
        let slug = after.split('/').next().unwrap_or(after);
        return slug.to_string();
    }
    n
}

/// Normalize an email address.
///
/// For gmail/googlemail: strip dots from local part, strip `+suffix`,
/// canonicalize domain to `gmail.com`.
/// For all other domains: lowercase only (no dot-stripping).
pub fn normalize_email(raw: &str) -> String {
    let n = raw.to_lowercase();
    let n = n.trim();
    let at = match n.find('@') {
        Some(i) => i,
        None => return n.to_string(),
    };
    let local = &n[..at];
    let domain = &n[at + 1..];
    if domain == "gmail.com" || domain == "googlemail.com" {
        let local = local.replace('.', "");
        let local = match local.find('+') {
            Some(pos) => local[..pos].to_string(),
            None => local,
        };
        format!("{}@gmail.com", local)
    } else {
        n.to_string()
    }
}

/// Normalize a GitHub username or profile URL to just the lowercased username.
///
/// Examples:
/// - `"https://github.com/Jane-Doe"` → `"jane-doe"`
/// - `"@Jane-Doe"` → `"jane-doe"`
pub fn normalize_github_username(raw: &str) -> String {
    let mut n = raw.to_lowercase();
    n = n.trim().to_string();
    // Strip protocol
    if let Some(rest) = n.strip_prefix("https://") {
        n = rest.to_string();
    } else if let Some(rest) = n.strip_prefix("http://") {
        n = rest.to_string();
    }
    // Strip www.
    if let Some(rest) = n.strip_prefix("www.") {
        n = rest.to_string();
    }
    // Strip github.com/
    if let Some(rest) = n.strip_prefix("github.com/") {
        n = rest.to_string();
    }
    // Strip @ prefix
    if let Some(rest) = n.strip_prefix('@') {
        n = rest.to_string();
    }
    // Strip trailing slashes
    while n.ends_with('/') {
        n.pop();
    }
    n
}

/// Normalize a Twitter/X handle or profile URL to just the lowercased handle.
///
/// Examples:
/// - `"https://twitter.com/JaneDoe"` → `"janedoe"`
/// - `"@JaneDoe"` → `"janedoe"`
pub fn normalize_twitter_handle(raw: &str) -> String {
    let mut n = raw.to_lowercase();
    n = n.trim().to_string();
    // Strip protocol
    if let Some(rest) = n.strip_prefix("https://") {
        n = rest.to_string();
    } else if let Some(rest) = n.strip_prefix("http://") {
        n = rest.to_string();
    }
    // Strip www.
    if let Some(rest) = n.strip_prefix("www.") {
        n = rest.to_string();
    }
    // Strip twitter.com/ or x.com/
    if let Some(rest) = n.strip_prefix("twitter.com/") {
        n = rest.to_string();
    } else if let Some(rest) = n.strip_prefix("x.com/") {
        n = rest.to_string();
    }
    // Strip @ prefix
    if let Some(rest) = n.strip_prefix('@') {
        n = rest.to_string();
    }
    // Strip trailing slashes
    while n.ends_with('/') {
        n.pop();
    }
    n
}

/// Dispatch normalizer based on identifier kind.
pub fn normalize_identifier_value(kind: IdentifierType, raw: &str) -> String {
    match kind {
        IdentifierType::Linkedin => normalize_linkedin_url(raw),
        IdentifierType::Email => normalize_email(raw),
        IdentifierType::Github => normalize_github_username(raw),
        IdentifierType::Twitter => normalize_twitter_handle(raw),
        IdentifierType::PersonalUrl => {
            let mut n = raw.to_lowercase();
            n = n.trim().to_string();
            while n.ends_with('/') {
                n.pop();
            }
            n
        }
        IdentifierType::NameCompany => raw.to_lowercase().trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linkedin_strips_to_slug() {
        assert_eq!(
            normalize_linkedin_url("https://www.linkedin.com/in/Jane-Doe-123/"),
            "jane-doe-123"
        );
        assert_eq!(
            normalize_linkedin_url("linkedin.com/in/jane-doe-123"),
            "jane-doe-123"
        );
    }

    #[test]
    fn email_strips_dots_and_plus_and_googlemail() {
        assert_eq!(
            normalize_email("Ja.ne+recruit@googlemail.com"),
            "jane@gmail.com"
        );
        // Non-gmail: dots NOT stripped, just lowercase
        assert_eq!(
            normalize_email("JANE@Example.com"),
            "jane@example.com"
        );
    }

    #[test]
    fn email_without_at_returns_lowercased_input() {
        // No `@` → no panic, just lowercased input unchanged in structure.
        assert_eq!(normalize_email("noemail"), "noemail");
        assert_eq!(normalize_email("NoEmail"), "noemail");
    }

    #[test]
    fn email_with_multiple_ats_is_stable_and_no_panic() {
        // Splits on the FIRST `@`; domain = "bar@baz.com" (not gmail) → lowercase only.
        let out = normalize_email("foo@bar@baz.com");
        assert_eq!(out, "foo@bar@baz.com");
    }

    #[test]
    fn github_to_username() {
        assert_eq!(
            normalize_github_username("https://github.com/Jane-Doe"),
            "jane-doe"
        );
        assert_eq!(normalize_github_username("@Jane-Doe"), "jane-doe");
    }

    #[test]
    fn twitter_handle() {
        assert_eq!(
            normalize_twitter_handle("https://twitter.com/JaneDoe"),
            "janedoe"
        );
        assert_eq!(normalize_twitter_handle("@JaneDoe"), "janedoe");
    }
}
