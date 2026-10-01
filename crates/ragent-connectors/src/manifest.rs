//! Connector manifest read/write and install staging (spec `connectors` T-004;
//! FR-002, FR-011, FR-027).
//!
//! # On-disk manifest
//!
//! An installed connector is a directory in the connector store holding a
//! `connector.json` manifest: the serialised [`ConnectorDescriptor`]
//! (spec `connectors` FR-002), pretty-printed as UTF-8 with a trailing newline.
//! The manifest records the connector's *non-secret* configuration only; a
//! secret lives in the encrypted credential store and is referenced by *name*
//! ([`ConnectorDescriptor::credential`]), never by value (FR-005).
//!
//! [`write_manifest`] writes the file; [`read_manifest`] reads it back and is the
//! reader the discovery scan ([`crate::store::scan`]) uses.
//!
//! # Install staging
//!
//! [`stage`] installs a connector from a local **directory**, a local
//! **`.zip` / `.tar.gz`** package, or an **`https://` URL** naming such a
//! package. The source is resolved into a staging directory, its descriptor is
//! read and validated, and only then is the tree committed into the store under
//! the connector id. Every refusal ([`StageError`]) leaves the store untouched
//! (FR-011, FR-027, FR-028, FR-029), and no MCP server is ever contacted
//! (FR-011): staging is a pure filesystem operation.
//!
//! [`install_descriptor`] is the same commit path for a descriptor that arrives
//! from the catalogue rather than from a source tree; the `/connectors add`
//! command layer (T-009) calls it after a catalogue lookup.
//!
//! Modelled on `ragent_plugins::add` so the two install paths read the same way.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ragent_types::guard::validate_relative_component;

use crate::descriptor::{ConnectorDescriptor, ConnectorId, ConnectorProvenance};
use crate::error::{ConnectorError, flatten_chain};
use crate::store::{MANIFEST_FILE, StoreDirs, StoreLedger};

/// Maximum accepted install-source archive size: 50 MiB.
pub const MAX_SOURCE_BYTES: u64 = 50 * 1024 * 1024;

/// Maximum total decompressed bytes extracted from one install archive.
///
/// A per-entry cap alone is not enough: an archive holding many
/// just-under-the-cap entries would otherwise write unbounded data into the
/// store. The running total across all entries is bounded by this value.
pub const MAX_EXTRACTED_BYTES: u64 = 256 * 1024 * 1024;

/// Wall-clock budget for one install-archive download.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// Read/write chunk size for streaming a download to disk.
const STREAM_CHUNK_BYTES: usize = 65_536;

/// Name of the per-store staging directory.
///
/// A dot-prefixed name keeps it out of the discovery scan, which skips dotfiles.
const STAGING_DIR: &str = ".staging";

/// A refusal or failure from [`stage`] / [`install_descriptor`].
///
/// Every variant is a contained, reportable condition; none panics and none
/// leaves a partial write in the store (FR-011, FR-027, FR-028, FR-029).
#[derive(Debug, thiserror::Error)]
pub enum StageError {
    /// The source URL uses a scheme other than `https` (FR-028). The offending
    /// source is carried so the report can name the rejected scheme.
    #[error("connector add: refusing non-https source {0}")]
    NotHttps(String),

    /// The archive exceeds [`MAX_SOURCE_BYTES`] (FR-011).
    #[error("connector add: archive is {size} bytes, over the {max} byte cap")]
    TooLarge {
        /// Archive size in bytes.
        size: u64,
        /// Cap in bytes.
        max: u64,
    },

    /// An archive entry path is absolute or contains a `..` component, so it
    /// would write outside the store (FR-029). The rejected entry is carried.
    #[error("connector add: refusing unsafe archive entry path {0}")]
    UnsafePath(String),

    /// A connector with the same id already exists in the store and `force` was
    /// not supplied (FR-027).
    #[error("connector add: connector id {0} already exists (use --force to overwrite)")]
    Exists(String),

