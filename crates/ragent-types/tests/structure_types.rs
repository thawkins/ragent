//! Structural-defect guard for the foundation types consolidated in REMPLAN.md M1.
//!
//! These tests scan the workspace `src/` trees to ensure the canonical types
//! that were consolidated in Milestone 1 (`Message`, `ImageData`,
//! `PermissionRequest`, `Permission`, `PermissionAction`, `PermissionRule`,
//! `PermissionChecker`, `ToolDefinition`, `ChatRequest`, `ChatMessage`,
//! `StreamEvent`, `ChatContent`, `ContentPart`, `PermissionDecision`) are
//! defined in exactly one crate and only re-exported everywhere else.
//!
//! They exist so that the duplication documented in REMPLAN.md §1 (defects
//! D2, D3, D8) cannot silently regress.
//!
//! The tests walk the source tree at compile-time-known locations relative
//! to `CARGO_MANIFEST_DIR` (the `ragent-types` crate), so they run without
//! any network access or build artefacts.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// Canonical homes for the consolidated types.  Each entry maps a type name
/// to the *single* file (relative to the workspace root) that is allowed to
/// define it with `pub struct <Name>` / `pub enum <Name>` / `pub type <Name>`.
const CANONICAL_HOMES: &[(&str, &str, &str)] = &[
    // Message family (D2) — canonical in ragent-types/src/message/mod.rs
    (
        "Message",
        "crates/ragent-types/src/message/mod.rs",
        "struct",
    ),
    (
        "MessagePart",
        "crates/ragent-types/src/message/mod.rs",
        "enum",
    ),
    (
        "ImageData",
        "crates/ragent-types/src/message/mod.rs",
        "struct",
    ),
    (
        "ToolCallState",
        "crates/ragent-types/src/message/mod.rs",
        "struct",
    ),
    (
        "ToolCallStatus",
        "crates/ragent-types/src/message/mod.rs",
        "enum",
    ),
    ("Role", "crates/ragent-types/src/message/mod.rs", "enum"),
    // Permission family (D3) — Permission* (action/rule/checker/request) in
    // ragent-config; PermissionDecision in ragent-types (used by events).
    (
        "Permission",
        "crates/ragent-config/src/permission.rs",
        "enum",
    ),
    (
        "PermissionAction",
        "crates/ragent-config/src/permission.rs",
        "enum",
    ),
    (
        "PermissionRule",
        "crates/ragent-config/src/permission.rs",
        "struct",
    ),
    (
        "PermissionRequest",
        "crates/ragent-config/src/permission.rs",
        "struct",
    ),
    (
        "PermissionChecker",
        "crates/ragent-config/src/permission.rs",
        "struct",
    ),
    (
        "PermissionRuleset",
        "crates/ragent-config/src/permission.rs",
        "type",
    ),
    (
        "PermissionDecision",
        "crates/ragent-types/src/permission.rs",
        "enum",
    ),
    // LLM primitive family (D8) — canonical in ragent-types/src/llm.rs
    ("ToolDefinition", "crates/ragent-types/src/llm.rs", "struct"),
    ("ChatRequest", "crates/ragent-types/src/llm.rs", "struct"),
    ("ChatMessage", "crates/ragent-types/src/llm.rs", "struct"),
    ("ChatContent", "crates/ragent-types/src/llm.rs", "enum"),
    ("ContentPart", "crates/ragent-types/src/llm.rs", "enum"),
    ("StreamEvent", "crates/ragent-types/src/llm.rs", "enum"),
    // Storage family (D1) — canonical in ragent-storage/src/storage.rs.
    // The agent crate's `storage/mod.rs` is now a re-export shim.
    // NOTE: `ragent-tools-extended::storage` defines its own `TodoRow` /
    // `MemoryRow` / `EmbeddingMatch` as part of the `StorageBackend` trait
    // surface (they are DTOs for the trait, not storage-row types).  Those
    // are intentional parallel definitions used by the extracted tools and
    // are NOT covered by this guard; only the storage-row types in
    // `ragent-storage` are.
    ("Storage", "crates/ragent-storage/src/storage.rs", "struct"),
    (
        "SessionRow",
        "crates/ragent-storage/src/storage.rs",
        "struct",
    ),
    (
        "KgEntityRow",
        "crates/ragent-storage/src/storage.rs",
        "struct",
    ),
    (
        "KgRelationshipRow",
        "crates/ragent-storage/src/storage.rs",
        "struct",
    ),
];

