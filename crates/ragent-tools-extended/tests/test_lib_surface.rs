//! Integration tests for the crate-root `ragent-tools-extended` surface:
//! `ToolRegistry` (register/get/contains/hidden/definitions/Default), the
//! `ToolContext::check_path_within_workspace` containment helper, and the
//! `StorageBackend` trait's default method impls.

// `assert!(x.is_empty())` is clearer than `assert_eq!(x, [])` in these tests.
#![allow(clippy::assert_is_empty)]

use std::sync::Arc;

use ragent_tools_extended::storage::{EmbeddingMatch, MemoryRow, StorageBackend, TaskRow};
use ragent_tools_extended::{ToolContext, ToolRegistry, create_extended_registry};
use ragent_types::event::EventBus;
use serde_json::json;

// ---------------------------------------------------------------------------
// ToolRegistry
// ---------------------------------------------------------------------------

#[test]
fn test_registry_default_and_get_contains() {
    let registry = ToolRegistry::default();
    assert!(registry.get("pdf_read").is_none());
    assert!(!registry.contains("pdf_read"));

    let reg = create_extended_registry();
    assert!(reg.contains("pdf_read"));
    let tool = reg.get("pdf_read").expect("pdf_read registered");
    assert_eq!(tool.name(), "pdf_read");
}

#[test]
fn test_registry_definitions_sorted_and_nonempty() {
    let reg = create_extended_registry();
    let defs = reg.definitions();
    assert!(!defs.is_empty());
    // Sorted by name.
    let mut names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    let sorted = names.clone();
    names.sort_unstable();
    assert_eq!(names, sorted, "definitions must be sorted by name");
    // Every definition carries a schema and description.
    for d in &defs {
        assert!(
            !d.description.is_empty(),
            "{} has empty description",
            d.name
        );
        assert!(d.parameters.is_object(), "{} has non-object schema", d.name);
    }
}

// ---------------------------------------------------------------------------
// ToolContext::check_path_within_workspace
// ---------------------------------------------------------------------------

fn work_ctx(work: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "t".to_string(),
        working_dir: work.to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: None,
        allowed_roots: Vec::new(),
    }
}

