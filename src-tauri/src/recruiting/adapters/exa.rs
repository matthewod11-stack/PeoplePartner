//! Exa search adapter for the recruiting module.
//!
//! Wraps Exa's `/search` endpoint (<https://exa.ai>). Exa is the recruiting
//! module's primary candidate-discovery source; the team-seeded `findSimilar`
//! variant (roadmap S1.1) and per-hit `getContents` (S1.1) will extend the
//! same types defined here.
//!
//! Key conventions:
//!   - The API key is read from the macOS Keychain via
//!     `keyring::get_provider_api_key(crate::recruiting::EXA_PROVIDER_ID)`
//!     at the command boundary; this module takes the key as a `&str`
//!     argument so unit tests never touch OS-level storage.
//!   - Wire format matches Exa verbatim via `#[serde(rename_all = "camelCase")]`.
//!   - All hit fields except `id` and `url` are `Option<T>` because Exa
//!     populates them inconsistently depending on search params (highlights,
//!     summary, contents are opt-in).

use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

const EXA_SEARCH_URL: &str = "https://api.exa.ai/search";
/// `/contents` fetches the full text/title for already-known URLs — the
/// `crawl_url` half of the intake `ContentResearch` trait (FHR-95).
const EXA_CONTENTS_URL: &str = "https://api.exa.ai/contents";
/// `/findSimilar` powers team-seeded discovery (`findSimilar` trait method,
/// roadmap S1.1) — "find people like these URLs".
const EXA_FIND_SIMILAR_URL: &str = "https://api.exa.ai/findSimilar";
const DEFAULT_NUM_RESULTS: u32 = 10;
/// Exa search mode: "neural" (semantic), "keyword" (lexical), "auto" (Exa picks).
/// "auto" is the right default for v1 — it lets Exa decide and removes a
/// premature tuning knob from the UX.
const DEFAULT_SEARCH_TYPE: &str = "auto";

// ============================================================================
// Wire types — request
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaSearchRequest<'a> {
    query: &'a str,
    num_results: u32,
    #[serde(rename = "type")]
    search_type: &'a str,
}

/// `/contents` request. `text: true` asks Exa to return the full document text
/// for each URL (the field `crawl_url` reads). `urls` is borrowed from the
/// caller's seed list.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaContentsRequest<'a> {
    urls: &'a [String],
    text: bool,
}

/// `/findSimilar` request — Exa takes a single seed URL and returns pages
/// similar to it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaFindSimilarRequest<'a> {
    url: &'a str,
    num_results: u32,
}

// ============================================================================
// Wire types — response
// ============================================================================

/// Top-level response from Exa's `/search` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaSearchResponse {
    pub results: Vec<ExaHit>,
    /// Exa's rewrite of the user query (only present for `neural` / `auto`).
    pub autoprompt_string: Option<String>,
    /// Server-side request ID — quote this when filing Exa support tickets.
    pub request_id: Option<String>,
}

/// One search hit. `id` and `url` are guaranteed on every result; every
/// other field is opt-in depending on Exa search params.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaHit {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    /// Relevance score in `[0.0, 1.0]`. `f32` is enough precision — Exa
    /// returns ~2 decimal places of meaningful signal.
    pub score: Option<f32>,
    /// Publication date as a raw string. Kept as `String` (not parsed) because
    /// Exa returns mixed formats (ISO 8601, `YYYY-MM-DD`, sometimes null) and
    /// the frontend just renders it.
    pub published_date: Option<String>,
    pub author: Option<String>,
    /// Full document text — only populated when fetched via `getContents`.
    pub text: Option<String>,
    /// Matched snippets — only when `highlights=true` was requested.
    pub highlights: Option<Vec<String>>,
    /// LLM-generated summary — only when `summary=true` was requested.
    pub summary: Option<String>,
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Error)]
pub enum ExaError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    /// HTTP 401 — Exa rejected the key. Distinct from `MissingKey` at the
    /// command layer, which means "no key was stored at all."
    #[error("exa rejected the api key")]
    InvalidKey,
    /// HTTP 429 — quota exhausted or rate-limit window hit.
    #[error("rate limited: {message}")]
    RateLimit { message: String },
    /// Any other non-2xx response.
    #[error("exa api returned {status}: {body}")]
    Api { status: u16, body: String },
    /// Body was 2xx but didn't deserialize into `ExaSearchResponse`.
    #[error("failed to parse exa response: {0}")]
    InvalidResponse(String),
}

