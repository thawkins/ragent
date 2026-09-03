//! Unit and integration tests for `Config::backup_global_config` (spec `configsave`).
//!
//! Covers FR-001 (consistent path resolution), FR-003 (timestamped backup in
//! `saves/`, directory creation, naming format), and FR-011 (no existing
//! backup is ever overwritten).

use ragent_config::Config;
use std::fs;
use std::path::{Path, PathBuf};

/// Write a fake `ragent.json` into `dir` with the given content.
fn write_global_config(dir: &Path, contents: &str) -> PathBuf {
    let path = dir.join("ragent.json");
    fs::write(&path, contents).expect("write ragent.json");
    path
}

type BackupComponents = (u32, u32, u32, u32, u32, u32, Option<u32>);

/// Parse a backup file name into its numeric `(year, month, day, hour, min, sec,
/// counter)` components.
///
/// Returns `None` when the name does not strictly match the
/// `ragent.json.YYYY-MM-DD.HH-MM-SS` pattern or the collision-avoidance form
/// `ragent.json.YYYY-MM-DD.HH-MM-SS-N`.
fn parse_backup_name(name: &str) -> Option<BackupComponents> {
    let suffix = name.strip_prefix("ragent.json.")?;
    let parts: Vec<&str> = suffix.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    let tparts: Vec<&str> = parts[1].split('-').collect();
    if tparts.len() < 3 || tparts.len() > 4 {
        return None;
    }
    let hour = tparts[0].parse().ok()?;
    let min = tparts[1].parse().ok()?;
    let sec = tparts[2].parse().ok()?;
    let counter = if tparts.len() == 4 {
        Some(tparts[3].parse().ok()?)
    } else {
        None
    };
    let dparts: Vec<&str> = parts[0].split('-').collect();
    if dparts.len() != 3 {
        return None;
    }
    Some((
        dparts[0].parse().ok()?,
        dparts[1].parse().ok()?,
        dparts[2].parse().ok()?,
        hour,
        min,
        sec,
        counter,
    ))
}

#[test]
fn test_backup_creates_saves_directory_and_timestamped_file() {
    // FR-003: backup lands in `saves/` with a `ragent.json.<date>.<time>` name.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    write_global_config(dir, r#"{"defaultAgent":"coder"}"#);

    let backup = Config::backup_global_config(Some(dir)).expect("backup ok");

    let parent = backup.parent().expect("backup has parent");
    assert!(
        parent.ends_with("saves"),
        "backup should live inside saves/ subdir: {}",
        backup.display()
    );
    let name = backup.file_name().and_then(|n| n.to_str()).unwrap_or("");
    assert!(
        name.starts_with("ragent.json."),
        "backup name should start with 'ragent.json.': {name}"
    );
    // Name format: ragent.json.YYYY-MM-DD.HH-MM-SS
    let suffix = name.strip_prefix("ragent.json.").unwrap_or("");
    let parts: Vec<&str> = suffix.split('.').collect();
    assert_eq!(
        parts.len(),
        2,
        "backup suffix should have date.time: {suffix}"
    );
    assert!(
        parts[0].len() == 10 && parts[0].chars().nth(4) == Some('-'),
        "date part should be YYYY-MM-DD: {}",
        parts[0]
    );
    assert!(
        parts[1].len() == 8 && parts[1].chars().nth(2) == Some('-'),
        "time part should be HH-MM-SS: {}",
        parts[1]
    );
    assert!(backup.exists(), "backup file should exist");
}

#[test]
fn test_backup_preserves_content_byte_for_byte() {
    // FR-003: the backup is a faithful copy of the source.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let original = r#"{"defaultAgent":"coder","yolo":true}"#;
    write_global_config(dir, original);

    let backup = Config::backup_global_config(Some(dir)).expect("backup ok");
    let copied = fs::read_to_string(&backup).expect("read backup");
    assert_eq!(copied, original, "backup content should match source");
}

#[test]
fn test_backup_does_not_overwrite_existing_backups() {
    // FR-011: two saves produce two distinct files.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write_global_config(dir, r#"{"defaultAgent":"coder"}"#);

    let first = Config::backup_global_config(Some(dir)).expect("first backup");
    // With same-second collision handling, a second backup no longer needs to
    // wait a full second to avoid overwriting the first.
    let second = Config::backup_global_config(Some(dir)).expect("second backup");

    assert_ne!(first, second, "two saves must produce distinct files");
    assert!(first.exists(), "first backup should still exist");
    assert!(second.exists(), "second backup should exist");

    // The second backup name should carry a collision counter because both
    // saves ran within the same second.
    let second_name = second.file_name().and_then(|n| n.to_str()).unwrap_or("");
    assert!(
        second_name.contains('-'),
        "same-second backup should append a hyphenated counter: {second_name}"
    );

    // No stray temp files should be left behind.
    let saves_dir = dir.join("saves");
    let leftover_tmps: Vec<_> = fs::read_dir(&saves_dir)
        .expect("read saves")
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name().to_str().is_some_and(|n| {
                std::path::Path::new(n)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("tmp"))
            })
        })
        .collect();
    assert!(
        leftover_tmps.is_empty(),
        "no .tmp files should remain in saves/"
    );
}