/// Return the workspace root (the directory containing `Cargo.toml` with the
/// `[workspace]` table).  We walk up from `CARGO_MANIFEST_DIR`
/// (which points at `crates/ragent-types`) until we find a `Cargo.toml`
/// that declares a `[workspace]` section.
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

/// Collect every `*.rs` file under `crates/*/src/` (excluding `target/`).
fn collect_src_files(workspace_root: &Path) -> Vec<PathBuf> {
    let crates_dir = workspace_root.join("crates");
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&crates_dir) {
        for entry in entries.flatten() {
            let crate_dir = entry.path();
            let src_dir = crate_dir.join("src");
            if src_dir.is_dir() {
                walk_rs(&src_dir, &mut out);
            }
        }
    }
    out
}

fn walk_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_rs(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
}

/// Check whether a source file defines `pub struct <Name>`, `pub enum <Name>`,
/// or `pub type <Name>` (as a fresh definition, not a re-export).  We look for
/// a line starting with the relevant keyword at item level.  Doc-comment and
/// code-comment lines are skipped.
fn defines_type(file_path: &Path, type_name: &str, kind: &str) -> bool {
    let Ok(contents) = fs::read_to_string(file_path) else {
        return false;
    };
    let keyword = match kind {
        "struct" => "pub struct",
        "enum" => "pub enum",
        "type" => "pub type",
        _ => return false,
    };
    let prefix = format!("{keyword} {type_name}");
    for raw in contents.lines() {
        let line = raw.trim_start();
        if line.starts_with("//") {
            continue;
        }
        if let Some(rest) = line.strip_prefix(&prefix) {
            let next = rest.chars().next();
            // The token must be followed by `{`, `(`, `;`, whitespace, or EOL
            // so we don't match `pub struct FooBar` when searching for `Foo`.
            if next.is_none() || matches!(next, Some('{' | '(' | ';' | ' ' | '\t' | '<' | '=')) {
                return true;
            }
        }
    }
    false
}

