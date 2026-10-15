//! Shared security guards (SECTASKS MS-05 / T-067 and T-068).
//!
//! Every earlier milestone closed its findings with one of a small number of
//! guard shapes: reject an option-like operand before it reaches `git`, confine a
//! joined path to a root, validate an identifier that becomes a path component,
//! clamp a server-supplied delay, and cap a buffered read. Those shapes were
//! re-implemented per crate, so a crate that forgot one silently reopened the
//! class.
//!
//! This module owns the canonical implementation of each shape so the call sites
//! can share one behaviour and one test suite. It deliberately validates *shape*
//! only - authorisation, permissions, and rate limits stay with their own
//! subsystems.
//!
//! Guards here never panic, and only [`contained_join`], [`reject_symlink_escape`]
//! and [`reject_symlink_components`] touch the filesystem (to detect a symlink
//! escape or refuse to operate on a symlinked path).

use std::path::{Component, Path, PathBuf};

/// Maximum delay honoured from a server-supplied `Retry-After`/backoff hint.
///
/// SEC-ragent-llm-003 (SECTASKS T-028/T-067): an untrusted upstream must not be
/// able to park a request loop for an arbitrary duration.
pub const MAX_RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// Maximum size of an identifier or single path component accepted by
/// [`validate_identifier`] and [`validate_relative_component`].
pub const MAX_IDENTIFIER_LEN: usize = 64;

/// Characters permitted in an identifier: ASCII alphanumerics, `.`, `_`, `-`.
///
/// Mirrors the `^[A-Za-z0-9._-]+$` shape used by the plugin manifest parser and
/// the plugin store's destination-directory confinement.
fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// Characters additionally permitted in an option-like check target: `/`.
///
/// A `git` ref or a relative path legitimately contains `/`; the check only has
/// to prevent the value being parsed as an *option*.
fn is_operand_char(c: char) -> bool {
    is_identifier_char(c) || c == '/'
}

