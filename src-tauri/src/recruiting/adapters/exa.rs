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
    /// Always `true` for candidate discovery — prevents the seed URL's own
    /// domain from appearing in results (mirrors TS adapter line ~166).
    exclude_source_domain: bool,
}

/// Options for the full-fidelity search variant. Borrowed slices so callers
/// pass `SearchQuery` fields without cloning.
#[derive(Debug)]
pub struct ExaSearchOpts<'a> {
    pub num_results: u32,
    pub include_domains: Option<&'a [String]>,
    pub exclude_domains: Option<&'a [String]>,
    /// E.g. `"people"` — default applied by the caller, not here.
    pub category: Option<&'a str>,
}

/// Full-fidelity search request that carries per-query result caps, domain
/// filters, and category. The 1-arg `search` function drops all of these;
/// `search_with` sends them. `skip_serializing_if` keeps the wire compact when
/// fields are `None`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExaSearchRequestFull<'a> {
    query: &'a str,
    num_results: u32,
    #[serde(rename = "type")]
    search_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_domains: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_domains: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<&'a str>,
}

impl<'a> ExaSearchRequestFull<'a> {
    fn from_parts(query: &'a str, opts: &ExaSearchOpts<'a>) -> Self {
        ExaSearchRequestFull {
            query,
            num_results: opts.num_results,
            search_type: DEFAULT_SEARCH_TYPE,
            include_domains: opts.include_domains,
            exclude_domains: opts.exclude_domains,
            category: opts.category,
        }
    }
}

// ============================================================================
// Wire types — response
// ============================================================================

/// Cost metadata returned by Exa — present when the account is billed
/// per-request. Absent on legacy plans / sandbox keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExaCostDollars {
    pub total: f64,
}

/// Top-level response from Exa's `/search` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExaSearchResponse {
    pub results: Vec<ExaHit>,
    /// Exa's rewrite of the user query (only present for `neural` / `auto`).
    pub autoprompt_string: Option<String>,
    /// Server-side request ID — quote this when filing Exa support tickets.
    pub request_id: Option<String>,
    /// Per-request billing cost in USD. `None` on plans that don't report it.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cost_dollars: Option<ExaCostDollars>,
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

// ============================================================================
// Egress audit (FHR-91)
// ============================================================================

/// DB handle for auditing Exa egress. `None` at a call site means "no pool
/// available" (unit tests, non-Tauri contexts) — never "skip the audit on
/// purpose" in production code.
#[derive(Clone)]
pub struct ExaAudit {
    pool: crate::db::DbPool,
}

impl ExaAudit {
    pub fn new(pool: crate::db::DbPool) -> Self {
        Self { pool }
    }
}

/// Write one audit row for an Exa request. Best-effort: an audit failure is
/// logged, never propagated — the same log-and-continue contract the chat
/// seam uses (#112).
///
/// The request text is redacted under the source's policy before it is stored.
/// Unlike the LLM seam we do NOT redact what is *sent* — the candidate's name
/// is the query — but a stray email must not land in an append-only table.
async fn audit_exa_attempt(
    audit: Option<&ExaAudit>,
    source: crate::audit::EgressSource,
    request: &str,
    outcome: &crate::audit::EgressOutcome,
) {
    let Some(audit) = audit else { return };

    let redacted = crate::pii::scan_and_redact_with(request, source.redaction_policy());
    let ctx = crate::audit::EgressAudit {
        source,
        conversation_id: None,
        employee_ids: vec![],
        query_category: None,
    };

    if let Err(e) =
        crate::audit::record_egress(&audit.pool, &ctx, &redacted.redacted_text, outcome).await
    {
        log::warn!("failed to audit exa egress ({}): {e}", source.as_str());
    }
}

