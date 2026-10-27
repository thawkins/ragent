//! Routing guard for FR-007's single argument-decode path (spec
//! `uniformtools`, T-012).
//!
//! FR-007 says the accumulated argument text of a completed tool call is decoded
//! by one shared helper - a JSON object, a JSON string that decodes to one, or
//! empty/whitespace as `{}` - and that the provider layer never decodes it at
//! all. The helper's own contract is pinned in
//! `crates/ragent-types/tests/test_tool_args.rs`; the loop's behaviour through it
//! is pinned in `test_processor_more_paths.rs`
//! (`test_whitespace_arguments_run_the_tool_with_an_empty_object` and
//! `test_encoded_string_arguments_run_the_tool_decoded`) and the TUI's in
//! `crates/ragent-tui/tests/test_tool_event_state.rs`.
//!
//! This file pins what a *caller* can get wrong, by scanning sources:
//!
//! * a harness site that re-implements the decode with a bare
//!   `serde_json::from_str` on argument text, which would reintroduce a second
//!   rule - it rejects whitespace, and it hands a double-encoded payload to the
//!   tool as a JSON *string*; and
//! * a provider module that decodes the fragments it must forward verbatim.

use std::fs;
use std::path::{Path, PathBuf};

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

/// Lines of `source` that parse a value out of tool-call argument text without
/// going through the shared helper.
///
/// Comment lines are not call sites. The three spellings recognised are the ones
/// the workspace uses for a call's argument text: the pending call's `args_json`,
/// a hook's `tool_input`, and the TUI's `args` parameter.
fn bare_argument_decodes(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("from_str"))
        .filter(|line| {
            line.contains("args_json")
                || line.contains("tool_input")
                || line.contains("(args)")
                || line.contains("(&args)")
        })
        .map(|line| line.trim().to_string())
        .collect()
}

/// FR-007: no harness site re-implements the decode.
///
/// The agent (loop and hooks), the TUI (which renders the same arguments) and
/// the binary's own CLI surfaces all handle a call's argument text, so all three
/// must reach it through the shared helper.
#[test]
fn no_harness_source_parses_argument_text_around_the_shared_helper() {
    for tree in ["crates/ragent-agent/src", "crates/ragent-tui/src", "src"] {
        let mut files = Vec::new();
        collect_rs(&workspace_root().join(tree), &mut files);
        assert!(!files.is_empty(), "expected sources under {tree}");
        for file in files {
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
            let bare = bare_argument_decodes(&source);
            assert!(
                bare.is_empty(),
                "{} decodes a tool call's argument text with `serde_json::from_str` \
                 instead of `ragent_types::tool_args::decode_tool_arguments` \
                 (spec `uniformtools` FR-007); offending lines: {bare:#?}",
                file.display()
            );
        }
    }
}

/// FR-007: every harness file that decodes argument text names the helper.
///
/// This is the positive half of the scan above: a file may be free of bare
/// parses simply by never decoding, so each known decode site is also asserted
/// to be routed.
#[test]
fn every_harness_decode_site_names_the_shared_helper() {
    for relative in [
        "crates/ragent-agent/src/session/processor.rs",
        "crates/ragent-agent/src/hooks/mod.rs",
        "crates/ragent-tui/src/app/event_handler.rs",
    ] {
        let source = read_workspace_file(relative);
        assert!(
            source.contains("ragent_types::tool_args::decode_tool_arguments"),
            "{relative} must decode argument text through \
             ragent_types::tool_args::decode_tool_arguments (FR-007)"
        );
    }
}

/// FR-007: the helper accepts exactly the three shapes and exposes one decoder.
///
/// A second public decode entry point, or a missing shape, would let one caller
/// read a payload differently from another - the failure FR-007 exists to stop.
#[test]
fn the_helper_is_the_only_decoder_and_it_handles_all_three_shapes() {
    let helper = read_workspace_file("crates/ragent-types/src/tool_args.rs");
    for shape in [
        "trimmed.is_empty()",
        "Value::String(inner)",
        "Value::Object(_)",
    ] {
        assert!(
            helper.contains(shape),
            "the shared decoder no longer handles `{shape}` (FR-007): a JSON \
             object, a JSON string that decodes to one, and empty/whitespace are \
             the three accepted shapes"
        );
    }
    let public_decodes = helper.matches("pub fn decode").count();
    assert_eq!(
        public_decodes, 1,
        "the helper module must expose exactly one decoder (FR-007); found \
         {public_decodes}"
    );
}
/// FR-007: the provider layer decodes nothing.
///
/// Providers emit `ToolCallDelta` fragments verbatim and leave the decode to the
/// harness, so one rule cannot diverge per provider.
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
        let source = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
        assert!(
            !source.contains("decode_tool_arguments"),
            "{} decodes tool-call arguments; the provider layer must forward \
             `ToolCallDelta` fragments verbatim (spec `uniformtools` FR-007)",
            file.display()
        );
    }
}