/// Reject an empty value or one that `git` (or another CLI) would parse as an
/// option rather than an operand.
///
/// SEC-ragent-tools-vcs-001/002 and SEC-ragent-plugins-001 (SECTASKS T-022,
/// T-001, T-068): `--upload-pack=<cmd>` and friends turn a "ref" or "branch"
/// parameter into command execution, so a leading `-` is fatal.
///
/// # Errors
///
/// Returns `Err` naming `label` when `value` is empty or begins with `-`.
///
/// # Examples
///
/// ```
/// use ragent_types::guard::reject_option_like;
///
/// assert!(reject_option_like("main", "branch").is_ok());
/// assert!(reject_option_like("--upload-pack=/bin/sh", "ref").is_err());
/// assert!(reject_option_like("", "branch").is_err());
/// ```
pub fn reject_option_like(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{label}` must not be empty"));
    }
    if value.starts_with('-') {
        return Err(format!(
            "`{label}` value '{value}' is rejected: it would be parsed by the \
             downstream command as an option rather than an operand"
        ));
    }
    Ok(())
}

/// Whether `value` is safe to forward to a CLI as a positional operand.
///
/// This is the predicate form of [`reject_option_like`] plus a character-set
/// check: non-empty, no leading `-`, and only `[A-Za-z0-9._/-]`. Used where a
/// caller drops an invalid value rather than failing (`git+` ref fragments).
///
/// # Examples
///
/// ```
/// use ragent_types::guard::is_safe_operand;
///
/// assert!(is_safe_operand("v1.2.3"));
/// assert!(is_safe_operand("feature/nested-branch"));
/// assert!(!is_safe_operand("--upload-pack=/bin/sh"));
/// assert!(!is_safe_operand("a b"));
/// ```
#[must_use]
pub fn is_safe_operand(value: &str) -> bool {
    !value.is_empty() && !value.starts_with('-') && value.chars().all(is_operand_char)
}

/// Validate an identifier that will be used as a single path component.
///
/// SEC-ragent-plugins-002 and SEC-ragent-agent-007 (SECTASKS T-002, T-067): the
/// value must match `^[A-Za-z0-9._-]+$`, be at most
/// [`MAX_IDENTIFIER_LEN`] characters, and contain no separators or `..`.
///
/// # Errors
///
/// Returns `Err` describing the first rule the value breaks.
///
/// # Examples
///
/// ```
/// use ragent_types::guard::validate_identifier;
///
/// assert!(validate_identifier("weather-lsp", "plugin id").is_ok());
/// assert!(validate_identifier("../x", "plugin id").is_err());
/// assert!(validate_identifier("/tmp/x", "plugin id").is_err());
/// ```
pub fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{label}` must not be empty"));
    }
    if value.len() > MAX_IDENTIFIER_LEN {
        return Err(format!(
            "`{label}` value is {} bytes; the maximum is {MAX_IDENTIFIER_LEN}",
            value.len()
        ));
    }
    if !value.chars().all(is_identifier_char) {
        return Err(format!(
            "`{label}` value '{value}' is rejected: only ASCII alphanumerics and \
             '.', '_', '-' are permitted"
        ));
    }
    // A leading `.` cannot climb out on its own, but `..` and `.` are reserved
    // path components and must never name an identifier.
    if value == "." || value == ".." || value.contains("..") {
        return Err(format!(
            "`{label}` value '{value}' is rejected: it contains a reserved path component"
        ));
    }
    Ok(())
}

/// Validate a relative path that will be joined onto a trusted root.
///
/// SEC-ragent-plugins-005/006 and SEC-ragent-bench-004 (SECTASKS T-062, T-059,
/// T-067): the value must be relative, free of `..` components, and free of
/// backslashes (which Windows treats as separators).
///
/// # Errors
///
/// Returns `Err` describing the first rule the value breaks.
///
/// # Examples
///
/// ```
/// use ragent_types::guard::validate_relative_component;
///
/// assert!(validate_relative_component("dist/index.js", "entry").is_ok());
/// assert!(validate_relative_component("/etc/passwd", "entry").is_err());
/// assert!(validate_relative_component("../../x", "entry").is_err());
/// ```
pub fn validate_relative_component(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("`{label}` must not be empty"));
    }
    if value.contains('\\') {
        return Err(format!(
            "`{label}` value '{value}' is rejected: backslash separators are not permitted"
        ));
    }
    let path = Path::new(value);
    // `is_absolute()` is true exactly when a `RootDir` component is present
    // (possibly behind a Windows prefix), so the second `is_absolute` check and
    // the `Component::RootDir` arm that used to follow this block were both
    // unreachable (F17/F18). Rejecting absolute paths here still rejects every
    // `RootDir` component.
    if path.is_absolute() {
        return Err(format!(
            "`{label}` value '{value}' is rejected: it must be relative to the plugin root"
        ));
    }
    // A Windows drive prefix (`C:/x`) or UNC path is absolute-in-effect, and
    // `Path::is_absolute` is false for it on Unix.
    if has_windows_prefix(value) {
        return Err(format!(
            "`{label}` value '{value}' is rejected: it must be relative to the plugin root"
        ));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!(
            "`{label}` value '{value}' is rejected: it must not contain '..' components"
        ));
    }
    Ok(())
}

/// Whether `value` carries a Windows drive-letter or UNC prefix (`C:/x`,
/// `C:x`, `//server/share`).
///
/// `std::path::Path` parses these as ordinary components on Unix, so they need
/// an explicit textual check before a join can be considered contained.
fn has_windows_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    let drive_letter = bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes.len() == 2 || bytes[2] == b'/' || bytes[2] == b'\\');
    // `//server/share` (the `\\` form is already refused above).
    let unc = value.starts_with("//");
    drive_letter || unc
}