/// POST a JSON body to an Exa endpoint and deserialize the response.
///
/// Shared by `search` / `search_with` / `get_contents` / `find_similar` — all
/// are the same request shape (x-api-key header, JSON body) and the same status
/// handling (401 → `InvalidKey`, 429 → `RateLimit`, other non-2xx → `Api`).
/// The caller fetches `api_key` from the Keychain (typically via
/// `keyring::get_provider_api_key(EXA_PROVIDER_ID)`) and is responsible for
/// translating `KeyringError::NotFound` into a higher-level "missing key"
/// condition — this function only sees the key as an opaque `&str`.
///
/// FHR-91: every attempt past this point writes an audit row — success or
/// failure. `request_desc` is the human-readable descriptor of what left the
/// machine (the query, or the URLs fetched).
async fn exa_post<B, R>(
    url: &str,
    api_key: &str,
    body: &B,
    audit: Option<&ExaAudit>,
    source: crate::audit::EgressSource,
    request_desc: &str,
) -> Result<R, ExaError>
where
    B: Serialize,
    R: DeserializeOwned,
{
    use crate::audit::EgressOutcome;

    let response = match Client::new()
        .post(url)
        .header("x-api-key", api_key)
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            audit_exa_attempt(
                audit,
                source,
                request_desc,
                &EgressOutcome::Error {
                    partial_chars: 0,
                    error: e.to_string(),
                },
            )
            .await;
            return Err(e.into());
        }
    };

    let status = response.status();
    if status.is_success() {
        let text = response.text().await?;
        audit_exa_attempt(
            audit,
            source,
            request_desc,
            &EgressOutcome::Ok {
                response_chars: text.chars().count(),
            },
        )
        .await;
        return serde_json::from_str(&text).map_err(|e| ExaError::InvalidResponse(e.to_string()));
    }

    let status_code = status.as_u16();
    let body = response.text().await.unwrap_or_default();
    let err = match status_code {
        401 => ExaError::InvalidKey,
        429 => ExaError::RateLimit { message: body },
        _ => ExaError::Api {
            status: status_code,
            body,
        },
    };
    audit_exa_attempt(
        audit,
        source,
        request_desc,
        &EgressOutcome::Error {
            partial_chars: 0,
            error: err.to_string(),
        },
    )
    .await;
    Err(err)
}

/// Execute a search against Exa's `/search` endpoint.
pub async fn search(
    query: &str,
    api_key: &str,
    audit: Option<&ExaAudit>,
) -> Result<ExaSearchResponse, ExaError> {
    let body = ExaSearchRequest {
        query,
        num_results: DEFAULT_NUM_RESULTS,
        search_type: DEFAULT_SEARCH_TYPE,
    };
    exa_post(
        EXA_SEARCH_URL,
        api_key,
        &body,
        audit,
        crate::audit::EgressSource::ExaSearch,
        query,
    )
    .await
}

/// Fetch full document text for the given URLs via Exa's `/contents` endpoint.
/// Backs the intake `crawl_url` step — the caller hands us a URL it already
/// has (e.g. a company website the user pasted) and we return its text.
pub async fn get_contents(
    urls: &[String],
    api_key: &str,
    audit: Option<&ExaAudit>,
) -> Result<ExaSearchResponse, ExaError> {
    let body = ExaContentsRequest { urls, text: true };
    exa_post(
        EXA_CONTENTS_URL,
        api_key,
        &body,
        audit,
        crate::audit::EgressSource::ExaContents,
        &urls.join(", "),
    )
    .await
}

/// Find pages similar to a seed URL via Exa's `/findSimilar` endpoint. Feeds
/// team-seeded candidate discovery ("find people like these team members").
/// `excludeSourceDomain: true` prevents the seed's own domain from showing up
/// in results (mirrors the TS adapter fidelity requirement).
pub async fn find_similar(
    url: &str,
    api_key: &str,
    audit: Option<&ExaAudit>,
) -> Result<ExaSearchResponse, ExaError> {
    let body = ExaFindSimilarRequest {
        url,
        num_results: DEFAULT_NUM_RESULTS,
        exclude_source_domain: true,
    };
    exa_post(
        EXA_FIND_SIMILAR_URL,
        api_key,
        &body,
        audit,
        crate::audit::EgressSource::ExaFindSimilar,
        url,
    )
    .await
}

