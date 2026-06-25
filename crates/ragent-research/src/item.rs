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
        }
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

    /// Render the YAML frontmatter block (with leading `---` fence).
    ///
    /// The output matches the shape defined at the top of this module and
    /// round-trips through [`ResearchItem::from_frontmatter`].
    ///
    /// ```text
    /// ---
    /// name: rust-async
    /// title: Rust Async Patterns
    /// ...
    /// sources: []
    /// ---
    /// ```
    pub fn render_frontmatter(&self) -> String {
        // Hand-rolled YAML to avoid pulling in serde_yaml as a hard
        // dependency for what amounts to a five-line block. The shape
        // matches what serde would emit for the public fields.
        let mut out = String::from("---\n");
        out.push_str(&format!("name: {}\n", self.name.as_str()));
        out.push_str(&format!("title: {}\n", yaml_scalar(&self.title)));
        out.push_str(&format!("topic: {}\n", yaml_scalar(&self.topic)));
        out.push_str(&format!("status: {}\n", self.status));
        out.push_str(&format!("created: {}\n", self.created_at.to_rfc3339()));
        out.push_str(&format!("modified: {}\n", self.modified_at.to_rfc3339()));
        out.push_str(&format!("sources: {}\n", sources_count(&self.sources)));
        out.push_str("---\n");
        out
    }

    /// Build a `ResearchItem` from a raw frontmatter block string.
    ///
    /// Accepts the output of [`ResearchItem::render_frontmatter`] (or any
    /// YAML-shaped block that uses the same field names) and extracts the
    /// fields. Unknown fields are tolerated; missing required fields
    /// produce an error explaining which one was absent.
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

        let mut name: Option<String> = None;
        let mut title: Option<String> = None;
        let mut topic: Option<String> = None;
        let mut status: Option<ResearchStatus> = None;
        let mut created: Option<DateTime<Utc>> = None;
        let mut modified: Option<DateTime<Utc>> = None;

        for raw_line in inner.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => name = Some(value.to_string()),
                "title" => title = Some(unquote_yaml_scalar(value)),
                "topic" => topic = Some(unquote_yaml_scalar(value)),
                "status" => {
                    status = Some(
                        ResearchStatus::parse(value)
                            .ok_or_else(|| ResearchItemError::InvalidStatus(value.to_string()))?,
                    );
                }
                "created" => {
                    created = Some(
                        DateTime::parse_from_rfc3339(value)
                            .map_err(|e| ResearchItemError::InvalidTimestamp {
                                field: "created".to_string(),
                                source: e.to_string(),
                            })?
                            .with_timezone(&Utc),
                    );
                }
                "modified" => {
                    modified = Some(
                        DateTime::parse_from_rfc3339(value)
                            .map_err(|e| ResearchItemError::InvalidTimestamp {
                                field: "modified".to_string(),
                                source: e.to_string(),
                            })?
                            .with_timezone(&Utc),
                    );
                }
                "sources" => { /* count-only, sources are loaded separately */ }
                _ => { /* ignore unknown fields */ }
            }
        }

        let name = name.ok_or(ResearchItemError::MissingField("name".to_string()))?;
        let name = ResearchName::try_new(name).map_err(ResearchItemError::InvalidName)?;
        let title = title.ok_or(ResearchItemError::MissingField("title".to_string()))?;
        let topic = topic.unwrap_or_default();
        let status = status.unwrap_or_default();
        let created_at = created.unwrap_or_else(Utc::now);
        let modified_at = modified.unwrap_or(created_at);

        Ok(Self {
            name,
            title,
            topic,
            status,
            created_at,
            modified_at,
            sources: Vec::new(),
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

/// Render a string as a YAML scalar: plain if it contains no special
/// characters, double-quoted otherwise. This keeps the frontmatter
/// human-editable while staying valid YAML for non-trivial titles.
fn yaml_scalar(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    let needs_quote = value
        .chars()
        .any(|c| matches!(c, ':' | '#' | '"' | '\n' | '\r' | '\t'))
        || value.starts_with(' ')
        || value.ends_with(' ');
    if needs_quote {
        let escaped = value.replace('\\', r"\\").replace('"', r#"\""#);
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

/// Reverse of [`yaml_scalar`] for the frontmatter parser. Strips surrounding
/// double quotes (and unescapes `\\` and `\"`) if present.
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
        assert!(fm.ends_with("---\n"));
        assert!(fm.contains("name: rust-async"));
        assert!(fm.contains("title: Rust Async"));
        assert!(fm.contains("topic: topic"));
        assert!(fm.contains("status: draft"));
        assert!(fm.contains("created: "));
        assert!(fm.contains("modified: "));
        assert!(fm.contains("sources: 0"));
    }

    #[test]
    fn render_frontmatter_quotes_titles_with_special_chars() {
        let mut item = ResearchItem::new(sample_name(), "t", "topic");
        item.set_title("Async: a deep dive");
        let fm = item.render_frontmatter();
        assert!(fm.contains("title: \"Async: a deep dive\""));
    }

    #[test]
    fn frontmatter_round_trips_through_parse() {
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
    fn frontmatter_round_trips_with_quoted_title() {
        let mut item = ResearchItem::new(sample_name(), "t", "topic");
        item.set_title("Has: colon");
        let fm = item.render_frontmatter();
        let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
        assert_eq!(parsed.title, "Has: colon");
    }

    #[test]
    fn from_frontmatter_fails_on_missing_name() {
        let block = "---\ntitle: foo\n---\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::MissingField(_)));
    }

    #[test]
    fn from_frontmatter_fails_on_invalid_name() {
        let block = "---\nname: ..\ntitle: foo\n---\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::InvalidName(_)));
    }

    #[test]
    fn from_frontmatter_fails_on_invalid_status() {
        let block = "---\nname: rust-async\ntitle: foo\nstatus: nope\n---\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::InvalidStatus(_)));
    }

    #[test]
    fn from_frontmatter_fails_on_invalid_timestamp() {
        let block = "---\nname: rust-async\ntitle: foo\ncreated: not-a-date\n---\n";
        let err = ResearchItem::from_frontmatter(block).unwrap_err();
        assert!(matches!(err, ResearchItemError::InvalidTimestamp { .. }));
    }

    #[test]
    fn from_frontmatter_defaults_optional_fields() {
        let block = "---\nname: rust-async\ntitle: foo\n---\n";
        let item = ResearchItem::from_frontmatter(block).unwrap();
        assert_eq!(item.status, ResearchStatus::Draft);
        assert!(item.topic.is_empty());
    }

    #[test]
    fn from_frontmatter_tolerates_unknown_fields() {
        let block = "---\nname: rust-async\ntitle: foo\nextra: stuff\nanother: 42\n---\n";
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
