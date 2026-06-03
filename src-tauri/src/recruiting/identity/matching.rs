//! Name/company matching functions — pure, deterministic.
//!
//! Ported from `@sourcerer/core` `identity-resolver.ts` matching functions.

/// Return `true` if two name strings refer to the same person.
///
/// Comparison is case-insensitive, ignores extra whitespace, and treats
/// token reordering as equal (e.g. "Jane Doe" == "Doe Jane").
pub fn names_match(a: &str, b: &str) -> bool {
    let na = a.to_lowercase();
    let na = na.trim();
    let nb = b.to_lowercase();
    let nb = nb.trim();
    if na == nb {
        return true;
    }
    let mut parts_a: Vec<&str> = na.split_whitespace().collect();
    let mut parts_b: Vec<&str> = nb.split_whitespace().collect();
    parts_a.sort_unstable();
    parts_b.sort_unstable();
    parts_a == parts_b
}

/// Classic Levenshtein edit distance between two strings (character-level).
///
/// Standard DP implementation.  Works correctly for ASCII; for multi-byte
/// Unicode the cost is computed over `char` indices (not bytes), matching the
/// TS implementation's per-character semantics.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    // dp[i][j] = edit distance between a[..i] and b[..j]
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(n + 1) {
        *cell = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a_chars[i - 1] == b_chars[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

/// Return `true` if two names are the same (`names_match`) or within 2 edit
/// distance of each other (Levenshtein on the lowercased/trimmed strings).
pub fn names_similar(a: &str, b: &str) -> bool {
    if names_match(a, b) {
        return true;
    }
    levenshtein(a.to_lowercase().trim(), b.to_lowercase().trim()) <= 2
}

/// Parse a `name_company` identifier value of the form `"Name @ Company"`.
///
/// Returns `(name, company)` with both parts trimmed, or `None` if the `@`
/// separator is absent.
///
/// Note: the TS `identity-resolver.ts` uses `|` as the internal separator in
/// `extractFromNameCompany`. This function is the *Rust surface* used by
/// `extract_identifiers` to construct name_company values, and the tests pin
/// the `@` format — so both the builder and the parser here use `@`.
pub fn split_name_company(value: &str) -> Option<(String, String)> {
    let mut parts = value.splitn(2, '@');
    let name = parts.next()?.trim().to_string();
    let company = parts.next()?.trim().to_string();
    if name.is_empty() || company.is_empty() {
        return None;
    }
    Some((name, company))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_handles_token_reorder() {
        assert!(names_match("Jane Doe", "Doe Jane"));
        assert!(names_match("jane doe", "Jane  Doe"));
        assert!(!names_match("Jane Doe", "John Doe"));
    }

    #[test]
    fn names_match_rejects_duplicate_token_length_mismatch() {
        // "Jane Jane" has two tokens; "Jane" has one. Token-set equality must
        // not treat a repeated token as a superset match.
        assert!(!names_match("Jane Jane", "Jane"));
    }

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("acme", "acme"), 0);
    }

    #[test]
    fn levenshtein_empty_string_boundaries() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("", ""), 0);
    }

    #[test]
    fn names_similar_within_two_edits() {
        assert!(names_similar("Jon Smith", "John Smith")); // 1 edit
        assert!(!names_similar("Jane Doe", "Mark Lee"));
    }

    #[test]
    fn extract_name_company_splits() {
        let (n, c) = split_name_company("Jane Doe @ Acme").unwrap();
        assert_eq!(n, "Jane Doe");
        assert_eq!(c, "Acme");
    }

    #[test]
    fn split_name_company_empty_name_returns_none() {
        // Empty name before the `@` is invalid — guard must reject it.
        assert_eq!(split_name_company("@ Acme"), None);
    }
}
