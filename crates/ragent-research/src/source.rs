//! Captured evidence backing a research item.
//!
//! Every `Source` represents a single piece of evidence that the gathering
//! engine either pulled from the web, read from the local filesystem, or
//! cross-referenced from an existing spec. Sources are the rows that populate
//! the **References Index** block at the bottom of `RESEARCH.md` (FR-011).
//!
//! The four variants map directly to the type column of the References Index
//! table:
//!
//! | Variant          | Type column | Typical use                              |
//! |------------------|-------------|------------------------------------------|
//! | [`Source::Web`]  | `web`       | Articles, blog posts, API docs           |
//! | [`Source::Local`]| `local`     | Project source files, READMEs, AGENTS.md |
//! | [`Source::Spec`] | `spec`      | Prior specs under `specs/`               |
//! | [`Source::Other`]| `other`     | Anything else (PDFs, transcripts, etc.)  |
//!
//! The extra-local type produced by `--sources-dir` (FR-019) is encoded as
//! [`Source::Local`] with the `LocalSourceKind::Extra` variant — see the
//! [`LocalSourceKind`] enum below.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Distinguishes in-project local sources from extra directories supplied
/// via the `--sources-dir` flag (FR-019).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LocalSourceKind {
    /// A file inside the project root that was discovered by the default
    /// local-gathering phase.
    #[default]
    InProject,
    /// A file supplied via `--sources-dir <path>` for an additional scan.
    /// Rendered as the "extra-local" type in the References Index.
    Extra,
}

impl LocalSourceKind {
    /// Type-column value used in the References Index table.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProject => "local",
            Self::Extra => "extra-local",
        }
    }
}

/// A captured piece of evidence used by a research item.
///
/// The `body_path` fields point at the supporting file on disk under
/// `research/<name>/sources/` (e.g. `web-01.md`, `local-03.md`). They are
/// `PathBuf` rather than `String` so that callers can use the existing
/// filesystem APIs to read them back.
///
/// The `body` field carries the captured text itself so the synthesis engine
/// and the supporting-file renderer have something meaningful to work with
/// (FR-007, FR-008, FR-021). Old `RESEARCH.md` files written before this
/// field existed deserialize with `body == ""` thanks to `#[serde(default)]`;
/// those items will simply have an empty body until re-gathered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum Source {
    /// A web URL (article, blog post, API doc, etc.).
    Web {
        /// Full URL of the captured page.
        url: String,
        /// Page title if known; empty string if the fetcher couldn't determine one.
        title: String,
        /// Timestamp at which the page was fetched.
        captured_at: DateTime<Utc>,
        /// Publication date of the page, parsed from embedded metadata (HTML
        /// `<meta>` tags, JSON-LD `datePublished`, or `<time datetime="...">`
        /// elements) when available. `None` when the page did not expose a
        /// parseable publication date, or when the source was loaded from an
        /// older `RESEARCH.md` that predates this field.
        #[serde(default)]
        published_at: Option<DateTime<Utc>>,
        /// Relative path to the supporting file under `research/<name>/sources/`.
        body_path: PathBuf,
        /// Captured page text, fenced into the supporting file at write time.
        /// Empty when the source was loaded from a pre-body-field `RESEARCH.md`.
        #[serde(default)]
        body: String,
        /// One-line note describing how relevant the page is to the search
        /// query that discovered it. Empty when the source predates this
        /// field or was supplied directly via `--from-url`.
        #[serde(default)]
        relevance: String,
        /// Name of the search tool that discovered this source (e.g.
        /// `"mf_search"` or `"websearch"`). Empty when the source predates
        /// this field or was supplied directly via `--from-url`.
        #[serde(default)]
        search_tool: String,
        /// Name(s) of the backend search engine(s) that returned this URL.
        /// For `mf_search` this is a comma-separated list like
        /// `"duckduckgo, brave"`; for `websearch` it is `"tavily"`. Empty when
        /// the source predates this field or was supplied directly via `--from-url`.
        #[serde(default)]
        search_engine: String,
    },
    /// A local file excerpted from the project or an extra sources dir.
    Local {
        /// Path of the captured file, relative to the project root.
        path: String,
        /// Kind of local source (in-project vs. extra).
        #[serde(default)]
        kind: LocalSourceKind,
        /// Timestamp at which the file was read.
        captured_at: DateTime<Utc>,
        /// Relative path to the supporting file under `research/<name>/sources/`.
        body_path: PathBuf,
        /// One-line note explaining why this file is relevant to the topic.
        relevance: String,
        /// Excerpted file text (the matching lines plus surrounding context),
        /// embedded in the supporting file at write time.
        #[serde(default)]
        body: String,
    },
    /// A prior spec under `specs/` cross-referenced from the research item.
    Spec {
        /// Spec identifier (directory name under `specs/`).
        spec_id: String,
        /// Timestamp at which the spec was read.
        captured_at: DateTime<Utc>,
        /// Optional one-line note describing why this spec is relevant.
        #[serde(default)]
        relevance: String,
    },
    /// Any other source not covered by the categories above (PDFs,
    /// transcripts, embedded images, etc.).
    Other {
        /// Free-form label describing the source.
        label: String,
        /// Timestamp at which the source was captured.
        captured_at: DateTime<Utc>,
        /// Relative path to the supporting file under `research/<name>/sources/`.
        body_path: PathBuf,
        /// Captured text content, embedded in the supporting file at write time.
        #[serde(default)]
        body: String,
    },
}

