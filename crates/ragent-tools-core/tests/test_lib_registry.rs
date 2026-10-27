//! Coverage-focused tests for `lib.rs`: the tool registry, the core registry
//! factory, and the path-containment helpers.
//!
//! A single registry sweep calls `name`/`description`/`parameters_schema`/
//! `permission_category` on every registered tool, which is the shared surface
//! every module exposes but few focused tests exercise.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_core::{
    CanonicalPathCache, Tool, ToolContext, ToolRegistry, check_path_within_allowed_roots_cached,
    check_path_within_any_root, check_path_within_any_root_cached, check_path_within_root,
    check_path_within_root_cached, create_core_registry,
};
use ragent_types::event::EventBus;
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("temp")
        .join(format!(
            "lib_reg_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Exercise the shared `Tool` surface of every registered tool.
#[test]
fn test_core_registry_tool_surface_is_complete() {
    let registry = create_core_registry();
    let names = registry.list();
    assert!(
        names.len() >= 20,
        "expected many tools, got {}",
        names.len()
    );

    // Names that must be present in the core registry. Five of these are
    // FR-023 `after` spellings: the rename (spec `uniformtools` T-031) changed
    // the registered name, not the tool.
    for expected in [
        "read",
        "Write",
        "create",
        "Edit",
        "multi_edit",
        "patch",
        "append_to_file",
        "copy_file",
        "move_file",
        "rm",
        "make_directory",
        "file_info",
        "diff_files",
        "Bash",
        "bash_reset",
        "open",
        "Grep",
        "Glob",
        "list",
        "think",
        "calculator",
        "get_env",
        "agent_complete",
    ] {
        assert!(
            registry.contains(expected),
            "core registry missing {expected}: {names:?}"
        );
    }

    for name in &names {
        let tool: Arc<dyn Tool> = registry.get(name).expect("registered tool resolves");
        assert_eq!(tool.name(), name.as_str(), "name() must match registry key");
        assert!(!tool.description().is_empty(), "{name} description empty");
        assert!(
            !tool.permission_category().is_empty(),
            "{name} permission_category empty"
        );
        let schema = tool.parameters_schema();
        assert_eq!(
            schema["type"], "object",
            "{name} schema must be an object: {schema}"
        );
    }
}

#[test]
fn test_registry_remove_and_clear_and_contains() {
    let registry = create_core_registry();
    assert!(registry.contains("read"));
    let removed = registry.remove("read");
    assert!(removed.is_some());
    assert!(!registry.contains("read"));
    assert!(registry.remove("read").is_none());

    registry.clear();
    assert_eq!(registry.list(), [] as [std::string::String; 0]);
    assert!(!registry.contains("read"));
}

#[test]
fn test_registry_definitions_lists_every_registered_tool() {
    let registry = create_core_registry();
    let visible = registry.definitions();
    assert!(visible.len() >= 20);
    // Definitions are sorted by name.
    let mut sorted = visible.iter().map(|d| d.name.clone()).collect::<Vec<_>>();
    sorted.sort();
    let actual: Vec<String> = visible.iter().map(|d| d.name.clone()).collect();
    assert_eq!(actual, sorted, "definitions must be name-sorted");
    for def in &visible {
        assert!(
            !def.description.is_empty(),
            "{} has no description",
            def.name
        );
        assert_eq!(def.parameters["type"], "object");
    }

    // Every registered tool is advertised: there is no hidden set.
    let names: Vec<&str> = visible.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"read"));
    assert!(names.contains(&"Write"));
    assert!(names.contains(&"Grep"));
}

#[test]
fn test_registry_default_is_empty() {
    let registry = ToolRegistry::default();
    assert_eq!(registry.list(), [] as [std::string::String; 0]);
}

#[test]
fn test_empty_registry_definitions_is_empty() {
    let registry = ToolRegistry::new();
    assert!(registry.definitions().is_empty());
}

fn ctx(dir: &Path, roots: Vec<PathBuf>) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(CanonicalPathCache::new()),
        allowed_roots: roots,
    }
}

#[test]
fn test_check_path_helpers_accept_and_reject() {
    let root = temp_dir("contain");
    let inside = root.join("sub").join("file.txt");
    fs::create_dir_all(inside.parent().unwrap()).unwrap();
    fs::write(&inside, "x").unwrap();

    // Non-existent paths resolve by walking up the existing prefix.
    let missing = root.join("sub").join("nope").join("deep.txt");

    assert!(check_path_within_any_root(&inside, &[root.as_path()]).is_ok());
    assert!(check_path_within_any_root(&missing, &[root.as_path()]).is_ok());
    assert!(check_path_within_root(&inside, &root).is_ok());
    assert!(check_path_within_any_root(&Path::new("/etc/passwd"), &[root.as_path()]).is_err());
    // No roots at all is a hard error.
    let err = check_path_within_any_root(&inside, &[]).unwrap_err();
    assert!(err.to_string().contains("no allowed roots"), "{err}");

    // Cached variants exercise the CanonicalPathCache path.
    let cache = CanonicalPathCache::new();
    assert!(check_path_within_any_root_cached(&inside, &[root.as_path()], &cache).is_ok());
    assert!(check_path_within_root_cached(&inside, &root, &cache).is_ok());
    assert!(
        check_path_within_any_root_cached(&Path::new("/etc/passwd"), &[root.as_path()], &cache)
            .is_err()
    );

    // The single entry point every tool calls.
    let _ = ctx(&root, vec![]);
    assert!(check_path_within_allowed_roots_cached(&inside, &root, &[], &cache).is_ok());
    let other = temp_dir("contain_other");
    assert!(
        check_path_within_allowed_roots_cached(
            &other.join("ok.txt"),
            &root,
            std::slice::from_ref(&other),
            &cache,
        )
        .is_ok()
    );
    assert!(
        check_path_within_allowed_roots_cached(
            &Path::new("/etc/passwd"),
            &root,
            std::slice::from_ref(&other),
            &cache,
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn test_symlink_alias_within_root_is_accepted() {
    use std::os::unix::fs::symlink;
    let root = temp_dir("alias_root");
    let alias_parent = temp_dir("alias_link");
    let link = alias_parent.join("link");
    // `alias_parent/link` resolves to `root`; a path under the link is within
    // the root by file identity even though the string prefix differs.
    let _ = symlink(&root, &link);

    let cache = CanonicalPathCache::new();
    // Canonicalisation already collapses the symlink here, exercising both the
    // component check and the alias (dev/inode) fallback.
    let target = link.join("child.txt");
    fs::write(root.join("child.txt"), "x").unwrap();
    assert!(check_path_within_any_root_cached(&target, &[root.as_path()], &cache).is_ok());
}

#[test]
fn test_registry_tool_roundtrip_via_dyn_dispatch() {
    // Execute a couple of tools through the `dyn Tool` handle to cover the
    // registry get + trait-object dispatch path.
    let dir = temp_dir("dyn");
    let registry = create_core_registry();

    let think = registry.get("think").unwrap();
    let ctx = ctx(&dir, vec![dir.clone()]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let out = think
            .execute(json!({"thought": "why"}), &ctx)
            .await
            .unwrap();
        assert_eq!(out.metadata.unwrap()["thinking"], true);
    });
    assert_eq!(think.permission_category(), "none");

    // Schema validation surface is reachable from the registry too.
    let read = registry.get("read").unwrap();
    assert!(read.parameters_schema()["properties"]["path"].is_object());
}