/// Join a validated relative path onto `root`, refusing any escape.
///
/// SEC-ragent-bench-004 and SEC-ragent-plugins-006 (SECTASKS T-059, T-062,
/// T-067): `Path::join` discards `root` entirely for an absolute component and a
/// `..` component climbs out of it, so both are rejected before the join. When
/// the joined path already exists, its canonical form is checked against the
/// canonicalised root, which also catches a symlink escape.
///
/// # Errors
///
/// Returns `Err` when the value fails [`validate_relative_component`], or when
/// the joined path canonicalises outside `root`.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ragent_types::guard::contained_join;
///
/// let root = Path::new("/srv/data");
/// assert_eq!(
///     contained_join(root, "cases/01.json", "case file").unwrap(),
///     root.join("cases/01.json")
/// );
/// assert!(contained_join(root, "../etc/passwd", "case file").is_err());
/// assert!(contained_join(root, "/etc/passwd", "case file").is_err());
/// ```
pub fn contained_join(root: &Path, relative: &str, label: &str) -> Result<PathBuf, String> {
    validate_relative_component(relative, label)?;

    let mut joined = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                // Unreachable after validation; kept so the loop stays exhaustive
                // without a wildcard arm.
                return Err(format!(
                    "`{label}` value '{relative}' is rejected: it escapes the root"
                ));
            }
            other => joined.push(other.as_os_str()),
        }
    }

    // SEC-058: canonicalise the *deepest existing ancestor* of the joined path,
    // not just the leaf. A not-yet-created leaf fails `canonicalize` (ENOENT),
    // so the old leaf-only check returned the raw path unchecked and a symlinked
    // intermediate component (`link/shadow`) escaped the root. When the root
    // itself cannot be canonicalised (e.g. it does not exist yet) the check is
    // skipped, matching the prior lenient behaviour for a not-yet-created root.
    let existing = deepest_existing_ancestor(&joined);
    if let (Ok(real), Ok(real_root)) = (existing.canonicalize(), root.canonicalize())
        && !real.starts_with(&real_root)
    {
        return Err(format!(
            "`{label}` value '{relative}' resolves outside the root via a symlink"
        ));
    }

    Ok(joined)
}

/// Return `path` itself when it exists, else its deepest existing ancestor.
///
/// Used by [`contained_join`] to canonicalise a path whose leaf may not exist
/// yet without giving up the symlink check on an intermediate component.
fn deepest_existing_ancestor(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    loop {
        if current.symlink_metadata().is_ok() {
            return current;
        }
        match current.parent() {
            Some(parent) if parent.as_os_str().is_empty() => return current,
            Some(parent) if parent == current => return current,
            Some(parent) => current = parent.to_path_buf(),
            None => return current,
        }
    }
}

/// Reject `path` when any of its components is a symlink.
///
/// SEC-006/SEC-059/SEC-030: a repository can commit a symlinked directory (or a
/// pre-placed temp-file symlink) that redirects a write outside the sandbox;
/// `canonicalize`-based checks that run only after the fact miss a symlink whose
/// target does not yet exist. This guard inspects each component with
/// `symlink_metadata` (which never follows the link) and walks upward from
/// `path` to the filesystem root. A component that does not exist yet is skipped
/// (its parent is still checked, so `link/new-file` is refused); the walk stops
/// at the first component that exists and is *not* a symlink, because everything
/// below a real directory is genuinely contained by it.
///
/// # Errors
///
/// Returns a message naming the first symlinked component found.
pub fn reject_symlink_components(path: &Path, label: &str) -> Result<(), String> {
    let mut current = path.to_path_buf();
    loop {
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "`{label}` path '{}' is a symlink; refusing to follow it",
                    current.display()
                ));
            }
            // A real (non-symlink) component: everything below it is contained.
            Ok(_) => return Ok(()),
            // Not present (ENOENT) or unreadable: this component cannot be a
            // symlink, but its *parent* still can, so keep walking upward.
            Err(_) => {}
        }
        match current.parent() {
            Some(parent) if parent.as_os_str().is_empty() => return Ok(()),
            Some(parent) if parent == current => return Ok(()),
            Some(parent) => current = parent.to_path_buf(),
            None => return Ok(()),
        }
    }
}

/// Reject a `path` whose deepest existing form canonicalises outside `root`.
///
/// SEC-016/SEC-006: unlike [`contained_join`], this works for a path that does
/// **not** yet exist. It walks `path` upward to the deepest ancestor that
/// exists, canonicalises both that ancestor and `root`, and requires the
/// ancestor to be inside `root`. A symlinked intermediate directory (the exact
/// SEC-016 case: a symlinked `index_dir` whose deeper `.../fts` leaf is a real
/// directory on the other side of the link) is therefore caught even though the
/// leaf itself is not a symlink.
///
/// # Errors
///
/// Returns a message when the deepest existing ancestor escapes `root`, or when
/// `root` cannot be canonicalised.
pub fn reject_symlink_escape(path: &Path, root: &Path, label: &str) -> Result<(), String> {
    let Ok(canonical_root) = root.canonicalize() else {
        return Err(format!(
            "`{label}` root '{}' cannot be resolved",
            root.display()
        ));
    };
    let mut current = path;
    loop {
        if let Ok(real) = current.canonicalize() {
            if real.starts_with(&canonical_root) {
                return Ok(());
            }
            return Err(format!(
                "`{label}` path '{}' resolves outside '{}' (via a symlink)",
                path.display(),
                root.display()
            ));
        }
        match current.parent() {
            Some(parent) if parent.as_os_str().is_empty() => break,
            Some(parent) if parent == current => break,
            Some(parent) => current = parent,
            None => break,
        }
    }
    // Nothing along the path exists yet, so there is no symlink to escape
    // through; the lexical components are the caller's responsibility.
    Ok(())
}

