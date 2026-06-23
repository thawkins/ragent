//! Integration tests for `/swarm` agent type assignment.

use ragent_team::team::{
    DEFAULT_AGENT_TYPE, SwarmSubtask, parse_decomposition, parse_decomposition_with_default,
};

#[test]
fn test_parse_decomposition_assigns_coder_to_test_task() {
    let raw = r#"{"tasks":[{"id":"s1","title":"Write parser tests","description":"Add unit tests for the new parser module","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(raw).expect("valid decomposition");
    assert_eq!(dec.tasks.len(), 1);
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("coder"));
}

#[test]
fn test_parse_decomposition_assigns_doc_writer_to_docs_task() {
    let raw = r#"{"tasks":[{"id":"s1","title":"Document public API","description":"Update rustdoc comments and README examples","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(raw).expect("valid decomposition");
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("doc-writer"));
}

#[test]
fn test_parse_decomposition_keeps_explicit_agent_type() {
    let raw = r#"{"tasks":[{"id":"s1","title":"Review code","description":"Audit the auth layer","depends_on":[],"agent_type":"security-reviewer"}]}"#;
    let dec = parse_decomposition(raw).expect("valid decomposition");
    assert_eq!(
        dec.tasks[0].agent_type.as_deref(),
        Some("security-reviewer")
    );
}

#[test]
fn test_parse_decomposition_unknown_explicit_falls_back_to_inference() {
    // Explicit "not-an-agent" is unknown; description contains "document" so
    // inference should pick doc-writer.
    let raw = r#"{"tasks":[{"id":"s1","title":"Write docs","description":"Document the API [agent: not-an-agent]","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(raw).expect("valid decomposition");
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("doc-writer"));
}

#[test]
fn test_default_agent_type_override_applies_when_no_inference() {
    let raw = r#"{"tasks":[{"id":"s1","title":"Vague","description":"Do something useful","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition_with_default(raw, Some("explore")).expect("valid decomposition");
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("explore"));
}

#[test]
fn test_default_agent_type_does_not_override_specific_inference() {
    let raw = r#"{"tasks":[{"id":"s1","title":"Fix crash","description":"Debug the segfault in the renderer","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition_with_default(raw, Some("explore")).expect("valid decomposition");
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("debug"));
}

#[test]
fn test_swarm_subtask_resolve_agent_type_strips_hint() {
    let mut task = SwarmSubtask {
        id: "s1".to_string(),
        title: "Review".to_string(),
        description: "Audit [agent: code-review] the API".to_string(),
        depends_on: vec![],
        agent_type: None,
        model: None,
    };
    task.resolve_agent_type(None);
    assert_eq!(task.agent_type.as_deref(), Some("code-review"));
    assert!(!task.description.contains("[agent:"));
}

#[test]
fn test_default_fallback_for_ambiguous_task() {
    let raw = r#"{"tasks":[{"id":"s1","title":"Do stuff","description":"Miscellaneous work","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(raw).expect("valid decomposition");
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some(DEFAULT_AGENT_TYPE));
}