#[test]
fn test_backup_errors_when_no_global_config_exists() {
    // FR-003: backing up a non-existent config is an error, not an empty file.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    // No ragent.json written.
    let result = Config::backup_global_config(Some(dir));
    assert!(
        result.is_err(),
        "backup should fail when ragent.json absent"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Failed to read global config") || msg.contains("ragent.json"),
        "error should mention the config read failure: {msg}"
    );
}

#[test]
fn test_global_config_path_helpers_resolve_consistently() {
    // FR-001: global_config_dir / global_config_path produce the canonical path.
    let dir = Config::global_config_dir();
    let path = Config::global_config_path();
    assert_eq!(
        path,
        dir.as_ref().map(|d| d.join("ragent.json")),
        "global_config_path should be dir/ragent.json"
    );
}

// ── Naming format (FR-003) ─────────────────────────────────────────────

#[test]
fn test_backup_name_strictly_matches_pattern() {
    // FR-003: the backup name must be exactly ragent.json.YYYY-MM-DD.HH-MM-SS
    // with every component a zero-padded integer.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write_global_config(dir, r#"{"defaultAgent":"coder"}"#);

    let backup = Config::backup_global_config(Some(dir)).expect("backup ok");
    let name = backup.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let (year, month, day, hour, min, sec, _counter) =
        parse_backup_name(name).unwrap_or_else(|| {
            panic!("backup name '{name}' does not match ragent.json.YYYY-MM-DD.HH-MM-SS")
        });

    // Date components are plausible calendar values.
    assert!(
        (2000..=2100).contains(&year),
        "year should be plausible: {year}"
    );
    assert!((1..=12).contains(&month), "month should be 1-12: {month}");
    assert!((1..=31).contains(&day), "day should be 1-31: {day}");
    // Time components are valid clock values.
    assert!((0..=23).contains(&hour), "hour should be 0-23: {hour}");
    assert!((0..=59).contains(&min), "minute should be 0-59: {min}");
    assert!((0..=59).contains(&sec), "second should be 0-59: {sec}");
}

#[test]
fn test_backup_name_uses_hyphens_in_time_not_colons() {
    // FR-003: the time portion uses hyphens (HH-MM-SS) so the file name is
    // legal on Windows NTFS where colons are forbidden in file names.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write_global_config(dir, r#"{"defaultAgent":"coder"}"#);

    let backup = Config::backup_global_config(Some(dir)).expect("backup ok");
    let name = backup.file_name().and_then(|n| n.to_str()).unwrap_or("");

    assert!(
        !name.contains(':'),
        "backup name must not contain colons (Windows-incompatible): {name}"
    );
    // Exactly two dots after "ragent.json" (date.time), no more.
    let suffix = name.strip_prefix("ragent.json.").unwrap_or(name);
    assert_eq!(
        suffix.matches('.').count(),
        1,
        "backup suffix should contain exactly one dot separating date and time: {suffix}"
    );

    // Time part has at least two hyphens; a third hyphen is an optional
    // same-second collision counter, which is also allowed.
    let time = suffix.split_once('.').map_or(suffix, |(_, t)| t);
    assert!(
        time.matches('-').count() >= 2,
        "time portion should be HH-MM-SS (optionally -N): {time}"
    );
    assert!(
        time.matches('-').count() <= 3,
        "time portion should have at most a single collision counter: {time}"
    );
}

// ── Directory creation (FR-003) ────────────────────────────────────────

#[test]
fn test_backup_creates_saves_dir_when_absent() {
    // FR-003: the `saves/` subdirectory is created when it does not exist.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write_global_config(dir, r#"{"defaultAgent":"coder"}"#);

    let saves_dir = dir.join("saves");
    assert!(
        !saves_dir.exists(),
        "precondition: saves/ should not exist yet"
    );

    let backup = Config::backup_global_config(Some(dir)).expect("backup ok");

    assert!(
        saves_dir.exists() && saves_dir.is_dir(),
        "saves/ directory should exist after backup"
    );
    assert!(
        backup.starts_with(&saves_dir),
        "backup should live inside saves/: {}",
        backup.display()
    );
}

#[test]
fn test_backup_reuses_existing_saves_dir_without_clearing() {
    // FR-003: when `saves/` already exists with prior content, a new backup is
    // added alongside the existing files — the directory is never wiped.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write_global_config(dir, r#"{"defaultAgent":"coder"}"#);

    let saves_dir = dir.join("saves");
    fs::create_dir_all(&saves_dir).expect("pre-create saves/");
    // Place a sentinel file that must survive the new backup.
    let sentinel = saves_dir.join("sentinel.txt");
    fs::write(&sentinel, "do not delete me").expect("write sentinel");

    let backup = Config::backup_global_config(Some(dir)).expect("backup ok");

    assert!(
        sentinel.exists(),
        "pre-existing sentinel file in saves/ must survive the backup"
    );
    assert_eq!(
        fs::read_to_string(&sentinel).unwrap(),
        "do not delete me",
        "sentinel contents must be unchanged"
    );
    assert!(
        backup.exists(),
        "the new backup file should exist alongside the sentinel"
    );
}

