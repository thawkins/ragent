//! `/plugins add` source handling: local directory, local `.zip`/`.tar.gz`
//! package, `https://` URL package file, or `git+<https-url>#<ref>[:<path>]`
//! git source (spec `plugins` T-006; FR-007, FR-010, FR-023).
//!
//! Four source forms are accepted, handled by [`add`]:
//!
//! - an existing local **directory**, copied into the store;
//! - a local **`.zip` / `.tar.gz`** package file, extracted into the store;
//! - an **`https://` URL** ending in `.zip` / `.tar.gz`, downloaded into the
//!   store's staging directory and then extracted;
//! - a **`git+<https-url>#<ref>[:<subpath>]`** git source, shallow-cloned into
//!   staging (a sparse checkout of `<subpath>` when present) so a plugin hosted
//!   in a subdirectory of a larger repository installs without downloading the
//!   whole tree. This is the form the store providers emit for vendor
//!   marketplace entries.
//!
//! Guards (all refuse with a structured error and leave the store untouched):
//!
//! - non-`https` URLs are refused ([`AddError::NotHttps`]);
//! - archives larger than [`MAX_ARCHIVE_BYTES`] (50 MiB) are refused
//!   ([`AddError::TooLarge`]);
//! - archive entries with `..` components or absolute paths are refused
//!   ([`AddError::UnsafePath`]);
//! - an existing plugin id refuses unless `force` ([`AddError::Exists`]);
//! - the installed tree must parse as a valid plugin manifest
//!   ([`AddError::NotAPlugin`] - no manifest, or [`AddError::Manifest`]).
//!
//! A manifest that contributes no JavaScript (a skill-only or MCP-only plugin,
//! the shape the official Claude marketplace ships) parses with no entry point
//! and installs normally; the plugin is inert until ragent gains a skills or
//! MCP-server bridge.
//!
//! After install the manifest is parsed and validated; the plugin is recorded
//! **enabled** in the store ledger (its tools/commands load at the next
//! session start) but no JavaScript executes during the install itself
//! (FR-023). Install targets the project store leg (or the `store_dir`
//! override, which supersedes it). Installs go through a staging directory
//! renamed into place so a failure never leaves a half-written plugin in the
//! store.

use std::path::{Component, Path, PathBuf};

use sha2::Digest as _;

use ragent_tools_core::guard::validate_identifier;

use crate::descriptor::detect_dialect;
use crate::error::{IoError, PluginError};
use crate::manifest::{ParsedManifest, parse_plugin_dir};
use crate::store::{StoreDirs, StoreLedger};

/// Maximum accepted archive size: 50 MiB (FR-010 size cap).
pub const MAX_ARCHIVE_BYTES: u64 = 50 * 1024 * 1024;

/// Maximum total decompressed bytes extracted from one archive (ANTIPAT M17).
///
/// The per-entry cap alone is not enough: an archive holding many
/// just-under-the-cap entries wrote unbounded data into the store. The running
/// total across all entries is bounded by this value.
pub const MAX_EXTRACTED_BYTES: u64 = 256 * 1024 * 1024;

/// Wall-clock budget for one plugin archive download (ANTIPAT M19).
///
/// Was a bare `from_secs(60)` inside [`download_and_extract`]; naming it keeps
/// the crate's other limits (`MAX_ARCHIVE_BYTES`, `FetchLimits::DEFAULT_TIMEOUT`)
/// company.
pub const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Read/write chunk size for streaming a download to disk.
const STREAM_CHUNK_BYTES: usize = 65_536;

/// The outcome of a successful [`add`].
#[derive(Debug)]
pub struct AddOutcome {
    /// The parsed, validated manifest of the installed plugin.
    pub parsed: ParsedManifest,
    /// The directory the plugin was installed into.
    pub installed_dir: PathBuf,
}

/// A refusal or failure from [`add`].
#[derive(Debug, thiserror::Error)]
pub enum AddError {
    /// The URL uses a scheme other than `https` (FR-010).
    #[error("plugin add: refusing non-https URL {0}")]
    NotHttps(String),

    /// The archive exceeds the [`MAX_ARCHIVE_BYTES`] size cap.
    #[error("plugin add: archive is {size} bytes, over the {max} byte cap")]
    TooLarge {
        /// Archive size in bytes.
        size: u64,
        /// Cap in bytes.
        max: u64,
    },

