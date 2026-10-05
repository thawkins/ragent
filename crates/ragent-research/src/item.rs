//! `ResearchItem` - the central data structure backing a `research/<name>/`
//! directory.
//!
//! Each `ResearchItem` corresponds 1:1 with a directory under `research/` and
//! owns the frontmatter, status, and source list that appear in the rendered
//! `RESEARCH.md`. The struct is the source of truth required by FR-005.
//!
//! ## YAML frontmatter
//!
//! A `ResearchItem` serializes to the YAML frontmatter block that lives at
//! the top of every `RESEARCH.md`. The shape is intentionally simple so the
//! serializer round-trips cleanly through `serde_yaml` (when the feature is
//! enabled by the caller) and `serde_json`.
//!
//! ```yaml
//! ---
//! name: rust-async
//! title: Rust Async Patterns
//! topic: async/await idioms in stable Rust
//! status: draft
//! created: 2024-01-15T10:30:00Z
//! modified: 2024-01-15T10:30:00Z
//! sources: []
//! ---
//! ```
//!
//! ## Mutability contract
//!
//! The setters (`set_status`, `set_title`, `add_source`) all bump the
//! `modified` timestamp automatically; callers never have to do that by
//! hand. This keeps the FR-005 "update modified timestamp on every write"
//! rule intact without leaking it into every call site.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::research_name::ResearchName;
use crate::source::Source;
use crate::status::ResearchStatus;

/// A single research item: `research/<name>/RESEARCH.md` plus its metadata.
///
/// The fields mirror the FR-005 frontmatter requirements:
///
/// - `name` - the validated URL-safe identifier (also the directory name).
/// - `title` - a human-readable title for display and search.
/// - `topic` - the original topic description that triggered the research.
/// - `status` - the lifecycle state (see [`ResearchStatus`]).
/// - `created_at` / `modified_at` - UTC timestamps for the FR-005
///   "created/modified" frontmatter fields.
/// - `sources` - the FR-011 References Index rows (empty until gathering
///   has captured at least one source).
/// - `output_format` - the output artifact requested via `--format` (FR-012).
/// - `model` - the LLM model used to perform the analysis, when an
///   LLM-backed analysis engine was wired in.
/// - `url_cloak` - whether source URLs were defanged in the rendered report
///   (`/research create --url-cloak`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchItem {
    /// Validated URL-safe identifier; also the directory name under `research/`.
    pub name: ResearchName,
    /// Human-readable title shown in `/research list` and the markdown header.
    pub title: String,
    /// Free-form topic description that originally triggered the research.
    pub topic: String,
    /// Lifecycle status - see [`ResearchStatus`].
    pub status: ResearchStatus,
    /// UTC timestamp at which the item was created.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp of the most recent edit; bumped by every mutating method.
    pub modified_at: DateTime<Utc>,
    /// Sources backing the References Index block in `RESEARCH.md`.
    pub sources: Vec<Source>,
    /// Sub-queries used by the web-gathering phase. Persisted in frontmatter so
    /// the `RESEARCH.md` Search Queries section survives reloads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queries: Vec<String>,
    /// Output artifact requested via `--format` (FR-012). Persisted in frontmatter
    /// so the rendered document reflects the original request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    /// Model used to perform the analysis (e.g. `anthropic/claude-sonnet-4`).
    /// Persisted in frontmatter as `Model:` so every `RESEARCH.md` records
    /// which model produced its synthesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Whether open-access recovery was enabled for this research run.
    ///
    /// When `true`, the frontmatter discloses that the report may contain
    /// sources whose full text was recovered from a legal OA copy rather than
    /// read directly from the original paywalled URL (FR-015).
    #[serde(default)]
    pub open_access_recovery: bool,
    /// Whether `--url-cloak` was enabled for this research run.
    ///
    /// When `true`, the `Sources` bullets and the `References Index` /
    /// `Sources Reference` tables in `RESEARCH.md` and `CORPA.md` emit web
    /// URLs defanged (`hxxps://` scheme + `[.]` dots, wrapped in a code span)
    /// so automated URL scanners do not treat them as live links.
    #[serde(default)]
    pub url_cloak: bool,
    /// Verbatim front-end invocation (e.g. `ragent research create --name x
    /// "topic" --tier full`) recorded in frontmatter so a future
    /// `/research update` command can replay the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation: Option<String>,
}

