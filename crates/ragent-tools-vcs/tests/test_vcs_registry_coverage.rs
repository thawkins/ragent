//! Coverage tests for the extracted VCS tool registry and the `Tool` trait
//! surface (`crate::lib.rs`, `crate::registry`).
//!
//! `create_vcs_registry()` registers every VCS tool, and `definitions()`
//! invokes `name()`, `description()`, and `parameters_schema()` on each one.
//! Exercising the registry therefore drives the per-tool metadata methods that
//! are otherwise only reachable through a live agent session, and pins the
//! registry's registration/visibility contract.

use std::sync::Arc;

use ragent_tools_vcs::registry::create_vcs_registry;
use ragent_tools_vcs::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// A minimal in-memory tool used to drive the registry's own methods without
/// depending on any of the real VCS tools.
struct StubTool {
    name: &'static str,
    category: &'static str,
}

#[async_trait::async_trait]
impl Tool for StubTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &'static str {
        "stub tool for registry tests"
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object" })
    }

    fn permission_category(&self) -> &str {
        self.category
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput {
            content: "stub".to_string(),
            metadata: None,
        })
    }
}

#[test]
fn test_create_vcs_registry_definitions_covers_every_tool() {
    let registry = create_vcs_registry();
    let defs = registry.definitions();

    // The registry registers the full GitHub + GitLab + local-git surface.
    assert!(
        defs.len() >= 40,
        "expected the full VCS tool surface, got {}",
        defs.len()
    );

    // `definitions()` sorts by name and each definition carries the tool's
    // real name, a non-empty description, and an object parameters schema.
    let mut names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    let sorted = {
        let mut copy = names.clone();
        copy.sort_unstable();
        copy
    };
    assert_eq!(names, sorted, "definitions must be sorted by name");
    names.dedup();

    for def in &defs {
        assert!(!def.name.is_empty(), "tool name must not be empty");
        assert!(
            !def.description.is_empty(),
            "tool {} must have a description",
            def.name
        );
        assert_eq!(
            def.parameters.get("type").and_then(Value::as_str),
            Some("object"),
            "tool {} must expose an object schema",
            def.name
        );
    }
}

#[test]
fn test_registry_get_list_and_permission_categories() {
    let registry = create_vcs_registry();
    let names = registry.list();
    assert_eq!(names.len(), registry.definitions().len());

    // Every registered tool is fetchable and reports a permission category
    // drawn from the shared `git:`/`github:`/`gitlab:` families.
    for name in &names {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("tool {name} must be retrievable"));
        assert_eq!(tool.name(), name, "lookup must round-trip the name");
        let category = tool.permission_category();
        assert!(
            category.starts_with("git:")
                || category.starts_with("github:")
                || category.starts_with("gitlab:"),
            "tool {name} has unexpected permission category {category}"
        );
    }

    assert!(registry.get("git_status").is_some());
    assert!(registry.get("github_list_issues").is_some());
    assert!(registry.get("gitlab_list_pipelines").is_some());
    assert!(registry.get("nonexistent_tool_xyz").is_none());
}

#[test]
fn test_registry_register_replaces_and_set_hidden_removes() {
    let registry = create_vcs_registry();

    // Registering a stub under a fresh name adds exactly one entry.
    let before = registry.list().len();
    registry.register(Arc::new(StubTool {
        name: "zz_stub_tool",
        category: "git:read",
    }));
    assert_eq!(registry.list().len(), before + 1);
    let stub = registry.get("zz_stub_tool").expect("stub registered");
    assert_eq!(stub.description(), "stub tool for registry tests");
    assert_eq!(stub.permission_category(), "git:read");

    // Registering again under the same name replaces, not duplicates.
    registry.register(Arc::new(StubTool {
        name: "zz_stub_tool",
        category: "git:write",
    }));
    assert_eq!(registry.list().len(), before + 1);
    assert_eq!(
        registry.get("zz_stub_tool").unwrap().permission_category(),
        "git:write"
    );

    // Hiding a tool removes it from the definitions but not the registry.
    registry.set_hidden(&["zz_stub_tool".to_string()]);
    let defs = registry.definitions();
    assert!(
        defs.iter().all(|d| d.name != "zz_stub_tool"),
        "hidden tool must not appear in definitions"
    );
    assert!(
        registry.get("zz_stub_tool").is_some(),
        "hidden tool remains registered"
    );

    // Replacing the hidden set re-exposes it.
    registry.set_hidden(&[]);
    assert!(
        registry
            .definitions()
            .iter()
            .any(|d| d.name == "zz_stub_tool"),
        "clearing the hidden set must re-expose the tool"
    );
}

#[test]
fn test_registry_default_is_empty_and_matches_new() {
    let a = ragent_tools_vcs::ToolRegistry::default();
    let b = ragent_tools_vcs::ToolRegistry::new();
    assert_eq!(a.list(), [] as [std::string::String; 0]);
    assert_eq!(b.list(), [] as [std::string::String; 0]);
    assert!(a.definitions().is_empty());
}