    /// An archive entry path contains `..` or is absolute (FR-010).
    #[error("plugin add: refusing unsafe archive entry path {0}")]
    UnsafePath(String),

    /// A plugin with the same id already exists in the store and `force` was
    /// not supplied.
    #[error("plugin add: plugin id {0} already exists (use --force to overwrite)")]
    Exists(String),

    /// The source (or the extracted archive) contains no recognisable plugin
    /// manifest, and the install source carried no marketplace-inline manifest
    /// to materialise (see [`crate::marketplace`]).
    #[error("plugin add: no plugin manifest found in {0}")]
    NotAPlugin(String),

    /// The installed manifest failed to parse.
    #[error("plugin add: {0}")]
    Manifest(#[from] PluginError),

    /// The source string matches no accepted form.
    #[error(
        "plugin add: unrecognised source {0} (expected a directory, a .zip/.tar.gz file, an https:// URL, or a git+<https-url>#<ref>:<path> source)"
    )]
    UnknownSource(String),

    /// A network or I/O failure during download/copy/extract. Uses [`IoError`]
    /// so the underlying error's source chain survives (ANTIPAT M15).
    #[error("plugin add: {0}")]
    Io(#[from] IoError),
}

/// How the plugin files arrived at staging.
enum Staging {
    /// The source was a directory; staging IS the source (no copy - we rename
    /// nothing, we copy on success).
    Existing(PathBuf),
    /// The source was an archive or a git checkout; files are staged under this
    /// directory and moved into the store on success.
    Extracted(PathBuf),
}

