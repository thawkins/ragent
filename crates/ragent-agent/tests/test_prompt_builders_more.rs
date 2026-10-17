//! Additional tests-only coverage for the session prompt builders
//! (`ragent_agent::session::prompt_builders`).
//!
//! The whole source file is compiled into this test crate through the documented
//! `#[path]` shim (the same source path, so the coverage tool attributes the
//! lines), and every public / `pub(crate)` builder is exercised so none is left
//! as dead code. Every `build_goal_loop_section` optional section, both
//! codeindex-guidance states, and the concise/detailed tool-reference builders
//! (including the schema-type / required / truncation branches) are covered.

use std::sync::Arc;

#[path = "../src/session/prompt_builders.rs"]
mod prompt_builders;

mod llm {
    pub use ragent_agent::llm::ToolDefinition;
}

mod tool {
    pub use ragent_agent::tool::ToolRegistry;
}

mod session {
    pub mod loop_state {
        pub use ragent_agent::session::loop_state::LoopSpec;
    }
}

use llm::ToolDefinition;
use prompt_builders::{
    TOOL_CALLING_GUIDANCE, build_codeindex_guidance_section_active,
    build_codeindex_guidance_section_disabled, build_detailed_tool_reference_from_defs,
    build_detailed_tool_reference_section, build_goal_loop_section, build_tool_reference_from_defs,
    build_tool_reference_section,
};
use session::loop_state::LoopSpec;

use ragent_agent::tool::{ToolRegistry, create_default_registry};
use serde_json::json;