    /// The descriptor's id is not a single safe path component, so it cannot
    /// name the store directory (FR-002).
    #[error("connector add: {detail}")]
    InvalidId {
        /// Why the id was rejected, as reported by the shared identifier guard.
        detail: String,
    },

    /// The source holds no recognisable connector descriptor.
    #[error("connector add: no connector manifest found in {0}")]
    NotAConnector(String),

    /// The source string matches no accepted form (FR-011).
    #[error(
        "connector add: unrecognised source {0} (expected a directory, a .zip/.tar.gz file, or an https:// URL)"
    )]
    UnknownSource(String),

    /// The resolved descriptor failed validation or a manifest could not be read
    /// or written (FR-002, FR-025).
    #[error("connector add: {0}")]
    Descriptor(#[from] ConnectorError),

    /// A filesystem or network failure. Carries the full source-error chain.
    #[error("connector add: {0}")]
    Io(String),
}

impl StageError {
    /// Construct an I/O error from any error implementing [`std::error::Error`],
    /// preserving the error's full source chain (ANTIPAT M15).
    fn io(err: impl std::error::Error + 'static) -> Self {
        Self::Io(flatten_chain(&err))
    }
}

/// The outcome of a successful install.
#[derive(Debug)]
pub struct StagedConnector {
    /// The validated descriptor written to the store.
    pub descriptor: ConnectorDescriptor,
    /// The connector directory created in the store.
    pub installed_dir: PathBuf,
}

/// Read a connector directory's `connector.json` manifest into a
/// [`ConnectorDescriptor`] (FR-002).
///
/// # Errors
///
/// Returns [`ConnectorError::Io`] when the manifest cannot be read, and
/// [`ConnectorError::ManifestParse`] when it does not deserialise into a
/// descriptor.
pub fn read_manifest(dir: &Path) -> Result<ConnectorDescriptor, ConnectorError> {
    let path = dir.join(MANIFEST_FILE);
    let bytes = std::fs::read(&path).map_err(ConnectorError::io)?;
    serde_json::from_slice(&bytes).map_err(|e| ConnectorError::ManifestParse {
        manifest: path,
        detail: e.to_string(),
    })
}

/// Write `descriptor` as the `connector.json` manifest in `dir` (FR-002).
///
/// The directory is created when missing. The manifest is the serialised
/// descriptor, pretty-printed with a trailing newline so it diffs cleanly.
///
/// # Errors
///
/// Returns [`ConnectorError::Io`] when the directory or file cannot be written,
/// and [`ConnectorError::ManifestParse`] when the descriptor cannot be
/// serialised.
pub fn write_manifest(dir: &Path, descriptor: &ConnectorDescriptor) -> Result<(), ConnectorError> {
    std::fs::create_dir_all(dir).map_err(ConnectorError::io)?;
    let mut bytes = serde_json::to_vec_pretty(descriptor).map_err(ConnectorError::io)?;
    bytes.push(b'\n');
    std::fs::write(dir.join(MANIFEST_FILE), bytes).map_err(ConnectorError::io)
}

/// Install a descriptor directly into `store` (the catalogue-install path).
///
/// This is the commit path shared with [`stage`] for a descriptor that arrives
/// from the catalogue rather than from a source tree, and it is what the
/// `/connectors add <id>` command layer calls after a catalogue lookup. The
/// descriptor is validated (FR-002, FR-025), the id is confined to the store, an
/// existing id is refused unless `force` (FR-027), and the connector is recorded
/// **disabled** so no server starts until `/connectors enable` (FR-011).
///
/// No MCP server is contacted (FR-011).
///
/// # Errors
///
/// See [`StageError`]; a refusal changes no file.
pub fn install_descriptor(
    store: &Path,
    mut descriptor: ConnectorDescriptor,
    force: bool,
) -> Result<StagedConnector, StageError> {
    descriptor.validate()?;
    let dest = resolve_dest(store, descriptor.id.as_str(), force)?;
    write_manifest(&dest, &descriptor)?;
    record_disabled(store, descriptor.id.as_str())?;
    Ok(StagedConnector {
        descriptor,
        installed_dir: dest,
    })
}

