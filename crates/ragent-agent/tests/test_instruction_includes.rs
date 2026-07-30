//! Tests for the `@<path>` include directive in instruction files
//! (`AGENTS.md`, `CLAUDE.md`, `.ragent.md`, `INSTRUCTIONS.md`).
//!
//! The `@<path>` mechanism is a C/C++ `#include`-style feature for making
//! instruction files modular: a line like `@docs/conventions.md`
//! is replaced in-place by the contents of the referenced file. The `@`
//! must appear in the first column; a leading `@@` escapes to a literal
//! `@` character.
//!
//! These tests exercise the behaviour end-to-end through the public
//! `collect_agents_md_content_with_discovery` loader, which is where
//! `@<path>` expansion is wired in.

use std::fs;
use tempfile::TempDir;

/// Load the discovered instruction file for a working dir and return its
/// expanded content.
fn load(dir: &std::path::Path) -> String {
    let (content, _discovery) = ragent_agent::agent::collect_agents_md_content_with_discovery(dir);
    content
}

/// A simple `@<path>` include is replaced by the included file's content.
#[test]
fn test_simple_include_expansion() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("conventions.md"),
        "Be excellent to each other.\n",
    )
    .unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Rules\n\n@conventions.md\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("Be excellent to each other."));
    assert!(!content.contains("@conventions.md"));
}

/// Includes are transitive: A includes B includes C, and C's content
/// appears in the final expansion.
#[test]
fn test_nested_transitive_include() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("c.md"), "from C\n").unwrap();
    fs::write(dir.path().join("b.md"), "from B\n@c.md\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "from A\n@b.md\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("from A"));
    assert!(content.contains("from B"));
    assert!(content.contains("from C"));
}

/// A direct self-include is detected as a cycle and skipped with a marker.
#[test]
fn test_cycle_direct_self_include() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "start\n@AGENTS.md\nend\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("start"));
    assert!(content.contains("end"));
    assert!(content.contains("include cycle skipped"));
}

/// A mutual include (A↔B) is detected and does not loop forever.
#[test]
fn test_cycle_mutual_include() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "A\n@b.md\n").unwrap();
    fs::write(dir.path().join("b.md"), "B\n@a.md\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "@a.md\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("A"));
    assert!(content.contains("B"));
    // The second include of a.md (from within b.md) is skipped.
    assert!(content.contains("include cycle skipped"));
}

/// A missing included file produces a marker comment, not a panic.
#[test]
fn test_missing_include_emits_marker() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("AGENTS.md"),
        "# Rules\n@does-not-exist.md\n",
    )
    .unwrap();

    let content = load(dir.path());
    assert!(content.contains("include missing: does-not-exist.md"));
    assert!(content.contains("# Rules"));
}

/// An `@<path>` that resolves outside the allowed roots via `..` is
/// rejected with a marker and its content is NOT inlined.
#[test]
fn test_path_escape_rejected() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("secret.md"), "TOP SECRET\n").unwrap();

    // Build a `../<outside_dir_name>/secret.md` path from the working dir.
    let outside_name = outside
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let relative = format!("../{outside_name}/secret.md");

    fs::write(dir.path().join("AGENTS.md"), format!("@{relative}\n")).unwrap();

    let content = load(dir.path());
    assert!(content.contains("include rejected (outside project)"));
    assert!(!content.contains("TOP SECRET"));
}

/// An absolute include path is rejected outright.
#[test]
fn test_absolute_path_rejected() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "@/etc/hostname\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("include rejected (absolute path)"));
}

/// The same file may be included by two *sibling* files without being
/// treated as a cycle (diamond include is fine).
#[test]
fn test_diamond_include_is_not_a_cycle() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("shared.md"), "SHARED\n").unwrap();
    fs::write(dir.path().join("left.md"), "LEFT\n@shared.md\n").unwrap();
    fs::write(dir.path().join("right.md"), "RIGHT\n@shared.md\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "@left.md\n@right.md\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("LEFT"));
    assert!(content.contains("RIGHT"));
    // SHARED appears once per include site (two times total).
    assert_eq!(content.matches("SHARED").count(), 2);
    assert!(!content.contains("cycle skipped"));
}

/// A directive that appears as part of a markdown heading line is left
/// untouched (no false-positive expansion) because `@` is not in column 0.
#[test]
fn test_directive_inside_heading_left_untouched() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("AGENTS.md"),
        "## The @mention mechanism is great\n",
    )
    .unwrap();

    let content = load(dir.path());
    assert!(content.contains("## The @mention mechanism is great"));
}

/// A leading `@@` escape sequence collapses to a single literal `@` and is
/// NOT treated as an include directive.
#[test]
fn test_leading_double_at_escapes_to_literal() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("conventions.md"), "INCLUDED\n").unwrap();
    fs::write(
        dir.path().join("AGENTS.md"),
        // First line escapes to literal `@conventions.md` (not an include);
        // second line is a real include.
        "@@conventions.md\n@conventions.md\n",
    )
    .unwrap();

    let content = load(dir.path());
    // The escaped line appears verbatim as `@conventions.md`.
    assert!(
        content.contains("@@conventions.md") || content.contains("\n@conventions.md\n"),
        "escaped literal line preserved: {content}"
    );
    // The real include pulled in the file content.
    assert!(content.contains("INCLUDED"));
}

/// An `@` directive with leading whitespace is NOT expanded (the `@` must
/// be in the first column).
#[test]
fn test_indented_at_is_not_a_directive() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("conventions.md"), "SHOULD NOT APPEAR\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "  @conventions.md\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("@conventions.md"));
    assert!(!content.contains("SHOULD NOT APPEAR"));
}

/// A double-quoted target allows spaces in the path.
#[test]
fn test_quoted_include_with_spaces() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("with spaces.md"), "QUOTED CONTENT\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "@\"with spaces.md\"\n").unwrap();

    let content = load(dir.path());
    assert!(content.contains("QUOTED CONTENT"));
    assert!(!content.contains("@\"with spaces.md\""));
}

/// An include path resolves relative to the *included* file's directory,
/// not always the project root.
#[test]
fn test_relative_path_resolves_from_included_file_dir() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("docs").join("sub")).unwrap();
    fs::write(
        dir.path().join("docs").join("sub").join("leaf.md"),
        "LEAF\n",
    )
    .unwrap();
    fs::write(dir.path().join("docs").join("index.md"), "@sub/leaf.md\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "@docs/index.md\n").unwrap();

    let content = load(dir.path());
    assert!(
        content.contains("LEAF"),
        "relative include from sub-dir: {content}"
    );
}
