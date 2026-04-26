//! Tests for atomic config writes with file locking (Milestone 4).
//!
//! Verifies that `atomic_config_update` produces correct JSON, survives
//! concurrent writers, and leaves no partial writes.

use ragent_tui::app::atomic_config_update;
use std::path::PathBuf;
use tempfile::TempDir;

// =========================================================================
// Basic correctness
// =========================================================================

#[test]
fn test_atomic_update_creates_new_file() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    atomic_config_update(&config, |json| {
        json["mcp"]["filesystem"] = serde_json::json!({
            "command": "node",
            "disabled": false,
        });
        Ok(())
    })
    .unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(content["mcp"]["filesystem"]["command"], "node");
    assert_eq!(content["mcp"]["filesystem"]["disabled"], false);
}

#[test]
fn test_atomic_update_merges_existing() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    // Write initial config.
    std::fs::write(
        &config,
        serde_json::to_string_pretty(&serde_json::json!({
            "username": "alice",
            "mcp": { "git": { "command": "git-mcp" } }
        }))
        .unwrap(),
    )
    .unwrap();

    // Add a new MCP server.
    atomic_config_update(&config, |json| {
        json["mcp"]["filesystem"] = serde_json::json!({ "command": "fs-mcp" });
        Ok(())
    })
    .unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    // Original data preserved.
    assert_eq!(content["username"], "alice");
    assert_eq!(content["mcp"]["git"]["command"], "git-mcp");
    // New data added.
    assert_eq!(content["mcp"]["filesystem"]["command"], "fs-mcp");
}

#[test]
fn test_atomic_update_updater_error_leaves_file_unchanged() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    std::fs::write(&config, r#"{"key":"original"}"#).unwrap();

    let result = atomic_config_update(&config, |_json| Err("simulated error".to_string()));
    assert!(result.is_err());

    // File should still contain the original content (updater never wrote).
    let content = std::fs::read_to_string(&config).unwrap();
    assert!(content.contains("original"), "file should be unchanged");
}

#[test]
fn test_atomic_update_no_tmp_file_left() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    atomic_config_update(&config, |json| {
        json["clean"] = serde_json::json!(true);
        Ok(())
    })
    .unwrap();

    // No stray temp files should remain — only ragent.json and the .lock file.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        entries
            .iter()
            .all(|n| n == "ragent.json" || n == "ragent.json.lock"),
        "unexpected files in dir: {entries:?}"
    );
}

#[test]
fn test_atomic_update_pretty_printed() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    atomic_config_update(&config, |json| {
        json["a"] = serde_json::json!(1);
        Ok(())
    })
    .unwrap();

    let raw = std::fs::read_to_string(&config).unwrap();
    assert!(raw.contains('\n'), "output should be pretty-printed");
}

// =========================================================================
// Concurrent writers
// =========================================================================

#[test]
fn test_concurrent_writers_no_corruption() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    // Seed an empty JSON object.
    std::fs::write(&config, "{}").unwrap();

    let num_threads = 10;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(num_threads));
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let path = config.clone();
        let bar = barrier.clone();
        handles.push(std::thread::spawn(move || {
            bar.wait(); // synchronise start
            let key = format!("server_{i}");
            let value = serde_json::json!({ "id": i });
            atomic_config_update(&path, |json| {
                json["custom"][&key] = value;
                Ok(())
            })
            .expect("atomic update should succeed");
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    // Verify the final file contains all entries and is valid JSON.
    let raw = std::fs::read_to_string(&config).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(&raw).expect("final file should be valid JSON");

    for i in 0..num_threads {
        let key = format!("server_{i}");
        assert_eq!(
            content["custom"][&key]["id"], i,
            "entry {key} should be present and correct"
        );
    }
}

#[test]
fn test_concurrent_writers_different_sections() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");
    std::fs::write(&config, "{}").unwrap();

    let path_custom: PathBuf = config.clone();
    let path_mcp: PathBuf = config.clone();

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let b1 = barrier.clone();
    let b2 = barrier.clone();

    let h1 = std::thread::spawn(move || {
        b1.wait();
        for i in 0..5 {
            let key = format!("custom_{i}");
            atomic_config_update(&path_custom, |json| {
                json["custom"][&key] = serde_json::json!(true);
                Ok(())
            })
            .unwrap();
        }
    });

    let h2 = std::thread::spawn(move || {
        b2.wait();
        for i in 0..5 {
            let key = format!("mcp_{i}");
            atomic_config_update(&path_mcp, |json| {
                json["mcp"][&key] = serde_json::json!(true);
                Ok(())
            })
            .unwrap();
        }
    });

    h1.join().unwrap();
    h2.join().unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();

    for i in 0..5 {
        assert_eq!(content["custom"][format!("custom_{i}")], true);
        assert_eq!(content["mcp"][format!("mcp_{i}")], true);
    }
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn test_atomic_update_empty_file() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    // Create an empty file (0 bytes).
    std::fs::write(&config, "").unwrap();

    atomic_config_update(&config, |json| {
        json["key"] = serde_json::json!("value");
        Ok(())
    })
    .unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(content["key"], "value");
}

#[test]
fn test_atomic_update_invalid_json_returns_error() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    // Write invalid JSON.
    std::fs::write(&config, "not json {{{").unwrap();

    let result = atomic_config_update(&config, |_| Ok(()));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("parse"), "error should mention parsing: {err}");
}

#[cfg(unix)]
#[test]
fn test_atomic_update_permission_denied() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let readonly = dir.path().join("readonly");
    std::fs::create_dir(&readonly).unwrap();

    let config = readonly.join("ragent.json");
    std::fs::write(&config, "{}").unwrap();
    // Make directory read-only so the .tmp file can't be written.
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = atomic_config_update(&config, |json| {
        json["x"] = serde_json::json!(1);
        Ok(())
    });
    assert!(result.is_err());

    // Restore permissions for cleanup.
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn test_atomic_update_multiple_sequential() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("ragent.json");

    for i in 0..20 {
        let key = format!("item_{i}");
        atomic_config_update(&config, |json| {
            json[&key] = serde_json::json!(i);
            Ok(())
        })
        .unwrap();
    }

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    for i in 0..20 {
        assert_eq!(content[format!("item_{i}")], i);
    }
}
