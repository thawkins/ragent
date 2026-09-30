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
//! [`ResearchName`] (FR-002 / FR-017) - invalid names are reported as
//! [`ResearchDependencyError::InvalidName`] so the spec author can fix the
//! typo before the spec lands.
//!
//! ## Why scan rather than formalise?
//!
//! PLAN.md files are free-form markdown. Hard-wiring the parser to a
//! specific section would make the dependency easy to miss when authors
//! reorganise their plans. Scanning the whole document lets `research:`
//! appear in a "Research" section, an "Overview", or as a top-of-file
//! sticky line - whichever the author prefers.

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
    #[must_use]
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
            Self::InvalidName {
                line,
                raw_name,
                source,
            } => write!(
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
                    out.push(ResearchDependency {
                        name,
                        line: line_number,
                    });
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

/// Parse the `research:` list from a SPEC.md frontmatter block. This
/// complements [`parse_research_dependencies`] which reads PLAN.md.
///
/// Returns the names in document order, deduplicated. Invalid entries
/// are ignored so the surface stays best-effort.
#[must_use]
pub fn parse_spec_frontmatter_research(frontmatter: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("research:") else {
            continue;
        };
        let rest = rest.trim();
        // Accept both inline `[a, b, c]` and a single bare name.
        let inner = rest
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(rest);
        for raw in inner.split(',') {
            let name = raw.trim().trim_matches('"').trim_matches('\'');
            if name.is_empty() {
                continue;
            }
            if seen.insert(name.to_string()) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Convenience wrapper: parse and return only the names in document order.
///
/// Returns `Err` for the same reasons as [`parse_research_dependencies`].
pub fn research_dependency_names(plan_md: &str) -> Result<Vec<String>, ResearchDependencyError> {
    Ok(parse_research_dependencies(plan_md)?
        .into_iter()
        .map(|d| d.name.into())
        .collect())
}

#[cfg(test)]
#[path = "../tests/inline/plan_dep_tests.rs"]
mod tests;
