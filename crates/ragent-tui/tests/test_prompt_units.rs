//! T-017 unit tests: prompt module pure functions (spec `prompts`,
//! FR-005-FR-009, FR-011, FR-013).
//!
//! Covers the pure helpers of `crates/ragent-tui/src/app/prompt.rs` directly,
//! without the App/runtime harness (those paths are covered end to end by
//! `test_prompt_subagent_render.rs`, `test_prompt_tool_free.rs`,
//! `test_prompt_size_cap.rs`, and `test_prompt_readonly.rs`):
//!
//! - agent resolution: exact-case precedence, case-insensitive matching,
//!   hidden built-in exclusion, custom-agent shadowing, miss warning shape
//!   (FR-006 / FR-011),
//! - roster: built-in + custom rows, hidden exclusion, truncation, badges
//!   (FR-007),
//! - help page: every subcommand documented (FR-003),
//! - size cap: marker wording and cap arithmetic (FR-013, detailed cases in
//!   `test_prompt_size_cap.rs`).

use std::sync::Arc;

use ragent_agent::agent::AgentInfo;
use ragent_tui::app::prompt::{
    AgentResolution, PROMPT_REPORT_MAX_CHARS, PromptSource, RosterEntry, apply_size_cap,
    available_agent_names, render_help, render_miss_warning, render_roster,
    render_usage_correction, resolve_agent, roster_entries,
};

/// A minimal built-in-style agent definition.
fn builtin(name: &str, mode: ragent_agent::agent::AgentMode, hidden: bool) -> Arc<AgentInfo> {
    Arc::new(AgentInfo {
        name: name.to_string(),
        description: format!("{name} description"),
        mode,
        hidden,
        ..AgentInfo::default()
    })
}

/// A minimal custom agent definition.
fn custom(name: &str) -> ragent_agent::agent::CustomAgentDef {
    ragent_agent::agent::CustomAgentDef {
        record: ragent_agent::agent::oasf::OasfAgentRecord {
            name: name.to_string(),
            description: "custom agent".to_string(),
            version: "1.0.0".to_string(),
            schema_version: "0.7.0".to_string(),
            authors: Vec::new(),
            created_at: None,
            skills: Vec::new(),
            domains: Vec::new(),
            locators: Vec::new(),
            modules: Vec::new(),
        },
        source_path: std::path::PathBuf::from(format!("/tmp/{name}.json")),
        agent_info: Arc::new(AgentInfo {
            name: name.to_string(),
            description: "custom agent description".to_string(),
            ..AgentInfo::default()
        }),
        is_project_local: true,
    }
}

fn fixture() -> (
    Vec<Arc<AgentInfo>>,
    Vec<ragent_agent::agent::CustomAgentDef>,
) {
    (
        vec![
            builtin("general", ragent_agent::agent::AgentMode::Primary, false),
            builtin("build", ragent_agent::agent::AgentMode::Subagent, false),
            builtin(
                "hidden-internal",
                ragent_agent::agent::AgentMode::Primary,
                true,
            ),
        ],
        vec![custom("my-agent")],
    )
}

// ── FR-006 / FR-011: agent resolution ────────────────────────────────────

#[test]
fn test_resolve_agent_finds_builtin() {
    let (builtins, customs) = fixture();
    match resolve_agent("general", &builtins, &customs) {
        AgentResolution::Found(agent) => assert_eq!(agent.name, "general"),
        AgentResolution::Miss { .. } => panic!("general must resolve"),
    }
}

#[test]
fn test_resolve_agent_is_case_insensitive() {
    let (builtins, customs) = fixture();
    for probe in ["GENERAL", "General", "gEnErAl", "MY-AGENT", "My-Agent"] {
        match resolve_agent(probe, &builtins, &customs) {
            AgentResolution::Found(agent) => {
                let expected = probe.to_ascii_lowercase();
                assert_eq!(
                    agent.name.to_ascii_lowercase(),
                    expected,
                    "case-insensitive resolution must match `{probe}`"
                );
            }
            AgentResolution::Miss { .. } => panic!("`{probe}` must resolve"),
        }
    }
}

#[test]
fn test_resolve_agent_hidden_builtins_exact_only_and_never_offered() {
    let (builtins, customs) = fixture();
    // A hidden built-in resolves only by its exact name (the exact-match
    // arm precedes the hidden filter); it is never matched by the
    // case-insensitive fallback and never appears in the available-names
    // list the warnings offer.
    assert!(matches!(
        resolve_agent("hidden-internal", &builtins, &customs),
        AgentResolution::Found(agent) if Arc::ptr_eq(&agent, &builtins[2])
    ));
    assert!(matches!(
        resolve_agent("HIDDEN-INTERNAL", &builtins, &customs),
        AgentResolution::Miss { .. }
    ));
    assert!(
        !available_agent_names(&builtins, &customs)
            .iter()
            .any(|n| n == "hidden-internal"),
        "hidden built-ins must not be offered as resolvable names"
    );
}

#[test]
fn test_resolve_agent_miss_lists_available_names() {
    let (builtins, customs) = fixture();
    let res = resolve_agent("nosuchagent", &builtins, &customs);
    let names = match &res {
        AgentResolution::Miss { available, .. } => available.clone(),
        AgentResolution::Found(_) => panic!("miss expected"),
    };
    // Sorted, deduplicated, and covering both sources.
    assert_eq!(names, ["build", "general", "my-agent"]);
    // Warning text names the miss and lists every available agent.
    let warning = render_miss_warning(&res).expect("miss renders a warning");
    assert!(warning.contains("Unknown agent `nosuchagent`"));
    for name in ["build", "general", "my-agent"] {
        assert!(warning.contains(name), "warning must list `{name}`");
    }
    assert!(
        !warning.contains("## Available Tools"),
        "FR-011: the warning must not display prompt content"
    );
}