/// Install a plugin into the store resolved by `dirs` (FR-007, FR-010).
///
/// `source` is one of the forms in the module docs. When `force` is false and
/// the plugin id already exists in the destination store, the install is
/// refused. The install goes to the project leg of `dirs` (or the override),
/// falling back to the global leg only when no project leg resolves.
///
/// # Errors
///
/// See [`AddError`]; every refusal leaves the store untouched (staging is
/// removed on failure; a rename into place is the only mutation of the store
/// itself, and only happens after validation has passed).
pub fn add(
    dirs: &StoreDirs,
    workdir: &Path,
    source: &str,
    force: bool,
) -> Result<AddOutcome, AddError> {
    let dest_store = dirs
        .project
        .clone()
        .or_else(|| dirs.global.clone())
        .ok_or_else(|| AddError::Io(IoError::message("no plugin store directory resolvable")))?;
    std::fs::create_dir_all(&dest_store).map_err(io_err)?;

    let staging_root = dest_store.join(".add-staging");
    std::fs::create_dir_all(&staging_root).map_err(io_err)?;
    // Unique staging dir per call; process id plus a nanosecond timestamp.
    let staging = staging_root.join(format!(
        "stage-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let result = add_inner(source, workdir, &staging);
    let staged = match result {
        Ok(staged) => staged,
        Err(err) => {
            let _ = std::fs::remove_dir_all(&staging); // INTENTIONAL: best-effort temp cleanup
            let _ = remove_if_empty(&staging_root); // INTENTIONAL: best-effort temp cleanup
            return Err(err);
        }
    };

    // Single-root extraction: if the staged tree holds exactly one directory
    // and no manifest at its own root, descend into that directory. GitHub-style
    // zips wrap the plugin in `<repo>-<ref>/`.
    let staged_root = match &staged {
        Staging::Existing(path) => path.clone(),
        Staging::Extracted(path) => descend_single_wrapper(path),
    };

    // Marketplace-inline manifest materialisation: when a store-browser git
    // source carried the `@<key>` suffix of a recorded marketplace document
    // and the checked-out tree holds no manifest (a manifest-less stub such
    // as the Claude marketplace's `*-lsp` entries), write the inline manifest
    // into `.claude-plugin/plugin.json` before validation. A tree that
    // already has a manifest is untouched (an upstream fix wins
    // automatically); a source without the suffix installs exactly as before.
    let staged_root = materialize_marketplace_manifest(staged_root, source);

    // Validate before touching the store: recognise + parse the manifest.
    let parsed = match detect_dialect(&staged_root) {
        Ok(Some(_)) => match parse_plugin_dir(&staged_root) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => {
                cleanup_staging(&staging, &staging_root);
                return Err(AddError::NotAPlugin(staged_root.display().to_string()));
            }
            Err(err) => {
                cleanup_staging(&staging, &staging_root);
                return Err(AddError::Manifest(err));
            }
        },
        Ok(None) => {
            cleanup_staging(&staging, &staging_root);
            return Err(AddError::NotAPlugin(staged_root.display().to_string()));
        }
        Err(err) => {
            cleanup_staging(&staging, &staging_root);
            return Err(AddError::Manifest(err));
        }
    };

    let id = parsed.descriptor.id.clone();
    // SEC-ragent-plugins-002 (T-002): the id becomes a directory name and may
    // be recursively deleted below, so it is confined to the store root before
    // it is used as a path. The manifest parsers already reject a non-component
    // id, but the sink re-asserts containment so a future parser change cannot
    // silently reintroduce an arbitrary write/delete primitive.
    let dest_dir = confined_dest_dir(&dest_store, &id)?;

    // ANTIPAT H2: record a deterministic digest of the staged content so a
    // substituted archive/checkout is at least detectable after the fact. The
    // digest is computed over the staged tree *before* the commit below, which
    // is the exact byte content that lands in the store.
    let staged_digest = content_digest(&staged_root)?;

    if dest_dir.exists() {
        if !force {
            cleanup_staging(&staging, &staging_root);
            return Err(AddError::Exists(id));
        }
        std::fs::remove_dir_all(&dest_dir).map_err(io_err)?;
    }

    // Commit: copy (existing dir) or move (extracted staging) into the store.
    let commit = match &staged {
        Staging::Existing(_) => copy_dir_recursive(&staged_root, &dest_dir).map(|_| ()),
        Staging::Extracted(_) => move_or_copy(&staged_root, &dest_dir),
    };
    if let Err(err) = commit {
        cleanup_staging(&staging, &staging_root);
        return Err(err);
    }
    cleanup_staging(&staging, &staging_root);

    // Re-parse from the installed location so paths in the descriptor point at
    // the store, not at staging.
    let installed_parsed = parse_plugin_dir(&dest_dir)
        .map_err(AddError::Manifest)?
        .ok_or_else(|| AddError::NotAPlugin(dest_dir.display().to_string()))?;

    // Record the plugin enabled in its store ledger so it loads at the next
    // session start. No JavaScript runs here (FR-023); enablement only sets the
    // persisted flag. The ledger lives in `dest_store` (the leg the install
    // targeted), matching where `/plugins enable` would write it.
    let mut ledger = StoreLedger::load(&dest_store);
    let state = ledger.state_mut(&id);
    state.enabled = true;
    // ANTIPAT H2: persist the observed digest of the installed content so a
    // later content swap is detectable (and so `/plugins` can display it).
    state.content_digest = Some(staged_digest.clone());
    ledger.save(&dest_store).map_err(|e| {
        AddError::Manifest(PluginError::Io(format!(
            "plugin installed but the enable flag could not be persisted: {e}"
        )))
    })?;
    tracing::info!(
        plugin = %id,
        digest = %staged_digest,
        "plugin installed; recorded content digest in the store ledger"
    );

    Ok(AddOutcome {
        parsed: installed_parsed,
        installed_dir: dest_dir,
    })
}

/// Resolve and materialise `source` into a staging area, without touching the
/// store.
///
/// The source is stripped of a trailing marketplace-inline manifest key (the
/// `@<sha256>` suffix a store browser attaches to a manifest-less stub's
/// source; see [`crate::marketplace`]) before it is classified: the suffix is
/// an install-time annotation, not part of the git/URL/directory form.
fn add_inner(source: &str, workdir: &Path, staging: &Path) -> Result<Staging, AddError> {
    let (source, _key) = crate::marketplace::split_manifest_key(source);
    // A git source (`git+<https-url>#<ref>[:<subpath>]`) is cloned into staging.
    // Handled before the plain-URL branch because it shares the `https` prefix.
    if source.starts_with("git+") {
        return git_clone_stage(source, staging).map(Staging::Extracted);
    }
    if source.starts_with("http://") {
        return Err(AddError::NotHttps(source.to_string()));
    }
    if source.strip_prefix("https://").is_some() {
        return download_and_extract(source, staging).map(Staging::Extracted);
    }
    if source.contains("://") {
        // Any other scheme (ftp, file, javascript, ...) is refused outright.
        return Err(AddError::NotHttps(source.to_string()));
    }

    let path = {
        let p = Path::new(source);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            workdir.join(p)
        }
    };

    if path.is_dir() {
        return Ok(Staging::Existing(path));
    }
    if path.is_file() {
        let extracted = extract_archive_into(&path, staging).map_err(classify_extract_failure)?;
        return Ok(Staging::Extracted(extracted));
    }
    Err(AddError::UnknownSource(source.to_string()))
}

