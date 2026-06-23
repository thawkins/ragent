//! Agent-type classification for swarm subtasks.
//!
//! Each subtask produced by `/swarm` decomposition may optionally carry an agent
//! type. This module provides a lightweight rule-based classifier that falls
//! back to keyword matching when the LLM omits the field or returns a value
//! that does not correspond to a registered agent.

// ── Classification schema ────────────────────────���───────────────────────────

/// Default agent type used when no explicit or inferred type is available.
pub const DEFAULT_AGENT_TYPE: &str = "general";

/// Agent types that the classifier may return.
pub const KNOWN_AGENT_TYPES: &[&str] = &[
    "general",
    "coder",
    "task",
    "architect",
    "ask",
    "debug",
    "code-review",
    "orchestrator",
    "doc-writer",
    "explore",
    "build",
    "plan",
    "security-reviewer",
];

/// Mapping from keyword stems to preferred agent type.
///
/// The first matching keyword wins. Keywords should be lowercase and avoid
/// ambiguous overlap where possible.
const KEYWORD_MAP: &[(&[&str], &str)] = &[
    // Security / review
    (
        &["security", "vulnerability", "audit", "exploit", "cve"],
        "security-reviewer",
    ),
    // Code review / quality
    (
        &[
            "review", "refactor", "clean up", "cleanup", "lint", "clippy", "quality",
        ],
        "code-review",
    ),
    // Documentation
    (
        &[
            "document",
            "doc",
            "readme",
            "changelog",
            "spec",
            "guide",
            "howto",
        ],
        "doc-writer",
    ),
    // Testing
    (
        &[
            "test",
            "tests",
            "testing",
            "unit test",
            "integration test",
            "benchmark",
        ],
        "coder",
    ),
    // Build / CI
    (
        &[
            "build", "compile", "cargo", "ci", "pipeline", "workflow", "release",
        ],
        "build",
    ),
    // Planning / design
    (
        &[
            "plan",
            "design",
            "architecture",
            "roadmap",
            "milestone",
            "structure",
        ],
        "architect",
    ),
    // Exploration / research
    (
        &[
            "explore",
            "research",
            "investigate",
            "find",
            "discover",
            "survey",
        ],
        "explore",
    ),
    // Debugging
    (
        &[
            "debug",
            "fix",
            "bug",
            "error",
            "crash",
            "failure",
            "regression",
        ],
        "debug",
    ),
    // General coding
    (
        &[
            "implement",
            "write",
            "create",
            "add",
            "feature",
            "code",
            "function",
        ],
        "coder",
    ),
];

// ── Classification API ──────────────────────────────────────────────────────

/// Extract an explicit agent type hint embedded in task text.
///
/// Supported forms (case-insensitive):
/// - `[agent: code-review]`
/// - `[agent_type: code-review]`
/// - `(agent: code-review)`
///
/// Returns the hint text with the surrounding marker stripped, or `None` if
/// no hint is present.
#[must_use]
pub fn extract_explicit_agent_type(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for prefix in ["[agent:", "[agent_type:", "(agent:", "(agent_type:"] {
        if let Some(start) = lower.find(prefix) {
            let rest = &text[start + prefix.len()..];
            let end = rest
                .find(']')
                .or_else(|| rest.find(')'))
                .unwrap_or(rest.len());
            let hint = rest[..end].trim();
            if !hint.is_empty() {
                return Some(hint.to_string());
            }
        }
    }
    None
}

/// Strip an explicit agent type hint from task text.
///
/// Removes the marker and any whitespace that was introduced around it.
/// Returns the original text unchanged if no hint is found.
#[must_use]
pub fn strip_explicit_agent_type_hint(text: &str) -> String {
    let lower = text.to_lowercase();
    for prefix in ["[agent:", "[agent_type:", "(agent:", "(agent_type:"] {
        if let Some(start) = lower.find(prefix) {
            let rest = &text[start + prefix.len()..];
            if let Some(end_offset) = rest.find(']').or_else(|| rest.find(')')) {
                let end = start + prefix.len() + end_offset + 1;
                let result = format!("{}{}", &text[..start], &text[end..]);
                return result.split_whitespace().collect::<Vec<_>>().join(" ");
            }
        }
    }
    text.to_string()
}

/// Infer an agent type from free-form task text.
///
/// Performs case-insensitive keyword matching against [`KEYWORD_MAP`].
/// Returns [`DEFAULT_AGENT_TYPE`] when no keyword matches.
#[must_use]
pub fn infer_agent_type(text: &str) -> String {
    let lower = text.to_lowercase();
    for (keywords, agent_type) in KEYWORD_MAP {
        for keyword in *keywords {
            if lower.contains(keyword) {
                return (*agent_type).to_string();
            }
        }
    }
    DEFAULT_AGENT_TYPE.to_string()
}