/// Create (or truncate) `path` owner-only (mode `0o600` on Unix).
///
/// SEC-022/SEC-024: config files routinely hold plaintext credentials, and a
/// plain `File::create`/`std::fs::write` uses the process umask (typically
/// `0644`), so any local user could read stored keys. Every config writer must
/// create its temp file through this helper. The mode is re-asserted for a
/// pre-existing file because `mode()` only applies at creation.
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] when the file cannot be created.
pub fn open_owner_only(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        // `mode(0o600)` only applies at creation; tighten a pre-existing file.
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600)); // INTENTIONAL: best-effort permission tighten (mode set at create)
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }
}

/// Like [`open_owner_only`] but refuses an existing path (`create_new`).
///
/// SEC-072: a predictable temp path (e.g. `<name>.json.tmp.<pid>`) can be
/// pre-created as a symlink by an attacker with write access to the config
/// directory; `create_new` gives `O_EXCL` semantics so the write fails instead
/// of following the link, and the `0o600` mode keeps the file owner-only.
///
/// # Errors
///
/// Returns [`std::io::ErrorKind::AlreadyExists`] when `path` already exists.
pub fn open_new_owner_only(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
    }
}

/// Clamp a server-supplied delay hint to [`MAX_RETRY_AFTER`].
///
/// SEC-ragent-llm-003 (SECTASKS T-028, T-067). A hint of zero is treated as
/// "unspecified" and clamped up to a single second so a caller cannot turn a
/// retry into a hot loop.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use ragent_types::guard::{MAX_RETRY_AFTER, clamp_retry_after};
///
/// assert_eq!(clamp_retry_after(Duration::from_secs(5)), Duration::from_secs(5));
/// assert_eq!(clamp_retry_after(Duration::from_secs(9999)), MAX_RETRY_AFTER);
/// assert_eq!(clamp_retry_after(Duration::ZERO), Duration::from_secs(1));
/// ```
#[must_use]
pub fn clamp_retry_after(hint: std::time::Duration) -> std::time::Duration {
    let floor = std::time::Duration::from_secs(1);
    if hint < floor {
        return floor;
    }
    if hint > MAX_RETRY_AFTER {
        return MAX_RETRY_AFTER;
    }
    hint
}

/// Truncate `bytes` to at most `max_bytes`, tolerating a partial UTF-8 boundary
/// at the cut point.
///
/// SEC-ragent-llm-005 and SEC-ragent-types-005 (SECTASKS T-028, T-035, T-067):
/// every capped read in the workspace needs the same "keep the prefix, do not
/// panic on a split character" behaviour.
///
/// # Examples
///
/// ```
/// use ragent_types::guard::cap_read;
///
/// let text = cap_read(b"hello world", 5);
/// assert_eq!(text, "hello");
/// // "e" is two bytes; a cut inside it drops only the split character.
/// let text = cap_read("caf\u{e9}".as_bytes(), 4);
/// assert_eq!(text, "caf");
/// ```
#[must_use]
pub fn cap_read(bytes: &[u8], max_bytes: usize) -> String {
    let end = if bytes.len() <= max_bytes {
        bytes.len()
    } else {
        max_bytes
    };
    match std::str::from_utf8(&bytes[..end]) {
        Ok(text) => text.to_string(),
        Err(err) => {
            // `valid_up_to` is always on a char boundary, so the prefix slice
            // cannot panic.
            let valid = err.valid_up_to();
            String::from_utf8_lossy(&bytes[..valid]).into_owned()
        }
    }
}
