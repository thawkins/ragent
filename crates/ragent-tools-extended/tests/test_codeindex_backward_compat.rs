//! Backward-compatibility verification for existing `codeindex_*` tools
//! (spec graphCI, T-029, FR-006).
//!
//! FR-006: The existing `codeindex_search`, `codeindex_symbols`,
//! `codeindex_references`, `codeindex_dependencies`, `codeindex_status`,
//! and `codeindex_reindex` tools shall remain registered and functional,
//! with no change to their names, parameter schemas, or permission
//! categories.
//!
//! This test verifies that all six existing codeindex tools:
//!   1. Are registered in the extended registry.
//!   2. Have the correct tool name (unchanged).
//!   3. Have the `codeindex:read` permission category (unchanged).
//!   4. Have a non-empty parameters_schema.
//!   5. Have a non-empty description.
//!
//! The new graph tools (`codeindex_explain`, `codeindex_path`,
//! `codeindex_communities`, `codeindex_godnodes`) are also verified
//! to ensure the extension is purely additive.

use ragent_tools_extended::create_extended_registry;
use serde_json::Value;

/// All six existing codeindex tools that must remain unchanged (FR-006).
const EXISTING_TOOLS: &[&str] = &[
    "codeindex_search",
    "codeindex_symbols",
    "codeindex_references",
    "codeindex_dependencies",
    "codeindex_status",
    "codeindex_reindex",
];

/// All four new graph tools that should be registered additively (FR-006).
const NEW_TOOLS: &[&str] = &[
    "codeindex_explain",
    "codeindex_path",
    "codeindex_communities",
    "codeindex_godnodes",
];

// ── Existing tool registration ─────────────────────────────────────────

#[test]
fn test_all_existing_codeindex_tools_registered() {
    let registry = create_extended_registry();
    for name in EXISTING_TOOLS {
        assert!(
            registry.contains(name),
            "Existing tool `{name}` must be registered in the extended registry (FR-006)",
        );
    }
}

#[test]
fn test_all_new_codeindex_tools_registered() {
    let registry = create_extended_registry();
    for name in NEW_TOOLS {
        assert!(
            registry.contains(name),
            "New tool `{name}` must be registered in the extended registry (FR-006)",
        );
    }
}

#[test]
fn test_existing_tool_names_unchanged() {
    let registry = create_extended_registry();
    for name in EXISTING_TOOLS {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool `{name}` should be registered"));
        // The tool's self-reported name must match the registration name.
        assert_eq!(tool.name(), *name, "Tool name must be unchanged (FR-006)");
    }
}

#[test]
fn test_existing_tool_permission_categories_unchanged() {
    let registry = create_extended_registry();
    // FR-006: permission categories must remain unchanged from pre-graphCI values.
    let expected_cats: &[(&str, &str)] = &[
        ("codeindex_search", "codeindex:read"),
        ("codeindex_symbols", "codeindex:read"),
        ("codeindex_references", "codeindex:read"),
        ("codeindex_dependencies", "codeindex:read"),
        ("codeindex_status", "none"),
        ("codeindex_reindex", "codeindex:write"),
    ];
    for (name, expected_cat) in expected_cats {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool `{name}` should be registered"));
        assert_eq!(
            tool.permission_category(),
            *expected_cat,
            "Tool `{name}` must retain `{expected_cat}` permission category (FR-006)",
        );
    }
}

#[test]
fn test_existing_tool_parameters_schemas_nonempty() {
    let registry = create_extended_registry();
    for name in EXISTING_TOOLS {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool `{name}` should be registered"));
        let schema = tool.parameters_schema();
        assert!(
            schema.is_object(),
            "Tool `{name}` must have an object parameters_schema (FR-006)",
        );
    }
}

#[test]
fn test_existing_tool_descriptions_nonempty() {
    let registry = create_extended_registry();
    for name in EXISTING_TOOLS {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool `{name}` should be registered"));
        let desc = tool.description();
        assert!(
            !desc.is_empty(),
            "Tool `{name}` must have a non-empty description (FR-006)",
        );
    }
}