impl ResearchItem {
    /// Create a fresh `ResearchItem` in the `Draft` state.
    ///
    /// `created_at` and `modified_at` are both set to `now` (UTC). The new
    /// item has no sources yet - gathering is expected to populate the
    /// `sources` vec via [`ResearchItem::add_source`] (or by mutating the
    /// field directly inside the crate).
    pub fn new(name: ResearchName, title: impl Into<String>, topic: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            name,
            title: title.into(),
            topic: topic.into(),
            status: ResearchStatus::Draft,
            created_at: now,
            modified_at: now,
            sources: Vec::new(),
            queries: Vec::new(),
            output_format: None,
            model: None,
            open_access_recovery: false,
            url_cloak: false,
            invocation: None,
        }
    }

    /// Replace the stored sub-queries and bump `modified_at`.
    pub fn set_queries(&mut self, queries: Vec<String>) -> &mut Self {
        self.queries = queries;
        self.touch();
        self
    }

    /// Update the lifecycle status and bump `modified_at`.
    ///
    /// Returns `&mut self` so callers can chain.
    pub fn set_status(&mut self, status: ResearchStatus) -> &mut Self {
        self.status = status;
        self.touch();
        self
    }

    /// Update the human-readable title and bump `modified_at`.
    pub fn set_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.title = title.into();
        self.touch();
        self
    }

    /// Append a captured source and bump `modified_at`.
    ///
    /// Sources appear in the References Index in insertion order, so the
    /// gathering phase should call this in the order it captures evidence.
    pub fn add_source(&mut self, source: Source) -> &mut Self {
        self.sources.push(source);
        self.touch();
        self
    }

    /// Number of sources currently captured.
    #[must_use]
    pub const fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// `true` if the item has at least one captured source.
    #[must_use]
    pub const fn has_sources(&self) -> bool {
        !self.sources.is_empty()
    }

    /// Bump `modified_at` to `now` without touching any other field.
    ///
    /// Visible to the crate so callers that mutate `sources` directly (e.g.
    /// the gathering engine) can still honour the FR-005 "update modified
    /// timestamp on every write" rule.
    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
    }

    /// Render the `RESEARCH.md` frontmatter block as clean YAML.
    ///
    /// The block uses standard YAML key/value syntax so it parses correctly
    /// in any Markdown/YAML previewer:
    ///
    /// ```text
    /// ---
    /// name: rust-async
    /// title: "Rust Async Patterns"
    /// topic: "async/await idioms"
    /// Model: "anthropic/claude-sonnet-4"
    /// status: draft
    /// created: 2024-01-15T10:30:00Z
    /// modified: 2024-01-15T10:30:00Z
    /// sources: 0 # see sources/ subdirectory
    /// queries: []
    /// ---
    /// ```
    ///
    /// Strings are wrapped in double quotes and internal double quotes are
    /// escaped so titles and topics containing colons or quotes still parse.
    /// The output still round-trips through [`ResearchItem::from_frontmatter`].
    #[must_use]
    pub fn render_frontmatter(&self) -> String {
        let mut out = String::from("---\n");
        out.push_str(&format!("name: {}\n", self.name.as_str()));
        out.push_str(&format!(
            "title: \"{}\"\n",
            strip_control_chars(&self.title)
                .replace(['\n', '\r'], " ")
                .replace('\"', "\\\"")
        ));
        out.push_str(&format!(
            "topic: \"{}\"\n",
            strip_control_chars(&self.topic)
                .replace(['\n', '\r'], " ")
                .replace('\"', "\\\"")
        ));
        if let Some(model) = &self.model {
            out.push_str(&format!(
                "Model: \"{}\"\n",
                strip_control_chars(model)
                    .replace(['\n', '\r'], " ")
                    .replace('\"', "\\\"")
            ));
        }
        out.push_str(&format!("status: {}\n", self.status));
        out.push_str(&format!("created: {}\n", self.created_at.to_rfc3339()));
        out.push_str(&format!("modified: {}\n", self.modified_at.to_rfc3339()));
        out.push_str(&format!("sources: {}\n", sources_count(&self.sources)));
        if self.queries.is_empty() {
            out.push_str("queries: []\n");
        } else {
            out.push_str("queries:\n");
            for q in &self.queries {
                out.push_str(&format!(
                    "  - \"{}\"\n",
                    strip_control_chars(q)
                        .replace(['\n', '\r'], " ")
                        .replace('\"', "\\\"")
                ));
            }
        }
        if let Some(fmt) = &self.output_format {
            out.push_str(&format!("requested_format: {fmt}\n"));
        }
        if self.open_access_recovery {
            out.push_str("open_access_recovery: true\n");
        }
        if self.url_cloak {
            out.push_str("url_cloak: true\n");
        }
        if let Some(inv) = &self.invocation {
            out.push_str(&format!(
                "invocation: \"{}\"\n",
                strip_control_chars(inv)
                    .replace(['\n', '\r'], " ")
                    .replace('"', "\\\"")
            ));
        }
        out.push_str("---\n\n");
        out
    }

    /// Build a `ResearchItem` from a raw frontmatter block string.
    ///
    /// Accepts the output of [`ResearchItem::render_frontmatter`] and remains
    /// backward compatible with the older plain YAML key/value format. Unknown
    /// fields are tolerated; missing required fields produce an error
    /// explaining which one was absent.
    ///
    /// `sources` is intentionally parsed as a count rather than a list -
    /// the full source list is loaded by the IO layer from
    /// `sources/web-NN.md` / `sources/local-NN.md` siblings and merged in
    /// after construction. This keeps the frontmatter compact.
    pub fn from_frontmatter(block: &str) -> Result<Self, ResearchItemError> {
        // Strip leading/trailing "---" fence if present.
        let trimmed = block.trim();
        let inner = trimmed
            .strip_prefix("---")
            .and_then(|s| s.strip_suffix("---"))
            .unwrap_or(trimmed);
        let inner = inner.trim();

        let mut fields: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut queries: Vec<String> = Vec::new();
        let mut current_key: Option<String> = None;
        let mut current_value: Vec<String> = Vec::new();

        fn commit_field(
            key: &mut Option<String>,
            value: &mut Vec<String>,
            fields: &mut std::collections::HashMap<String, String>,
            queries: &mut Vec<String>,
        ) {
            if let Some(k) = key.take() {
                if k == "queries" {
                    for v in value.drain(..) {
                        let v = v.trim();
                        if v == "[]" {
                            queries.clear();
                        } else if let Some(item) = v.strip_prefix("- ") {
                            let item = item.trim();
                            if !item.is_empty() {
                                queries.push(unquote_yaml_scalar(item));
                            }
                        }
                    }
                } else {
                    fields.insert(k, value.join(" ").trim().to_string());
                    value.clear();
                }
            }
        }

        for raw_line in inner.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                commit_field(
                    &mut current_key,
                    &mut current_value,
                    &mut fields,
                    &mut queries,
                );
                continue;
            }

            if let Some((key, inline_value)) = parse_frontmatter_label(line) {
                commit_field(
                    &mut current_key,
                    &mut current_value,
                    &mut fields,
                    &mut queries,
                );
                current_key = Some(key.clone());
                if key == "queries" {
                    if inline_value == "[]" {
                        queries.clear();
                        current_key = None;
                    }
                    // Non-empty inline query values are not expected; list items follow.
                } else {
                    current_value.push(inline_value);
                }
            } else if current_key.is_some() {
                current_value.push(line.to_string());
            }
        }
        commit_field(
            &mut current_key,
            &mut current_value,
            &mut fields,
            &mut queries,
        );

        let name = fields
            .remove("name")
            .ok_or(ResearchItemError::MissingField("name".to_string()))?;
        let name = ResearchName::try_new(name).map_err(ResearchItemError::InvalidName)?;
        let title = fields
            .remove("title")
            .ok_or(ResearchItemError::MissingField("title".to_string()))?;
        let topic = fields.remove("topic").unwrap_or_default();
        let status = fields
            .remove("status")
            .map(|v| {
                ResearchStatus::parse(&v).ok_or_else(|| ResearchItemError::InvalidStatus(v.clone()))
            })
            .transpose()?
            .unwrap_or_default();
        let created_at = match fields.remove("created") {
            Some(v) => DateTime::parse_from_rfc3339(&v)
                .map_err(|e| ResearchItemError::InvalidTimestamp {
                    field: "created".to_string(),
                    source: e.to_string(),
                })?
                .with_timezone(&Utc),
            None => Utc::now(),
        };
        let modified_at = match fields.remove("modified") {
            Some(v) => DateTime::parse_from_rfc3339(&v)
                .map_err(|e| ResearchItemError::InvalidTimestamp {
                    field: "modified".to_string(),
                    source: e.to_string(),
                })?
                .with_timezone(&Utc),
            None => created_at,
        };
        let _ = fields.remove("sources"); // INTENTIONAL: optional field removal, absent is fine
        let output_format = fields.remove("requested_format");
        let model = fields.remove("model").map(|v| unquote_yaml_scalar(&v));
        let open_access_recovery = frontmatter_bool(&mut fields, "open_access_recovery");
        let url_cloak = frontmatter_bool(&mut fields, "url_cloak");
        let invocation = fields.remove("invocation").map(|v| unquote_yaml_scalar(&v));

        Ok(Self {
            name,
            title: unquote_yaml_scalar(&title),
            topic: unquote_yaml_scalar(&topic),
            status,
            created_at,
            modified_at,
            sources: Vec::new(),
            queries,
            output_format,
            model,
            open_access_recovery,
            url_cloak,
            invocation,
        })
    }
}