/// Install a connector from `source` into the store resolved by `dirs`
/// (FR-011, FR-027).
///
/// `source` is a local directory holding a descriptor, a local `.zip`/`.tar.gz`
/// package holding one, or an `https://` URL naming such a package. A local
/// source is resolved relative to `workdir`. The install targets the project
/// store leg (or the `store_dir` override, which supersedes it), falling back to
/// the global leg only when no project leg resolves.
///
/// When `force` is false and the connector id already exists in the destination
/// store, the install is refused ([`StageError::Exists`], FR-027). A committed
/// connector is recorded **disabled** (FR-011); no MCP server is contacted.
///
/// # Errors
///
/// See [`StageError`]. Staging is removed on failure and a rename into place is
/// the only mutation of the store itself, so a refusal never leaves a
/// half-installed connector.
pub fn stage(
    dirs: &StoreDirs,
    workdir: &Path,
    source: &str,
    force: bool,
) -> Result<StagedConnector, StageError> {
    let store = dest_store(dirs)?;
    std::fs::create_dir_all(&store).map_err(StageError::io)?;
    let staging_root = store.join(STAGING_DIR);
    std::fs::create_dir_all(&staging_root).map_err(StageError::io)?;
    let staging = staging_root.join(staging_name());

    let resolved = match resolve_source(source, workdir, &staging) {
        Ok(resolved) => resolved,
        Err(err) => {
            cleanup_staging(&staging, &staging_root);
            return Err(err);
        }
    };
    let base = match &resolved {
        Staged::Existing(path) | Staged::Extracted(path) => path.clone(),
    };
    let root = descend_single_wrapper(&base);

    let mut descriptor = match load_descriptor(&root) {
        Ok(descriptor) => descriptor,
        Err(err) => {
            cleanup_staging(&staging, &staging_root);
            return Err(err);
        }
    };
    // The install form, not the packaged manifest, decides provenance and
    // source: a descriptor shipped inside an archive is still a local install.
    descriptor.provenance = install_provenance(source);
    descriptor.source = source.to_string();
    if let Err(err) = descriptor.validate() {
        cleanup_staging(&staging, &staging_root);
        return Err(StageError::Descriptor(err));
    }

    let dest = match resolve_dest(&store, descriptor.id.as_str(), force) {
        Ok(dest) => dest,
        Err(err) => {
            cleanup_staging(&staging, &staging_root);
            return Err(err);
        }
    };

    let commit = match &resolved {
        Staged::Existing(_) => copy_dir_recursive(&root, &dest),
        Staged::Extracted(_) => move_or_copy(&root, &dest),
    };
    if let Err(err) = commit {
        cleanup_staging(&staging, &staging_root);
        return Err(err);
    }
    cleanup_staging(&staging, &staging_root);

    // Re-write the manifest from the normalised descriptor so the stored copy
    // carries the resolved provenance and source rather than the packaged ones.
    write_manifest(&dest, &descriptor)?;
    record_disabled(&store, descriptor.id.as_str())?;
    Ok(StagedConnector {
        descriptor,
        installed_dir: dest,
    })
}

/// How the source files arrived at staging.
enum Staged {
    /// The source was a directory; staging IS the source (copied on success).
    Existing(PathBuf),
    /// The source was an archive or a download; files are staged under this
    /// directory and moved into the store on success.
    Extracted(PathBuf),
}

