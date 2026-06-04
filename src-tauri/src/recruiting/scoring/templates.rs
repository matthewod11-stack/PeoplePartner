//! Prompt-template loader for the scoring engine — a faithful Rust port of the
//! Sourcerer TS `packages/ai/src/template-loader.ts`.
//!
//! The three `scoring-*.md` templates are vendored byte-for-byte from the TS
//! reference and embedded via `include_str!`. Loading parses the YAML
//! front-matter (`name` / `version` / `changelog`) and interpolates
//! `{{variable}}` placeholders. Error messages mirror the TS originals so the
//! behavior is diff-comparable against the reference.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

/// `{{variableName}}` placeholder — `\w+` keys only, mirroring the TS
/// `interpolate` regex `/\{\{(\w+)\}\}/g`. Placeholders with non-word
/// characters (e.g. `{{foo-bar}}`) are left untouched, exactly as in TS.
static PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{(\w+)\}\}").expect("placeholder regex should compile"));

/// Context map for template interpolation (TS `TemplateContext`).
pub type TemplateContext = HashMap<String, String>;

/// Errors raised while parsing or rendering a prompt template. Messages are
/// kept close to the TS reference's thrown errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("Missing template variables: {0}")]
    MissingVariables(String),
    #[error("Prompt template {0} is missing YAML front-matter")]
    MissingFrontMatter(String),
    #[error("Prompt template {0} has unterminated YAML front-matter")]
    UnterminatedFrontMatter(String),
    #[error("Invalid front-matter line in prompt template {name}: {line}")]
    InvalidFrontMatterLine { name: String, line: String },
    #[error("Prompt template {0} front-matter name must be \"{0}\"")]
    NameMismatch(String),
    #[error("Prompt template {0} front-matter version must be a positive integer")]
    InvalidVersion(String),
    #[error("Prompt template {0} front-matter changelog is required")]
    MissingChangelog(String),
}

/// Parsed YAML front-matter (TS `PromptMetadata`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptMetadata {
    pub name: String,
    pub version: u32,
    pub changelog: String,
}

/// A loaded template: its front-matter plus the body (TS `PromptTemplate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplate {
    pub metadata: PromptMetadata,
    pub content: String,
}

/// Parse a raw `.md` template (YAML front-matter + body) into metadata and
/// content. Faithful to the TS `parsePromptTemplate`: requires a leading
/// `---\n ... \n---\n` block, validates `name` matches, `version` is a positive
/// integer, and `changelog` is present.
pub fn parse_prompt_template(name: &str, raw: &str) -> Result<PromptTemplate, TemplateError> {
    let after_open = raw
        .strip_prefix("---\n")
        .ok_or_else(|| TemplateError::MissingFrontMatter(name.to_string()))?;
    let end = after_open
        .find("\n---\n")
        .ok_or_else(|| TemplateError::UnterminatedFrontMatter(name.to_string()))?;
    let front_matter = &after_open[..end];
    let body = &after_open[end + "\n---\n".len()..];
    // TS strips a single leading newline from the body (`.replace(/^\n/, '')`).
    let content = body.strip_prefix('\n').unwrap_or(body).to_string();

    let metadata = parse_front_matter(name, front_matter)?;
    Ok(PromptTemplate { metadata, content })
}

/// The scoring prompt templates, vendored byte-for-byte from the Sourcerer TS
/// reference (`packages/ai/prompts/`) and embedded via `include_str!`. The
/// MS2 pipeline references these by variant rather than string name so a typo
/// is a compile error, not a runtime "template not found".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoringPrompt {
    /// `scoring-signal-extract.md` — per-candidate signal extraction (S2.2).
    SignalExtract,
    /// `scoring-narrative.md` — per-candidate narrative brief (S2.3).
    Narrative,
    /// `scoring-batch.md` — whole-slate batch scoring (experimental).
    Batch,
}

