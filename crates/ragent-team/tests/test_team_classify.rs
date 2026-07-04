//! Integration tests for `ragent-team` agent-type classification.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/team/classify.rs`
//! (T-006 of the testconsolidate spec). All tested functions are public via
//! `ragent_team::` re-exports, so no `#[path]` re-import is needed.

use ragent_team::{
    extract_explicit_agent_type, infer_agent_type, is_known_agent_type, resolve_agent_type,
    strip_explicit_agent_type_hint,
};

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