/// Classify an extraction failure into a structured [`AddError`].
///
/// An [`ExtractError`] carries either a structured [`std::io::Error`] (whose
/// source chain is preserved) or an already-formatted message.
fn classify_extract_failure(err: ExtractError) -> AddError {
    match err {
        ExtractError::UnsafePath(p) => AddError::UnsafePath(p),
        ExtractError::Io(io) => AddError::Io(IoError::new(io)),
        ExtractError::Message(msg) => AddError::Io(IoError::message(msg)),
    }
}

/// A parsed `git+<https-url>#<ref>[:<subpath>]` source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSource {
    /// The `https` repository remote.
    pub url: String,
    /// The ref to check out; `None` uses the remote default branch (HEAD).
    pub ref_name: Option<String>,
    /// A repository-relative subdirectory to install; `None` installs the root.
    pub subpath: Option<String>,
}

/// Parse a `git+<https-url>#<ref>[:<subpath>]` source string.
///
/// Only `https://` remotes are accepted for a store-supplied source; a
/// `file://` remote is additionally accepted so a local git mirror can be
/// installed, mirroring the local-directory source the same command already
/// accepts. Every other scheme (ssh, git, http) returns `None`, so it can never
/// reach the clone (FR-024).
#[must_use]
pub fn parse_git_source(source: &str) -> Option<GitSource> {
    let rest = source.strip_prefix("git+")?;
    let (url, fragment) = match rest.split_once('#') {
        Some((url, fragment)) => (url, Some(fragment)),
        None => (rest, None),
    };
    if url.trim().is_empty() || !(url.starts_with("https://") || url.starts_with("file://")) {
        return None;
    }
    let (ref_name, subpath) = match fragment {
        None => (None, None),
        Some(fragment) => match fragment.split_once(':') {
            Some((r, path)) => (git_ref(r), git_subpath(path)),
            None => (git_ref(fragment), None),
        },
    };
    Some(GitSource {
        url: url.to_string(),
        ref_name,
        subpath,
    })
}