#[test]
fn each_consolidated_type_has_exactly_one_definition() {
    let workspace_root = workspace_root();
    let src_files = collect_src_files(&workspace_root);

    let mut failures: Vec<String> = Vec::new();

    for (type_name, canonical_rel, kind) in CANONICAL_HOMES {
        let canonical_abs = workspace_root.join(canonical_rel);
        let canonical_display = canonical_rel.to_string();

        let mut definers: Vec<String> = Vec::new();

        for file in &src_files {
            if !defines_type(file, type_name, kind) {
                continue;
            }
            let rel = file.strip_prefix(&workspace_root).map_or_else(
                |_| file.display().to_string(),
                |p| p.to_string_lossy().to_string(),
            );

            if rel == canonical_display {
                // The one allowed definition site.
                continue;
            }

            definers.push(rel);
        }

        if !definers.is_empty() {
            failures.push(format!(
                "type `{type_name}` is defined outside its canonical home `{canonical_display}`:\n  - {}",
                definers.join("\n  - ")
            ));
        }

        // Also confirm the canonical home actually still defines it (guards
        // against the type being accidentally deleted entirely).
        if !canonical_abs.exists() {
            failures.push(format!(
                "type `{type_name}` canonical home `{canonical_display}` does not exist"
            ));
        } else if !defines_type(&canonical_abs, type_name, kind) {
            failures.push(format!(
                "type `{type_name}` is no longer defined in its canonical home `{canonical_display}`"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "structural-defect guard failed (see REMPLAN.md M1):\n{}\n",
        failures.join("\n\n")
    );
}

#[test]
fn no_ragent_types_llm_legacy_types_remain() {
    // The unused legacy types (LlmProvider, LlmResponse, Usage, ModelInfo,
    // ProviderConfig) were deleted from ragent-types/src/llm.rs in T1.3.
    // Guard against them being re-introduced.
    let workspace_root = workspace_root();
    let llm_file = workspace_root.join("crates/ragent-types/src/llm.rs");
    let contents = fs::read_to_string(&llm_file)
        .expect("ragent-types/src/llm.rs must exist for this guard to run");
    for legacy in [
        "pub trait LlmProvider",
        "pub struct LlmResponse",
        "pub struct Usage",
        "pub struct ModelInfo",
        "pub struct ProviderConfig",
    ] {
        assert!(
            !contents.contains(legacy),
            "legacy type `{legacy}` should not be re-introduced in ragent-types/src/llm.rs (see REMPLAN.md M1 / T1.3)"
        );
    }
}

#[test]
fn no_path_attributes_to_ragent_team_in_agent() {
    // REMPLAN.md M3: the 27 `#[path = "../../../ragent-team/..."]` attributes
    // that previously compiled ragent-team sources into ragent-agent have been
    // removed. The team sources are now native to ragent-agent. This test
    // guards against any `#[path` attribute that references `ragent-team`
    // being re-introduced in `crates/ragent-agent/src/`.
    let workspace_root = workspace_root();
    let agent_src = workspace_root.join("crates/ragent-agent/src");
    let files = collect_src_files(&workspace_root);

    let mut failures: Vec<String> = Vec::new();
    for file in &files {
        // Only check files under crates/ragent-agent/src/
        if !file.starts_with(&agent_src) {
            continue;
        }
        let Ok(contents) = fs::read_to_string(file) else {
            continue;
        };
        for (lineno, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[path") && trimmed.contains("ragent-team") {
                let rel = file.strip_prefix(&workspace_root).map_or_else(
                    |_| file.display().to_string(),
                    |p| p.to_string_lossy().to_string(),
                );
                failures.push(format!(
                    "{rel}:{} : found `#[path]` attribute referencing ragent-team: {trimmed}",
                    lineno + 1
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "REMPLAN.md M3 guard: no `#[path]` attributes referencing ragent-team \
         should appear in crates/ragent-agent/src/:\n  - {}",
        failures.join("\n  - ")
    );
}
#[test]
fn no_ragent_core_alias_in_source_files() {
    // REMPLAN.md M4: the `ragent_core` alias (package = "ragent-agent") has
    // been retired. No `.rs` file under `crates/` or `src/` should contain
    // the identifier `ragent_core` (either as `ragent_core::` or
    // `use ragent_agent as ragent_core`).
    //
    // Historical references in `CHANGELOG.md`, `docs/`, `README.md`, and
    // `SPEC.md` are acceptable and not checked here.
    let workspace_root = workspace_root();
    let files = collect_src_files(&workspace_root);

    let mut failures: Vec<String> = Vec::new();
    for file in &files {
        let Ok(contents) = fs::read_to_string(file) else {
            continue;
        };
        for (lineno, line) in contents.lines().enumerate() {
            if line.contains("ragent_core") {
                let rel = file.strip_prefix(&workspace_root).map_or_else(
                    |_| file.display().to_string(),
                    |p| p.to_string_lossy().to_string(),
                );
                failures.push(format!(
                    "{rel}:{} : found `ragent_core` reference: {}",
                    lineno + 1,
                    line.trim()
                ));
            }
        }
    }

    // Also check src/main.rs (it's not under crates/*/src/)
    let main_rs = workspace_root.join("src/main.rs");
    if let Ok(contents) = fs::read_to_string(&main_rs) {
        for (lineno, line) in contents.lines().enumerate() {
            if line.contains("ragent_core") {
                failures.push(format!(
                    "src/main.rs:{} : found `ragent_core` reference: {}",
                    lineno + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "REMPLAN.md M4 guard: no `ragent_core` references should appear in \
         crate source files or src/main.rs:\n  - {}",
        failures.join("\n  - ")
    );
}