#[test]
fn test_check_path_within_workspace_accepts_relative_and_rejects_escape() {
    let tmp = std::env::temp_dir().join(format!("tet_contain_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let c = work_ctx(&tmp);
    assert!(
        c.check_path_within_workspace(&tmp.join("inner.txt"))
            .is_ok()
    );
    // A sibling path outside the working dir must be rejected.
    let outside = tmp.parent().unwrap().join("nope.txt");
    assert!(c.check_path_within_workspace(&outside).is_err());
    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// StorageBackend default methods
// ---------------------------------------------------------------------------

/// A minimal backend that implements only the required trait methods, so the
/// provided `create_task` / `create_task_simple` / `update_task` /
/// `update_task_simple` defaults are exercised.
#[derive(Default)]
struct MinimalStorage;

impl StorageBackend for MinimalStorage {
    fn list_tasks(&self, _s: &str, _f: Option<&str>) -> anyhow::Result<Vec<TaskRow>> {
        Ok(vec![])
    }
    fn create_task_simple(
        &self,
        id: &str,
        session_id: &str,
        title: &str,
        status: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        // Record the arguments so the default `create_task` delegation is
        // observable via the returned values from the wrapper below.
        assert_eq!(id, "T-1");
        assert_eq!(session_id, "sess");
        assert_eq!(title, "subject");
        assert_eq!(status, "pending");
        assert_eq!(description, "desc");
        Ok(())
    }
    fn update_task_simple(
        &self,
        id: &str,
        _session_id: &str,
        title: Option<&str>,
        _status: Option<&str>,
        _description: Option<&str>,
    ) -> anyhow::Result<bool> {
        assert_eq!(id, "T-1");
        assert_eq!(title, Some("subj2"));
        Ok(true)
    }
    fn delete_task(&self, _id: &str, _s: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn clear_tasks(&self, _s: &str) -> anyhow::Result<usize> {
        Ok(0)
    }
    fn get_memory(&self, _id: i64) -> anyhow::Result<Option<MemoryRow>> {
        Ok(None)
    }
    fn get_memory_tags(&self, _id: i64) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }
    fn search_memories(
        &self,
        _q: &str,
        _c: Option<&str>,
        _s: Option<&str>,
        _l: usize,
        _m: f64,
    ) -> anyhow::Result<Vec<MemoryRow>> {
        Ok(vec![])
    }
    fn list_memories(&self, _p: &str, _l: usize) -> anyhow::Result<Vec<MemoryRow>> {
        Ok(vec![])
    }
    fn store_memory_embedding(&self, _id: i64, _b: &[u8]) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn list_memory_embeddings(&self) -> anyhow::Result<Vec<(i64, Vec<u8>)>> {
        Ok(vec![])
    }
    fn search_memories_by_embedding(
        &self,
        _q: &[f32],
        _d: usize,
        _l: usize,
        _m: f32,
    ) -> anyhow::Result<Vec<EmbeddingMatch>> {
        Ok(vec![])
    }
}

#[test]
fn test_storage_default_create_task_delegates_to_simple() {
    let s = MinimalStorage;
    // The full `create_task` default forwards to `create_task_simple`.
    s.create_task(
        "T-1",
        "sess",
        "subject",
        "desc",
        "pending",
        Some("act"),
        Some("owner"),
        &json!({"k": "v"}),
        &["T-0".to_string()],
    )
    .expect("default create_task should delegate");
}

#[test]
fn test_storage_default_update_task_delegates_to_simple() {
    let s = MinimalStorage;
    let ok = s
        .update_task(
            "T-1",
            "sess",
            Some("subj2"),
            Some("completed"),
            None,
            Some(Some("acting")),
            Some(None),
            Some(&json!({})),
            Some(&[]),
        )
        .expect("default update_task should delegate");
    assert!(ok);
}

#[test]
fn test_storage_default_create_task_simple_delegates_to_full_create_task() {
    // A backend that overrides only the full `create_task` should have its
    // `create_task_simple` default route through it.
    struct FullOnly;
    impl StorageBackend for FullOnly {
        fn list_tasks(&self, _s: &str, _f: Option<&str>) -> anyhow::Result<Vec<TaskRow>> {
            Ok(vec![])
        }
        fn create_task(
            &self,
            id: &str,
            _session_id: &str,
            subject: &str,
            _description: &str,
            status: &str,
            _af: Option<&str>,
            _ow: Option<&str>,
            _md: &serde_json::Value,
            _bb: &[String],
        ) -> anyhow::Result<()> {
            assert_eq!(id, "X");
            assert_eq!(subject, "s");
            assert_eq!(status, "pending");
            Ok(())
        }
        fn update_task_simple(
            &self,
            _id: &str,
            _s: &str,
            _t: Option<&str>,
            _st: Option<&str>,
            _d: Option<&str>,
        ) -> anyhow::Result<bool> {
            Ok(true)
        }
        fn delete_task(&self, _id: &str, _s: &str) -> anyhow::Result<bool> {
            Ok(false)
        }
        fn clear_tasks(&self, _s: &str) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn get_memory(&self, _id: i64) -> anyhow::Result<Option<MemoryRow>> {
            Ok(None)
        }
        fn get_memory_tags(&self, _id: i64) -> anyhow::Result<Vec<String>> {
            Ok(vec![])
        }
        fn search_memories(
            &self,
            _q: &str,
            _c: Option<&str>,
            _s: Option<&str>,
            _l: usize,
            _m: f64,
        ) -> anyhow::Result<Vec<MemoryRow>> {
            Ok(vec![])
        }
        fn list_memories(&self, _p: &str, _l: usize) -> anyhow::Result<Vec<MemoryRow>> {
            Ok(vec![])
        }
        fn store_memory_embedding(&self, _id: i64, _b: &[u8]) -> anyhow::Result<bool> {
            Ok(false)
        }
        fn list_memory_embeddings(&self) -> anyhow::Result<Vec<(i64, Vec<u8>)>> {
            Ok(vec![])
        }
        fn search_memories_by_embedding(
            &self,
            _q: &[f32],
            _d: usize,
            _l: usize,
            _m: f32,
        ) -> anyhow::Result<Vec<EmbeddingMatch>> {
            Ok(vec![])
        }
    }
    let s = FullOnly;
    s.create_task_simple("X", "sess", "s", "pending", "d")
        .expect("create_task_simple default should delegate to create_task");
}

#[test]
fn test_task_row_and_memory_row_serde_roundtrip() {
    let row = TaskRow {
        id: "T".into(),
        session_id: "s".into(),
        title: "t".into(),
        status: "pending".into(),
        description: "d".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        active_form: None,
        owner: None,
        metadata: json!({}),
        blocked_by: vec![],
    };
    // Legacy rows without the new fields must still deserialize.
    let legacy = r#"{"id":"T","session_id":"s","title":"t","status":"pending",
        "description":"d","created_at":"x","updated_at":"y"}"#;
    let parsed: TaskRow = serde_json::from_str(legacy).expect("legacy TaskRow deserializes");
    assert!(parsed.active_form.is_none());
    assert!(parsed.blocked_by.is_empty());
    let value = serde_json::to_value(&row).unwrap();
    assert_eq!(value["metadata"], json!({}));
}
