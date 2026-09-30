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
//! [`Source::Local`] with the `LocalSourceKind::Extra` variant - see the
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
#[allow(clippy::large_enum_variant)]
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
        /// `"openalex, wikipedia"`; for `websearch` it is `"tavily"`. Empty when
        /// the source predates this field or was supplied directly via `--from-url`.
        #[serde(default)]
        search_engine: String,
        /// HTTP `Content-Type` reported by the fetcher, when available. Stored
        /// so that the research layer can recover PDF counts from historical
        /// `RESEARCH.md` files.
        #[serde(default)]
        content_type: Option<String>,
        /// Page-type classification reported by the fetcher.
        #[serde(default)]
        page_type: Option<String>,
        /// Classified media type of the source. One of `"page"`, `"pdf"`, or
        /// `"youtube"`. Defaults to `"page"` for sources that predate this field.
        #[serde(default = "default_media_type")]
        media_type: String,
        /// Human language detected from the page's extracted text at fetch
        /// time (e.g. `"English"`, `"French"`), when the fetcher performs
        /// language detection. `None` when no language could be confidently
        /// detected, the fetcher does not detect languages, or the source
        /// predates this field.
        #[serde(default)]
        language: Option<String>,
        /// Author name extracted from the page's embedded metadata at fetch
        /// time (e.g. `"Jane Doe"`), when the fetcher is able to determine one.
        /// `None` when the page did not expose parseable author information, the
        /// fetcher does not extract authors, or the source predates this field.
        #[serde(default)]
        author: Option<String>,
        /// Open-access recovery metadata. Populated when the source body was
        /// recovered from a legal OA copy via Unpaywall or Europe PMC
        /// (FR-010). `None` when the source was captured directly from the
        /// original URL or predates this field.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        oa_recovery: Option<Box<crate::open_access::RecoveredOpenAccess>>,
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

/// Default `media_type` used when a historical `RESEARCH.md` is loaded and
/// the field is missing.
fn default_media_type() -> String {
    "page".to_string()
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

    /// Media-type classifier used in the References Index table.
    ///
    /// Only [`Source::Web`] carries a meaningful media type (`"page"`, `"pdf"`,
    /// or `"youtube"`). Local, spec, and other sources do not have a media
    /// classifier and return `"-"`.
    #[must_use]
    pub fn media_type(&self) -> &str {
        match self {
            Self::Web { media_type, .. } => media_type,
            _ => "-",
        }
    }

    /// Detected human language of the source, when known.
    ///
    /// Only [`Source::Web`] carries a detected language (e.g. `"English"`,
    /// `"French"`), populated by the fetcher at capture time. Local, spec,
    /// and other sources do not have a meaningful language and return `None`.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        match self {
            Self::Web { language, .. } => language.as_deref(),
            _ => None,
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

    /// Name(s) of the backend search engine(s) that returned this URL.
    ///
    /// Only [`Source::Web`] carries a `search_engine` value (e.g.
    /// `"openalex, wikipedia"`, `"exa"`, `"tavily"`). Local, spec, and other
    /// sources do not have a search engine and return an empty string.
    #[must_use]
    pub fn search_engine(&self) -> &str {
        match self {
            Self::Web { search_engine, .. } => search_engine,
            _ => "",
        }
    }

    /// Name of the search tool that discovered this source.
    ///
    /// Only [`Source::Web`] carries a `search_tool` value (e.g. `"mf_search"`,
    /// `"websearch"`). Local, spec, and other sources do not have a search tool
    /// and return an empty string.
    #[must_use]
    pub fn search_tool(&self) -> &str {
        match self {
            Self::Web { search_tool, .. } => search_tool,
            _ => "",
        }
    }

    /// Author name of the source, when known.
    ///
    /// Only [`Source::Web`] carries an extracted author (e.g. `"Jane Doe"`).
    /// Local, spec, and other sources do not have a meaningful author and return
    /// `None`.
    #[must_use]
    pub fn author(&self) -> Option<&str> {
        match self {
            Self::Web { author, .. } => author.as_deref(),
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
    /// no body by design - it points at the spec directory itself).
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

    /// Path to the supporting file for variants that have one.
    #[must_use]
    pub fn body_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Web { body_path, .. }
            | Self::Local { body_path, .. }
            | Self::Other { body_path, .. } => Some(body_path.as_path()),
            Self::Spec { .. } => None,
        }
    }

    /// `true` when the source has a non-empty captured body. Used by the
    /// synthesis engine to skip empty rows when computing prompt budgets.
    #[must_use]
    pub fn has_body(&self) -> bool {
        self.body().is_some_and(|b| !b.is_empty())
    }

    /// Open-access recovery metadata, when the source body was recovered from a
    /// legal OA copy instead of being fetched directly from the original URL.
    #[must_use]
    pub fn oa_recovery(&self) -> Option<&crate::open_access::RecoveredOpenAccess> {
        match self {
            Self::Web { oa_recovery, .. } => oa_recovery.as_deref(),
            _ => None,
        }
    }

    /// Human-readable disclosure of an open-access recovery for this source.
    ///
    /// Returns `None` when the source was captured directly from the original
    /// URL or is not a web source. When a legal OA copy was recovered, the
    /// note includes the recovery service, the recovered URL, and the version
    /// and license when reported (FR-015).
    #[must_use]
    pub fn oa_recovery_note(&self) -> Option<String> {
        let r = self.oa_recovery()?;
        let version = r.version.as_deref().unwrap_or("unspecified");
        let license = r.license.as_deref().unwrap_or("unspecified");
        Some(format!(
            "recovered from {source} ({url}); version={version}, license={license}",
            source = r.source,
            url = r.url
        ))
    }

    /// Numeric relevance rank used by the `max_synthesis_sources` cap
    /// (Milestone E-003). Higher numbers = more relevant.
    ///
    /// The rank is derived from the relevance label string produced by
    /// `crate::web_gatherer::compute_relevance_label`. Sources without a
    /// relevance label (local, spec, other, or pre-relevance-field web
    /// sources) receive a default rank of 5 so they are treated as
    /// medium-relevance and are not unfairly excluded by the cap.
    ///
    /// | Label prefix                          | Rank |
    /// |---------------------------------------|------|
    /// | `Very high`                           |    8 |
    /// | `High`                                |    7 |
    /// | `Medium-high`                         |    6 |
    /// | `Medium`                              |    5 |
    /// | `Match score unavailable`             |    5 |
    /// | `Low`                                 |    3 |
    /// | `Very low`                            |    1 |
    /// | (no relevance string)                 |    5 |
    #[must_use]
    pub fn relevance_rank(&self) -> u8 {
        let label = match self.relevance() {
            Some(r) if !r.is_empty() => r,
            _ => return 5,
        };
        let lc = label.to_lowercase();
        if lc.starts_with("very high") {
            8
        } else if lc.starts_with("high") {
            7
        } else if lc.starts_with("medium-high") {
            6
        } else if lc.starts_with("medium") || lc.starts_with("match score unavailable") {
            5
        } else if lc.starts_with("low") {
            3
        } else if lc.starts_with("very low") {
            1
        } else {
            5
        }
    }
}

#[cfg(test)]
#[path = "../tests/inline/source_tests.rs"]
mod tests;