impl ScoringPrompt {
    /// The template's canonical name — must equal the `name:` front-matter key.
    pub fn template_name(self) -> &'static str {
        match self {
            ScoringPrompt::SignalExtract => "scoring-signal-extract",
            ScoringPrompt::Narrative => "scoring-narrative",
            ScoringPrompt::Batch => "scoring-batch",
        }
    }

    /// The raw, byte-identical template contents embedded at compile time.
    pub fn raw(self) -> &'static str {
        match self {
            ScoringPrompt::SignalExtract => include_str!("prompts/scoring-signal-extract.md"),
            ScoringPrompt::Narrative => include_str!("prompts/scoring-narrative.md"),
            ScoringPrompt::Batch => include_str!("prompts/scoring-batch.md"),
        }
    }

    /// Parse the embedded template into validated metadata + body.
    pub fn load(self) -> Result<PromptTemplate, TemplateError> {
        parse_prompt_template(self.template_name(), self.raw())
    }

    /// Load the template and interpolate `context` into its body. Returns the
    /// metadata alongside the rendered content (TS `renderTemplate`).
    pub fn render(self, context: &TemplateContext) -> Result<PromptTemplate, TemplateError> {
        let template = self.load()?;
        let content = interpolate(&template.content, context)?;
        Ok(PromptTemplate {
            metadata: template.metadata,
            content,
        })
    }
}

/// Parse the `key: value` lines of a front-matter block into validated
/// metadata. Faithful to the TS `parseFrontMatter`.
fn parse_front_matter(name: &str, front_matter: &str) -> Result<PromptMetadata, TemplateError> {
    let mut values: HashMap<String, String> = HashMap::new();
    for line in front_matter.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let colon = trimmed.find(':');
        let Some(colon) = colon else { continue };
        let key = trimmed[..colon].trim().to_string();
        let value = trimmed[colon + 1..]
            .trim()
            .trim_matches(|c| c == '\'' || c == '"')
            .to_string();
        values.insert(key, value);
    }

    // Validation order mirrors the TS `parseFrontMatter`.
    let parsed_name = values.get("name");
    if parsed_name.map(String::as_str) != Some(name) {
        return Err(TemplateError::NameMismatch(name.to_string()));
    }
    // A positive integer: reject 0, negatives, and non-numeric values.
    let version = match values.get("version").and_then(|v| v.parse::<u32>().ok()) {
        Some(v) if v >= 1 => v,
        _ => return Err(TemplateError::InvalidVersion(name.to_string())),
    };
    let changelog = match values.get("changelog") {
        Some(c) if !c.is_empty() => c.clone(),
        _ => return Err(TemplateError::MissingChangelog(name.to_string())),
    };

    Ok(PromptMetadata {
        name: name.to_string(),
        version,
        changelog,
    })
}

