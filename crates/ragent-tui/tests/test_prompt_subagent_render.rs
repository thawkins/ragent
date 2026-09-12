//! T-006 regression tests: `/prompt subagent` (FR-005, spec `prompts`).
//!
//! Proves the subagent-mode report is a *forced-mode* assembly of the same
//! inputs the session loop uses:
//!
//! - `## Sub-Agent Completion Protocol (MANDATORY - HARD REQUIREMENT)` present,
//! - `## Sub-Agent Spawning` absent,
//! - tool reference is the detailed form with interactive tools excluded,
//! - header summary shows the subagent mode.
//!
//! The mode-forcing rules live in `handle_prompt_render`
//! (`crates/ragent-tui/src/app/slash.rs`): the forced mode picks the detailed
//! tool reference when `subagent_mode` is set and feeds the assembler the
//! same agent the session loop would, so the tests run the canonical paths
//! end to end rather than re-implementing the assembly.

mod support;
use support::make_app;

/// Run a `/prompt` subcommand and return the rendered assistant text.
///
/// `handle_prompt_render` calls `tokio::task::block_in_place`, which is only
/// legal on the multi-threaded runtime, so the harness spins one up (the
/// production TUI runs on the multi-thread runtime as well).
fn prompt_report(input: &str) -> String {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread runtime")
        .block_on(async move {
            let mut app = make_app();
            app.execute_slash_command(input);
            app.messages
                .last()
                .map(|m| m.text_content())
                .unwrap_or_default()
        })
}

#[test]
fn test_prompt_subagent_completion_protocol_present() {
    let text = prompt_report("/prompt subagent general");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("## Sub-Agent Completion Protocol (MANDATORY - HARD REQUIREMENT)"),
        "FR-005: subagent report must include the completion-protocol section, got: {flat}"
    );
}

#[test]
fn test_prompt_subagent_spawning_section_absent() {
    let text = prompt_report("/prompt subagent general");
    assert!(
        !text.contains("## Sub-Agent Spawning"),
        "FR-005: subagent report must NOT include the spawning section, got: {text}"
    );
}

#[test]
fn test_prompt_subagent_excludes_interactive_tools() {
    let text = prompt_report("/prompt subagent general");
    // The detailed tool reference renders tool headings as `### \`name\``.
    // (Plain-text mentions of `ask_user` in the AGENTS.md guidance are not
    // tool-reference entries and are irrelevant to FR-005.)
    for tool in ["ask_user", "question"] {
        let heading = format!("### `{tool}`");
        assert!(
            !text.contains(&heading),
            "FR-005: subagent tool reference must exclude `{tool}`, got: {text}"
        );
    }
}

#[test]
fn test_prompt_subagent_header_shows_subagent_mode() {
    let text = prompt_report("/prompt subagent general");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("mode: subagent"),
        "FR-005: header summary must show the subagent mode, got: {flat}"
    );
}

#[test]
fn test_prompt_primary_spawning_present_completion_absent() {
    // `general` is a primary-mode agent; assert the natural primary contrast.
    let text = prompt_report("/prompt primary general");
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("## Sub-Agent Spawning"),
        "primary report must include the spawning section (FR-005 contrast), got: {flat}"
    );
    assert!(
        !flat.contains("Completion Protocol"),
        "primary report must NOT include the completion-protocol section, got: {flat}"
    );
}
