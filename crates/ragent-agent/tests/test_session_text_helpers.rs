//! External tests for the session text/permission/prompt helpers
//! (`session::text_toolcalls`, `session::prompt_builders`, `session::verification`
//! and `session::permissions`) that are `pub(crate)` and therefore unreachable
//! from an integration test through the crate's public API alone.
//!
//! The crate-private modules are included directly with the documented
//! `#[path]` shim pattern (see `tests/test_text_toolcalls.rs` for the worked
//! example): the source files are compiled into the test crate and small
//! re-export shims supply the `super::` / `crate::` names each file imports.
//! Public helpers are exercised through their normal crate paths.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Shims: `session::text_toolcalls`
// ---------------------------------------------------------------------------

#[path = "../src/session/text_toolcalls.rs"]
mod text_toolcalls;

// Satisfies the source file's `use super::history::PendingToolCall;`.
mod history {
    pub struct PendingToolCall {
        pub id: String,
        pub name: String,
        pub args_json: String,
    }
}

// Mirror of the private tag constants in the source module.
const OPEN: &str = "\u{e5b6}call\u{e5b6}";
const CLOSE: &str = "\u{e5b6}/call\u{e5b6}";

use text_toolcalls::{blank_spans, extract_text_tool_calls_with_spans};

fn extract_text_tool_calls(text: &str) -> Vec<history::PendingToolCall> {
    extract_text_tool_calls_with_spans(text).0
}

// ---------------------------------------------------------------------------
// Shims: `session::prompt_builders`
// ---------------------------------------------------------------------------

#[path = "../src/session/prompt_builders.rs"]
mod prompt_builders;