/// Errors that can occur while parsing a `ResearchItem` from a frontmatter block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchItemError {
    /// A required field was missing from the frontmatter block.
    MissingField(String),
    /// The `name` field did not satisfy FR-002.
    InvalidName(crate::research_name::ResearchNameError),
    /// The `status` field contained an unknown value.
    InvalidStatus(String),
    /// A timestamp field could not be parsed as RFC-3339.
    InvalidTimestamp {
        /// Field name (`"created"` or `"modified"`).
        field: String,
        /// Underlying parse error.
        source: String,
    },
}

impl std::fmt::Display for ResearchItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => {
                write!(f, "research frontmatter missing required field: {field}")
            }
            Self::InvalidName(e) => write!(f, "invalid research name: {e}"),
            Self::InvalidStatus(s) => write!(f, "unknown research status: '{s}'"),
            Self::InvalidTimestamp { field, source } => {
                write!(f, "invalid {field} timestamp: {source}")
            }
        }
    }
}

impl std::error::Error for ResearchItemError {}

/// Parse a single frontmatter label line.
///
/// Supports the current markdown style (`**label:** value`), the legacy
/// italic style (`*label:* value`), and the older plain YAML style
/// (`label: value`). Returns the lowercase label and the inline value
/// (which may be empty for list-valued fields such as `queries`).
fn parse_frontmatter_label(line: &str) -> Option<(String, String)> {
    let line = line.trim();

    // Bold markdown label: **label:** rest
    if let Some(rest) = line.strip_prefix("**")
        && let Some((label, value)) = rest.split_once(":**")
    {
        return Some((label.trim().to_lowercase(), value.trim().to_string()));
    }

    // Italic markdown label: *label:* rest
    if let Some(rest) = line.strip_prefix("*")
        && let Some((label, value)) = rest.split_once(":*")
    {
        return Some((label.trim().to_lowercase(), value.trim().to_string()));
    }

    // Plain YAML-style key: value.
    if let Some((key, value)) = line.split_once(':') {
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/')
        {
            return Some((key.to_lowercase(), value.to_string()));
        }
    }

    None
}

