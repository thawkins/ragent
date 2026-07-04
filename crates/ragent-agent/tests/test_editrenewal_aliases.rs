//! Tests for the legacy `edit`/`multiedit` aliases and deprecation warnings
//! (editrenewal FR-012, T-015).
//!
//! Verifies that:
//! - The default tool registry exposes `edit`, `multi_edit`, and the legacy
//!   `multiedit` alias.
//! - The `multiedit` alias description marks it as deprecated.
//! - The `multi_edit` tool description references the canonical parameter
//!   names.
//! - The `edit` tool description instructs the model to include context and
//!   states that old_string must be unique.

use ragent_agent::tool::create_default_registry;
use ragent_llm::llm::ToolDefinition;

fn def_for(tool_name: &str) -> ToolDefinition {
    let registry = create_default_registry();
    registry
        .definitions()
        .into_iter()
        .find(|d| d.name == tool_name)
        .unwrap_or_else(|| panic!("tool '{tool_name}' should be registered"))
}

fn names_sorted() -> Vec<String> {
    let registry = create_default_registry();
    let mut names: Vec<String> = registry.definitions().into_iter().map(|d| d.name).collect();
    names.sort();
    names
}

#[test]
fn test_registry_contains_edit_and_multi_edit_and_legacy_alias() {
    let names = names_sorted();
    assert!(
        names.iter().any(|n| n == "edit"),
        "edit tool must be registered: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "multi_edit"),
        "multi_edit tool must be registered: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "multiedit"),
        "legacy multiedit alias must be registered: {names:?}"
    );
}

#[test]
fn test_multiedit_alias_description_marks_deprecation() {
    let def = def_for("multiedit");
    assert!(
        def.description.contains("Deprecated") || def.description.contains("deprecated"),
        "multiedit alias description should mention deprecation: {}",
        def.description
    );
    assert!(
        def.description.contains("multi_edit"),
        "multiedit alias description should point to multi_edit: {}",
        def.description
    );
}

#[test]
fn test_multi_edit_description_references_canonical_params() {
    let def = def_for("multi_edit");
    assert!(
        def.description.contains("file_path"),
        "multi_edit description should mention file_path: {}",
        def.description
    );
    assert!(
        def.description.contains("old_string"),
        "multi_edit description should mention old_string: {}",
        def.description
    );
    assert!(
        def.description.contains("new_string"),
        "multi_edit description should mention new_string: {}",
        def.description
    );
}

#[test]
fn test_edit_description_states_uniqueness_and_context() {
    let def = def_for("edit");
    assert!(
        def.description.contains("exactly once") || def.description.contains("unique"),
        "edit description should state old_string must be unique: {}",
        def.description
    );
    assert!(
        def.description.contains("context"),
        "edit description should instruct including context: {}",
        def.description
    );
}

#[test]
fn test_edit_schema_declares_canonical_params() {
    let def = def_for("edit");
    let schema_str = def.parameters.to_string();
    assert!(
        schema_str.contains("file_path"),
        "edit schema should list file_path"
    );
    assert!(
        schema_str.contains("old_string"),
        "edit schema should list old_string"
    );
    assert!(
        schema_str.contains("new_string"),
        "edit schema should list new_string"
    );
}

#[test]
fn test_multi_edit_schema_declares_canonical_params() {
    let def = def_for("multi_edit");
    let schema_str = def.parameters.to_string();
    assert!(
        schema_str.contains("file_path"),
        "multi_edit schema should list file_path"
    );
    assert!(
        schema_str.contains("old_string"),
        "multi_edit schema should list old_string"
    );
    assert!(
        schema_str.contains("new_string"),
        "multi_edit schema should list new_string"
    );
}
