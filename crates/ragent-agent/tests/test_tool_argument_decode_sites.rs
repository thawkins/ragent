//! Guard and contract tests for FR-007's single argument-decode path (spec
//! `uniformtools`).
//!
//! FR-007 says accumulated tool-call argument text is decoded **once** per call
//! per step, by one shared helper, and that the provider layer never decodes
//! arguments at all. The helper's own behaviour is pinned in
//! `crates/ragent-types/tests/test_tool_args.rs`; this file pins the two things
//! a caller can get wrong:
//!
//! * a decode site that bypasses the helper (a fresh `serde_json::from_str` on
//!   argument text would reintroduce a second rule), and
//! * a provider module that decodes the fragments it is supposed to forward
//!   verbatim.
//!
//! It also pins the contract of the `*_decoded` hook entry points, which exist so
//! the dispatch pipeline's one decode is reused by the guard matcher and the
//! Claude event payload instead of being repeated per hook.

use std::fs;
use std::path::{Path, PathBuf};

use ragent_agent::hooks::{
    HookConfig, HookTrigger, PostToolUseResult, PreToolUseResult, run_post_tool_use_hooks,
    run_post_tool_use_hooks_decoded, run_pre_tool_use_hooks, run_pre_tool_use_hooks_decoded,
};
use serde_json::{Value, json};
use tempfile::TempDir;

/// The workspace root, resolved from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

/// Every `*.rs` file under `dir`, recursively.
fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Read a workspace-relative source file.
fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// FR-007: the provider layer decodes nothing.
// ---------------------------------------------------------------------------

#[test]
fn provider_layer_never_decodes_tool_arguments() {
    let providers = workspace_root().join("crates/ragent-llm/src/providers");
    let mut files = Vec::new();
    collect_rs(&providers, &mut files);
    assert!(
        !files.is_empty(),
        "expected provider sources under {}",
        providers.display()
    );
    for file in files {
        let source = fs::read_to_string(&file).unwrap_or_else(|e| {
            panic!("cannot read {}: {e}", file.display());
        });
        assert!(
            !source.contains("decode_tool_arguments"),
            "{} decodes tool-call arguments; the provider layer must forward \
             `ToolCallDelta` fragments verbatim and leave the single decode to \
             the harness (spec `uniformtools` FR-007)",
            file.display()
        );
    }
}

#[test]
fn provider_argument_helper_serialises_the_object_form_without_parsing() {
    // `tool_arguments_json` is the one place a provider turns the OpenAI
    // `function.arguments` field into the harness's `args_json` string. It must
    // *encode* the object form rather than parse either form.
    let source = read_workspace_file("crates/ragent-llm/src/providers/tool_cache.rs");
    let start = source
        .find("pub(crate) fn tool_arguments_json")
        .expect("tool_arguments_json still exists");
    let body = &source[start..];
    let body = &body[..body.find("\n}").expect("function body ends")];
    assert!(
        !body.contains("from_str"),
        "tool_arguments_json must not parse the argument text (FR-007):\n{body}"
    );
    assert!(
        body.contains("serde_json::to_string"),
        "tool_arguments_json still serialises the whole-object form:\n{body}"
    );
}

// ---------------------------------------------------------------------------
// FR-007: the dispatch pipeline decodes exactly once.
// ---------------------------------------------------------------------------

#[test]
fn dispatch_pipeline_holds_the_single_shared_decode_of_arguments() {
    // T-014 moved the *dispatch* decode into the one ordered pipeline, so the
    // pipeline decodes a call's arguments exactly once.
    let pipeline = read_workspace_file("crates/ragent-agent/src/session/dispatch.rs");
    // Count call sites only: the module documentation names the helper too.
    let pipeline_sites = pipeline
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("decode_tool_arguments("))
        .count();
    assert_eq!(
        pipeline_sites, 1,
        "the dispatch pipeline must decode a call's arguments exactly once \
         (FR-007); found {pipeline_sites} call sites in session/dispatch.rs"
    );

    // The agent loop may inspect a pending call's arguments for decisions it
    // makes *before* dispatch - `writes_in_spec_dir` gates the spec
    // task-completion heuristic on whether a write tool actually touched the
    // spec's own directory - but it must do so through the one shared helper,
    // never with a second JSON rule of its own.
    let loop_source = read_workspace_file("crates/ragent-agent/src/session/processor.rs");
    let loop_sites = loop_source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("decode_tool_arguments"))
        .count();
    let shared_sites = loop_source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("ragent_types::tool_args::decode_tool_arguments"))
        .count();
    assert_eq!(
        loop_sites, shared_sites,
        "every argument decode in the agent loop must go through \
         ragent_types::tool_args::decode_tool_arguments (FR-007, FR-012); \
         found {loop_sites} decode sites but only {shared_sites} through the \
         shared helper in processor.rs"
    );
}

#[test]
fn tui_renders_arguments_through_the_shared_decoder() {
    // The TUI must not carry a second JSON rule of its own: every site that
    // turns a call's raw argument text into a value goes through the helper.
    let source = read_workspace_file("crates/ragent-tui/src/app/event_handler.rs");
    assert!(
        source.contains("ragent_types::tool_args::decode_tool_arguments"),
        "the TUI's tool-call renderer no longer uses the shared decoder (FR-007)"
    );
    assert!(
        !source.contains("from_str::<serde_json::Value>(args_json)")
            && !source.contains("from_str::<serde_json::Value>(&args_json)")
            && !source.contains("from_str::<serde_json::Value>(args)"),
        "a TUI tool-call site decodes argument text without the shared helper \
         (FR-007)"
    );
}