/// Resolve `source` into a staging directory without touching the store.
fn resolve_source(source: &str, workdir: &Path, staging: &Path) -> Result<Staged, StageError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(StageError::UnknownSource(source.to_string()));
    }
    if trimmed.starts_with("http://") {
        return Err(StageError::NotHttps(trimmed.to_string()));
    }
    if trimmed.starts_with("https://") {
        return download_and_extract(trimmed, staging).map(Staged::Extracted);
    }
    if trimmed.contains("://") {
        // Any other scheme (ftp, file, javascript, ...) is refused outright.
        return Err(StageError::NotHttps(trimmed.to_string()));
    }

    let path = {
        let p = Path::new(trimmed);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            workdir.join(p)
        }
    };
    if path.is_dir() {
        return Ok(Staged::Existing(path));
    }
    if path.is_file() {
        return extract_archive_into(&path, staging).map(Staged::Extracted);
    }
    Err(StageError::UnknownSource(trimmed.to_string()))
}

/// Read and validate the descriptor from a staged source tree.
fn load_descriptor(root: &Path) -> Result<ConnectorDescriptor, StageError> {
    if root.join(MANIFEST_FILE).is_file() {
        return read_manifest(root).map_err(StageError::Descriptor);
    }
    match single_json_file(root)? {
        Some(path) => {
            let bytes = std::fs::read(&path).map_err(StageError::io)?;
            serde_json::from_slice(&bytes).map_err(|e| {
                StageError::Descriptor(ConnectorError::ManifestParse {
                    manifest: path,
                    detail: e.to_string(),
                })
            })
        }
        None => Err(StageError::NotAConnector(root.display().to_string())),
    }
}

/// The single `*.json` file directly under `root`, when exactly one exists.
fn single_json_file(root: &Path) -> Result<Option<PathBuf>, StageError> {
    let mut found: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(root).map_err(StageError::io)?.flatten() {
        let path = entry.path();
        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if is_json && path.is_file() {
            found.push(path);
        }
    }
    if found.len() == 1 {
        Ok(found.pop())
    } else {
        Ok(None)
    }
}

/// When a source wraps the connector in one top-level folder that holds the
/// manifest, descend into that folder (GitHub-style `<repo>-<ref>/` wrappers).
fn descend_single_wrapper(root: &Path) -> PathBuf {
    if root.join(MANIFEST_FILE).is_file() {
        return root.to_path_buf();
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return root.to_path_buf();
    };
    let children: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    if children.len() == 1 && children[0].join(MANIFEST_FILE).is_file() {
        return children[0].clone();
    }
    root.to_path_buf()
}

/// The destination store leg: the project store, or the global fallback.
fn dest_store(dirs: &StoreDirs) -> Result<PathBuf, StageError> {
    dirs.destination()
        .map(Path::to_path_buf)
        .ok_or_else(|| StageError::Io("no connector store directory resolvable".to_string()))
}

/// Confine `id` to the store and refuse an existing destination unless `force`
/// (FR-027).
fn resolve_dest(store: &Path, id: &str, force: bool) -> Result<PathBuf, StageError> {
    ConnectorId::new(id).map_err(|err| StageError::InvalidId {
        detail: err.to_string(),
    })?;
    let dest = store.join(id);
    if dest.exists() {
        if !force {
            return Err(StageError::Exists(id.to_string()));
        }
        std::fs::remove_dir_all(&dest).map_err(StageError::io)?;
    }
    Ok(dest)
}

/// Record `id` as disabled in the store ledger so no server starts until
/// `/connectors enable` (FR-011).
fn record_disabled(store: &Path, id: &str) -> Result<(), StageError> {
    let mut ledger = StoreLedger::load(store);
    ledger.state_mut(id).enabled = false;
    ledger.save(store).map_err(StageError::Descriptor)?;
    Ok(())
}

/// The provenance an install form implies.
fn install_provenance(source: &str) -> ConnectorProvenance {
    if source.trim_start().starts_with("https://") {
        ConnectorProvenance::Url
    } else {
        ConnectorProvenance::Local
    }
}

/// A unique per-call staging directory name.
fn staging_name() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("stage-{}-{nanos}", std::process::id())
}

