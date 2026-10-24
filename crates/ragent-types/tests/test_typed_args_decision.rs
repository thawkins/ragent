//! Structural guard for the T-025 typed-argument decision (spec
//! `uniformtools` FR-001).
//!
//! FR-001 makes `MessagePart::ToolCall { tool, call_id, state }` the one
//! canonical tool-call record, carrying decoded JSON `input`. Three
//! `ragent-types` fields hold that JSON as `serde_json::Value`:
//!
//! * `ToolCallState::input` / `ToolCallState::output`
//!   (`src/message/mod.rs`),
//! * `ContentPart::ToolUse::input` (`src/llm.rs`),
//! * `ToolDefinition::parameters` (`src/llm.rs`).
//!
//! T-025 decided to keep `Value` at all three sites and to record why in
//! `specs/uniformtools/T-025-TYPED-ARGS-DECISION.md`. These tests pin that
//! decision so it cannot drift back into an undocumented `TODO` or into a
//! lossy typed field:
//!
//! 1. each field is still declared with the `Value` type it was decided to keep;
//! 2. no `TODO: Replace \`Value\` with a typed ...` marker remains in `src/`;
//! 3. each site carries the doc comment that states the decision and points at
//!    the record;
//! 4. the record itself still exists and names all three sites.
//!
//! The scan is hermetic: it reads the crate's own `src/` tree from
//! `CARGO_MANIFEST_DIR` and the record from the workspace root, with no network
//! access and no build artefact.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// The marker T-025 had to resolve, in the exact spelling the three sites used.
const REMOVED_MARKER: &str = "TODO: Replace `Value` with a typed";

/// Path of the decision record, relative to the workspace root.
const DECISION_RECORD: &str = "specs/uniformtools/T-025-TYPED-ARGS-DECISION.md";

/// Read a file under the crate's `src/` tree.
fn read_src(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()))
}

/// Return the workspace root (the directory whose `Cargo.toml` declares
/// `[workspace]`), walking up from `CARGO_MANIFEST_DIR`.
fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut dir: &Path = &manifest_dir;
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists()
            && let Ok(contents) = fs::read_to_string(&candidate)
            && contents.contains("[workspace]")
        {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => panic!(
                "could not locate workspace root above {}",
                manifest_dir.display()
            ),
        }
    }
}

/// Collect every `*.rs` file under the crate's `src/` tree.
fn collect_src_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&src, &mut out);
    out
}

/// Assert that `haystack` contains `needle`, with a message naming the site.
fn assert_contains(haystack: &str, needle: &str, site: &str) {
    assert!(
        haystack.contains(needle),
        "{site} no longer contains {needle:?}; the T-025 typed-argument decision \
         (specs/uniformtools/T-025-TYPED-ARGS-DECISION.md) must be revisited if \
         that field's type or documentation changes"
    );
}

// ---------------------------------------------------------------------------
// 1. The decided field types cannot be narrowed silently.
// ---------------------------------------------------------------------------

#[test]
fn test_tool_call_state_keeps_json_input_and_output() {
    let source = read_src("message/mod.rs");
    assert_contains(&source, "pub input: Value,", "ToolCallState::input");
    assert_contains(
        &source,
        "pub output: Option<Value>,",
        "ToolCallState::output",
    );
}

#[test]
fn test_content_part_tool_use_keeps_json_input() {
    let source = read_src("llm.rs");
    // The `input: Value` field inside `ContentPart::ToolUse`.
    assert_contains(&source, "input: Value,", "ContentPart::ToolUse::input");
}

#[test]
fn test_tool_definition_keeps_json_schema_parameters() {
    let source = read_src("llm.rs");
    assert_contains(
        &source,
        "pub parameters: Value,",
        "ToolDefinition::parameters",
    );
}

// ---------------------------------------------------------------------------
// 2. No `TODO: Replace Value with a typed ...` marker survives in `src/`.
// ---------------------------------------------------------------------------

#[test]
fn test_no_typed_value_todo_marker_remains_in_src() {
    let mut offenders = Vec::new();
    for path in collect_src_files() {
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        if contents.contains(REMOVED_MARKER) {
            offenders.push(path.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "T-025 resolved the typed-`Value` markers in ragent-types; found the \
         removed marker {REMOVED_MARKER:?} again in: {offenders:?}. Either keep \
         `Value` and document why (see \
         specs/uniformtools/T-025-TYPED-ARGS-DECISION.md) or land the typed \
         replacement with its own rationale"
    );
}

// ---------------------------------------------------------------------------
// 3. Each site documents the decision and points at the record.
// ---------------------------------------------------------------------------

/// Every decided site must mention the record, so a reader who lands on the
/// field finds the rationale without a search.
const DOCUMENTED_SITES: &[(&str, &str)] = &[
    ("message/mod.rs", "pub input: Value,"),
    ("message/mod.rs", "pub output: Option<Value>,"),
    ("llm.rs", "input: Value,"),
    ("llm.rs", "pub parameters: Value,"),
];

#[test]
fn test_every_decided_field_documents_the_decision_record() {
    let marker = "T-025-TYPED-ARGS-DECISION.md";
    for (file, field) in DOCUMENTED_SITES {
        let source = read_src(file);
        let lines: Vec<&str> = source.lines().collect();
        let field_line = lines
            .iter()
            .position(|line| line.trim_start().starts_with(field))
            .unwrap_or_else(|| panic!("{file} no longer declares {field:?}"));

        // The doc comment sits immediately above the field: walk back over the
        // contiguous run of `///` lines that ends at the declaration.
        let doc_block: String = lines[..field_line]
            .iter()
            .rev()
            .take_while(|line| line.trim_start().starts_with("///"))
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            doc_block.contains(marker),
            "{file}: {field} must document the T-025 decision and name {marker}; \
             its doc comment was:\n{doc_block}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. The decision record exists and names all three sites.
// ---------------------------------------------------------------------------

#[test]
fn test_decision_record_exists_and_names_each_site() {
    let path = workspace_root().join(DECISION_RECORD);
    let record = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("decision record {} is missing: {e}", path.display()));

    for expected in [
        "crates/ragent-types/src/message/mod.rs",
        "crates/ragent-types/src/llm.rs",
        "ToolCallState::input",
        "ContentPart::ToolUse::input",
        "ToolDefinition::parameters",
        "FR-001",
        "NFR-002",
    ] {
        assert!(
            record.contains(expected),
            "the T-025 decision record no longer mentions {expected:?}, so it no \
             longer documents the site it is referenced from"
        );
    }
}