impl Source {
    /// Type-column value used in the References Index table.
    #[must_use]
    pub const fn type_str(&self) -> &'static str {
        match self {
            Self::Web { .. } => "web",
            Self::Local { kind, .. } => kind.as_str(),
            Self::Spec { .. } => "spec",
            Self::Other { .. } => "other",
        }
    }

    /// Title or label for this source, used in the References Index table.
    #[must_use]
    pub fn title(&self) -> &str {
        match self {
            Self::Web { title, .. } => title,
            Self::Local { path, .. } => path,
            Self::Spec { spec_id, .. } => spec_id,
            Self::Other { label, .. } => label,
        }
    }

    /// Path or URL for this source, used in the References Index table.
    #[must_use]
    pub fn path_or_url(&self) -> &str {
        match self {
            Self::Web { url, .. } => url,
            Self::Local { path, .. } => path,
            Self::Spec { spec_id, .. } => spec_id,
            Self::Other { label, .. } => label,
        }
    }

    /// Timestamp at which the source was captured.
    #[must_use]
    pub const fn captured_at(&self) -> DateTime<Utc> {
        match self {
            Self::Web { captured_at, .. }
            | Self::Local { captured_at, .. }
            | Self::Spec { captured_at, .. }
            | Self::Other { captured_at, .. } => *captured_at,
        }
    }

    /// Publication date of the source, when known.
    ///
    /// Only [`Source::Web`] carries a publication date (parsed from the page's
    /// embedded metadata at fetch time). Local, spec, and other sources do not
    /// have a meaningful publication date and always return `None`.
    #[must_use]
    pub const fn published_at(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Web { published_at, .. } => *published_at,
            _ => None,
        }
    }

    /// Optional relevance note for local, spec, and web sources.
    #[must_use]
    pub fn relevance(&self) -> Option<&str> {
        match self {
            Self::Local { relevance, .. }
            | Self::Spec { relevance, .. }
            | Self::Web { relevance, .. } => Some(relevance),
            Self::Other { .. } => None,
        }
    }

    /// Captured body text for variants that carry one. Returns `Some(body)`
    /// for web/local/other sources, and `None` for `Source::Spec` (which has
    /// no body by design — it points at the spec directory itself).
    ///
    /// When the source was loaded from an older `RESEARCH.md` that predates
    /// the `body` field, this returns an empty string.
    #[must_use]
    pub const fn body(&self) -> Option<&str> {
        match self {
            Self::Web { body, .. } | Self::Local { body, .. } | Self::Other { body, .. } => {
                Some(body.as_str())
            }
            Self::Spec { .. } => None,
        }
    }

    /// `true` when the source has a non-empty captured body. Used by the
    /// synthesis engine to skip empty rows when computing prompt budgets.
    #[must_use]
    pub fn has_body(&self) -> bool {
        self.body().is_some_and(|b| !b.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn type_str_for_each_variant() {
        let web = Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: "page text".into(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        assert_eq!(web.type_str(), "web");

        let local = Source::Local {
            path: "src/lib.rs".into(),
            kind: LocalSourceKind::InProject,
            captured_at: dt(),
            body_path: PathBuf::from("sources/local-01.md"),
            relevance: "Main library entry".into(),
            body: "fn main() {}".into(),
        };
        assert_eq!(local.type_str(), "local");

        let extra = Source::Local {
            path: "extra/notes.md".into(),
            kind: LocalSourceKind::Extra,
            captured_at: dt(),
            body_path: PathBuf::from("sources/local-02.md"),
            relevance: "External notes".into(),
            body: String::new(),
        };
        assert_eq!(extra.type_str(), "extra-local");

        let spec = Source::Spec {
            spec_id: "researchsystem".into(),
            captured_at: dt(),
            relevance: "The spec under design".into(),
        };
        assert_eq!(spec.type_str(), "spec");

        let other = Source::Other {
            label: "Interview transcript".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/other-01.md"),
            body: "Q: ... A: ...".into(),
        };
        assert_eq!(other.type_str(), "other");
    }

    #[test]
    fn title_and_path_or_url_for_each_variant() {
        let web = Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        assert_eq!(web.title(), "Example");
        assert_eq!(web.path_or_url(), "https://example.com");

        let local = Source::Local {
            path: "src/lib.rs".into(),
            kind: LocalSourceKind::InProject,
            captured_at: dt(),
            body_path: PathBuf::from("sources/local-01.md"),
            relevance: "Main library entry".into(),
            body: String::new(),
        };
        assert_eq!(local.title(), "src/lib.rs");
        assert_eq!(local.path_or_url(), "src/lib.rs");

        let spec = Source::Spec {
            spec_id: "researchsystem".into(),
            captured_at: dt(),
            relevance: "The spec under design".into(),
        };
        assert_eq!(spec.title(), "researchsystem");
        assert_eq!(spec.path_or_url(), "researchsystem");
    }

    #[test]
    fn captured_at_is_accessible_for_each_variant() {
        let now = dt();
        let web = Source::Web {
            published_at: None,
            url: "u".into(),
            title: "t".into(),
            captured_at: now,
            body_path: PathBuf::from("x"),
            body: String::new(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        assert_eq!(web.captured_at(), now);
    }

    #[test]
    fn serde_round_trip_web() {
        let s = Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: "page text".into(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn serde_round_trip_web_backward_compatible_without_body() {
        let json = r#"{
            "source_type": "web",
            "url": "https://example.com",
            "title": "Example",
            "captured_at": "2024-01-15T10:30:00Z",
            "body_path": "sources/web-01.md"
        }"#;
        let s: Source = serde_json::from_str(json).unwrap();
        if let Source::Web { body, .. } = s {
            assert_eq!(body, "");
        } else {
            panic!("expected Web variant");
        }
    }

    #[test]
    fn serde_round_trip_local_with_default_kind() {
        let json = r#"{
            "source_type": "local",
            "path": "src/lib.rs",
            "captured_at": "2024-01-15T10:30:00Z",
            "body_path": "sources/local-01.md",
            "relevance": "Entry point"
        }"#;
        let s: Source = serde_json::from_str(json).unwrap();
        if let Source::Local { kind, body, .. } = s {
            assert_eq!(kind, LocalSourceKind::InProject);
            assert_eq!(body, "");
        } else {
            panic!("expected Local variant");
        }
    }

    #[test]
    fn serde_round_trip_spec() {
        let s = Source::Spec {
            spec_id: "foo".into(),
            captured_at: dt(),
            relevance: "Related".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn serde_round_trip_other() {
        let s = Source::Other {
            label: "PDF".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/other-01.md"),
            body: "transcript".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn serde_round_trip_other_backward_compatible_without_body() {
        let json = r#"{
            "source_type": "other",
            "label": "PDF",
            "captured_at": "2024-01-15T10:30:00Z",
            "body_path": "sources/other-01.md"
        }"#;
        let s: Source = serde_json::from_str(json).unwrap();
        if let Source::Other { body, .. } = s {
            assert_eq!(body, "");
        } else {
            panic!("expected Other variant");
        }
    }

    #[test]
    fn body_and_has_body_for_each_variant() {
        let web = Source::Web {
            published_at: None,
            url: "u".into(),
            title: "t".into(),
            captured_at: dt(),
            body_path: PathBuf::from("x"),
            body: "hello".into(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        assert_eq!(web.body(), Some("hello"));
        assert!(web.has_body());

        let empty = Source::Web {
            published_at: None,
            url: "u".into(),
            title: "t".into(),
            captured_at: dt(),
            body_path: PathBuf::from("x"),
            body: String::new(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        assert_eq!(empty.body(), Some(""));
        assert!(!empty.has_body());

        let spec = Source::Spec {
            spec_id: "s".into(),
            captured_at: dt(),
            relevance: String::new(),
        };
        assert_eq!(spec.body(), None);
        assert!(!spec.has_body());
    }

    #[test]
    fn local_source_kind_default() {
        assert_eq!(LocalSourceKind::default(), LocalSourceKind::InProject);
    }

    #[test]
    fn serde_round_trip_web_with_relevance() {
        let s = Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: "page text".into(),
            relevance: "High — title matches query terms".into(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.relevance(), Some("High — title matches query terms"));
    }

    #[test]
    fn web_relevance_returns_empty_as_some() {
        let s = Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        };
        assert_eq!(s.relevance(), Some(""));
    }
}
