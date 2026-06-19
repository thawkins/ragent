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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalSourceKind {
    /// A file inside the project root that was discovered by the default
    /// local-gathering phase.
    InProject,
    /// A file supplied via `--sources-dir <path>` for an additional scan.
    /// Rendered as the "extra-local" type in the References Index.
    Extra,
}

impl LocalSourceKind {
    /// Type-column value used in the References Index table.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProject => "local",
            Self::Extra => "extra-local",
        }
    }
}

impl Default for LocalSourceKind {
    fn default() -> Self {
        Self::InProject
    }
}

/// A captured piece of evidence used by a research item.
///
/// The `body_path` fields point at the supporting file on disk under
/// `research/<name>/sources/` (e.g. `web-01.md`, `local-03.md`). They are
/// `PathBuf` rather than `String` so that callers can use the existing
/// filesystem APIs to read them back.
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
        /// Relative path to the supporting file under `research/<name>/sources/`.
        body_path: PathBuf,
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
    },
}

impl Source {
    /// Type-column value used in the References Index table.
    pub fn type_str(&self) -> &'static str {
        match self {
            Self::Web { .. } => "web",
            Self::Local { kind, .. } => kind.as_str(),
            Self::Spec { .. } => "spec",
            Self::Other { .. } => "other",
        }
    }

    /// Title or label for this source, used in the References Index table.
    pub fn title(&self) -> &str {
        match self {
            Self::Web { title, .. } => title,
            Self::Local { path, .. } => path,
            Self::Spec { spec_id, .. } => spec_id,
            Self::Other { label, .. } => label,
        }
    }

    /// Path or URL for this source, used in the References Index table.
    pub fn path_or_url(&self) -> &str {
        match self {
            Self::Web { url, .. } => url,
            Self::Local { path, .. } => path,
            Self::Spec { spec_id, .. } => spec_id,
            Self::Other { label, .. } => label,
        }
    }

    /// Timestamp at which the source was captured.
    pub fn captured_at(&self) -> DateTime<Utc> {
        match self {
            Self::Web { captured_at, .. }
            | Self::Local { captured_at, .. }
            | Self::Spec { captured_at, .. }
            | Self::Other { captured_at, .. } => *captured_at,
        }
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
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
        };
        assert_eq!(web.type_str(), "web");

        let local = Source::Local {
            path: "src/lib.rs".into(),
            kind: LocalSourceKind::InProject,
            captured_at: dt(),
            body_path: PathBuf::from("sources/local-01.md"),
            relevance: "Main library entry".into(),
        };
        assert_eq!(local.type_str(), "local");

        let extra = Source::Local {
            path: "extra/notes.md".into(),
            kind: LocalSourceKind::Extra,
            captured_at: dt(),
            body_path: PathBuf::from("sources/local-02.md"),
            relevance: "External notes".into(),
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
        };
        assert_eq!(other.type_str(), "other");
    }

    #[test]
    fn title_and_path_or_url_for_each_variant() {
        let web = Source::Web {
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
        };
        assert_eq!(web.title(), "Example");
        assert_eq!(web.path_or_url(), "https://example.com");

        let local = Source::Local {
            path: "src/lib.rs".into(),
            kind: LocalSourceKind::InProject,
            captured_at: dt(),
            body_path: PathBuf::from("sources/local-01.md"),
            relevance: "Main library entry".into(),
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
            url: "u".into(),
            title: "t".into(),
            captured_at: now,
            body_path: PathBuf::from("x"),
        };
        assert_eq!(web.captured_at(), now);
    }

    #[test]
    fn serde_round_trip_web() {
        let s = Source::Web {
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: dt(),
            body_path: PathBuf::from("sources/web-01.md"),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
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
        if let Source::Local { kind, .. } = s {
            assert_eq!(kind, LocalSourceKind::InProject);
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
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn local_source_kind_default() {
        assert_eq!(LocalSourceKind::default(), LocalSourceKind::InProject);
    }
}