/// Reverse of the legacy [`yaml_scalar`] for the frontmatter parser. Strips
/// surrounding double quotes (and unescapes `\\` and `\"`) if present.
fn unquote_yaml_scalar(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(inner) = trimmed.strip_prefix('"')
        && let Some(inner) = inner.strip_suffix('"')
    {
        return inner.replace(r#"\""#, "\"").replace(r"\\", "\\");
    }
    trimmed.to_string()
}

/// Parse a boolean frontmatter field (`key: true`): missing or any value
/// other than case-insensitive `"true"` counts as false. Shared by the
/// boolean flags (`open_access_recovery`, `url_cloak`).
fn frontmatter_bool(fields: &mut std::collections::HashMap<String, String>, key: &str) -> bool {
    fields
        .remove(key)
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Render the `sources:` line as a comment-style count placeholder. The
/// detailed source list is loaded from supporting files by the IO layer;
/// the frontmatter just records the count for at-a-glance inspection.
fn sources_count(sources: &[Source]) -> String {
    format!("{} # see sources/ subdirectory", sources.len())
}

/// Strip non-printable control characters that would corrupt `RESEARCH.md`.
///
/// LLM text streams and PDF extraction can emit C0 control codes
/// (0x00-0x1F), DEL (0x7F), and C1 control codes (0x80-0x9F). These make
/// markdown editors detect the file as binary. Newlines (`\n`) and tabs
/// (`\t`) are preserved so multi-line findings render correctly; carriage
/// returns are collapsed to spaces.
///
/// Defence-in-depth: the analysis parser and document assembler both call
/// this so binary never reaches disk even when a provider returns garbage
/// tokens or a PDF extractor emits raw font-encoding bytes.
#[must_use]
pub(crate) fn strip_control_chars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\n' | '\t' => out.push(ch),
            '\r' => out.push(' '),
            c if c.is_control() => continue,
            _ => out.push(ch),
        }
    }
    out
}

