//! Vault storage: notes on disk and their assets (FR-005, FR-007).
//!
//! A vault is a directory of Markdown notes, each with YAML frontmatter, plus a
//! per-note assets directory for attachments. This module owns paths and the
//! byte-exact frontmatter round-trip; parsing lives in [`crate::models::note`].

use std::path::{Path, PathBuf};

use crate::core::error::{Error, Result};

/// Directory name holding binary attachments.
pub const ASSETS_DIR: &str = "assets";

/// Directory name holding per-note attachments.
pub const NOTE_ASSETS_DIR: &str = ".assets";

/// A research vault rooted at a directory.
#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    /// Open a vault rooted at `root`, creating it when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the root exists as a file, or cannot be
    /// created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|err| Error::io(&root, err))?;
        Ok(Self { root })
    }

    /// The vault root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path of the note with `id`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidNoteId`] when the id could escape the vault
    /// (path separators, `..`, a leading dot, or an empty id).
    pub fn note_path(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        Ok(self.root.join(format!("{id}.md")))
    }

    /// Directory holding the note's attachments (FR-007).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidNoteId`] for the same reasons as [`Self::note_path`].
    pub fn note_assets_dir(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        Ok(self.root.join(ASSETS_DIR).join(id))
    }

    /// Create the note's assets directory, returning its path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the directory cannot be created.
    pub fn ensure_note_assets_dir(&self, id: &str) -> Result<PathBuf> {
        let dir = self.note_assets_dir(id)?;
        std::fs::create_dir_all(&dir).map_err(|err| Error::io(&dir, err))?;
        Ok(dir)
    }

    /// The vault's index file (`INDEX.md`, FR-032).
    pub fn index_path(&self) -> PathBuf {
        self.root.join("INDEX.md")
    }

    /// Refuse a note id that already exists (FR-006).
    ///
    /// Collisions are reported with both the id and the existing path, and are
    /// never resolved by auto-suffixing: a silently renamed note is a broken
    /// citation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoteIdConflict`] when the note exists.
    pub fn ensure_available(&self, id: &str) -> Result<()> {
        let path = self.note_path(id)?;
        if path.exists() {
            return Err(Error::NoteIdConflict {
                id: id.to_string(),
                path,
            });
        }
        Ok(())
    }
}

/// Validate a note id against path traversal.
///
/// Ids are used as file names, so anything that could address another
/// directory is rejected outright (NFR-001: no user path reaches an `unwrap`
/// or a surprising join).
fn validate_id(id: &str) -> Result<()> {
    let reject = |reason: &str| {
        Err(Error::InvalidNoteId {
            id: id.to_string(),
            reason: reason.to_string(),
        })
    };

    if id.is_empty() {
        return reject("id must not be empty");
    }
    if id.len() > 128 {
        return reject("id must be at most 128 characters");
    }
    if id.starts_with('.') {
        return reject("id must not start with a dot");
    }
    if id.contains('/') || id.contains('\\') {
        return reject("id must not contain a path separator");
    }
    if id.contains("..") {
        return reject("id must not contain `..`");
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return reject("id must be ASCII alphanumeric, `-`, `_`, or `.`");
    }

    Ok(())
}