// ============================================================================
// Search
// ============================================================================

/// POST a JSON body to an Exa endpoint and deserialize the response.
///
/// Shared by `search` / `get_contents` / `find_similar` — all three are the
/// same request shape (x-api-key header, JSON body) and the same status
/// handling (401 → `InvalidKey`, 429 → `RateLimit`, other non-2xx → `Api`).
/// The caller fetches `api_key` from the Keychain (typically via
/// `keyring::get_provider_api_key(EXA_PROVIDER_ID)`) and is responsible for
/// translating `KeyringError::NotFound` into a higher-level "missing key"
/// condition — this function only sees the key as an opaque `&str`.
async fn exa_post<B, R>(url: &str, api_key: &str, body: &B) -> Result<R, ExaError>
where
    B: Serialize,
    R: DeserializeOwned,
{
    let response = Client::new()
        .post(url)
        .header("x-api-key", api_key)
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        let text = response.text().await?;
        return serde_json::from_str(&text)
            .map_err(|e| ExaError::InvalidResponse(e.to_string()));
    }

    let status_code = status.as_u16();
    let body = response.text().await.unwrap_or_default();
    Err(match status_code {
        401 => ExaError::InvalidKey,
        429 => ExaError::RateLimit { message: body },
        _ => ExaError::Api {
            status: status_code,
            body,
        },
    })
}

/// Execute a search against Exa's `/search` endpoint.
pub async fn search(query: &str, api_key: &str) -> Result<ExaSearchResponse, ExaError> {
    let body = ExaSearchRequest {
        query,
        num_results: DEFAULT_NUM_RESULTS,
        search_type: DEFAULT_SEARCH_TYPE,
    };
    exa_post(EXA_SEARCH_URL, api_key, &body).await
}

/// Fetch full document text for the given URLs via Exa's `/contents` endpoint.
/// Backs the intake `crawl_url` step — the caller hands us a URL it already
/// has (e.g. a company website the user pasted) and we return its text.
pub async fn get_contents(urls: &[String], api_key: &str) -> Result<ExaSearchResponse, ExaError> {
    let body = ExaContentsRequest { urls, text: true };
    exa_post(EXA_CONTENTS_URL, api_key, &body).await
}

