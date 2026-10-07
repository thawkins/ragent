//! Integration tests for the shared `ragent-surface` helpers (T-505).
//!
//! The plugin and connector surfaces delegate their attribution, subcommand
//! tokeniser, store-directory resolution, harness step model, and schema
//! sample generator to this crate, so these tests pin the one implementation.

use std::time::Duration;

use serde_json::json;

use ragent_surface::harness::{StepOutcome, sample_for_schema, step, truncate};
use ragent_surface::help::{attribution, subcommand_of};
use ragent_surface::store::{StoreDirs, store_dirs_at};

#[test]
fn attribution_omits_an_empty_subcommand_and_keeps_the_trigger() {
    assert_eq!(attribution("/plugins", ""), "From: /plugins");
    assert_eq!(attribution("/plugins", "list"), "From: /plugins list");
    assert_eq!(attribution("/connectors", "  "), "From: /connectors");
    assert_eq!(
        attribution("/connectors", "stores"),
        "From: /connectors stores"
    );
}

#[test]
fn subcommand_of_returns_the_first_token_or_empty() {
    assert_eq!(subcommand_of(""), "");
    assert_eq!(subcommand_of("   "), "");
    assert_eq!(subcommand_of("stores --check"), "stores");
    assert_eq!(subcommand_of("  list  --verbose "), "list");
}

#[test]
fn store_dirs_at_orders_global_then_project_and_honours_the_override() {
    let root = std::path::Path::new("target/temp/work");
    let global_root = std::path::Path::new("/home/user/.config/ragent");
    let dirs = store_dirs_at(root, None, Some(global_root), "plugins");
    assert_eq!(
        dirs.global.as_deref(),
        Some(global_root.join("plugins").as_path())
    );
    assert_eq!(
        dirs.project.as_deref(),
        Some(root.join(".ragent").join("plugins").as_path())
    );
    assert_eq!(dirs.destination(), dirs.project.as_deref());

    let override_dir = std::path::Path::new("/elsewhere/store");
    let overridden = store_dirs_at(root, Some(override_dir), None, "connectors");
    assert_eq!(overridden.project.as_deref(), Some(override_dir));
    assert!(overridden.global.is_none());
}

#[test]
fn destination_falls_back_to_the_global_leg() {
    let dirs = StoreDirs {
        project: None,
        global: Some("/g".into()),
    };
    assert_eq!(dirs.destination(), Some(std::path::Path::new("/g")));
    let empty = StoreDirs {
        project: None,
        global: None,
    };
    assert!(empty.destination().is_none());
}

#[test]
fn truncate_marks_only_over_long_text() {
    assert_eq!(truncate("short", 10), "short");
    assert_eq!(truncate("abcdefghij", 10), "abcdefghij");
    assert_eq!(truncate("abcdefghijk", 10), "abcdefghij...");
}

#[test]
fn step_carries_its_name_outcome_elapsed_and_detail() {
    let record = step(
        "discovery",
        StepOutcome::Pass,
        Duration::from_millis(7),
        Some("ok".to_string()),
    );
    assert_eq!(record.name, "discovery");
    assert_eq!(record.outcome, StepOutcome::Pass);
    assert_eq!(record.elapsed, Duration::from_millis(7));
    assert_eq!(record.detail.as_deref(), Some("ok"));
}

#[test]
fn sample_for_schema_generates_typed_values_and_honours_constraints() {
    assert_eq!(
        sample_for_schema(&json!({"type": "string"})),
        json!("sample")
    );
    assert_eq!(sample_for_schema(&json!({"type": "integer"})), json!(0));
    assert_eq!(sample_for_schema(&json!({"type": "number"})), json!(0.0));
    assert_eq!(sample_for_schema(&json!({"type": "boolean"})), json!(false));
    assert_eq!(sample_for_schema(&json!({"type": "null"})), json!(null));
    assert_eq!(
        sample_for_schema(&json!({"const": "fixed"})),
        json!("fixed")
    );
    assert_eq!(
        sample_for_schema(&json!({"type": "string", "default": "d"})),
        json!("d")
    );
    assert_eq!(
        sample_for_schema(&json!({"type": "string", "enum": ["a", "b"]})),
        json!("a")
    );
    // Array-form `type` takes the first string entry.
    assert_eq!(
        sample_for_schema(&json!({"type": ["integer", "null"]})),
        json!(0)
    );
}

#[test]
fn sample_for_schema_recurses_into_objects_and_arrays() {
    let schema = json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "limit": {"type": "integer"}
        }
    });
    assert_eq!(
        sample_for_schema(&schema),
        json!({"path": "sample", "limit": 0})
    );

    let array = json!({"type": "array", "items": {"type": "boolean"}});
    assert_eq!(sample_for_schema(&array), json!([false]));

    let empty_array = json!({"type": "array"});
    assert_eq!(sample_for_schema(&empty_array), json!([]));
}