/// A git ref, treating an empty value or `HEAD` as "the default branch".
///
/// The ref is validated against a safe charset before it can reach `git`:
/// a `git+` source is untrusted content (it is emitted from a marketplace
/// document fetched over the network or typed by a social-engineered user),
/// and `git` parses a leading `-` as an *option*, not a positional. Options
/// such as `--upload-pack=<cmd>` execute a program, which escapes the plugin
/// sandbox entirely (SEC-ragent-plugins-001). Anything that is not
/// `[A-Za-z0-9._/-]+` - and anything beginning with `-` - is refused.
fn git_ref(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("HEAD") {
        None
    } else if is_safe_git_argument(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// A repository-relative subpath, normalised without a leading `./`.
///
/// Validated with the same charset as [`git_ref`]: the subpath reaches
/// `git sparse-checkout set <subpath>`, where a leading `-` would again be
/// parsed as an option (SEC-ragent-plugins-001).
fn git_subpath(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches("./");
    if trimmed.is_empty() {
        None
    } else if is_safe_git_argument(trimmed) && !trimmed.contains("..") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Return `true` when `value` is safe to hand to `git` as a positional
/// argument: a non-empty `[A-Za-z0-9._/-]` string that does not begin with
/// `-` (which git would parse as an option) and contains no `..` segment.
///
/// See SEC-ragent-plugins-001: without this gate, a `git+` ref of
/// `--upload-pack=/bin/sh -c true` reaches `Command::new("git")` and executes
/// a program with the user's privileges.
///
/// Delegates to the shared [`ragent_tools_core::guard::is_safe_operand`]
/// (SECTASKS MS-05 T-068) and adds this crate's stricter `..` rule.
fn is_safe_git_argument(value: &str) -> bool {
    ragent_tools_core::guard::is_safe_operand(value) && !value.contains("..")
}

/// Materialise a marketplace-inline manifest for a manifest-less stub.
///
/// When `source` carried the `@<key>` suffix a store browser attaches to a
/// manifest-less stub (see [`crate::marketplace`]), the plugin name is derived
/// from the source's git subpath (its final path segment), and the recorded
/// inline manifest is written into `.claude-plugin/plugin.json` under
/// `staged_root` (or, for a sparse checkout layout, `<staged_root>/<name>`).
/// Any other source - no suffix, an unrecorded key, a tree that already has
/// a recognisable manifest - returns `staged_root` unchanged.
fn materialize_marketplace_manifest(staged_root: PathBuf, source: &str) -> PathBuf {
    // Strip the key first so the name parse below never sees the suffix.
    let (stripped, Some(key)) = crate::marketplace::split_manifest_key(source) else {
        return staged_root;
    };
    // The plugin name is the subpath's final segment
    // (`git+...#main:plugins/rust-analyzer-lsp` -> `rust-analyzer-lsp`).
    let name = stripped
        .rsplit(['#', ':', '/'])
        .map(str::trim)
        .find(|piece| !piece.is_empty());
    let Some(name) = name else {
        return staged_root;
    };
    crate::marketplace::materialize_at(staged_root, name, Some(key))
}

/// Clone a `git+<https-url>#<ref>[:<subpath>]` source into `staging`.
///
/// Installs a whole repository, or a single subdirectory when a `:<subpath>`
/// suffix is present, via a shallow sparse `git clone`, so a large marketplace
/// repository is not downloaded in full. Git must be on `PATH`; a clone/fetch
/// failure is a contained [`AddError::Io`] and git is never allowed to prompt
/// interactively (credentials for a private repo are refused, not requested).
fn git_clone_stage(source: &str, staging: &Path) -> Result<PathBuf, AddError> {
    let Some(spec) = parse_git_source(source) else {
        return Err(AddError::UnknownSource(source.to_string()));
    };
    std::fs::create_dir_all(staging).map_err(io_err)?;
    let repo_dir = staging.join("repo");
    let repo = repo_dir.to_string_lossy().into_owned();

    git_run(
        &["clone", "--depth", "1", "--sparse", &spec.url, &repo],
        None,
    )?;
    if let Some(ref_name) = spec.ref_name.as_deref() {
        // `--` terminates option parsing: even though `git_ref` already rejects
        // a leading `-`, the separator keeps that invariant enforced by git
        // itself if the validator is ever relaxed (SEC-ragent-plugins-001).
        git_run(
            &["fetch", "--depth", "1", "--", "origin", ref_name],
            Some(&repo_dir),
        )?;
        git_run(&["checkout", "--", "FETCH_HEAD"], Some(&repo_dir))?;
    }
    if let Some(subpath) = spec.subpath.as_deref() {
        git_run(&["sparse-checkout", "set", "--", subpath], Some(&repo_dir))?;
    }

    let root = match spec.subpath.as_deref() {
        Some(subpath) => repo_dir.join(subpath),
        None => repo_dir,
    };
    if !root.is_dir() {
        return Err(AddError::NotAPlugin(root.display().to_string()));
    }
    Ok(root)
}

/// Join a plugin id onto the store root, refusing anything that is not a
/// single safe directory component inside that root.
///
/// A plugin id is a path-shaped value that arrives from untrusted content (a
/// manifest fetched from a marketplace or a local directory the user was
/// talked into installing). `Path::join` honours `..` segments and *replaces*
/// the base entirely when the argument is absolute, so an id of
/// `../../../../home/user/.ssh` or `/home/user/.config/autostart` would write
///   and, on `--force`, recursively delete - outside the store
/// (SEC-ragent-plugins-002). Only `[A-Za-z0-9._-]+` is accepted.
///
/// Delegates the charset + single-component rule to the shared
/// [`ragent_tools_core::guard::validate_identifier`] predicate (ANTIPAT M14) so the
/// install sink and the manifest parser (`manifest::sanitize_declared_id`) can
/// never drift apart.
fn confined_dest_dir(store: &Path, id: &str) -> Result<PathBuf, AddError> {
    if validate_identifier(id, "plugin id").is_err() {
        return Err(AddError::Io(IoError::message(format!(
            "plugin add: refusing plugin id {id:?}; an id must be a single component matching [A-Za-z0-9._-]+"
        ))));
    }
    Ok(store.join(id))
}

/// Run one non-interactive `git` invocation, returning a contained error on a
/// missing binary or a non-zero exit.
fn git_run(args: &[&str], cwd: Option<&Path>) -> Result<(), AddError> {
    let verb = args.first().copied().unwrap_or("git");
    let mut command = std::process::Command::new("git");
    command
        .args(args)
        // Never prompt for credentials; a private repo fails instead of hanging.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "");
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let output = command
        .output()
        .map_err(|e| AddError::Io(IoError::message(format!("git {verb}: {e}"))))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AddError::Io(IoError::message(format!(
            "git {verb}: {}",
            stderr.trim()
        ))));
    }
    Ok(())
}

/// Download and extract an archive URL into `staging`.
fn download_and_extract(url: &str, staging: &Path) -> Result<PathBuf, AddError> {
    let Some(kind) = ArchiveKind::from_path(url) else {
        return Err(AddError::UnknownSource(url.to_string()));
    };

    std::fs::create_dir_all(staging).map_err(io_err)?;
    let archive_path = staging.join("download.archive");

    let client = reqwest::blocking::Client::builder()
        // SEC-ragent-plugins-005 (SECTASKS T-062): `Policy::limited` follows an
        // https -> http downgrade. Reuse the store-fetch policy that stops any
        // non-https redirect, so a plaintext mirror cannot substitute the archive.
        .redirect(crate::store_fetch::https_only_redirects())
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|e| AddError::Io(IoError::message(format!("http client: {e}"))))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| AddError::Io(IoError::message(format!("download {url}: {e}"))))?;
    if !response.status().is_success() {
        return Err(AddError::Io(IoError::message(format!(
            "download {url}: HTTP {}",
            response.status()
        ))));
    }

    // Enforce the size cap; use Content-Length when available, then stream with
    // a byte counter so an unbounded body still trips the cap.
    if let Some(len) = response.content_length()
        && len > MAX_ARCHIVE_BYTES
    {
        return Err(AddError::TooLarge {
            size: len,
            max: MAX_ARCHIVE_BYTES,
        });
    }
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; STREAM_CHUNK_BYTES];
    use std::io::Read as _;
    loop {
        let read = response
            .read(&mut chunk)
            .map_err(|e| AddError::Io(IoError::message(format!("download {url}: {e}"))))?;
        if read == 0 {
            break;
        }
        if (bytes.len() + read) as u64 > MAX_ARCHIVE_BYTES {
            return Err(AddError::TooLarge {
                size: (bytes.len() + read) as u64,
                max: MAX_ARCHIVE_BYTES,
            });
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    std::fs::write(&archive_path, &bytes).map_err(io_err)?;

    extract_archive(&bytes, kind, staging, Some(&archive_path)).map_err(classify_extract_failure)
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
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
            .and_then(|e| e.to_str())
        {
            Some("zip") => Some(Self::Zip),
            Some("tgz") => Some(Self::TarGz),
            _ => None,
        }
    }
}

