//! Integration tests for `ragent-team` swarm decomposition parsing & prompting.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/team/swarm.rs`
//! (T-006 of the testconsolidate spec). All tested functions are public via
//! `ragent_team::` re-exports.

use ragent_team::{
    build_decomposition_user_prompt, parse_decomposition, parse_decomposition_with_default,
};

#[test]
fn test_parse_clean_json() {
    let input = r#"{"tasks":[{"id":"s1","title":"Setup","description":"Do setup","depends_on":[]},{"id":"s2","title":"Build","description":"Build it","depends_on":["s1"]}]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks.len(), 2);
    assert_eq!(dec.tasks[0].id, "s1");
    assert_eq!(dec.tasks[1].depends_on, vec!["s1"]);
}

#[test]
fn test_parse_with_markdown_fences() {
    let input = r#"```json
{"tasks":[{"id":"s1","title":"Only task","description":"Do it","depends_on":[]}]}
```"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks.len(), 1);
}

#[test]
fn test_parse_with_trailing_commas() {
    let input = r#"{"tasks":[{"id":"s1","title":"A","description":"B","depends_on":[],},]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks.len(), 1);
}

#[test]
fn test_parse_invalid_json() {
    let input = "not json at all";
    assert!(parse_decomposition(input).is_err());
}

#[test]
fn test_parse_normalises_agent_type() {
    let input = r#"{"tasks":[{"id":"s1","title":"Write tests","description":"Add unit tests for parser","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("coder"));
}

#[test]
fn test_parse_strips_embedded_agent_hint() {
    let input = r#"{"tasks":[{"id":"s1","title":"Review","description":"Audit [agent: code-review] the API","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("code-review"));
    assert!(!dec.tasks[0].description.contains("[agent:"));
}

#[test]
fn test_parse_respects_default_agent_type_override() {
    let input = r#"{"tasks":[{"id":"s1","title":"Vague","description":"Do something","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition_with_default(input, Some("explore")).unwrap();
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("explore"));
}

#[test]
fn test_build_user_prompt() {
    let prompt = build_decomposition_user_prompt("Build a REST API");
    assert!(prompt.contains("Build a REST API"));
    assert!(prompt.contains("Decompose"));
}
