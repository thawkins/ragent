//! Tests for the `agent.response_language` standing prompt section.
//!
//! Models whose training data is dominated by a non-English language drift back
//! to it when the prompt carries no explicit constraint, and a conversation-level
//! correction is lost at the next compaction. The section is therefore part of
//! the system prompt, which is rebuilt from config on every step.
//!
//! These tests pin the properties that make the feature safe: an omitted key
//! still emits the English section (so no setup is needed), a blank value is the
//! explicit opt-out and emits nothing, a set value emits the named language, and
//! the section reaches both a tool-bearing agent and the tool-free early-return
//! path.

use std::path::Path;

use ragent_agent::agent::{
    AgentInfo, AgentMode, build_response_language_section,
    build_system_prompt_with_storage_and_memory_and_config, builtin_agents,
};
use ragent_config::Config;

/// The built-in agent named `name`, which must exist.
fn builtin(name: &str) -> &'static AgentInfo {
    builtin_agents()
        .iter()
        .map(AsRef::as_ref)
        .find(|agent| agent.name == name)
        .unwrap_or_else(|| panic!("built-in agent {name} must exist"))
}

/// A default config whose `response_language` is set to `language`.
///
/// `Config` has a private bookkeeping field, so a struct literal cannot be
/// built from this crate; the field is set on a defaulted value instead.
fn with_language(language: &str) -> Config {
    let mut config = Config::default();
    config.response_language = Some(language.to_string());
    config
}

/// A tool-free copy of the `general` agent (a single step skips project context).
fn tool_free(agent: &AgentInfo) -> AgentInfo {
    AgentInfo {
        max_steps: Some(1),
        ..agent.clone()
    }
}

#[test]
fn test_response_language_section_defaults_to_english() {
    // No configuration at all still pins the answer language: a model that would
    // otherwise drift back to its training language is constrained out of the box.
    let config = Config::default();
    let section = build_response_language_section(Some(&config)).expect("section is emitted");
    assert!(section.starts_with("## Response Language"), "{section}");
    assert!(section.contains("in English"), "{section}");
}

#[test]
fn test_response_language_section_is_absent_when_blank() {
    for blank in ["", "   ", "\t\n"] {
        let config = with_language(blank);
        assert!(
            build_response_language_section(Some(&config)).is_none(),
            "a whitespace-only language ({blank:?}) must emit no section"
        );
    }
}

#[test]
fn test_response_language_section_is_absent_without_config() {
    assert!(build_response_language_section(None).is_none());
}

#[test]
fn test_response_language_section_names_the_language() {
    let config = with_language("English");
    let section = build_response_language_section(Some(&config)).expect("section is emitted");
    assert!(section.starts_with("## Response Language"), "{section}");
    assert!(section.contains("in English"), "{section}");
    // The wording must state that it outranks the conversation, otherwise a
    // model treats a later correction or compacted summary as authoritative.
    assert!(section.contains("outranks"), "{section}");
    assert!(section.contains("summary"), "{section}");
}

#[test]
fn test_response_language_section_trims_surrounding_space() {
    let config = with_language("  English  ");
    let section = build_response_language_section(Some(&config)).expect("section is emitted");
    assert!(section.contains("in English,"), "{section}");
    assert!(!section.contains("in   English"), "{section}");
}

#[test]
fn test_response_language_reaches_a_tool_bearing_prompt() {
    let config = with_language("English");
    let prompt = build_system_prompt_with_storage_and_memory_and_config(
        builtin("general"),
        Path::new("."),
        "",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&config),
    );
    assert!(
        prompt.contains("## Response Language"),
        "a tool-bearing agent must carry the section"
    );
    assert!(prompt.contains("in English"), "the language is named");
}

#[test]
fn test_response_language_reaches_a_tool_free_prompt() {
    // A single-step agent takes the early return that skips project context;
    // the language rule must be emitted before that return.
    let agent = tool_free(builtin("general"));
    let config = with_language("English");
    let prompt = build_system_prompt_with_storage_and_memory_and_config(
        &agent,
        Path::new("."),
        "",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&config),
    );
    assert!(
        prompt.contains("## Response Language"),
        "a tool-free agent must still carry the section"
    );
    // The early return is what makes this test meaningful: project context is
    // skipped there, so the section is the only thing it added.
    assert!(
        !prompt.contains("## Project Structure"),
        "the tool-free path skips project context"
    );
}

#[test]
fn test_response_language_absent_from_a_prompt_when_blank() {
    // A blank value is the opt-out: the user asked for the model's own default,
    // so no section is emitted at all.
    let agent = builtin("general");
    let config = with_language("");
    let prompt = build_system_prompt_with_storage_and_memory_and_config(
        agent,
        Path::new("."),
        "",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&config),
    );
    assert!(
        !prompt.contains("## Response Language"),
        "a blank language is the explicit opt-out"
    );
}

#[test]
fn test_tool_free_agent_mode_is_preserved() {
    // Guard for `tool_free`: the helper must not accidentally change the mode,
    // or the tool-free test would assert against the wrong code path.
    assert_eq!(tool_free(builtin("general")).mode, AgentMode::Primary);
}