/// Extraction failure (converted to [`AddError`] at the module boundary).
///
/// The `Io` arm keeps the structured [`std::io::Error`] so its source chain
/// survives the conversion to [`AddError::Io`] (ANTIPAT M15); a failure that
/// only has an already-formatted detail uses `Message`.
#[derive(Debug)]
enum ExtractError {
    UnsafePath(String),
    Io(std::io::Error),
    Message(String),
}

/// Extract a local archive file into `staging`, returning the extraction root.
fn extract_archive_into(file: &Path, staging: &Path) -> Result<PathBuf, ExtractError> {
    let Some(kind) = ArchiveKind::from_path(&file.to_string_lossy()) else {
        return Err(ExtractError::Message(format!(
            "unsupported archive type for {}",
            file.display()
        )));
    };
    let size = file.metadata().map_err(ExtractError::Io)?.len();
    if size > MAX_ARCHIVE_BYTES {
        return Err(ExtractError::Message(format!(
            "archive is {size} bytes, over the {MAX_ARCHIVE_BYTES} byte cap"
        )));
    }
    let bytes = std::fs::read(file).map_err(ExtractError::Io)?;
    extract_archive(&bytes, kind, staging, None)
}

/// Extract `bytes` (a zip or tar.gz archive) into `staging`, enforcing the
/// per-entry path guard, and return the extraction root (`staging` itself).
fn extract_archive(
    bytes: &[u8],
    kind: ArchiveKind,
    staging: &Path,
    remove_after: Option<&Path>,
) -> Result<PathBuf, ExtractError> {
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(ExtractError::Message(format!(
            "archive is {} bytes, over the {MAX_ARCHIVE_BYTES} byte cap",
            bytes.len()
        )));
    }
    std::fs::create_dir_all(staging).map_err(ExtractError::Io)?;
    let result = match kind {
        ArchiveKind::Zip => extract_zip(bytes, staging),
        ArchiveKind::TarGz => extract_tar_gz(bytes, staging),
    };
    if let Some(path) = remove_after {
        let _ = std::fs::remove_file(path); // INTENTIONAL: best-effort temp cleanup
    }
    result.map(|_| staging.to_path_buf())
}