// ---------------------------------------------------------------------------
// The `*_decoded` hook entry points reuse the caller's decode.
// ---------------------------------------------------------------------------

fn guard_hook(trigger: HookTrigger, matcher: &str) -> HookConfig {
    HookConfig {
        trigger,
        command: "true".to_string(),
        timeout_secs: 30,
        plugin_root: None,
        matcher: Some(matcher.to_string()),
    }
}

#[test]
fn pre_tool_use_decoded_matches_the_raw_entry_point() {
    let hook = guard_hook(HookTrigger::PreToolUse, "Bash(git commit:*)");
    let hooks = vec![hook];
    // The double-encoded payload decodes to the object the guard is written
    // against, so both entry points agree.
    let encoded = serde_json::to_string(r#"{"command":"git commit -m x"}"#)
        .expect("encode the argument object");
    let decoded = ragent_types::tool_args::decode_tool_arguments(&encoded)
        .expect("the encoded payload decodes");

    let from_raw = run_pre_tool_use_hooks(
        &hooks,
        Path::new("."),
        "Bash",
        &encoded,
        "sess-decode-pre",
        None,
    );
    let from_decoded = run_pre_tool_use_hooks_decoded(
        &hooks,
        Path::new("."),
        "Bash",
        &encoded,
        Some(&decoded),
        "sess-decode-pre",
        None,
    );
    assert!(
        matches!(from_raw, PreToolUseResult::NoDecision),
        "the raw entry point defers to the command, not a hook decision: {from_raw:?}"
    );
    assert!(
        matches!(from_decoded, PreToolUseResult::NoDecision),
        "the decoded entry point behaves identically: {from_decoded:?}"
    );

    // A payload that will not decode is matched by raw text only: the guard sees
    // "git commit" in neither the text nor a decoded object, so it does not run.
    let garbage = "{not valid json";
    assert!(matches!(
        run_pre_tool_use_hooks_decoded(
            &hooks,
            Path::new("."),
            "Bash",
            garbage,
            None,
            "sess-decode-pre-garbage",
            None
        ),
        PreToolUseResult::NoDecision
    ));
}

#[tokio::test]
async fn post_tool_use_decoded_matches_the_raw_entry_point() {
    let hooks = vec![guard_hook(HookTrigger::PostToolUse, "Write")];
    let encoded = serde_json::to_string(r#"{"file_path":"a.rs"}"#).expect("encode");
    let decoded = ragent_types::tool_args::decode_tool_arguments(&encoded).expect("decodes");

    let from_raw = run_post_tool_use_hooks(
        &hooks,
        Path::new("."),
        "Write",
        &encoded,
        r#"{"content":"ok"}"#,
        true,
        "sess-decode-post",
        None,
    )
    .await;
    let from_decoded = run_post_tool_use_hooks_decoded(
        &hooks,
        Path::new("."),
        "Write",
        &encoded,
        Some(&decoded),
        r#"{"content":"ok"}"#,
        true,
        "sess-decode-post",
        None,
    )
    .await;

    for (label, result) in [("raw", from_raw), ("decoded", from_decoded)] {
        match result {
            PostToolUseResult::Ok {
                modified_output,
                additional_context,
            } => {
                assert!(
                    modified_output.is_none(),
                    "{label}: the guard matched, and `true` returned nothing to modify"
                );
                assert!(additional_context.is_empty(), "{label}: no guidance");
            }
            other => panic!("{label}: expected Ok, got {other:?}"),
        }
    }
}

/// The Claude event payload carries the caller's decoded object rather than a
/// re-parse, so a hook reads `tool_input` even when the wire text was the
/// double-encoded string form.
#[tokio::test]
async fn decoded_hook_payload_carries_the_callers_argument_object() {
    let dir = TempDir::new().expect("tempdir");
    let capture = dir.path().join("stdin.json");
    let script = dir.path().join("capture.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ncat > '{}'\nexit 0\n", capture.display()),
    )
    .expect("write script");
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {}", script.display()),
        timeout_secs: 30,
        plugin_root: Some(PathBuf::from("/plugins/guard")),
        matcher: None,
    }];

    let encoded = serde_json::to_string(r#"{"path":"a.rs"}"#).expect("encode");
    let decoded: Value = ragent_types::tool_args::decode_tool_arguments(&encoded).expect("decodes");
    let result = run_pre_tool_use_hooks_decoded(
        &hooks,
        dir.path(),
        "read",
        &encoded,
        Some(&decoded),
        "sess-decode-payload",
        None,
    );
    assert!(matches!(result, PreToolUseResult::NoDecision));

    let raw = fs::read_to_string(&capture).expect("hook wrote its stdin");
    let payload: Value = serde_json::from_str(&raw).expect("payload is JSON");
    assert_eq!(
        payload["tool_input"],
        json!({"path": "a.rs"}),
        "the payload carries the decoded argument object: {payload}"
    );

    // With no decode available the field is `null`, never a guessed object.
    let _ = fs::remove_file(&capture);
    let result = run_pre_tool_use_hooks_decoded(
        &hooks,
        dir.path(),
        "read",
        "{not valid json",
        None,
        "sess-decode-payload-null",
        None,
    );
    assert!(matches!(result, PreToolUseResult::NoDecision));
    let raw = fs::read_to_string(&capture).expect("hook wrote its stdin again");
    let payload: Value = serde_json::from_str(&raw).expect("payload is JSON");
    assert!(
        payload["tool_input"].is_null(),
        "an undecodable payload must not become a guessed object: {payload}"
    );
}