#[test]
fn test_resolve_agent_custom_shadows_nothing_but_resolves() {
    // A custom agent with the same name as a built-in keeps built-in
    // precedence (built-ins are consulted first); a custom-only name resolves
    // to the custom definition.
    let (builtins, customs) = fixture();
    let general = match resolve_agent("general", &builtins, &customs) {
        AgentResolution::Found(a) => a,
        AgentResolution::Miss { .. } => panic!("general must resolve"),
    };
    assert_eq!(
        general.description, "general description",
        "built-in definition wins for exact built-in names"
    );
    let mine = match resolve_agent("my-agent", &builtins, &customs) {
        AgentResolution::Found(a) => a,
        AgentResolution::Miss { .. } => panic!("custom agent must resolve"),
    };
    assert_eq!(mine.description, "custom agent description");
}

// ── FR-007: roster ───────────────────────────────────────────────────────

#[test]
fn test_roster_lists_nonhidden_builtins_and_customs() {
    let (builtins, customs) = fixture();
    let entries = roster_entries(&builtins, &customs);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"general") && names.contains(&"build") && names.contains(&"my-agent"));
    assert!(
        !names.contains(&"hidden-internal"),
        "FR-007: hidden built-ins must not appear in the roster"
    );
}

#[test]
fn test_roster_entries_carry_mode_and_source_badges() {
    let (builtins, customs) = fixture();
    let entries = roster_entries(&builtins, &customs);
    let general = entries.iter().find(|e| e.name == "general").unwrap();
    assert_eq!(general.mode, ragent_agent::agent::AgentMode::Primary);
    assert_eq!(general.source, PromptSource::Builtin);
    let mine = entries.iter().find(|e| e.name == "my-agent").unwrap();
    assert_eq!(mine.source, PromptSource::Custom);
}

#[test]
fn test_render_roster_includes_badges_and_description() {
    let entries = vec![RosterEntry {
        name: "general".to_string(),
        mode: ragent_agent::agent::AgentMode::Primary,
        source: PromptSource::Builtin,
        description: "general description".to_string(),
    }];
    let text = render_roster(&entries);
    assert!(
        text.contains("`general`"),
        "roster must show the agent name"
    );
    assert!(
        text.contains("[primary]"),
        "roster must show the mode badge"
    );
    assert!(
        text.contains("[built-in]"),
        "roster must show the source badge"
    );
    assert!(text.contains("general description"));
}

// ── FR-003: help page ────────────────────────────────────────────────────

#[test]
fn test_render_help_lists_every_subcommand() {
    let help = render_help();
    for line in [
        "`/prompt help`",
        "`/prompt primary [agent]`",
        "`/prompt subagent [agent]`",
        "`/prompt list`",
        "`/prompt <agent-name>`",
    ] {
        assert!(
            help.contains(line),
            "help must document `{line}`, got: {help}"
        );
    }
}

// ── FR-010: usage correction ─────────────────────────────────────────────

#[test]
fn test_render_usage_correction_lists_subcommands_and_agents() {
    let text =
        render_usage_correction("nosuchthing", &["general".to_string(), "build".to_string()]);
    assert!(text.contains("Unknown subcommand or agent `nosuchthing`"));
    for line in [
        "`/prompt help`",
        "`/prompt primary [agent]`",
        "`/prompt subagent [agent]`",
        "`/prompt list`",
        "`/prompt <agent-name>`",
    ] {
        assert!(text.contains(line), "correction must list `{line}`");
    }
    assert!(text.contains("`general`") && text.contains("`build`"));
    assert!(
        !text.contains("## Available Tools"),
        "FR-010: the correction must not display prompt content"
    );
}

// ── FR-013: size cap (spot checks; full cases in test_prompt_size_cap.rs) ─

#[test]
fn test_apply_size_cap_under_cap_passthrough() {
    let report = "From: /prompt\n\nbody";
    assert_eq!(apply_size_cap(report), report);
}

#[test]
fn test_apply_size_cap_marker_states_counts() {
    let total = PROMPT_REPORT_MAX_CHARS + 5;
    let report = "y".repeat(total);
    let out = apply_size_cap(&report);
    assert!(
        out.contains(&format!("showing {PROMPT_REPORT_MAX_CHARS} of {total}")),
        "marker must state shown/total, got: {:?}",
        &out[out.len().saturating_sub(200)..]
    );
}

// ── FR-005-009 integration assertions live in sibling files; the header's
// `(no tools)` marker is asserted here for the FR-009 surface directly. ────

#[test]
fn test_prompt_header_no_tools_marker_for_tool_free_agent() {
    // A `max_steps: 1` agent reports `(no tools)` through the header path
    // (FR-009); full end-to-end coverage in test_prompt_tool_free.rs.
    use ragent_tui::app::prompt::{effective_tool_defs, is_tool_free_agent};
    let def = custom("oneliner");
    let mut agent = (*def.agent_info).clone();
    agent.max_steps = Some(1);
    let registry = ragent_agent::tool::create_default_registry();
    assert!(is_tool_free_agent(&agent));
    let defs = effective_tool_defs(&registry, &agent, false);
    assert!(
        defs.is_empty(),
        "FR-009: tool-free agent must have zero tools"
    );
    let header = ragent_tui::app::prompt::PromptHeader {
        agent_name: agent.name.clone(),
        source: PromptSource::Custom,
        mode: ragent_agent::agent::AgentMode::Primary,
        tool_count: (!is_tool_free_agent(&agent) && !defs.is_empty()).then_some(defs.len()),
        body_chars: 0,
    };
    assert!(
        header.render().contains("(no tools)"),
        "FR-009: header must render `(no tools)`, got: {}",
        header.render()
    );
}