fn extract_zip(bytes: &[u8], staging: &Path) -> Result<(), ExtractError> {
    use std::io::{Cursor, Read as _};

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| ExtractError::Message(format!("zip open: {e}")))?;
    // ANTIPAT M17: the per-entry cap does not bound the total, so track a
    // running total across every entry and abort past MAX_EXTRACTED_BYTES.
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ExtractError::Message(format!("zip entry {i}: {e}")))?;
        let name = entry.name().to_string();
        if entry.is_dir() {
            continue;
        }
        let rel = safe_relative_path(&name)?;
        let dest = staging.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(ExtractError::Io)?;
        }
        let mut body = Vec::new();
        entry
            .read_to_end(&mut body)
            .map_err(|e| ExtractError::Message(format!("zip entry {name}: {e}")))?;
        if body.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(ExtractError::Message(format!(
                "zip entry {name} decompressed past the size cap"
            )));
        }
        total = total.saturating_add(body.len() as u64);
        if total > MAX_EXTRACTED_BYTES {
            return Err(ExtractError::Message(format!(
                "archive decompressed past the {MAX_EXTRACTED_BYTES} byte total cap"
            )));
        }
        std::fs::write(&dest, &body).map_err(ExtractError::Io)?;
    }
    Ok(())
}

fn extract_tar_gz(bytes: &[u8], staging: &Path) -> Result<(), ExtractError> {
    use std::io::{Cursor, Read as _};

    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes.to_vec()));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| ExtractError::Message(format!("tar entries: {e}")))?;
    // ANTIPAT M17: bound the total decompressed size across all entries.
    let mut total: u64 = 0;
    for entry in entries {
        let mut entry = entry.map_err(|e| ExtractError::Message(format!("tar entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| ExtractError::Message(format!("tar entry path: {e}")))?
            .to_path_buf();
        let name = path.to_string_lossy().replace('\\', "/");
        let rel = safe_relative_path(&name)?;
        if !(entry.header().entry_type().is_file()) {
            continue;
        }
        let dest = staging.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(ExtractError::Io)?;
        }
        let mut body = Vec::new();
        entry
            .read_to_end(&mut body)
            .map_err(|e| ExtractError::Message(format!("tar entry {name}: {e}")))?;
        if body.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(ExtractError::Message(format!(
                "tar entry {name} decompressed past the size cap"
            )));
        }
        total = total.saturating_add(body.len() as u64);
        if total > MAX_EXTRACTED_BYTES {
            return Err(ExtractError::Message(format!(
                "archive decompressed past the {MAX_EXTRACTED_BYTES} byte total cap"
            )));
        }
        std::fs::write(&dest, &body).map_err(ExtractError::Io)?;
    }
    Ok(())
}

/// Reject absolute paths and any path containing a `..` component; normalise
/// separators. Returns the sanitised relative path on success.
fn safe_relative_path(name: &str) -> Result<PathBuf, ExtractError> {
    let normalised = name.replace('\\', "/");
    let rel = Path::new(&normalised);
    if rel.is_absolute() {
        return Err(ExtractError::UnsafePath(name.to_string()));
    }
    for component in rel.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            // ParentDir escapes the store; RootDir/Prefix are absolute
            // (handled above, but keep the match exhaustive).
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ExtractError::UnsafePath(name.to_string()));
            }
        }
    }
    Ok(rel.to_path_buf())
}

