//! Dependency seams for the intake engine. The only async/IO-touching surface;
//! tests inject the fakes in `testing`. Production impls (Exa-wired
//! `ContentResearch`, app-LLM `IntakeProvider`) are deferred follow-ups.

use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use super::node::NodeId;
use super::schemas::{CompanyIntel, CrawledContent, Message, ProfileAnalysis, ProfileInput, SimilarResult};

#[derive(Debug, Error)]
pub enum IntakeError {
    #[error("unknown node: {0:?}")]
    UnknownNode(NodeId),
    #[error("cannot submit response: conversation is done")]
    SubmitAfterDone,
    #[error("provider error: {0}")]
    Provider(String),
    #[error("research error: {0}")]
    Research(String),
    #[error("deserialize error: {0}")]
    Deserialize(String),
    #[error("unsupported conversation state version: {0}")]
    UnsupportedStateVersion(u32),
}

#[async_trait]
pub trait IntakeProvider: Send + Sync {
    /// Structured JSON output from the LLM. Returns raw `Value`; callers
    /// deserialize into their concrete type. `Value` (not generic `<T>`) keeps
    /// the trait object-safe.
    async fn structured_output(&self, messages: Vec<Message>, schema_name: &str)
        -> Result<serde_json::Value, IntakeError>;

    /// Free-text completion from the LLM — the raw assistant text, with no
    /// JSON-extraction post-processing. Scoring's narrative generator (S2.3)
    /// rides on this; signal/score extraction use `structured_output`. `model`
    /// optionally overrides the provider's default model for this call.
    ///
    /// (This trait is shared by intake and scoring — it is the recruiting
    /// module's single LLM seam, kept under the `IntakeProvider` name to scope
    /// the FHR-77 diff; a rename to a neutral `RecruitingLlm` is a clean
    /// follow-up.)
    async fn chat(&self, messages: Vec<Message>, model: Option<&str>)
        -> Result<String, IntakeError>;
}

#[async_trait]
pub trait ContentResearch: Send + Sync {
    async fn crawl_url(&self, url: &str) -> Result<CrawledContent, IntakeError>;
    async fn analyze_company(&self, content: &CrawledContent) -> Result<CompanyIntel, IntakeError>;
    async fn analyze_profile(&self, input: &ProfileInput) -> Result<ProfileAnalysis, IntakeError>;
    async fn find_similar(&self, urls: &[String]) -> Result<Vec<SimilarResult>, IntakeError>;
}

pub trait Clock: Send + Sync {
    fn now_iso8601(&self) -> String;
}

/// Real clock (used by production wiring, not tests).
pub struct SystemClock;
impl Clock for SystemClock {
    fn now_iso8601(&self) -> String { chrono::Utc::now().to_rfc3339() }
}

#[derive(Clone)]
pub struct IntakeDeps {
    pub provider: Arc<dyn IntakeProvider>,
    pub research: Arc<dyn ContentResearch>,
    pub clock: Arc<dyn Clock>,
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use std::sync::Mutex;

    /// Returns scripted JSON values (for `structured_output`) and scripted
    /// free text (for `chat`) in order; errors when the relevant queue is empty.
    pub struct FakeProvider {
        queue: Mutex<std::collections::VecDeque<serde_json::Value>>,
        text_queue: Mutex<std::collections::VecDeque<String>>,
    }
    impl FakeProvider {
        pub fn new(values: Vec<serde_json::Value>) -> Self {
            Self {
                queue: Mutex::new(values.into_iter().collect()),
                text_queue: Mutex::new(std::collections::VecDeque::new()),
            }
        }
        /// Script the free-text responses returned by `chat`, in order.
        pub fn with_texts(self, texts: Vec<String>) -> Self {
            Self {
                text_queue: Mutex::new(texts.into_iter().collect()),
                ..self
            }
        }
    }
    #[async_trait]
    impl IntakeProvider for FakeProvider {
        async fn structured_output(&self, _m: Vec<Message>, _s: &str)
            -> Result<serde_json::Value, IntakeError> {
            self.queue.lock().unwrap().pop_front()
                .ok_or_else(|| IntakeError::Provider("FakeProvider exhausted".into()))
        }
        async fn chat(&self, _m: Vec<Message>, _model: Option<&str>)
            -> Result<String, IntakeError> {
            self.text_queue.lock().unwrap().pop_front()
                .ok_or_else(|| IntakeError::Provider("FakeProvider text queue exhausted".into()))
        }
    }

    /// Canned research results.
    #[derive(Default)]
    pub struct FakeResearch {
        pub company: Option<CompanyIntel>,
        pub profiles: Vec<ProfileAnalysis>,
        pub similar: Vec<SimilarResult>,
    }
    #[async_trait]
    impl ContentResearch for FakeResearch {
        async fn crawl_url(&self, url: &str) -> Result<CrawledContent, IntakeError> {
            Ok(CrawledContent { url: url.into(), title: None, text: "fixture".into(),
                crawled_at: "2026-06-01T00:00:00Z".into(), adapter: "fake".into() })
        }
        async fn analyze_company(&self, content: &CrawledContent) -> Result<CompanyIntel, IntakeError> {
            self.company.clone().ok_or_else(|| IntakeError::Research("no canned company".into()))
                .map(|mut c| { c.url = content.url.clone(); c })
        }
        async fn analyze_profile(&self, _i: &ProfileInput) -> Result<ProfileAnalysis, IntakeError> {
            self.profiles.first().cloned()
                .ok_or_else(|| IntakeError::Research("no canned profile".into()))
        }
        async fn find_similar(&self, _u: &[String]) -> Result<Vec<SimilarResult>, IntakeError> {
            Ok(self.similar.clone())
        }
    }

    pub struct FixedClock(pub String);
    impl FixedClock { pub fn new(s: &str) -> Self { Self(s.into()) } }
    impl Clock for FixedClock { fn now_iso8601(&self) -> String { self.0.clone() } }

    /// Convenience: build `IntakeDeps` from fakes.
    pub fn fake_deps(provider: FakeProvider, research: FakeResearch) -> IntakeDeps {
        IntakeDeps {
            provider: Arc::new(provider),
            research: Arc::new(research),
            clock: Arc::new(FixedClock::new("2026-06-01T00:00:00Z")),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn fake_provider_returns_scripted_values() {
            let p = FakeProvider::new(vec![serde_json::json!({"title": "Eng"})]);
            let v = p.structured_output(vec![], "RoleParameters").await.unwrap();
            assert_eq!(v["title"], "Eng");
        }

        #[tokio::test]
        async fn fake_provider_errors_when_exhausted() {
            let p = FakeProvider::new(vec![]);
            assert!(p.structured_output(vec![], "X").await.is_err());
        }

        #[tokio::test]
        async fn fake_provider_returns_scripted_text_from_chat() {
            let p = FakeProvider::new(vec![]).with_texts(vec!["a candidate narrative".into()]);
            let out = p.chat(vec![], None).await.unwrap();
            assert_eq!(out, "a candidate narrative");
        }

        #[test]
        fn fixed_clock_is_deterministic() {
            let c = FixedClock::new("2026-06-01T00:00:00Z");
            assert_eq!(c.now_iso8601(), "2026-06-01T00:00:00Z");
        }
    }
}