fn def(name: &str, description: &str, parameters: serde_json::Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

// ---------------------------------------------------------------------------
// Goal-loop section: every optional branch.
// ---------------------------------------------------------------------------

#[test]
fn goal_loop_section_minimal_skips_every_optional_section() {
    let spec = LoopSpec::new("coder", "all tests pass");
    let section = build_goal_loop_section(&spec);
    assert!(section.contains("## Goal Loop"));
    assert!(section.contains("`coder`"));
    assert!(section.contains("all tests pass"));
    assert!(section.contains("Iterate plan-act-observe"));
    assert!(!section.contains("### Scope boundaries"));
    assert!(!section.contains("### Read-only constraints"));
    assert!(!section.contains("### Tool set"));
    assert!(!section.contains("### Budget"));
    assert!(!section.contains("verification command"));
}

#[test]
fn goal_loop_section_renders_every_constraint() {
    let mut spec = LoopSpec::new("architect", "ship the feature");
    spec.verify_cmd = Some("cargo test".to_string());
    spec.scope = vec!["src/**".to_string(), "tests/**".to_string()];
    spec.read_only = vec!["vendor/**".to_string()];
    spec.tool_set = vec!["read".to_string()];
    spec.max_steps = Some(7);
    spec.cost_limit = Some(1234);
    let section = build_goal_loop_section(&spec);
    assert!(section.contains("verification command `cargo test`"));
    assert!(section.contains("src/**"));
    assert!(section.contains("### Read-only constraints"));
    assert!(section.contains("### Tool set"));
    assert!(section.contains("at most 7 loop iterations"));
    assert!(section.contains("at most 1234 accumulated tokens"));
}

#[test]
fn goal_loop_section_cost_only_budget() {
    // A cost limit with no step limit renders only the cost budget line.
    let mut spec = LoopSpec::new("general", "reduce spend");
    spec.cost_limit = Some(999);
    let section = build_goal_loop_section(&spec);
    assert!(section.contains("at most 999 accumulated tokens"));
    assert!(!section.contains("Step budget"));
    assert!(!section.contains("### Scope boundaries"));
}

#[test]
fn goal_loop_section_step_only_budget() {
    let mut spec = LoopSpec::new("general", "ship");
    spec.max_steps = Some(3);
    let section = build_goal_loop_section(&spec);
    assert!(section.contains("at most 3 loop iterations"));
    assert!(!section.contains("Cost budget"));
}

#[test]
fn goal_loop_section_verify_cmd_only() {
    let mut spec = LoopSpec::new("coder", "green build");
    spec.verify_cmd = Some("cargo test".to_string());
    let section = build_goal_loop_section(&spec);
    assert!(section.contains("verification command `cargo test`"));
    assert!(!section.contains("### Tool set"));
    assert!(!section.contains("### Budget"));
}

// ---------------------------------------------------------------------------
// Codeindex guidance + tool-calling constant.
// ---------------------------------------------------------------------------

#[test]
fn codeindex_guidance_builders_cover_both_states() {
    let active = build_codeindex_guidance_section_active();
    let disabled = build_codeindex_guidance_section_disabled();
    assert!(active.contains("Code Intelligence"));
    assert!(active.contains("MANDATORY"));
    assert!(disabled.contains("not active"));
    assert_ne!(active, disabled);
}

#[test]
fn tool_calling_guidance_is_present() {
    assert!(TOOL_CALLING_GUIDANCE.contains("Tool Use - Critical Instructions"));
    assert!(TOOL_CALLING_GUIDANCE.contains("IMMEDIATELY"));
}

// ---------------------------------------------------------------------------
// Concise tool reference.
// ---------------------------------------------------------------------------

#[test]
fn tool_reference_from_defs_empty_and_non_empty() {
    assert_eq!(build_tool_reference_from_defs(&[]), "");
    let defs = vec![def("demo", &"x".repeat(300), json!({}))];
    let section = build_tool_reference_from_defs(&defs);
    assert!(section.starts_with("## Available Tools"));
    assert!(section.contains("- `demo` - "));
    // The 120-byte description cap keeps the line short.
    assert!(section.len() < 300);
}

#[test]
fn tool_reference_section_from_registry_matches_defs() {
    let registry = create_default_registry();
    let concise = build_tool_reference_section(&registry);
    assert_eq!(
        concise,
        build_tool_reference_from_defs(&registry.definitions())
    );
    // An empty registry produces an empty concise section.
    assert_eq!(build_tool_reference_section(&ToolRegistry::new()), "");
}

// ---------------------------------------------------------------------------
// Detailed tool reference: schema type / required / anyOf / oneOf branches.
// ---------------------------------------------------------------------------

#[test]
fn detailed_tool_reference_covers_schema_type_branches() {
    assert_eq!(build_detailed_tool_reference_from_defs(&[]), "");

    let d = def(
        "demo",
        "Demo\ndescription\rwith newlines",
        json!({
            "properties": {
                "alpha": {"type": "string", "description": "first"},
                "beta": {"anyOf": [{"type": "string"}, {"type": "number"}]},
                "gamma": {"oneOf": [{"type": "number"}]},
                "delta": {"description": "no type at all"}
            },
            "required": ["alpha", "beta"]
        }),
    );
    let section = build_detailed_tool_reference_from_defs(&[d]);
    assert!(section.contains("### `demo`"));
    assert!(section.contains("- `alpha` (`string` (required)"));
    assert!(section.contains("- `beta` (`anyOf` (required)"));
    assert!(section.contains("- `gamma` (`anyOf`)"));
    assert!(section.contains("- `delta` (`any`)"));
    // Newlines/CR in the description are collapsed.
    assert!(!section.contains('\r'));
}

#[test]
fn detailed_tool_reference_handles_def_without_properties() {
    let d = def("noargs", "no parameters", json!({}));
    let section = build_detailed_tool_reference_from_defs(&[d]);
    assert!(section.contains("### `noargs`"));
    assert!(!section.contains("- `"));
}

#[test]
fn detailed_tool_reference_from_registry_matches_defs() {
    let registry = create_default_registry();
    assert_eq!(
        build_detailed_tool_reference_section(&registry),
        build_detailed_tool_reference_from_defs(&registry.definitions())
    );
    assert_eq!(
        build_detailed_tool_reference_section(&ToolRegistry::new()),
        ""
    );
}

#[test]
fn detailed_tool_reference_truncates_long_parameter_descriptions() {
    let d = def(
        "wide",
        "wide tool",
        json!({
            "properties": {
                "p": {"type": "string", "description": "y".repeat(400)}
            }
        }),
    );
    let section = build_detailed_tool_reference_from_defs(&[d]);
    assert!(section.contains("- `p` (`string`) - "));
    // The parameter description is capped at 140 bytes.
    assert!(
        section
            .lines()
            .any(|l| l.starts_with("- `p`") && l.len() < 200)
    );
}

#[test]
fn registry_arc_is_shareable() {
    let registry: Arc<ToolRegistry> = Arc::new(create_default_registry());
    assert!(!registry.definitions().is_empty());
}