/// When an archive wraps the plugin in one top-level folder with no manifest
/// at the archive root, descend into that folder.
fn descend_single_wrapper(root: &Path) -> PathBuf {
    let Ok(entries) = std::fs::read_dir(root) else {
        return root.to_path_buf();
    };
    let entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    if entries.len() == 1 && entries[0].is_dir() {
        // Descend only if the wrapper has no manifest of its own but the
        // child does.
        if detect_dialect(root).ok().flatten().is_none()
            && detect_dialect(&entries[0]).ok().flatten().is_some()
        {
            return entries[0].clone();
        }
    }
    root.to_path_buf()
}

/// Copy a directory tree, refusing symlink entries (ANTIPAT M18).
///
/// The previous implementation used `entry.file_type()` (from `read_dir`, i.e.
/// `symlink_metadata`) only to choose directory-vs-file, then called
/// `std::fs::copy`, which *follows* the link. A symlink inside a
/// user-supplied plugin directory was therefore dereferenced and the target's
/// contents copied into the store. A symlink is now refused with a contained
/// error rather than dereferenced.
fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), AddError> {
    std::fs::create_dir_all(to).map_err(io_err)?;
    for entry in std::fs::read_dir(from).map_err(io_err)?.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let ft = entry.file_type().map_err(io_err)?;
        if ft.is_symlink() {
            return Err(AddError::Io(IoError::message(format!(
                "plugin install: refusing symlink {} (symlinks are not copied into the store)",
                src.display()
            ))));
        }
        if ft.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(io_err)?;
        }
    }
    Ok(())
}

/// Try rename (same filesystem fast path); fall back to copy + remove.
fn move_or_copy(from: &Path, to: &Path) -> Result<(), AddError> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_dir_recursive(from, to)?;
    std::fs::remove_dir_all(from).map_err(io_err)
}

/// Remove the per-call staging directory entirely and prune the `.add-staging`
/// root when it becomes empty.
///
/// Removing the whole per-call directory (not just the extracted plugin root)
/// also discards the transient git working tree created for a
/// `git+<url>#<ref>:<subpath>` source, whose `.git` metadata lives *above* the
/// selected subpath: leaving it behind would leak a checkout into the store and
/// make the parent directory unremovable.
fn cleanup_staging(staging: &Path, staging_root: &Path) {
    let _ = std::fs::remove_dir_all(staging); // INTENTIONAL: best-effort temp cleanup
    let _ = remove_if_empty(staging_root); // INTENTIONAL: best-effort temp cleanup
}

/// Remove the staging directory, ignoring "not empty" (the caller only uses
/// this once the staged contents have been moved or copied out).
fn remove_if_empty(dir: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir(dir); // INTENTIONAL: best-effort temp cleanup
    Ok(())
}

fn io_err(e: std::io::Error) -> AddError {
    AddError::Io(IoError::new(e))
}

/// Compute a deterministic SHA-256 digest of a directory tree's contents.
///
/// ANTIPAT H2: plugin installs had no integrity record at all, so a
/// TLS-terminating mirror (or a swapped local directory) could substitute the
/// payload undetected. Hashing the staged tree before it is committed to the
/// store gives a stable fingerprint that is persisted in the store ledger;
/// a later content swap is then detectable, and the digest is surfaced in
/// `/plugins` output.
///
/// The digest covers, for every regular file below `root` (relative path
/// sorted, depth-first):
///
/// - the relative path (with `/` separators) and a length prefix,
/// - the file's byte length and its bytes.
///
/// Directory entries and (refused) symlinks contribute nothing of their own.
///
/// # Errors
///
/// Returns [`AddError::Io`] when the tree cannot be walked or a file cannot be
/// read.
fn content_digest(root: &Path) -> Result<String, AddError> {
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), AddError> {
        for entry in std::fs::read_dir(dir).map_err(io_err)?.flatten() {
            let path = entry.path();
            let ft = entry.file_type().map_err(io_err)?;
            if ft.is_symlink() {
                // Symlinks never reach the store (see `copy_dir_recursive`), so
                // they must not influence the digest either.
                continue;
            }
            if ft.is_dir() {
                collect(&path, out)?;
            } else {
                out.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    collect(root, &mut files)?;
    files.sort();

    let mut hasher = sha2::Sha256::new();
    for path in &files {
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel = rel.to_string_lossy().replace('\\', "/");
        hasher.update((rel.len() as u64).to_be_bytes());
        hasher.update(rel.as_bytes());
        let bytes = std::fs::read(path).map_err(io_err)?;
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}
