//! Inline tests for `plan_dep.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn parses_single_top_level_declaration() {
    let plan = "# Plan\n\nSome intro text.\n\nresearch: rust-async\n\n## Tasks\n";
    let deps = parse_research_dependencies(plan).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "rust-async");
    assert_eq!(deps[0].line, 5);
}

#[test]
fn parses_multiple_declarations_in_order() {
    let plan = "research: alpha\n\nresearch: beta\n\nresearch: gamma\n";
    let deps = parse_research_dependencies(plan).unwrap();
    assert_eq!(
        deps.iter()
            .map(super::ResearchDependency::as_str)
            .collect::<Vec<_>>(),
        vec!["alpha", "beta", "gamma"]
    );
}

#[test]
fn tolerates_whitespace_around_name() {
    let plan = "research:    foo-bar   \n";
    let deps = parse_research_dependencies(plan).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "foo-bar");
}

#[test]
fn tolerates_backticked_names() {
    let plan = "research: `rust-async`\n";
    let deps = parse_research_dependencies(plan).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "rust-async");
}

#[test]
fn deduplicates_repeated_declarations() {
    let plan = "research: alpha\nresearch: beta\nresearch: alpha\n";
    let deps = parse_research_dependencies(plan).unwrap();
    let names: Vec<&str> = deps.iter().map(super::ResearchDependency::as_str).collect();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[test]
fn ignores_lines_inside_fenced_code_blocks() {
    let plan = "\
research: real-dep

```markdown
research: not-a-real-dep
```
";
    let deps = parse_research_dependencies(plan).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "real-dep");
}

#[test]
fn strips_inline_comments() {
    let plan = "research: alpha # this is the primary dep\n";
    let deps = parse_research_dependencies(plan).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "alpha");
}

#[test]
fn is_case_sensitive_and_only_matches_lowercase_prefix() {
    // The parser is intentionally case-sensitive: `Research:` (capital R)
    // is not a dependency declaration. This avoids false positives in
    // headings like `## Research:` or prose sentences that start with
    // the word "Research:".
    let plan = "## Research: some heading\nresearch: alpha\nResearch: beta\n";
    let deps = parse_research_dependencies(plan).unwrap();
    // Only the lowercase-prefix line is captured.
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "alpha");
}

#[test]
fn rejects_invalid_name_with_specific_line_and_error() {
    let plan = "# Plan\n\nresearch: 1bad\n";
    let err = parse_research_dependencies(plan).unwrap_err();
    match err {
        ResearchDependencyError::InvalidName {
            line,
            raw_name,
            source,
        } => {
            assert_eq!(line, 3);
            assert_eq!(raw_name, "1bad");
            assert!(matches!(
                source,
                ResearchNameError::InvalidStart { ch: '1' }
            ));
        }
        other => panic!("expected InvalidName, got {other:?}"),
    }
}

#[test]
fn rejects_path_traversal_in_name() {
    let plan = "research: ../etc\n";
    let err = parse_research_dependencies(plan).unwrap_err();
    assert!(matches!(err, ResearchDependencyError::InvalidName { .. }));
    if let ResearchDependencyError::InvalidName { source, .. } = err {
        assert!(matches!(source, ResearchNameError::PathTraversal { .. }));
    }
}

#[test]
fn rejects_empty_name_with_specific_error() {
    let plan = "research:\n";
    let err = parse_research_dependencies(plan).unwrap_err();
    match err {
        ResearchDependencyError::EmptyName { line } => assert_eq!(line, 1),
        other => panic!("expected EmptyName, got {other:?}"),
    }
}

#[test]
fn rejects_empty_name_when_only_whitespace_after_colon() {
    let plan = "research:    \n";
    let err = parse_research_dependencies(plan).unwrap_err();
    assert!(matches!(
        err,
        ResearchDependencyError::EmptyName { line: 1 }
    ));
}

#[test]
fn returns_empty_vec_when_no_declarations() {
    let plan = "# Plan\n\nNothing here.\n";
    let deps = parse_research_dependencies(plan).unwrap();
    assert!(deps.is_empty());
}

#[test]
fn returns_empty_vec_for_empty_input() {
    let deps = parse_research_dependencies("").unwrap();
    assert!(deps.is_empty());
}

#[test]
fn does_not_match_inside_inline_code_spans() {
    // Inline backticks make the parser skip the line entirely - we
    // treat the entire `research:` literal as documentation.
    let plan = "We discussed the `research: alpha` dependency here.\n";
    let deps = parse_research_dependencies(plan).unwrap();
    assert!(
        deps.is_empty(),
        "inline code spans must not produce dependencies: {deps:?}"
    );
}

#[test]
fn research_dependency_names_returns_just_strings() {
    let plan = "research: alpha\nresearch: beta\n";
    let names = research_dependency_names(plan).unwrap();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
}

#[test]
fn error_display_includes_line_and_raw_name() {
    let err = ResearchDependencyError::InvalidName {
        line: 42,
        raw_name: "BAD".to_string(),
        source: ResearchNameError::InvalidStart { ch: 'B' },
    };
    let msg = err.to_string();
    assert!(msg.contains("line 42"), "msg: {msg}");
    assert!(msg.contains("BAD"), "msg: {msg}");
}

#[test]
fn empty_name_error_includes_line_number() {
    let err = ResearchDependencyError::EmptyName { line: 7 };
    let msg = err.to_string();
    assert!(msg.contains("line 7"), "msg: {msg}");
}

#[test]
fn parses_inside_md_with_realistic_layout() {
    let plan = "\
# Implementation Plan: Example Feature

## Overview

This plan implements the example feature. See the prior
research for context.

research: example-feature-research

## Tasks

| ID | Title | Requirement |
|----|-------|-------------|
| T-001 | Setup | FR-001 |
";
    let deps = parse_research_dependencies(plan).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "example-feature-research");
}
