//! Shared atomic JSON-config splice for the `/plugins` and `/connectors`
//! `marketplace` command families.
//!
//! Both families edit the user-global `ragent.json` with the same minimal,
//! atomic write (temp file in the same directory, then rename) and both must
//! drop the config crate's on-disk load cache afterwards. The implementation
//! lived as a byte-identical private `write_config` in each crate; it lives here
//! once.

use std::path::Path;

use serde_json::Value;

/// Write `value` back to `path` atomically, creating the parent directory when
/// needed, then drop the config crate's on-disk load cache.
///
/// The temp file is written in the same directory (so the rename is atomic on
/// one filesystem) with a `<name>.json.tmp.<pid>` suffix. The load-cache drop is
/// required because this bypasses `Config::save`: a `Config::load` earlier in
/// the process would otherwise keep serving the pre-edit value.
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
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    // Write, flush to disk, then rename: the `sync_all` makes the temp file
    // durable before the rename can expose it, so a crash after the rename
    // cannot leave a target whose contents were never flushed (matching the
    // `Config::save` durability guarantee).
    {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&tmp)
            .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        file.write_all(&bytes)
            .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        file.sync_all()
            .map_err(|e| format!("cannot flush {}: {e}", tmp.display()))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
    ragent_config::Config::invalidate_load_cache();
    Ok(())
}