mod llm {
    pub use ragent_agent::llm::ToolDefinition;
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

use ragent_agent::tool::create_default_registry;

// ---------------------------------------------------------------------------
// Shims: `session::permissions`
// ---------------------------------------------------------------------------

// The whole source module is compiled into this test binary, but the tests
// here exercise only its hardwired/allowlist helpers; the remaining
// permission-prompt and repeat-guard items are compiled but unused in this
// binary and are covered through their own crate paths elsewhere.
#[allow(dead_code)]
#[path = "../src/session/permissions.rs"]
mod perms_src;

mod event {
    pub use ragent_agent::event::EventBus;
}

mod permission {
    pub use ragent_agent::permission::{PermissionAction, PermissionChecker};
}

use event::EventBus;
use permission::{PermissionAction, PermissionChecker};
use perms_src::{
    SKILL_ALWAYS_ALLOWED_TOOLS, is_hardwired_auto_approved_tool, is_hardwired_codeindex_tool,
    strip_timeout_prefix,
};

use ragent_agent::session::permissions::{
    DESTRUCTIVE_GIT_TOOLS, DESTRUCTIVE_TOOLS, RepeatTracker, TOOL_REPEAT_LIMIT,
    check_permission_with_prompt, check_tool_repeat_guard, extract_resource_from_input_for_test,
    is_destructive_tool_for_test, is_interactive_tool,
};

// ---------------------------------------------------------------------------
// text_toolcalls
// ---------------------------------------------------------------------------

#[test]
fn test_bare_json_array_single_element() {
    let calls = extract_text_tool_calls(r#"[{"name":"read","arguments":{"path":"a"}}]"#);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read");
    // Recovered calls carry a synthetic id.
    assert!(calls[0].id.starts_with("text-"));
}

#[test]
fn test_bare_json_argument_key_aliases() {
    for key in ["arguments", "input", "parameters"] {
        let text = format!(r#"{{"name":"grep","{key}":{{"pattern":"x"}}}}"#);
        let calls = extract_text_tool_calls(&text);
        assert_eq!(calls.len(), 1, "key {key}");
        let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
        assert_eq!(args["pattern"], "x");
    }
}

#[test]
fn test_bare_json_null_arguments_become_empty_object() {
    let calls = extract_text_tool_calls(r#"{"name":"think","arguments":null}"#);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].args_json, "{}");
}

#[test]
fn test_bare_json_no_arguments_key_is_empty_object() {
    let calls = extract_text_tool_calls(r#"{"name":"list_files"}"#);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].args_json, "{}");
}

#[test]
fn test_blank_name_is_not_a_call() {
    assert!(extract_text_tool_calls(r#"{"name":"   ","arguments":{}}"#).is_empty());
}

#[test]
fn test_multiple_markup_blocks_recovered_in_order() {
    let text = format!(
        "{OPEN}{{\"name\":\"read\",\"arguments\":{{}}}}{CLOSE} and \
         {OPEN}{{\"name\":\"grep\",\"arguments\":{{}}}}{CLOSE} and \
         {OPEN}{{\"name\":\"think\",\"arguments\":{{}}}}{CLOSE}"
    );
    let calls = extract_text_tool_calls(&text);
    let names: Vec<&str> = calls.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["read", "grep", "think"]);
}

#[test]
fn test_function_calls_wrapper_dialect() {
    let text = "<function_calls>\n<function=bash>\n<parameter=command>\nls\n\
                </parameter>\n</function>\n</function_calls>";
    let calls = extract_text_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "bash");
    let args: Value = serde_json::from_str(&calls[0].args_json).unwrap();
    assert_eq!(args["command"], "ls");
}

#[test]
fn test_unterminated_block_yields_nothing() {
    let text = format!("{OPEN}{{\"name\":\"read\",\"arguments\":{{}}}}");
    assert!(extract_text_tool_calls(&text).is_empty());
}

#[test]
fn test_empty_function_name_is_ignored() {
    let text = format!("{OPEN}<function=>\n</function>{CLOSE}");
    assert!(extract_text_tool_calls(&text).is_empty());
}

#[test]
fn test_xml_parameter_without_closing_tag_is_dropped() {
    let text = format!("{OPEN}<function=read>\n<parameter=path>\nsrc/main.rs\n</function>{CLOSE}");
    let calls = extract_text_tool_calls(&text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read");
    // The unterminated parameter is dropped, so the call carries no arguments.
    assert_eq!(calls[0].args_json, "{}");
}

#[test]
fn test_blank_spans_noop_and_out_of_range() {
    let mut buffer = String::from("hello world");
    // Empty span list is a no-op.
    blank_spans(&mut buffer, &[]);
    assert_eq!(buffer, "hello world");
    // A span running past the buffer end is ignored (no panic, no change).
    blank_spans(&mut buffer, &[(0, 999)]);
    assert_eq!(buffer, "hello world");
}

// ---------------------------------------------------------------------------
// prompt_builders
// ---------------------------------------------------------------------------

#[test]
fn test_tool_calling_guidance_is_present() {
    assert!(TOOL_CALLING_GUIDANCE.contains("Tool Use - Critical Instructions"));
    assert!(TOOL_CALLING_GUIDANCE.contains("IMMEDIATELY"));
}

#[test]
fn test_goal_loop_section_minimal_spec_skips_empty_sections() {
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
fn test_goal_loop_section_renders_every_constraint() {
    let mut spec = LoopSpec::new("architect", "ship the feature");
    spec.verify_cmd = Some("cargo test".to_string());
    spec.scope = vec!["src/**".to_string()];
    spec.read_only = vec!["tests/**".to_string()];
    spec.tool_set = vec!["read".to_string()];
    spec.max_steps = Some(7);
    spec.cost_limit = Some(1234);
    let section = build_goal_loop_section(&spec);
    assert!(section.contains("verification command `cargo test`"));
    assert!(section.contains("src/**"));
    assert!(section.contains("tests/**"));
    assert!(section.contains("### Tool set"));
    assert!(section.contains("at most 7 loop iterations"));
    assert!(section.contains("at most 1234 accumulated tokens"));
}

#[test]
fn test_codeindex_guidance_active_and_disabled_differ() {
    let active = build_codeindex_guidance_section_active();
    let disabled = build_codeindex_guidance_section_disabled();
    assert!(active.contains("Code Intelligence"));
    assert!(active.contains("MANDATORY"));
    assert!(disabled.contains("not active"));
    assert_ne!(active, disabled);
}

fn tool_def(name: &str, description: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({"properties": {}}),
    }
}

#[test]
fn test_tool_reference_from_defs_empty_is_empty_string() {
    assert_eq!(build_tool_reference_from_defs(&[]), "");
    assert_eq!(build_detailed_tool_reference_from_defs(&[]), "");
}

#[test]
fn test_tool_reference_from_defs_lists_names_and_truncates() {
    let defs = vec![tool_def("demo", &"x".repeat(300))];
    let section = build_tool_reference_from_defs(&defs);
    assert!(section.starts_with("## Available Tools"));
    assert!(section.contains("- `demo` - "));
    assert!(section.ends_with('\n'));
    // The 120-byte description cap keeps the section compact.
    assert!(section.len() < 300);
}

#[test]
fn test_detailed_tool_reference_renders_schema_types_and_required() {
    let def = ToolDefinition {
        name: "demo".to_string(),
        description: "Demo\ndescription".to_string(),
        parameters: json!({
            "properties": {
                "alpha": {"type": "string", "description": "first"},
                "beta": {"anyOf": [{"type": "string"}]},
                "gamma": {"oneOf": [{"type": "number"}]},
                "delta": {"description": "no type"}
            },
            "required": ["alpha", "beta"]
        }),
    };
    let section = build_detailed_tool_reference_from_defs(&[def]);
    assert!(section.contains("### `demo`"));
    assert!(section.contains("- `alpha` (`string` (required)"));
    assert!(section.contains("- `beta` (`anyOf` (required)"));
    assert!(section.contains("- `gamma` (`anyOf`)"));
    assert!(section.contains("- `delta` (`any`)"));
    // Newlines in the description are collapsed onto one line.
    assert!(!section.contains("Demo\ndescription"));
}

#[test]
fn test_detailed_tool_reference_handles_def_without_properties() {
    let def = ToolDefinition {
        name: "noargs".to_string(),
        description: "no parameters".to_string(),
        parameters: json!({}),
    };
    let section = build_detailed_tool_reference_from_defs(&[def]);
    assert!(section.contains("### `noargs`"));
    assert!(!section.contains("- `"));
}

#[test]
fn test_registry_backed_section_builders_match_defs() {
    let registry = create_default_registry();
    let concise = build_tool_reference_section(&registry);
    let detailed = build_detailed_tool_reference_section(&registry);
    assert!(concise.starts_with("## Available Tools"));
    assert_eq!(
        concise,
        build_tool_reference_from_defs(&registry.definitions())
    );
    assert_eq!(
        detailed,
        build_detailed_tool_reference_from_defs(&registry.definitions())
    );
}

// ---------------------------------------------------------------------------
// verification (public API through the normal crate path)
// ---------------------------------------------------------------------------

use ragent_agent::session::verification::{
    VerificationOutcome, run_verification_command, verification_failure_observation,
};

#[test]
fn test_verification_outcome_accessors() {
    let success = VerificationOutcome::Success("all good".to_string());
    assert!(success.passed());
    assert_eq!(success.output(), "all good");
    assert_eq!(success.label(), "verification command passed");

    let failure = VerificationOutcome::Failure {
        reason: "exit code 1".to_string(),
        output: "boom".to_string(),
    };
    assert!(!failure.passed());
    assert_eq!(failure.output(), "boom");
    assert_eq!(failure.label(), "verification command failed (exit code 1)");
}

#[test]
fn test_verification_failure_observation_embeds_command_and_output() {
    let outcome = VerificationOutcome::Failure {
        reason: "exit code 2".to_string(),
        output: "2 tests failed".to_string(),
    };
    let observation = verification_failure_observation("cargo test", &outcome);
    assert!(observation.contains("cargo test"));
    assert!(observation.contains("verification command failed (exit code 2)"));
    assert!(observation.contains("2 tests failed"));
}

#[tokio::test]
async fn test_run_verification_command_success_and_failure() {
    let dir = std::env::temp_dir();

    let ok = run_verification_command("echo hello", &dir).await.unwrap();
    assert!(ok.passed());
    assert!(ok.output().contains("hello"));

    let bad = run_verification_command("exit 3", &dir).await.unwrap();
    assert!(!bad.passed());
    assert_eq!(bad.label(), "verification command failed (exit code 3)");
}

// ---------------------------------------------------------------------------
// permissions
// ---------------------------------------------------------------------------

#[test]
fn test_codeindex_tools_are_hardwired_allowed() {
    for name in [
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_reindex",
    ] {
        assert!(is_hardwired_codeindex_tool(name), "{name}");
    }
    assert!(!is_hardwired_codeindex_tool("grep"));
    // The graph tools are not part of the six-name auto-approved subset.
    assert!(!is_hardwired_codeindex_tool("codeindex_explain"));
}

#[test]
fn test_skill_always_allowed_tools_set() {
    assert_eq!(SKILL_ALWAYS_ALLOWED_TOOLS.len(), 7);
    for name in [
        "think",
        "ask_user",
        "agent_complete",
        "model_info",
        "ragent_info",
        "tool_info",
        "commands_info",
    ] {
        assert!(SKILL_ALWAYS_ALLOWED_TOOLS.contains(&name), "{name}");
    }
}

#[test]
fn test_hardwired_auto_approved_predicate_branches() {
    assert!(is_hardwired_auto_approved_tool("codeindex_search"));
    assert!(is_hardwired_auto_approved_tool("team_status"));
    assert!(is_hardwired_auto_approved_tool("task_create"));
    assert!(is_hardwired_auto_approved_tool("new_agent"));
    assert!(is_hardwired_auto_approved_tool("ask_user"));
    for name in ["model_info", "ragent_info", "tool_info", "commands_info"] {
        assert!(is_hardwired_auto_approved_tool(name), "{name}");
    }
    assert!(!is_hardwired_auto_approved_tool("bash"));
    assert!(!is_hardwired_auto_approved_tool("read"));
}

#[test]
fn test_interactive_tool_predicate() {
    assert!(is_interactive_tool("ask_user"));
    assert!(!is_interactive_tool("bash"));
    assert!(!is_interactive_tool("ask_user_extra"));
}

#[test]
fn test_destructive_tool_constants_and_predicate() {
    assert!(DESTRUCTIVE_TOOLS.contains(&"rm"));
    assert!(DESTRUCTIVE_TOOLS.contains(&"config_write"));
    assert!(DESTRUCTIVE_GIT_TOOLS.contains(&"git_merge"));
    assert!(is_destructive_tool_for_test("rm", &json!({})));
    assert!(!is_destructive_tool_for_test("read", &json!({})));
}

#[test]
fn test_destructive_git_invocation_flags() {
    assert!(is_destructive_tool_for_test(
        "git_push",
        &json!({"force": true})
    ));
    assert!(!is_destructive_tool_for_test("git_push", &json!({})));
    assert!(is_destructive_tool_for_test(
        "git_reset",
        &json!({"mode": "hard"})
    ));
    assert!(is_destructive_tool_for_test(
        "git_reset",
        &json!({"mode": "keep"})
    ));
    assert!(!is_destructive_tool_for_test(
        "git_reset",
        &json!({"mode": "soft"})
    ));
    assert!(is_destructive_tool_for_test(
        "git_checkout",
        &json!({"paths": ["a"]})
    ));
    assert!(is_destructive_tool_for_test(
        "git_checkout",
        &json!({"force": true})
    ));
    assert!(!is_destructive_tool_for_test(
        "git_checkout",
        &json!({"branch": "main"})
    ));
    assert!(is_destructive_tool_for_test(
        "git_stash",
        &json!({"action": "drop"})
    ));
    assert!(is_destructive_tool_for_test(
        "git_stash",
        &json!({"action": "clear"})
    ));
    assert!(!is_destructive_tool_for_test(
        "git_stash",
        &json!({"action": "push"})
    ));
    assert!(is_destructive_tool_for_test(
        "git_tag",
        &json!({"action": "delete"})
    ));
    assert!(!is_destructive_tool_for_test(
        "git_tag",
        &json!({"action": "list"})
    ));
    // Merge / cherry-pick always rewrite history and count as destructive.
    assert!(is_destructive_tool_for_test("git_merge", &json!({})));
    assert!(is_destructive_tool_for_test("git_cherry_pick", &json!({})));
}

#[test]
fn test_bash_destructive_command_detection() {
    assert!(is_destructive_tool_for_test(
        "bash",
        &json!({"command": "rm -rf target"})
    ));
    assert!(is_destructive_tool_for_test(
        "bash",
        &json!({"command": "cargo install ripgrep"})
    ));
    assert!(is_destructive_tool_for_test(
        "bash",
        &json!({"command": "ls && npm install"})
    ));
    assert!(!is_destructive_tool_for_test(
        "bash",
        &json!({"command": "ls -la"})
    ));
    assert!(!is_destructive_tool_for_test(
        "bash",
        &json!({"command": "cargo test"})
    ));
    // A missing command field is not destructive.
    assert!(!is_destructive_tool_for_test("bash", &json!({})));
}

#[test]
fn test_extract_resource_from_input_key_precedence() {
    assert_eq!(
        extract_resource_from_input_for_test(&json!({"path": "a.rs"}), "read"),
        "a.rs"
    );
    assert_eq!(
        extract_resource_from_input_for_test(&json!({"command": "ls"}), "bash"),
        "ls"
    );
    assert_eq!(
        extract_resource_from_input_for_test(&json!({"url": "https://x"}), "webfetch"),
        "https://x"
    );
    assert_eq!(
        extract_resource_from_input_for_test(&json!({"pattern": "fn main"}), "grep"),
        "fn main"
    );
    assert_eq!(
        extract_resource_from_input_for_test(&json!({"query": "rust"}), "websearch"),
        "rust"
    );
    // Fallback to the tool name when none of the listed keys is present.
    assert_eq!(
        extract_resource_from_input_for_test(&json!({}), "think"),
        "tool:think"
    );
    // `path` takes precedence over `command`.
    assert_eq!(
        extract_resource_from_input_for_test(&json!({"path": "p", "command": "c"}), "read"),
        "p"
    );
}

#[test]
fn test_strip_timeout_prefix_branches() {
    assert_eq!(
        strip_timeout_prefix("timeout 600 cargo build"),
        "cargo build"
    );
    assert_eq!(strip_timeout_prefix("timeout 10 ls -la"), "ls -la");
    assert_eq!(strip_timeout_prefix("ls -la"), "ls -la");
    // `timeout` not followed by whitespace is not a wrapper.
    assert_eq!(strip_timeout_prefix("timeoutx ls"), "timeoutx ls");
    // A non-numeric timeout value is not stripped.
    assert_eq!(strip_timeout_prefix("timeout foo bar"), "timeout foo bar");
    // A bare `timeout` with no value is returned unchanged.
    assert_eq!(strip_timeout_prefix("timeout"), "timeout");
    // Surrounding whitespace is trimmed.
    assert_eq!(strip_timeout_prefix("  ls  "), "ls");
}

#[test]
fn test_tool_repeat_limit_constant() {
    assert_eq!(TOOL_REPEAT_LIMIT, 5);
}

#[tokio::test]
async fn test_repeat_guard_denies_after_limit_in_unattended_run() {
    let trackers: Arc<Mutex<HashMap<String, RepeatTracker>>> = Arc::new(Mutex::new(HashMap::new()));
    let bus = Arc::new(EventBus::new(16));
    let input = json!({"path": "src/lib.rs"});

    for _ in 0..TOOL_REPEAT_LIMIT {
        let result =
            check_tool_repeat_guard(&trackers, &bus, "s1", "read", &input, true, false, 0).await;
        assert!(result.is_none());
    }
    let denied =
        check_tool_repeat_guard(&trackers, &bus, "s1", "read", &input, true, false, 0).await;
    let message = denied.expect("the sixth identical call must be denied");
    assert!(message.contains("auto-denied"));
}

#[tokio::test]
async fn test_repeat_guard_resets_on_different_args() {
    let trackers: Arc<Mutex<HashMap<String, RepeatTracker>>> = Arc::new(Mutex::new(HashMap::new()));
    let bus = Arc::new(EventBus::new(16));

    for _ in 0..=TOOL_REPEAT_LIMIT {
        let _ = check_tool_repeat_guard(
            &trackers,
            &bus,
            "s1",
            "read",
            &json!({"path": "a"}),
            true,
            false,
            0,
        )
        .await;
    }
    // A different key resets the consecutive counter, so this is not denied.
    let result = check_tool_repeat_guard(
        &trackers,
        &bus,
        "s1",
        "read",
        &json!({"path": "b"}),
        true,
        false,
        0,
    )
    .await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_repeat_guard_interactive_prompt_times_out_to_denial() {
    let trackers: Arc<Mutex<HashMap<String, RepeatTracker>>> = Arc::new(Mutex::new(HashMap::new()));
    let bus = Arc::new(EventBus::new(16));
    let input = json!({"path": "src/lib.rs"});

    for _ in 0..TOOL_REPEAT_LIMIT {
        let _ =
            check_tool_repeat_guard(&trackers, &bus, "s1", "read", &input, false, false, 0).await;
    }
    // The sixth interactive call publishes a prompt and waits; a zero-second
    // window times out to a denial.
    let denied =
        check_tool_repeat_guard(&trackers, &bus, "s1", "read", &input, false, false, 0).await;
    let message = denied.expect("the interactive prompt must deny on timeout");
    assert!(message.contains("no user reply"));
}

#[tokio::test]
async fn test_permission_hardwired_info_tools_auto_approved() {
    for name in ["model_info", "ragent_info", "tool_info", "commands_info"] {
        let checker = Arc::new(RwLock::new(PermissionChecker::new(Vec::new())));
        let bus = Arc::new(EventBus::new(16));
        let action = check_permission_with_prompt(
            &checker,
            &bus,
            "s",
            "tool:execute",
            name,
            name,
            Some(false),
            None,
            false,
            120,
            false,
        )
        .await
        .expect("permission check should succeed");
        assert_eq!(action, PermissionAction::Allow, "{name}");
    }
}

#[tokio::test]
async fn test_permission_security_approved_satisfies_bare_ask() {
    let checker = Arc::new(RwLock::new(PermissionChecker::new(Vec::new())));
    let bus = Arc::new(EventBus::new(16));

    // No matching rule -> Ask; the analyzer approval is honoured instead of
    // prompting (the caller would otherwise block on a live user).
    let action = check_permission_with_prompt(
        &checker,
        &bus,
        "s",
        "tool:execute",
        "tool:mystery",
        "mystery_tool",
        Some(false),
        None,
        false,
        120,
        true,
    )
    .await
    .expect("permission check should succeed");
    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_permission_auto_approve_short_circuits() {
    let checker = Arc::new(RwLock::new(PermissionChecker::new(Vec::new())));
    let bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &bus,
        "s",
        "tool:execute",
        "tool:bash",
        "bash",
        Some(true),
        None,
        false,
        120,
        false,
    )
    .await
    .expect("permission check should succeed");
    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_permission_checkpoint_overrides_allow_then_denies_on_timeout() {
    let checker = Arc::new(RwLock::new(PermissionChecker::new(Vec::new())));
    checker.write().record_always("tool:execute", "*");
    let bus = Arc::new(EventBus::new(16));

    // An allow rule meets a forced destructive checkpoint: the request is
    // routed to a human prompt with a zero-second window, which times out to
    // a denial (FR-024).
    let action = check_permission_with_prompt(
        &checker,
        &bus,
        "s",
        "tool:execute",
        "rm -rf target",
        "rm",
        Some(false),
        None,
        true,
        0,
        false,
    )
    .await
    .expect("permission check should succeed");
    assert_eq!(action, PermissionAction::Deny);
}