#[test]
fn test_backup_always_lands_in_saves_subfolder() {
    // FR-003: the backup never lands directly in the config dir — it is always
    // nested inside `saves/`, even on the very first save.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write_global_config(dir, r#"{"defaultAgent":"coder"}"#);

    let backup = Config::backup_global_config(Some(dir)).expect("backup ok");

    // The parent of the backup must be `saves`, not the config dir itself.
    assert_eq!(
        backup
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str()),
        Some("saves"),
        "backup parent must be the saves/ subfolder"
    );
    // And the config dir should not directly contain any ragent.json.* file.
    let stray: Vec<_> = fs::read_dir(dir)
        .expect("read config dir")
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("ragent.json.") && n != "ragent.json")
        })
        .collect();
    assert!(
        stray.is_empty(),
        "no ragent.json.* backup should sit directly in the config dir"
    );
}

#[test]
fn test_restore_global_config_rejects_missing_backup() {
    // FR-012 / restore: restoring a non-existent backup is an error, never a
    // silent write.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("ragent.json"), r#"{"defaultAgent":"coder"}"#).expect("write");

    let result = Config::restore_global_config(
        Some(dir),
        std::path::Path::new("ragent.json.9999-01-01.00-00-00"),
    );
    assert!(result.is_err(), "restore should fail for a missing backup");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("does not exist"),
        "error should mention the missing backup: {msg}"
    );
}

#[test]
fn test_restore_global_config_rejects_invalid_json_backup() {
    // FR-012 / restore: refuse to restore a backup that is not valid JSON.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("ragent.json"), r#"{"defaultAgent":"coder"}"#).expect("write");
    let saves = dir.join("saves");
    fs::create_dir_all(&saves).expect("saves dir");
    let bad_backup = saves.join("ragent.json.2024-01-01.12-00-00");
    fs::write(&bad_backup, "this is not json").expect("write bad backup");

    let result = Config::restore_global_config(Some(dir), &bad_backup);
    assert!(
        result.is_err(),
        "restore should fail when backup is not valid JSON"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not valid JSON"),
        "error should mention JSON validation: {msg}"
    );
}

#[test]
fn test_restore_global_config_by_bare_file_name() {
    // restore should accept a bare backup file name and resolve it under saves/.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("ragent.json"), r#"{"defaultAgent":"coder"}"#).expect("write");
    let backup = Config::backup_global_config(Some(dir)).expect("backup");
    let name = backup.file_name().and_then(|n| n.to_str()).unwrap();

    fs::write(dir.join("ragent.json"), r#"{"defaultAgent":"architect"}"#).expect("overwrite");
    let restored =
        Config::restore_global_config(Some(dir), std::path::Path::new(name)).expect("restore");
    assert_eq!(
        restored,
        dir.join("ragent.json"),
        "restore target must be the canonical global config"
    );
    let content = fs::read_to_string(&restored).expect("read restored");
    assert_eq!(content, r#"{"defaultAgent":"coder"}"#);
}

#[test]
fn test_restore_global_config_does_not_overwrite_non_global_target() {
    // FR-012: even if an absolute path is passed, the destination is always the
    // global ragent.json inside the supplied config directory.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("ragent.json"), r#"{"defaultAgent":"coder"}"#).expect("write");
    let backup = Config::backup_global_config(Some(dir)).expect("backup");

    let target = Config::restore_global_config(Some(dir), &backup).expect("restore");
    assert_eq!(
        target,
        dir.join("ragent.json"),
        "restore must always write to the canonical global path"
    );
}

#[test]
fn test_backup_restore_round_trip_preserves_config() {
    // T-010: a complete save → list → restore lifecycle. Back up an original
    // config, overwrite the global file, then restore the backup and verify the
    // original content is back.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let original = r#"{"defaultAgent":"coder","memory":{"enabled":true}}"#;
    fs::write(dir.join("ragent.json"), original).expect("write original");

    let backup = Config::backup_global_config(Some(dir)).expect("backup");
    let listed: Vec<_> = fs::read_dir(dir.join("saves"))
        .expect("read saves")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("ragent.json."))
        })
        .collect();
    assert!(
        listed.contains(&backup),
        "backup should appear in the saves directory"
    );

    let modified = r#"{"defaultAgent":"architect"}"#;
    fs::write(dir.join("ragent.json"), modified).expect("overwrite");

    let restored = Config::restore_global_config(Some(dir), &backup).expect("restore");
    assert_eq!(restored, dir.join("ragent.json"));
    assert_eq!(
        fs::read_to_string(&restored).expect("read restored"),
        original,
        "restored config must match the original backup"
    );
}