/// Full-fidelity search: passes per-query result caps + domain filters +
/// category to the wire (params the 1-arg `search` drops). Used by the
/// tiered discovery executor (S1.1).
pub async fn search_with(
    query: &str,
    opts: &ExaSearchOpts<'_>,
    api_key: &str,
    audit: Option<&ExaAudit>,
) -> Result<ExaSearchResponse, ExaError> {
    let body = ExaSearchRequestFull::from_parts(query, opts);
    exa_post(
        EXA_SEARCH_URL,
        api_key,
        &body,
        audit,
        crate::audit::EgressSource::ExaSearch,
        query,
    )
    .await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod audit_tests {
    use super::*;
    use crate::audit::{EgressOutcome, EgressSource};

    async fn test_pool() -> crate::db::DbPool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect :memory: pool");
        crate::db::run_migrations_for_tests(&pool)
            .await
            .expect("migrations");
        pool
    }

    #[tokio::test]
    async fn exa_attempt_writes_audit_row() {
        let pool = test_pool().await;
        let audit = ExaAudit::new(pool.clone());
        audit_exa_attempt(
            Some(&audit),
            EgressSource::ExaSearch,
            "staff engineer at Acme",
            &EgressOutcome::Ok { response_chars: 10 },
        )
        .await;

        let (source, request): (String, String) = sqlx::query_as(
            "SELECT source, request_redacted FROM audit_log ORDER BY rowid DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("exa search must leave an audit row");
        assert_eq!(source, "exa_search");
        assert_eq!(request, "staff engineer at Acme");
    }

    #[tokio::test]
    async fn exa_attempt_redacts_email_in_audited_request() {
        let pool = test_pool().await;
        let audit = ExaAudit::new(pool.clone());
        audit_exa_attempt(
            Some(&audit),
            EgressSource::ExaContents,
            "https://x.com/p?contact=sarah.chen@acme.com",
            &EgressOutcome::Ok { response_chars: 1 },
        )
        .await;

        let request: String = sqlx::query_scalar(
            "SELECT request_redacted FROM audit_log ORDER BY rowid DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            !request.contains("sarah.chen@acme.com"),
            "candidate email persisted to the append-only audit log: {request}"
        );
        assert!(request.contains("[EMAIL_REDACTED]"));
    }

    #[tokio::test]
    async fn exa_attempt_without_audit_context_is_a_noop() {
        // The adapter is also used from contexts with no DB pool (tests,
        // future CLI); absence of a pool must not panic or block the request.
        audit_exa_attempt(
            None,
            EgressSource::ExaSearch,
            "query",
            &EgressOutcome::Ok { response_chars: 0 },
        )
        .await;
    }
}

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
        assert_eq!(
            hit.title.as_deref(),
            Some("Acme — Developer Infrastructure")
        );
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
        assert_eq!(
            hit.score,
            Some(0.91),
            "similarity score drives SimilarResult"
        );
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
        let body = ExaFindSimilarRequest {
            url: "https://x.com",
            num_results: 10,
            exclude_source_domain: true,
        };
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["url"], "https://x.com");
        assert_eq!(
            v["numResults"], 10,
            "num_results must serialize as numResults"
        );
    }

    #[test]
    fn search_with_request_serializes_all_optional_params_camelcase() {
        let domains = vec!["linkedin.com".to_string()];
        let excl = vec!["pinterest.com".to_string()];
        let opts = ExaSearchOpts {
            num_results: 7,
            include_domains: Some(&domains),
            exclude_domains: Some(&excl),
            category: Some("people"),
        };
        let body = ExaSearchRequestFull::from_parts("staff rust engineer", &opts);
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["query"], "staff rust engineer");
        assert_eq!(v["numResults"], 7);
        assert_eq!(v["includeDomains"][0], "linkedin.com");
        assert_eq!(v["excludeDomains"][0], "pinterest.com");
        assert_eq!(v["category"], "people");
        assert_eq!(v["type"], "auto");
    }

    #[test]
    fn search_with_omits_none_params() {
        let opts = ExaSearchOpts {
            num_results: 10,
            include_domains: None,
            exclude_domains: None,
            category: None,
        };
        let body = ExaSearchRequestFull::from_parts("q", &opts);
        let v = serde_json::to_value(&body).unwrap();
        assert!(
            v.get("includeDomains").is_none(),
            "None include_domains must be omitted"
        );
        assert!(v.get("excludeDomains").is_none());
        assert!(v.get("category").is_none());
        assert_eq!(v["numResults"], 10);
    }

    #[test]
    fn find_similar_request_carries_exclude_source_domain() {
        let body = ExaFindSimilarRequest {
            url: "https://x.com/a",
            num_results: 10,
            exclude_source_domain: true,
        };
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(
            v["excludeSourceDomain"], true,
            "fidelity: findSimilar must exclude the seed's own domain"
        );
    }

    #[test]
    fn response_deserializes_cost_dollars_total() {
        let json =
            r#"{"results":[{"id":"a","url":"https://e.com"}],"costDollars":{"total":0.012}}"#;
        let parsed: ExaSearchResponse = serde_json::from_str(json).expect("parse with costDollars");
        assert_eq!(parsed.cost_dollars.as_ref().unwrap().total, 0.012);
    }

    #[test]
    fn existing_sparse_fixture_still_parses_without_cost_dollars() {
        let parsed: ExaSearchResponse =
            serde_json::from_str(FIXTURE_SPARSE).expect("sparse still parses");
        assert!(
            parsed.cost_dollars.is_none(),
            "costDollars is Option, absent in sparse fixture"
        );
    }
}