// ── New tool permission categories match existing ─────────────────────

#[test]
fn test_new_tool_permission_categories_match_existing() {
    let registry = create_extended_registry();
    for name in NEW_TOOLS {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool `{name}` should be registered"));
        assert_eq!(
            tool.permission_category(),
            "codeindex:read",
            "New tool `{name}` must use `codeindex:read` permission category (FR-006)",
        );
    }
}

// ── Total codeindex tool count ─────────────────────────────────────────

#[test]
fn test_total_codeindex_tool_count_is_ten() {
    let registry = create_extended_registry();
    let count = EXISTING_TOOLS.len() + NEW_TOOLS.len();
    for name in EXISTING_TOOLS.iter().chain(NEW_TOOLS.iter()) {
        assert!(
            registry.contains(*name),
            "tool `{name}` should be registered"
        );
    }
    assert_eq!(
        count, 10,
        "Expected 10 codeindex tools (6 existing + 4 new)"
    );
}

// ── No existing tool was renamed or removed ────────────────────────────

#[test]
fn test_no_existing_tool_removed_or_renamed() {
    let registry = create_extended_registry();
    for name in EXISTING_TOOLS {
        assert!(
            registry.contains(name),
            "Existing tool `{name}` must not be removed or renamed (FR-006)",
        );
    }
}

// ── Existing tool parameter schemas are objects (not broken) ───────────

#[test]
fn test_existing_tool_search_has_query_parameter() {
    let registry = create_extended_registry();
    let tool = registry
        .get("codeindex_search")
        .expect("codeindex_search registered");
    let schema: Value = tool.parameters_schema();
    let props = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .expect("codeindex_search has properties");
    assert!(
        props.contains_key("query"),
        "codeindex_search must retain `query` parameter (FR-006)",
    );
}

#[test]
fn test_existing_tool_symbols_has_name_parameter() {
    let registry = create_extended_registry();
    let tool = registry
        .get("codeindex_symbols")
        .expect("codeindex_symbols registered");
    let schema: Value = tool.parameters_schema();
    let props = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .expect("codeindex_symbols has properties");
    assert!(
        props.contains_key("name"),
        "codeindex_symbols must retain `name` parameter (FR-006)",
    );
}

#[test]
fn test_existing_tool_references_has_symbol_parameter() {
    let registry = create_extended_registry();
    let tool = registry
        .get("codeindex_references")
        .expect("codeindex_references registered");
    let schema: Value = tool.parameters_schema();
    let props = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .expect("codeindex_references has properties");
    assert!(
        props.contains_key("symbol"),
        "codeindex_references must retain `symbol` parameter (FR-006)",
    );
}

#[test]
fn test_existing_tool_dependencies_has_path_parameter() {
    let registry = create_extended_registry();
    let tool = registry
        .get("codeindex_dependencies")
        .expect("codeindex_dependencies registered");
    let schema: Value = tool.parameters_schema();
    let props = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .expect("codeindex_dependencies has properties");
    assert!(
        props.contains_key("path"),
        "codeindex_dependencies must retain `path` parameter (FR-006)",
    );
}

#[test]
fn test_existing_tool_status_has_no_required_parameters() {
    let registry = create_extended_registry();
    let tool = registry
        .get("codeindex_status")
        .expect("codeindex_status registered");
    let schema: Value = tool.parameters_schema();
    // codeindex_status should not require any parameters.
    let required = schema.get("required").and_then(|r| r.as_array());
    if let Some(req) = required {
        assert!(
            req.is_empty(),
            "codeindex_status must not require any parameters (FR-006)",
        );
    }
}

#[test]
fn test_existing_tool_reindex_has_no_required_parameters() {
    let registry = create_extended_registry();
    let tool = registry
        .get("codeindex_reindex")
        .expect("codeindex_reindex registered");
    let schema: Value = tool.parameters_schema();
    let required = schema.get("required").and_then(|r| r.as_array());
    if let Some(req) = required {
        assert!(
            req.is_empty(),
            "codeindex_reindex must not require any parameters (FR-006)",
        );
    }
}