/// Copy a directory tree into the store, refusing symlink entries.
///
/// A symlink inside a user-supplied connector directory is refused with a
/// contained error rather than dereferenced, so a link cannot smuggle a target
/// outside the source tree into the store.
fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), StageError> {
    std::fs::create_dir_all(to).map_err(StageError::io)?;
    for entry in std::fs::read_dir(from).map_err(StageError::io)?.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let file_type = entry.file_type().map_err(StageError::io)?;
        if file_type.is_symlink() {
            return Err(StageError::Io(format!(
                "refusing symlink {} (symlinks are not copied into the store)",
                src.display()
            )));
        }
        if file_type.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(StageError::io)?;
        }
    }
    Ok(())
}

/// Try a rename (the same-filesystem fast path); fall back to copy + remove.
fn move_or_copy(from: &Path, to: &Path) -> Result<(), StageError> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_dir_recursive(from, to)?;
    std::fs::remove_dir_all(from).map_err(StageError::io)
}

/// Remove the per-call staging directory and prune the `.staging` root when it
/// becomes empty.
fn cleanup_staging(staging: &Path, staging_root: &Path) {
    let _ = std::fs::remove_dir_all(staging); // INTENTIONAL: best-effort temp cleanup
    let _ = std::fs::remove_dir(staging_root); // INTENTIONAL: best-effort temp cleanup
}

/// Download and extract an archive URL into `staging`.
fn download_and_extract(url: &str, staging: &Path) -> Result<PathBuf, StageError> {
    let Some(kind) = ArchiveKind::from_path(url) else {
        return Err(StageError::UnknownSource(url.to_string()));
    };
    std::fs::create_dir_all(staging).map_err(StageError::io)?;

    let client = reqwest::blocking::Client::builder()
        // Refuse an https -> http redirect so a plaintext mirror cannot
        // substitute the archive (FR-028).
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "https" {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(StageError::io)?;
    let mut response = client.get(url).send().map_err(StageError::io)?;
    if !response.status().is_success() {
        return Err(StageError::Io(format!(
            "download {url}: HTTP {}",
            response.status()
        )));
    }

    if let Some(len) = response.content_length()
        && len > MAX_SOURCE_BYTES
    {
        return Err(StageError::TooLarge {
            size: len,
            max: MAX_SOURCE_BYTES,
        });
    }

    use std::io::Read as _;
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; STREAM_CHUNK_BYTES];
    loop {
        let read = response
            .read(&mut chunk)
            .map_err(|e| StageError::Io(format!("download {url}: {e}")))?;
        if read == 0 {
            break;
        }
        if (bytes.len() + read) as u64 > MAX_SOURCE_BYTES {
            return Err(StageError::TooLarge {
                size: (bytes.len() + read) as u64,
                max: MAX_SOURCE_BYTES,
            });
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    extract_archive(&bytes, kind, staging)
}

/// Which archive format a package uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveKind {
    Zip,
    TarGz,
}

impl ArchiveKind {
    fn from_path(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".tar.gz") {
            return Some(Self::TarGz);
        }
        match Path::new(&lower)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("zip") => Some(Self::Zip),
            Some("tgz") => Some(Self::TarGz),
            _ => None,
        }
    }
}

/// Extract a local archive file into `staging`, returning the extraction root.
fn extract_archive_into(file: &Path, staging: &Path) -> Result<PathBuf, StageError> {
    let Some(kind) = ArchiveKind::from_path(&file.to_string_lossy()) else {
        return Err(StageError::UnknownSource(file.display().to_string()));
    };
    let size = file.metadata().map_err(StageError::io)?.len();
    if size > MAX_SOURCE_BYTES {
        return Err(StageError::TooLarge {
            size,
            max: MAX_SOURCE_BYTES,
        });
    }
    let bytes = std::fs::read(file).map_err(StageError::io)?;
    extract_archive(&bytes, kind, staging)
}

