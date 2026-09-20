//! Tests for registering new graph tools in the tool registry (spec graphCI, T-027, FR-006).
//!
//! FR-006: The existing codeindex tools shall remain registered and
//! functional, with no change to their names, parameter schemas, or
//! permission categories. The new graph tools (codeindex_explain,
//! codeindex_path, codeindex_communities, codeindex_godnodes) shall also
//! be registered.

use ragent_tools_extended::create_extended_registry;

// ── New graph tools are registered ───────────────────────────────────────

#[test]
fn test_codeindex_explain_registered() {
    let registry = create_extended_registry();
    assert!(
        registry.contains("codeindex_explain"),
        "codeindex_explain should be registered in the extended registry"
    );
}

#[test]
fn test_codeindex_path_registered() {
    let registry = create_extended_registry();
    assert!(
        registry.contains("codeindex_path"),
        "codeindex_path should be registered in the extended registry"
    );
}

#[test]
fn test_codeindex_communities_registered() {
    let registry = create_extended_registry();
    assert!(
        registry.contains("codeindex_communities"),
        "codeindex_communities should be registered in the extended registry"
    );
}

#[test]
fn test_codeindex_godnodes_registered() {
    let registry = create_extended_registry();
    assert!(
        registry.contains("codeindex_godnodes"),
        "codeindex_godnodes should be registered in the extended registry"
    );
}

// ── Existing codeindex tools remain registered (FR-006) ────────────────

#[test]
fn test_existing_codeindex_tools_remain_registered() {
    let registry = create_extended_registry();
    for name in &[
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_reindex",
    ] {
        assert!(
            registry.contains(name),
            "Existing tool {name} should remain registered (FR-006)"
        );
    }
}

// ── All codeindex tools have the correct permission category ────────────

#[test]
fn test_all_codeindex_tools_have_codeindex_read_permission() {
    let registry = create_extended_registry();
    // Read-only tools that must have the "codeindex:read" permission category.
    for name in &[
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_explain",
        "codeindex_path",
        "codeindex_communities",
        "codeindex_godnodes",
    ] {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool {name} should be registered"));
        assert_eq!(
            tool.permission_category(),
            "codeindex:read",
            "Tool {name} should have permission category 'codeindex:read'"
        );
    }
    // codeindex_reindex is a write operation and has its own category.
    let reindex = registry
        .get("codeindex_reindex")
        .expect("codeindex_reindex should be registered");
    assert_eq!(
        reindex.permission_category(),
        "codeindex:write",
        "codeindex_reindex should have permission category 'codeindex:write'"
    );
}

// ── Total codeindex tool count ──────────────────────────────────────────

#[test]
fn test_total_codeindex_tool_count() {
    let registry = create_extended_registry();
    // 6 existing + 4 new = 10 codeindex tools.
    let codeindex_names: Vec<String> = [
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_reindex",
        "codeindex_explain",
        "codeindex_path",
        "codeindex_communities",
        "codeindex_godnodes",
    ]
    .into_iter()
    .filter(|name| registry.contains(name))
    .map(String::from)
    .collect();
    assert_eq!(
        codeindex_names.len(),
        10,
        "Expected 10 codeindex tools (6 existing + 4 new), found {}",
        codeindex_names.len()
    );
}
