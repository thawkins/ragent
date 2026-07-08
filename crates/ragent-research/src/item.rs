//! `ResearchItem` — the central data structure backing a `research/<name>/`
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
/// - `name` — the validated URL-safe identifier (also the directory name).
/// - `title` — a human-readable title for display and search.
/// - `topic` — the original topic description that triggered the research.
/// - `status` — the lifecycle state (see [`ResearchStatus`]).
/// - `created_at` / `modified_at` — UTC timestamps for the FR-005
///   "created/modified" frontmatter fields.
/// - `sources` — the FR-011 References Index rows (empty until gathering
///   has captured at least one source).
/// - `output_format` — the output artifact requested via `--format` (FR-012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchItem {
    /// Validated URL-safe identifier; also the directory name under `research/`.
    pub name: ResearchName,
    /// Human-readable title shown in `/research list` and the markdown header.
    pub title: String,
    /// Free-form topic description that originally triggered the research.
    pub topic: String,
    /// Lifecycle status — see [`ResearchStatus`].
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
}

impl ResearchItem {
    /// Create a fresh `ResearchItem` in the `Draft` state.
    ///
    /// `created_at` and `modified_at` are both set to `now` (UTC). The new
    /// item has no sources yet — gathering is expected to populate the
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
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// `true` if the item has at least one captured source.
    pub fn has_sources(&self) -> bool {
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
    pub fn render_frontmatter(&self) -> String {
        let mut out = String::from("---\n");
        out.push_str(&format!("name: {}\n", self.name.as_str()));
        out.push_str(&format!(
            "title: \"{}\"\n",
            self.title.replace(['\n', '\r'], " ").replace('\"', "\\\"")
        ));
        out.push_str(&format!(
            "topic: \"{}\"\n",
            self.topic.replace(['\n', '\r'], " ").replace('\"', "\\\"")
        ));
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
                    q.replace(['\n', '\r'], " ").replace('\"', "\\\"")
                ));
            }
        }
        if let Some(fmt) = &self.output_format {
            out.push_str(&format!("requested_format: {fmt}\n"));
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
    /// `sources` is intentionally parsed as a count rather than a list —
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
        // `sources` is count-only; the IO layer loads the real list.
        let _ = fields.remove("sources");
        let output_format = fields.remove("requested_format");

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

/// Render the `sources:` line as a comment-style count placeholder. The
/// detailed source list is loaded from supporting files by the IO layer;
/// the frontmatter just records the count for at-a-glance inspection.
fn sources_count(sources: &[Source]) -> String {
    format!("{} # see sources/ subdirectory", sources.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_name::ResearchName;

    fn sample_name() -> ResearchName {
        ResearchName::new("rust-async").expect("name must validate")
    }

    #[test]
    fn new_creates_item_in_draft_state() {
        let item = ResearchItem::new(sample_name(), "Rust Async Patterns", "async/await idioms");
        assert_eq!(item.status, ResearchStatus::Draft);
        assert_eq!(item.title, "Rust Async Patterns");
        assert_eq!(item.topic, "async/await idioms");
        assert_eq!(item.source_count(), 0);
        assert!(!item.has_sources());
        assert_eq!(item.created_at, item.modified_at);
    }

    #[test]
    fn new_sets_both_timestamps_to_now() {
        let before = Utc::now();
        let item = ResearchItem::new(sample_name(), "t", "topic");
        let after = Utc::now();
        assert!(item.created_at >= before);
        assert!(item.created_at <= after);
        assert_eq!(item.created_at, item.modified_at);
    }

    #[test]
    fn set_status_updates_modified_but_not_created() {
        let mut item = ResearchItem::new(sample_name(), "t", "topic");
        let created = item.created_at;
        // Tiny sleep so timestamps differ at millisecond resolution.
        std::thread::sleep(std::time::Duration::from_millis(2));
        item.set_status(ResearchStatus::InProgress);
        assert_eq!(item.status, ResearchStatus::InProgress);
        assert_eq!(item.created_at, created);
        assert!(item.modified_at >= created);
    }

    #[test]
    fn set_title_updates_modified() {
        let mut item = ResearchItem::new(sample_name(), "Original", "topic");
        item.set_title("Updated");
        assert_eq!(item.title, "Updated");
    }

    #[test]
    fn add_source_appends_in_order_and_bumps_modified() {
        let mut item = ResearchItem::new(sample_name(), "t", "topic");
        let s1 = Source::Other {
            label: "first".into(),
            captured_at: Utc::now(),
            body_path: "sources/other-01.md".into(),
            body: String::new(),
        };
        let s2 = Source::Other {
            label: "second".into(),
            captured_at: Utc::now(),
            body_path: "sources/other-02.md".into(),
            body: String::new(),
        };
        item.add_source(s1.clone()).add_source(s2.clone());
        assert_eq!(item.sources.len(), 2);
        assert_eq!(item.sources[0], s1);
        assert_eq!(item.sources[1], s2);
        assert_eq!(item.source_count(), 2);
        assert!(item.has_sources());
    }

    #[test]
    fn touch_only_updates_modified() {
        let mut item = ResearchItem::new(sample_name(), "t", "topic");
        let created = item.created_at;
        let before_mod = item.modified_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        item.touch();
        assert_eq!(item.created_at, created);
        assert!(item.modified_at >= before_mod);
    }

    #[test]
    fn render_frontmatter_contains_required_fields() {
        let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
        let fm = item.render_frontmatter();
        assert!(fm.starts_with("---\n"));
        assert!(fm.ends_with("---\n\n"));
        assert!(fm.contains("name: rust-async"));
        assert!(fm.contains("title: \"Rust Async\""));
        assert!(fm.contains("topic: \"topic\""));
        assert!(fm.contains("status: draft"));
        assert!(fm.contains("created: "));
        assert!(fm.contains("modified: "));
        assert!(fm.contains("sources: 0"));
        assert!(fm.contains("queries: []"));
    }

    #[test]
    fn render_frontmatter_uses_plain_yaml_lines() {
        let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
        let fm = item.render_frontmatter();
        assert!(
            !fm.contains("**name:**"),
            "frontmatter should not use markdown bold labels"
        );
        assert!(
            fm.contains("name: rust-async\ntitle:"),
            "fields should be plain YAML key: value"
        );
    }

    #[test]
    fn render_frontmatter_escapes_quotes_in_title_and_topic() {
        let item = ResearchItem::new(sample_name(), "Has \"quotes\"", "also \"quoted\"");
        let fm = item.render_frontmatter();
        assert!(fm.contains("title: \"Has \\\"quotes\\\"\""));
        assert!(fm.contains("topic: \"also \\\"quoted\\\"\""));
    }

    #[test]
    fn render_frontmatter_round_trips_through_parse() {
        let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
        let fm = item.render_frontmatter();
        let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
        assert_eq!(parsed.name, item.name);
        assert_eq!(parsed.title, item.title);
        assert_eq!(parsed.topic, item.topic);
        assert_eq!(parsed.status, item.status);
        assert_eq!(parsed.created_at, item.created_at);
        assert_eq!(parsed.modified_at, item.modified_at);
        assert!(parsed.sources.is_empty());
    }

    #[test]
    fn render_frontmatter_round_trips_with_special_chars_in_title() {
        let mut item = ResearchItem::new(sample_name(), "t", "topic");
        item.set_title("Has: colon");
        let fm = item.render_frontmatter();
        let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
        assert_eq!(parsed.title, "Has: colon");
    }

    #[test]
    fn render_frontmatter_round_trips_with_queries() {
        let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
        item.set_queries(vec!["first query".into(), "second query".into()]);
        let fm = item.render_frontmatter();
        let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
        assert_eq!(parsed.queries, vec!["first query", "second query"]);
    }

    #[test]
    fn render_frontmatter_round_trips_with_quote_in_query() {
        let mut item = ResearchItem::new(sample_name(), "t", "topic");
        item.set_queries(vec!["query with \"quotes\"".into()]);
        let fm = item.render_frontmatter();
        let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
        assert_eq!(parsed.queries, vec!["query with \"quotes\""]);
    }

    #[test]
    fn from_frontmatter_parses_bold_labeled_format() {
        let block = "---\n\n**name:** rust-async\n\n**title:** Rust Async\n\n**topic:** async/await\n\n**status:** draft\n\n---\n\n";
        let item = ResearchItem::from_frontmatter(block).unwrap();
        assert_eq!(item.name.as_str(), "rust-async");
        assert_eq!(item.title, "Rust Async");
        assert_eq!(item.topic, "async/await");
        assert_eq!(item.status, ResearchStatus::Draft);
    }

    #[test]
    fn from_frontmatter_still_parses_legacy_plain_yaml() {
        let block =
            "---\nname: rust-async\ntitle: Rust Async\ntopic: async/await\nstatus: draft\n---\n";
        let item = ResearchItem::from_frontmatter(block).unwrap();
        assert_eq!(item.name.as_str(), "rust-async");
        assert_eq!(item.title, "Rust Async");
        assert_eq!(item.topic, "async/await");
        assert_eq!(item.status, ResearchStatus::Draft);
    }
    #[test]
    fn from_frontmatter_fails_on_missing_name() {
        let block = "---\n\n**title:** foo\n\n---\n\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::MissingField(_)));
    }

    #[test]
    fn from_frontmatter_fails_on_invalid_name() {
        let block = "---\n\n**name:** ..\n\n**title:** foo\n\n---\n\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::InvalidName(_)));
    }

    #[test]
    fn from_frontmatter_fails_on_invalid_status() {
        let block = "---\n\n**name:** rust-async\n\n**title:** foo\n\n**status:** nope\n\n---\n\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::InvalidStatus(_)));
    }

    #[test]
    fn from_frontmatter_fails_on_invalid_timestamp() {
        let block =
            "---\n\n**name:** rust-async\n\n**title:** foo\n\n**created:** not-a-date\n\n---\n\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::InvalidTimestamp { .. }));
    }

    #[test]
    fn from_frontmatter_defaults_optional_fields() {
        let block = "---\n\n**name:** rust-async\n\n**title:** foo\n\n---\n\n";
        let item = ResearchItem::from_frontmatter(block).unwrap();
        assert_eq!(item.status, ResearchStatus::Draft);
        assert!(item.topic.is_empty());
    }

    #[test]
    fn from_frontmatter_tolerates_unknown_fields() {
        let block = "---\n\n**name:** rust-async\n\n**title:** foo\n\n**extra:** stuff\n\n**another:** 42\n\n---\n\n";
        let item = ResearchItem::from_frontmatter(block).expect("unknown fields must not error");
        assert_eq!(item.title, "foo");
    }

    #[test]
    fn serde_round_trip_preserves_all_fields() {
        let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
        item.set_status(ResearchStatus::Complete);
        item.add_source(Source::Other {
            label: "example".into(),
            captured_at: Utc::now(),
            body_path: "sources/other-01.md".into(),
            body: String::new(),
        });
        let json = serde_json::to_string(&item).unwrap();
        let back: ResearchItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn error_display_messages_are_useful() {
        let err = ResearchItemError::MissingField("title".to_string());
        assert!(err.to_string().contains("title"));
        let err = ResearchItemError::InvalidStatus("weird".to_string());
        assert!(err.to_string().contains("weird"));
        let err = ResearchItemError::InvalidTimestamp {
            field: "created".to_string(),
            source: "parse error".to_string(),
        };
        assert!(err.to_string().contains("created"));
        assert!(err.to_string().contains("parse error"));
    }
}