/// Maximum length of a derived research item title, in characters.
///
/// Topics can be long free-form descriptions; using the whole string verbatim
/// as a title produces unwieldy headers in `/research list` and
/// `RESEARCH.md`. We cap at a generous width and break on a word boundary so
/// the title stays scannable while still summarising the topic content.
pub const DERIVED_TITLE_MAX_CHARS: usize = 100;

/// Maximum length of the final `title` field written to `RESEARCH.md`
/// frontmatter. The title is derived from the summary so the displayed
/// headline reflects the actual synthesis rather than the original prompt.
pub const RESEARCH_TITLE_MAX_CHARS: usize = 80;

/// Derive a human-readable research item title from the user-supplied topic.
///
/// When `topic` is non-empty, the full topic is used (trimmed) so the title
/// actually summarises the research subject rather than being truncated to its
/// first word. Extremely long topics are capped at [`DERIVED_TITLE_MAX_CHARS`]
/// characters on a word boundary with a trailing ellipsis.
///
/// When `topic` is empty, fall back to `from_url` (the `--from-url` case where
/// the session later derives the real topic from the fetched page), and
/// finally to `"Research"` when neither is available.
///
/// This is the single source of truth used by the CLI, TUI, and HTTP server
/// entry points so all three produce identical titles for the same inputs.
pub fn derive_title(topic: &str, from_url: Option<&str>) -> String {
    derive_title_full(topic, from_url, None)
}

/// Derive a research item title with an optional `--from-file` path fallback.
///
/// Same as [`derive_title`] but also accepts a `from_file` path that is used
/// when neither `topic` nor `from_url` is available, so the title reflects the
/// seed document rather than defaulting to `"Research"`.
#[must_use]
pub fn derive_title_full(topic: &str, from_url: Option<&str>, from_file: Option<&str>) -> String {
    derive_title_files(
        topic,
        from_url,
        &from_file
            .map(std::string::ToString::to_string)
            .into_iter()
            .collect::<Vec<_>>(),
    )
}

/// Derive a research item title with optional repeatable `--from-file` paths.
///
/// Same as [`derive_title`] but accepts zero or more local file paths. The
/// first non-empty path is used as the title fallback when no topic or
/// `--from-url` is supplied.
#[must_use]
pub fn derive_title_files(topic: &str, from_url: Option<&str>, from_files: &[String]) -> String {
    let trimmed = topic.trim();
    if !trimmed.is_empty() {
        return cap_title(trimmed, DERIVED_TITLE_MAX_CHARS);
    }
    if let Some(url) = from_url.map(str::trim).filter(|s| !s.is_empty()) {
        return url.to_string();
    }
    if let Some(path) = from_files
        .iter()
        .map(String::as_str)
        .find(|s| !s.trim().is_empty())
    {
        return path.to_string();
    }
    "Research".to_string()
}

/// Reduce `text` to a title suitable for the `RESEARCH.md` frontmatter.
///
/// The returned string is capped at [`RESEARCH_TITLE_MAX_CHARS`] characters
/// and broken on a word boundary, with a trailing ellipsis when truncation
/// occurs. A single over-long word is hard-truncated rather than overflowing.
#[must_use]
pub fn truncate_title(text: &str) -> String {
    cap_title(text.trim(), RESEARCH_TITLE_MAX_CHARS)
}

/// Cap `title` to `limit` characters on a word boundary, appending an
/// ellipsis when truncation occurs. A single over-long word is hard-truncated
/// rather than overflowing.
fn cap_title(title: &str, limit: usize) -> String {
    if title.chars().count() <= limit {
        return title.to_string();
    }
    // Walk char indices up to the limit, then roll back to the last whitespace
    // so we don't split a word in half.
    let limit_byte = title
        .char_indices()
        .take(limit)
        .last()
        .map_or(title.len(), |(i, c)| i + c.len_utf8());
    let head = &title[..limit_byte];
    let cut = head
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(limit_byte, |(i, _)| i);
    let mut out = title[..cut].trim_end().to_string();
    out.push_str("...");
    out
}

#[cfg(test)]
#[path = "../tests/inline/item_tests.rs"]
mod tests;