/// Resolve the agent type for a subtask using the full fallback chain.
///
/// 1. Use the explicit value if it is non-empty and known.
/// 2. Look for an explicit hint embedded in the description/title.
/// 3. Infer a specific agent type from keywords in the combined title + description.
/// 4. If no specific type was inferred, use the user-provided swarm default.
/// 5. Fall back to [`DEFAULT_AGENT_TYPE`].
///
/// The `available` predicate should return `true` for agent types that are
/// registered in the current runtime.
#[must_use]
pub fn resolve_agent_type(
    title: &str,
    description: &str,
    explicit: Option<&str>,
    default_agent_type: Option<&str>,
    available: impl Fn(&str) -> bool,
) -> String {
    let fallback = default_agent_type.unwrap_or(DEFAULT_AGENT_TYPE);

    // 1. Explicit value from structured decomposition.
    if let Some(value) = explicit {
        let value = value.trim();
        if !value.is_empty() && available(value) {
            return value.to_string();
        }
    }

    // 2. Embedded hint in title or description.
    let combined = format!("{title}\n{description}");
    if let Some(hint) = extract_explicit_agent_type(&combined) {
        let hint = hint.trim();
        if available(hint) {
            return hint.to_string();
        }
    }

    // 3. Keyword inference.
    let inferred = infer_agent_type(&combined);
    if inferred != DEFAULT_AGENT_TYPE && available(&inferred) {
        return inferred;
    }

    // 4. User-provided swarm default (overrides the generic fallback).
    if available(fallback) && fallback != DEFAULT_AGENT_TYPE {
        return fallback.to_string();
    }

    if available(&inferred) {
        return inferred;
    }

    // 5. Final fallback.
    DEFAULT_AGENT_TYPE.to_string()
}

/// Check whether `agent_type` is one of the well-known built-in names.
#[must_use]
pub fn is_known_agent_type(agent_type: &str) -> bool {
    KNOWN_AGENT_TYPES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(agent_type))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_explicit_agent_type() {
        assert_eq!(
            extract_explicit_agent_type("Review code [agent: code-review]"),
            Some("code-review".to_string())
        );
        assert_eq!(
            extract_explicit_agent_type("(agent_type: doc-writer) write docs"),
            Some("doc-writer".to_string())
        );
        assert_eq!(extract_explicit_agent_type("No hint here"), None);
    }

    #[test]
    fn test_infer_agent_type() {
        assert_eq!(infer_agent_type("Write unit tests for the parser"), "coder");
        assert_eq!(
            infer_agent_type("Refactor the networking module"),
            "code-review"
        );
        assert_eq!(infer_agent_type("Document the public API"), "doc-writer");
        assert_eq!(infer_agent_type("Investigate memory usage"), "explore");
        assert_eq!(infer_agent_type("Something vague"), "general");
    }

    #[test]
    fn test_strip_explicit_agent_type_hint() {
        assert_eq!(
            strip_explicit_agent_type_hint("Review code [agent: code-review] thoroughly"),
            "Review code thoroughly"
        );
        assert_eq!(
            strip_explicit_agent_type_hint("(agent_type: doc-writer) write docs"),
            "write docs"
        );
        assert_eq!(
            strip_explicit_agent_type_hint("No hint here"),
            "No hint here"
        );
    }

    #[test]
    fn test_resolve_agent_type() {
        let available =
            |s: &str| s == "general" || s == "code-review" || s == "doc-writer" || s == "coder";

        // explicit wins
        assert_eq!(
            resolve_agent_type("x", "y", Some("code-review"), None, available),
            "code-review"
        );

        // hint wins over inference
        assert_eq!(
            resolve_agent_type(
                "Review",
                "[agent: doc-writer] write docs",
                None,
                None,
                available
            ),
            "doc-writer"
        );

        // inference
        assert_eq!(
            resolve_agent_type("Document API", "Add rustdocs", None, None, available),
            "doc-writer"
        );

        // unavailable explicit falls back to inference
        assert_eq!(
            resolve_agent_type(
                "Fix bug",
                "debug crash",
                Some("unknown-agent"),
                None,
                available
            ),
            "general"
        );

        // default override (used when no specific type is inferred)
        assert_eq!(
            resolve_agent_type("x", "y", None, Some("coder"), available),
            "coder"
        );

        // inferred specific type takes precedence over default override
        assert_eq!(
            resolve_agent_type("Fix bug", "debug crash", None, Some("coder"), available),
            "coder"
        );

        // unavailable default falls back to general
        assert_eq!(
            resolve_agent_type("x", "y", None, Some("unknown-agent"), available),
            "general"
        );
    }

    #[test]
    fn test_is_known_agent_type() {
        assert!(is_known_agent_type("coder"));
        assert!(is_known_agent_type("Coder"));
        assert!(!is_known_agent_type("not-an-agent"));
    }
}
