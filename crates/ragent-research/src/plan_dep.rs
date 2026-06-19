//! Parser for `research: <name>` dependency declarations in PLAN.md files
//! (FR-015).
//!
//! A spec under `specs/` can declare a dependency on a research item by
//! adding a top-level line of the form
//!
//! ```markdown
//! research: rust-async
//! ```
//!
//! anywhere outside code fences and HTML comments. This module scans a
//! PLAN.md string and returns one [`ResearchDependency`] per valid line,
//! in document order. The dependency's name is validated as a
//! [`ResearchName`] (FR-002 / FR-017) — invalid names are reported as
//! [`ResearchDependencyError::InvalidName`] so the spec author can fix the
//! typo before the spec lands.
//!
//! ## Why scan rather than formalise?
//!
//! PLAN.md files are free-form markdown. Hard-wiring the parser to a
//! specific section would make the dependency easy to miss when authors
//! reorganise their plans. Scanning the whole document lets `research:`
//! appear in a "Research" section, an "Overview", or as a top-of-file
//! sticky line — whichever the author prefers.

use crate::research_name::{ResearchName, ResearchNameError};
use serde::{Deserialize, Serialize};

/// One parsed `research: <name>` line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchDependency {
    /// The validated research name declared on the line.
    pub name: ResearchName,
    /// The 1-based line number in the source document where the
    /// declaration was found. Useful for error messages and editor
    /// integration.
    pub line: usize,
}

impl ResearchDependency {
    /// Borrow the validated research name as a string slice.
    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

/// Errors emitted by [`parse_research_dependencies`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchDependencyError {
    /// The line matched the `research:` prefix but the supplied name
    /// failed FR-002 / FR-017 validation.
    InvalidName {
        /// The 1-based line number where the bad declaration appeared.
        line: usize,
        /// The original (untrimmed) name as written by the spec author.
        raw_name: String,
        /// The underlying [`ResearchNameError`].
        source: ResearchNameError,
    },
    /// The `research:` line was empty (no name supplied).
    EmptyName {
        /// The 1-based line number where the empty declaration appeared.
        line: usize,
    },
}

impl std::fmt::Display for ResearchDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName { line, raw_name, source } => write!(
                f,
                "PLAN.md line {line}: invalid research dependency '{raw_name}': {source}"
            ),
            Self::EmptyName { line } => write!(
                f,
                "PLAN.md line {line}: research dependency is missing a name"
            ),
        }
    }
}

impl std::error::Error for ResearchDependencyError {}

/// Parse every `research: <name>` declaration from a PLAN.md string.
///
/// The parser:
///
/// - ignores lines inside triple-backtick fenced code blocks (```);
/// - ignores lines inside inline code spans (single backticks);
/// - accepts any amount of whitespace after the colon;
/// - ignores inline comments introduced by `#`;
/// - deduplicates repeated declarations (the first occurrence wins, in
///   document order) so a spec that mentions the same research in two
///   places still gets exactly one dependency entry.
///
/// Returns either the list of dependencies, or the first validation
/// error encountered. Validation is fail-fast: the spec author should fix
/// the typo before the spec lands, so emitting only the first error keeps
/// the failure mode obvious.
pub fn parse_research_dependencies(
    plan_md: &str,
) -> Result<Vec<ResearchDependency>, ResearchDependencyError> {
    let mut out = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut in_fence = false;

    for (index, raw_line) in plan_md.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();

        // Track fenced code blocks (``` or ~~~). Dependency declarations
        // inside fenced blocks are treated as documentation, not as live
        // declarations.
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        // Strip inline comments. Only treat `#` as a comment when it's at
        // the start of a token, not inside a name (which can't contain `#`
        // anyway thanks to FR-002, but be defensive).
        let line_without_comment = match line.find(" #") {
            Some(pos) => &line[..pos],
            None => line,
        };

        let Some(rest) = line_without_comment.strip_prefix("research:") else {
            continue;
        };
        let trimmed = rest.trim();

        // Strip surrounding inline backticks if present, e.g.
        // `research: \`rust-async\``.
        let stripped = trimmed
            .strip_prefix('`')
            .and_then(|s| s.strip_suffix('`'))
            .unwrap_or(trimmed);

        if stripped.is_empty() {
            return Err(ResearchDependencyError::EmptyName { line: line_number });
        }

        match ResearchName::try_new(stripped) {
            Ok(name) => {
                if seen.insert(name.as_str().to_string()) {
                    out.push(ResearchDependency { name, line: line_number });
                }
            }
            Err(source) => {
                return Err(ResearchDependencyError::InvalidName {
                    line: line_number,
                    raw_name: stripped.to_string(),
                    source,
                });
            }
        }
    }

    Ok(out)
}

/// Convenience wrapper: parse and return only the names in document order.
///
/// Returns `Err` for the same reasons as [`parse_research_dependencies`].
pub fn research_dependency_names(
    plan_md: &str,
) -> Result<Vec<String>, ResearchDependencyError> {
    Ok(parse_research_dependencies(plan_md)?
        .into_iter()
        .map(|d| d.name.into())
        .collect())
}

#[cfg(test)]
mod tests {
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
            deps.iter().map(|d| d.as_str()).collect::<Vec<_>>(),
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
        let names: Vec<&str> = deps.iter().map(|d| d.as_str()).collect();
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
            ResearchDependencyError::InvalidName { line, raw_name, source } => {
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
        assert!(matches!(
            err,
            ResearchDependencyError::InvalidName { .. }
        ));
        if let ResearchDependencyError::InvalidName { source, .. } = err {
            assert!(matches!(
                source,
                ResearchNameError::PathTraversal { .. }
            ));
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
        assert!(matches!(err, ResearchDependencyError::EmptyName { line: 1 }));
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
        // Inline backticks make the parser skip the line entirely — we
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
}