/// Extract `bytes` (a zip or tar.gz archive) into `staging`, enforcing the
/// per-entry path guard, and return the extraction root (`staging` itself).
fn extract_archive(bytes: &[u8], kind: ArchiveKind, staging: &Path) -> Result<PathBuf, StageError> {
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(StageError::TooLarge {
            size: bytes.len() as u64,
            max: MAX_SOURCE_BYTES,
        });
    }
    std::fs::create_dir_all(staging).map_err(StageError::io)?;
    match kind {
        ArchiveKind::Zip => extract_zip(bytes, staging)?,
        ArchiveKind::TarGz => extract_tar_gz(bytes, staging)?,
    }
    Ok(staging.to_path_buf())
}

/// Extract a zip archive, bounding both the per-entry and the total decompressed
/// size.
fn extract_zip(bytes: &[u8], staging: &Path) -> Result<(), StageError> {
    use std::io::{Cursor, Read as _};

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| StageError::Io(format!("zip open: {e}")))?;
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| StageError::Io(format!("zip entry {i}: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let rel = safe_relative_path(&name)?;
        let dest = staging.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(StageError::io)?;
        }
        let mut body = Vec::new();
        entry
            .read_to_end(&mut body)
            .map_err(|e| StageError::Io(format!("zip entry {name}: {e}")))?;
        if body.len() as u64 > MAX_SOURCE_BYTES {
            return Err(StageError::TooLarge {
                size: body.len() as u64,
                max: MAX_SOURCE_BYTES,
            });
        }
        total = total.saturating_add(body.len() as u64);
        if total > MAX_EXTRACTED_BYTES {
            return Err(StageError::Io(format!(
                "archive decompressed past the {MAX_EXTRACTED_BYTES} byte total cap"
            )));
        }
        std::fs::write(&dest, &body).map_err(StageError::io)?;
    }
    Ok(())
}

/// Extract a `.tar.gz` archive, bounding both the per-entry and the total
/// decompressed size.
fn extract_tar_gz(bytes: &[u8], staging: &Path) -> Result<(), StageError> {
    use std::io::{Cursor, Read as _};

    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes.to_vec()));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| StageError::Io(format!("tar entries: {e}")))?;
    let mut total: u64 = 0;
    for entry in entries {
        let mut entry = entry.map_err(|e| StageError::Io(format!("tar entry: {e}")))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| StageError::Io(format!("tar entry path: {e}")))?
            .to_path_buf();
        let name = path.to_string_lossy().replace('\\', "/");
        let rel = safe_relative_path(&name)?;
        let dest = staging.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(StageError::io)?;
        }
        let mut body = Vec::new();
        entry
            .read_to_end(&mut body)
            .map_err(|e| StageError::Io(format!("tar entry {name}: {e}")))?;
        if body.len() as u64 > MAX_SOURCE_BYTES {
            return Err(StageError::TooLarge {
                size: body.len() as u64,
                max: MAX_SOURCE_BYTES,
            });
        }
        total = total.saturating_add(body.len() as u64);
        if total > MAX_EXTRACTED_BYTES {
            return Err(StageError::Io(format!(
                "archive decompressed past the {MAX_EXTRACTED_BYTES} byte total cap"
            )));
        }
        std::fs::write(&dest, &body).map_err(StageError::io)?;
    }
    Ok(())
}

/// Reject an archive entry that would escape the store: an absolute path, a
/// Windows prefix, or any `..` component (FR-029).
///
/// The rule delegates to the shared
/// [`validate_relative_component`](ragent_types::guard::validate_relative_component)
/// guard (ANTIPAT M14) so the archive sink and the file tools cannot drift apart.
fn safe_relative_path(name: &str) -> Result<PathBuf, StageError> {
    let normalised = name.replace('\\', "/");
    if validate_relative_component(&normalised, "archive entry").is_err() {
        return Err(StageError::UnsafePath(name.to_string()));
    }
    Ok(PathBuf::from(normalised))
}
