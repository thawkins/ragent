//! `/connectors add` install operation and source classification (spec
//! `connectors` T-009; FR-011, FR-027, FR-028, FR-029).
//!
//! This module is the install *operation* the command glue drives. It classifies
//! the `<id|source>` argument and installs accordingly:
//!
//! - a **catalogue id** is matched against the already-fetched catalogue and the
//!   matching descriptor is installed with
//!   [`crate::manifest::install_descriptor`];
//! - a **local directory**, a local **`.zip`/`.tar.gz`** package, or an
//!   **`https://` URL** installs through [`crate::manifest::stage`], which owns
//!   the filesystem staging and the archive guards.
//!
//! Every refusal is a contained [`AddError`] and changes no file:
//!
//! - a non-`https` scheme is refused ([`StageError::NotHttps`], FR-028);
//! - an archive entry that would escape the store is refused
//!   ([`StageError::UnsafePath`], FR-029);
//! - an existing connector id is refused unless `force`
//!   ([`StageError::Exists`], FR-027);
//! - a catalogue id the catalogue does not hold is refused
//!   ([`AddError::CatalogueNotFound`]).
//!
//! A committed connector is recorded **disabled** (FR-011); no MCP server is
//! contacted by an install.

use std::path::Path;

use crate::descriptor::ConnectorDescriptor;
use crate::manifest::{StageError, StagedConnector, install_descriptor, stage};
use crate::store::StoreDirs;

/// How the `<id|source>` argument of `/connectors add` is interpreted (FR-011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallSource {
    /// A catalogue connector id, matched against the fetched catalogue.
    CatalogueId(String),
    /// A local directory, local archive path, or an `https://` URL, resolved
    /// against the working directory by [`crate::manifest::stage`].
    Local(String),
}

/// Classify the `<id|source>` argument (FR-011).
///
/// A source carrying a scheme (`https://`, `http://`, ...), an existing path
/// under `workdir`, or a known archive extension is [`InstallSource::Local`] so
/// [`crate::manifest::stage`] reports its specific refusal. A path-shaped source
/// (one with a separator) that does not exist is also local, so a mistyped path
/// is reported as an unrecognised source rather than as a missing catalogue id.
/// Any other bare token is a [`InstallSource::CatalogueId`].
#[must_use]
pub fn classify_source(source: &str, workdir: &Path) -> InstallSource {
    let trimmed = source.trim();
    if trimmed.contains("://") {
        return InstallSource::Local(trimmed.to_string());
    }

    let path = {
        let p = Path::new(trimmed);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            workdir.join(p)
        }
    };
    if path.exists() || has_archive_extension(trimmed) {
        return InstallSource::Local(trimmed.to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return InstallSource::Local(trimmed.to_string());
    }
    InstallSource::CatalogueId(trimmed.to_string())
}

/// Whether `name` ends in a known install-archive extension.
fn has_archive_extension(name: &str) -> bool {
    let path = Path::new(name);
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("tgz") => true,
        Some(ext) if ext.eq_ignore_ascii_case("gz") => path
            .file_stem()
            .and_then(|s| Path::new(s).extension())
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("tar")),
        _ => false,
    }
}

/// A refusal or failure from [`add`].
#[derive(Debug, thiserror::Error)]
pub enum AddError {
    /// The source is a local directory, archive, or URL and its install was
    /// refused or failed. Wraps the staging error verbatim so the specific
    /// refusal (non-`https`, unsafe entry, id collision, unreadable manifest) is
    /// reported (FR-027, FR-028, FR-029).
    #[error("{0}")]
    Stage(#[from] StageError),

    /// The catalogue holds no connector with this id (FR-011).
    #[error("connector add: catalogue holds no connector with id {0}")]
    CatalogueNotFound(String),
}

/// Install the connector named by `source` (FR-011, FR-027, FR-028, FR-029).
///
/// `source` is a catalogue id, a local directory, a local `.zip`/`.tar.gz`
/// package, or an `https://` URL. A local relative source resolves against
/// `workdir`. A catalogue id is matched against `catalogue` (the descriptors a
/// catalogue fetch produced); when no entry carries that id the install is
/// refused ([`AddError::CatalogueNotFound`]).
///
/// When `force` is false and the connector id already exists in the destination
/// store, the install is refused (FR-027). A committed connector is recorded
/// **disabled** (FR-011); no MCP server is contacted.
///
/// # Errors
///
/// See [`AddError`]. Every refusal leaves the store untouched.
pub fn add(
    dirs: &StoreDirs,
    workdir: &Path,
    source: &str,
    force: bool,
    catalogue: &[ConnectorDescriptor],
) -> Result<StagedConnector, AddError> {
    match classify_source(source, workdir) {
        InstallSource::Local(local) => Ok(stage(dirs, workdir, &local, force)?),
        InstallSource::CatalogueId(id) => {
            let store = dirs
                .destination()
                .ok_or_else(|| AddError::Stage(StageError::Io(NO_STORE_DETAIL.to_string())))?;
            let descriptor = catalogue
                .iter()
                .find(|d| d.id.as_str() == id)
                .cloned()
                .ok_or_else(|| AddError::CatalogueNotFound(id.clone()))?;
            Ok(install_descriptor(store, descriptor, force)?)
        }
    }
}

/// The refusal detail when no connector store leg resolves.
const NO_STORE_DETAIL: &str = "connector add: no connector store directory resolvable";
