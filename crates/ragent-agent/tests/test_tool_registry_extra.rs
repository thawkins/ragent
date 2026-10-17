//! Tests-only coverage for the tool-registry surface in `tool/mod.rs`:
//! register/get/list/definitions/hidden/remove_all/version, the definition
//! cache invalidation, the allowed-tool helpers, and the `ToolContext` default
//! registry.

use std::sync::Arc;

use ragent_agent::tool::{
    Tool, ToolContext, ToolOutput, ToolRegistry, build_allowed_tool_set, create_default_registry,
    is_allowed_tool,
};
use serde_json::{Value, json};

/// A minimal in-test tool used to drive the registry without depending on any
/// real tool implementation.
struct StubTool {
    name: String,
    category: String,
}

#[async_trait::async_trait]
impl Tool for StubTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &'static str {
        "stub tool"
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn permission_category(&self) -> &str {
        &self.category
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput {
            content: "ok".to_string(),
            ..Default::default()
        })
    }
}

fn stub(name: &str) -> Arc<dyn Tool> {
    Arc::new(StubTool {
        name: name.to_string(),
        category: "stub:exec".to_string(),
    })
}

#[test]
fn registry_register_get_list_and_version() {
    let registry = ToolRegistry::new();
    assert_eq!(registry.list().len(), 0);
    let v0 = registry.version();

    registry.register(stub("alpha"));
    registry.register(stub("beta"));
    assert_eq!(
        registry.list(),
        vec!["alpha".to_string(), "beta".to_string()]
    );
    assert!(registry.version() > v0, "register bumps the version");

    let alpha = registry.get("alpha").expect("alpha present");
    assert_eq!(alpha.name(), "alpha");
    assert_eq!(alpha.permission_category(), "stub:exec");
    assert!(registry.get("missing").is_none());
}

#[test]
fn registry_remove_all_is_a_noop_for_empty_and_removes_named() {
    let registry = ToolRegistry::new();
    registry.register(stub("a"));
    registry.register(stub("b"));

    registry.remove_all(&[]);
    assert_eq!(registry.list().len(), 2, "empty list is a no-op");

    registry.remove_all(&["a".to_string(), "ghost".to_string()]);
    assert_eq!(registry.list(), vec!["b".to_string()]);
}

#[test]
fn definitions_cache_is_shared_then_invalidated_on_hidden_change() {
    let registry = ToolRegistry::new();
    registry.register(stub("a"));
    registry.register(stub("b"));

    let first = registry.definitions();
    let second = registry.definitions();
    assert!(
        Arc::ptr_eq(&first, &second),
        "the sorted-definition cache is reused while unchanged"
    );
    assert_eq!(first.len(), 2);
    assert!(first.windows(2).all(|w| w[0].name <= w[1].name));

    registry.set_hidden(&["b".to_string()]);
    let after_hidden = registry.definitions();
    assert!(
        !Arc::ptr_eq(&first, &after_hidden),
        "hidden change invalidates cache"
    );
    assert_eq!(after_hidden.len(), 1);
    assert_eq!(after_hidden[0].name, "a");
}

#[test]
fn hidden_and_hidden_definitions() {
    let registry = create_default_registry();
    assert_eq!(registry.hidden().len(), 0);

    registry.set_hidden(&["read".to_string(), "bash".to_string()]);
    assert_eq!(
        registry.hidden(),
        vec!["bash".to_string(), "read".to_string()]
    );

    let hidden_defs = registry.hidden_definitions();
    let names: Vec<&str> = hidden_defs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["bash", "read"]);

    // Hidden tools remain registered and executable.
    assert!(registry.get("read").is_some());
    let defs = registry.definitions();
    assert!(!defs.iter().any(|d| d.name == "read"));
}

#[test]
fn default_registry_exposes_expected_tools() {
    let registry = create_default_registry();
    for name in ["read", "bash", "think", "plan_enter"] {
        assert!(
            registry.list().contains(&name.to_string()),
            "default registry must expose {name}"
        );
    }
    assert!(registry.all_tools().len() >= registry.definitions().len());
}

#[test]
fn default_tool_registry_is_a_shared_empty_registry() {
    let a = ToolContext::default_tool_registry();
    let b = ToolContext::default_tool_registry();
    assert!(
        Arc::ptr_eq(&a, &b),
        "the default registry is process-shared"
    );
    assert_eq!(a.list().len(), 0);
}

#[test]
fn build_allowed_tool_set_none_is_empty_and_some_adds_always_allowed() {
    let none = build_allowed_tool_set(None::<&[String]>);
    assert_eq!(none.len(), 0);

    let allowed = build_allowed_tool_set(Some(&["read".to_string()]));
    assert!(allowed.contains("read"));
    // The always-allowed safety surface (codeindex + skill tools) is folded in.
    assert!(
        allowed.contains("codeindex_search"),
        "always-allowed codeindex tools must be present"
    );

    assert!(is_allowed_tool("read", &allowed));
    assert!(!is_allowed_tool("bash", &allowed));
}
