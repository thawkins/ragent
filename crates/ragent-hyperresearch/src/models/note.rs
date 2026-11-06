//! Note model and byte-exact frontmatter round-trip (FR-005).
//!
//! A note is a YAML frontmatter block followed by a Markdown body. The
//! frontmatter is kept as the raw text it was read as, *as well as* the parsed
//! [`NoteMeta`], because writing back a re-serialised struct would reorder keys
//! and requote strings — the round-trip has to be byte-identical.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::error::{Error, Result};

/// Delimiter that opens and closes a frontmatter block.
pub const FRONTMATTER_FENCE: &str = "---";

/// Frontmatter fields of a vault note.
///
/// Fields are optional so a hand-written note that omits most of them still
/// loads; `unknown` carries anything this build does not model so a round-trip
/// preserves it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMeta {
    /// Stable note id.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Topic grouping used by the index (FR-032).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Source URL this note was captured from, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Creation timestamp, RFC 3339 UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Last-updated timestamp, RFC 3339 UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

/// A note as loaded from the vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// Parsed frontmatter.
    pub meta: NoteMeta,
    /// Markdown body, without the frontmatter block.
    pub body: String,
    /// The frontmatter exactly as read, so a no-op write reproduces it.
    pub raw_frontmatter: String,
    /// Where the note was loaded from; empty for a note built in memory.
    pub path: PathBuf,
}

impl Note {
    /// Parse a note from file text.
    ///
    /// A document with no frontmatter block is accepted as a note with empty
    /// metadata and the whole text as its body, which is how a note written by
    /// hand in an editor arrives.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Frontmatter`] when a block is opened but never closed,
    /// or when it is not valid YAML.
    pub fn parse(path: impl Into<PathBuf>, text: &str) -> Result<Self> {
        let path = path.into();

        let Some(rest) = text.strip_prefix(FRONTMATTER_FENCE) else {
            return Ok(Self {
                meta: NoteMeta::default(),
                body: text.to_string(),
                raw_frontmatter: String::new(),
                path,
            });
        };

        // `strip_prefix` left the newline that ends the opening fence.
        let rest = rest.strip_prefix('\n').unwrap_or(rest);

        let Some(end) = find_fence_end(rest) else {
            return Err(Error::Frontmatter {
                path,
                reason: "frontmatter block is opened but never closed".to_string(),
            });
        };

        let raw_frontmatter = &rest[..end];
        let body = rest[end..]
            .trim_start_matches(FRONTMATTER_FENCE)
            .trim_start_matches(['\r', '\n'])
            .to_string();

        let meta: NoteMeta =
            serde_norway::from_str(raw_frontmatter).map_err(|err| Error::Frontmatter {
                path: path.clone(),
                reason: err.to_string(),
            })?;

        Ok(Self {
            meta,
            body,
            raw_frontmatter: raw_frontmatter.to_string(),
            path,
        })
    }

    /// Render the note back to file text.
    ///
    /// The frontmatter is reproduced from [`Self::raw_frontmatter`] verbatim
    /// when it exists, so an unmodified note writes back byte-identically
    /// (FR-005).
    pub fn render(&self) -> String {
        if self.raw_frontmatter.is_empty() {
            return self.body.clone();
        }

        format!(
            "{FRONTMATTER_FENCE}\n{}{FRONTMATTER_FENCE}\n{}",
            ensure_trailing_newline(&self.raw_frontmatter),
            self.body
        )
    }
}

/// Find the byte offset where the closing fence line begins.
fn find_fence_end(text: &str) -> Option<usize> {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == FRONTMATTER_FENCE {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

/// Guarantee the string ends with exactly one newline.
fn ensure_trailing_newline(text: &str) -> String {
    let trimmed = text.trim_end_matches(['\r', '\n']);
    format!("{trimmed}\n")
}