/// Find pages similar to a seed URL via Exa's `/findSimilar` endpoint. Feeds
/// team-seeded candidate discovery ("find people like these team members").
pub async fn find_similar(url: &str, api_key: &str) -> Result<ExaSearchResponse, ExaError> {
    let body = ExaFindSimilarRequest {
        url,
        num_results: DEFAULT_NUM_RESULTS,
    };
    exa_post(EXA_FIND_SIMILAR_URL, api_key, &body).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Realistic Exa `/search` response — trimmed from a live call, covers
    /// the common-case hit (id, url, title, score, publishedDate, author).
    const FIXTURE_FULL: &str = r#"{
        "results": [
            {
                "id": "https://example.com/jane-doe",
                "url": "https://example.com/jane-doe",
                "title": "Senior Rust Engineer at Acme",
                "score": 0.87,
                "publishedDate": "2024-03-15",
                "author": "Jane Doe"
            }
        ],
        "autopromptString": "rust engineers berlin",
        "requestId": "req_abc123"
    }"#;

    /// Sparse fixture — only the fields Exa guarantees on every hit. Locks
    /// down `Option<T>` handling for the long tail of partial responses.
    const FIXTURE_SPARSE: &str = r#"{
        "results": [
            {"id": "abc", "url": "https://example.org/post"}
        ]
    }"#;

    #[test]
    fn deserializes_full_response() {
        let parsed: ExaSearchResponse =
            serde_json::from_str(FIXTURE_FULL).expect("parse full fixture");
        assert_eq!(parsed.results.len(), 1);
        let hit = &parsed.results[0];
        assert_eq!(hit.url, "https://example.com/jane-doe");
        assert_eq!(hit.title.as_deref(), Some("Senior Rust Engineer at Acme"));
        assert_eq!(hit.score, Some(0.87));
        assert_eq!(hit.published_date.as_deref(), Some("2024-03-15"));
        assert_eq!(hit.author.as_deref(), Some("Jane Doe"));
        assert!(hit.text.is_none());
        assert!(hit.highlights.is_none());
        assert!(hit.summary.is_none());
        assert_eq!(
            parsed.autoprompt_string.as_deref(),
            Some("rust engineers berlin")
        );
        assert_eq!(parsed.request_id.as_deref(), Some("req_abc123"));
    }

    #[test]
    fn deserializes_sparse_response_with_only_required_fields() {
        let parsed: ExaSearchResponse =
            serde_json::from_str(FIXTURE_SPARSE).expect("parse sparse fixture");
        assert_eq!(parsed.results.len(), 1);
        let hit = &parsed.results[0];
        assert_eq!(hit.id, "abc");
        assert_eq!(hit.url, "https://example.org/post");
        assert!(hit.title.is_none(), "title is Option");
        assert!(hit.score.is_none(), "score is Option");
        assert!(hit.published_date.is_none(), "publishedDate is Option");
        assert!(parsed.autoprompt_string.is_none(), "autoprompt is Option");
        assert!(parsed.request_id.is_none(), "requestId is Option");
    }

    /// `/contents` response — same `{results: [...]}` envelope as `/search`,
    /// but the hit carries populated `text` (the whole reason to call it).
    /// `crawl_url` reads `results[0].text`.
    const FIXTURE_CONTENTS: &str = r#"{
        "results": [
            {
                "id": "https://acme.com",
                "url": "https://acme.com",
                "title": "Acme — Developer Infrastructure",
                "text": "Acme builds developer infrastructure. We use Rust and Go."
            }
        ],
        "requestId": "req_contents_1"
    }"#;

    /// `/findSimilar` response — identical envelope to `/search`; hits carry a
    /// relevance `score` used as the similarity value downstream.
    const FIXTURE_FIND_SIMILAR: &str = r#"{
        "results": [
            {
                "id": "https://github.com/similar-dev",
                "url": "https://github.com/similar-dev",
                "title": "Similar Dev",
                "score": 0.91
            }
        ]
    }"#;

    #[test]
    fn deserializes_contents_response_with_text() {
        let parsed: ExaSearchResponse =
            serde_json::from_str(FIXTURE_CONTENTS).expect("parse contents fixture");
        assert_eq!(parsed.results.len(), 1);
        let hit = &parsed.results[0];
        assert_eq!(hit.url, "https://acme.com");
        assert_eq!(hit.title.as_deref(), Some("Acme — Developer Infrastructure"));
        assert!(
            hit.text.as_deref().unwrap().contains("Rust and Go"),
            "contents must populate the document text crawl_url reads"
        );
    }

    #[test]
    fn deserializes_find_similar_response() {
        let parsed: ExaSearchResponse =
            serde_json::from_str(FIXTURE_FIND_SIMILAR).expect("parse findSimilar fixture");
        assert_eq!(parsed.results.len(), 1);
        let hit = &parsed.results[0];
        assert_eq!(hit.url, "https://github.com/similar-dev");
        assert_eq!(hit.score, Some(0.91), "similarity score drives SimilarResult");
    }

    #[test]
    fn contents_request_serializes_urls_and_text_flag() {
        // Wire-shape guard: Exa expects camelCase `text` + a `urls` array.
        let body = ExaContentsRequest {
            urls: &["https://acme.com".to_string()],
            text: true,
        };
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["urls"][0], "https://acme.com");
        assert_eq!(v["text"], true);
    }

    #[test]
    fn find_similar_request_serializes_camelcase_num_results() {
        let body = ExaFindSimilarRequest { url: "https://x.com", num_results: 10 };
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["url"], "https://x.com");
        assert_eq!(v["numResults"], 10, "num_results must serialize as numResults");
    }
}