/// Interpolate `{{variableName}}` placeholders in a template string.
///
/// Faithful to the TS `interpolate`: substitutes every `{{word}}` whose key is
/// present in `context`; any placeholder with no value is collected and, if any
/// remain, the call fails with the missing keys (in first-seen order).
pub fn interpolate(template: &str, context: &TemplateContext) -> Result<String, TemplateError> {
    let mut missing: Vec<String> = Vec::new();
    let result = PLACEHOLDER.replace_all(template, |caps: &regex::Captures<'_>| {
        let key = &caps[1];
        match context.get(key) {
            Some(value) => value.clone(),
            None => {
                // Record each missing key once, in first-seen order, and leave
                // the placeholder literal (TS returns `{{key}}`).
                if !missing.iter().any(|m| m == key) {
                    missing.push(key.to_string());
                }
                caps[0].to_string()
            }
        }
    });

    if !missing.is_empty() {
        return Err(TemplateError::MissingVariables(missing.join(", ")));
    }
    Ok(result.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(pairs: &[(&str, &str)]) -> TemplateContext {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn interpolate_substitutes_known_placeholders() {
        let out = interpolate(
            "Hello {{name}}, you are a {{role}}.",
            &ctx(&[("name", "Sarah"), ("role", "staff engineer")]),
        )
        .expect("all variables present");
        assert_eq!(out, "Hello Sarah, you are a staff engineer.");
    }

    #[test]
    fn interpolate_errors_on_missing_variables() {
        let err = interpolate(
            "{{talentProfile}} then {{evidence}} then {{evidence}}",
            &ctx(&[("talentProfile", "P")]),
        )
        .expect_err("evidence is missing");
        // Faithful to TS: distinct missing keys, in first-seen order.
        assert_eq!(err, TemplateError::MissingVariables("evidence".to_string()));
    }

    #[test]
    fn interpolate_leaves_non_word_placeholders_untouched() {
        // The TS regex is `\{\{(\w+)\}\}` — `{{foo-bar}}` has a non-word char,
        // so it never matches and must survive verbatim (not error as missing).
        let out = interpolate("keep {{foo-bar}} as-is", &ctx(&[])).expect("no \\w+ placeholder");
        assert_eq!(out, "keep {{foo-bar}} as-is");
    }

    // ---- parse_prompt_template ----------------------------------------

    #[test]
    fn parse_prompt_template_extracts_metadata_and_body() {
        let raw = "---\nname: scoring-x\nversion: 2\nchangelog: v2 - did a thing\n---\n\nBody line one.\n";
        let parsed = parse_prompt_template("scoring-x", raw).expect("valid template");
        assert_eq!(parsed.metadata.name, "scoring-x");
        assert_eq!(parsed.metadata.version, 2);
        assert_eq!(parsed.metadata.changelog, "v2 - did a thing");
        // Leading newline after the closing fence is stripped (TS `.replace(/^\n/, '')`).
        assert_eq!(parsed.content, "Body line one.\n");
    }

    #[test]
    fn parse_prompt_template_rejects_name_mismatch() {
        let raw = "---\nname: other\nversion: 1\nchangelog: c\n---\nBody\n";
        let err = parse_prompt_template("scoring-x", raw).expect_err("name must match");
        assert_eq!(err, TemplateError::NameMismatch("scoring-x".to_string()));
    }

    #[test]
    fn parse_prompt_template_rejects_non_positive_version() {
        let raw = "---\nname: scoring-x\nversion: 0\nchangelog: c\n---\nBody\n";
        let err = parse_prompt_template("scoring-x", raw).expect_err("version must be >= 1");
        assert_eq!(err, TemplateError::InvalidVersion("scoring-x".to_string()));
    }

    #[test]
    fn parse_prompt_template_rejects_missing_changelog() {
        let raw = "---\nname: scoring-x\nversion: 1\n---\nBody\n";
        let err = parse_prompt_template("scoring-x", raw).expect_err("changelog required");
        assert_eq!(err, TemplateError::MissingChangelog("scoring-x".to_string()));
    }

    // ---- ScoringPrompt registry (vendored include_str! templates) ------

    #[test]
    fn signal_extract_template_loads_with_expected_metadata() {
        let t = ScoringPrompt::SignalExtract.load().expect("vendored template parses");
        assert_eq!(t.metadata.name, "scoring-signal-extract");
        assert_eq!(t.metadata.version, 2, "must track the v2 TS reference");
        // The body carries the grounding constraint that makes scoring honest.
        assert!(t.content.contains("CRITICAL GROUNDING CONSTRAINT"));
        assert!(t.content.contains("{{talentProfile}}"));
    }

    #[test]
    fn render_interpolates_a_vendored_template() {
        let rendered = ScoringPrompt::SignalExtract
            .render(&ctx(&[
                ("talentProfile", "Staff backend engineer"),
                ("evidence", "<evidence>built a payments ledger</evidence>"),
                ("evidenceIds", "ev-1\nev-2"),
            ]))
            .expect("all signal-extract variables supplied");
        assert!(
            rendered.content.contains("Staff backend engineer"),
            "talentProfile must be interpolated"
        );
        assert!(
            !rendered.content.contains("{{talentProfile}}"),
            "no placeholder may survive a successful render"
        );
        assert_eq!(rendered.metadata.name, "scoring-signal-extract");
    }

    /// Integrity guard: every vendored template parses, its front-matter `name`
    /// matches its variant, and the data-handling / grounding constraints that
    /// make scoring trustworthy are present. This is the in-repo proxy for the
    /// byte-identity `diff` against the TS reference (CI can't reach that repo);
    /// an accidental edit to a vendored `.md` trips this.
    #[test]
    fn all_vendored_templates_parse_and_carry_their_constraints() {
        for prompt in [
            ScoringPrompt::SignalExtract,
            ScoringPrompt::Narrative,
            ScoringPrompt::Batch,
        ] {
            let t = prompt.load().unwrap_or_else(|e| panic!("{} failed to load: {e}", prompt.template_name()));
            assert_eq!(t.metadata.name, prompt.template_name());
            assert!(t.metadata.version >= 1);
            assert!(
                t.content.contains("CRITICAL DATA-HANDLING CONSTRAINT"),
                "{} must keep the prompt-injection sandbox",
                prompt.template_name()
            );
            assert!(
                t.content.contains("{{talentProfile}}"),
                "{} must reference the talent profile",
                prompt.template_name()
            );
        }
    }
}
