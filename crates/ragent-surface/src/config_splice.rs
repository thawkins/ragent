//! Shared atomic JSON-config splice for the `/plugins` and `/connectors`
//! `marketplace` command families.
//!
//! Both families edit the user-global `ragent.json` with the same minimal,
//! atomic write (temp file in the same directory, then rename) and both must
//! drop the config crate's on-disk load cache afterwards. The implementation
//! lived as a byte-identical private `write_config` in each crate; it lives here
//! once.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

/// Monotonic counter mixed into the temp-file name so two writers in the same
/// process never collide on one suffix.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `value` back to `path` atomically, creating the parent directory when
/// needed, then drop the config crate's on-disk load cache.
///
/// The temp file is written in the same directory (so the rename is atomic on
/// one filesystem) with an unpredictable `<name>.json.tmp.<pid>.<seq>` suffix,
/// created `O_EXCL` and owner-only. SEC-024/SEC-072: the config holds plaintext
/// credentials, so the temp is opened `0o600` (the rename carries the mode to
/// the target), and `create_new` refuses a pre-placed symlink at the temp path
/// rather than following it. The load-cache drop is required because this
/// bypasses `Config::save`: a `Config::load` earlier in the process would
/// otherwise keep serving the pre-edit value.
///
/// # Errors
///
/// Returns a short, user-facing message when the parent directory cannot be
/// created, the value cannot be serialised, or the temp write or rename fails.
pub fn write_atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| format!("cannot serialise global config: {e}"))?;

    // Try a few suffixes so a stale temp from a crashed run does not wedge the
    // write; `create_new` makes each attempt safe against a symlink clobber.
    let mut tmp = PathBuf::new();
    let mut opened = None;
    for _ in 0..8 {
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        tmp = path.with_extension(format!("json.tmp.{}.{seq}.{nanos}", std::process::id()));
        match ragent_types::guard::open_new_owner_only(&tmp) {
            Ok(file) => {
                opened = Some(file);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("cannot write {}: {e}", tmp.display())),
        }
    }
    let Some(mut file) = opened else {
        return Err(format!(
            "cannot write {}: no free temp file name",
            tmp.display()
        ));
    };
    // Write, flush to disk, then rename: the `sync_all` makes the temp file
    // durable before the rename can expose it, so a crash after the rename
    // cannot leave a target whose contents were never flushed (matching the
    // `Config::save` durability guarantee).
    if let Err(e) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(&tmp); // INTENTIONAL: best-effort temp cleanup
        return Err(format!("cannot write {}: {e}", tmp.display()));
    }
    drop(file);
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp); // INTENTIONAL: best-effort temp cleanup
        return Err(format!("cannot replace {}: {e}", path.display()));
    }
    ragent_config::Config::invalidate_load_cache();
    Ok(())
}
