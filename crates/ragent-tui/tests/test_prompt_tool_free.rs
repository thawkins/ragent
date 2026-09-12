//! T-007 regression tests: tool-free agent path with the `(no tools)` marker
//! (FR-009, spec `prompts`).
//!
//! FR-009: WHILE the resolved agent is tool-free (the assembler's tool gate
//! yields no tool sections, `agent/mod.rs:2533-2536`), the renderer SHALL
//! display the prompt body without any `## Available Tools` section and SHALL
//! state `(no tools)` in the header summary instead of a tool count.
//!
//! No built-in agent is currently tool-free (all declare `max_steps: 1024` or
//! `None`), so the end-to-end test installs a synthetic custom agent with
//! `max_steps: 1` through the real OASF loader path
//! (`agent::custom::record_to_agent_info`) and runs `/prompt primary` through
//! the production slash dispatcher.
//!
//! `handle_prompt_render` calls `tokio::task::block_in_place`, which is only
//! legal on the multi-threaded runtime, so the harness spins one up (the
//! production TUI runs on the multi-thread runtime as well).

use std::path::Path;

use ragent_agent::agent::custom::{CustomAgentDef, record_to_agent_info};
use ragent_agent::agent::oasf::{OasfAgentRecord, OasfModule, RAGENT_MODULE_TYPE};
use ragent_tui::app::prompt::{effective_tool_defs, is_tool_free_agent};
use serde_json::json;

mod support;
use support::make_app;

/// Run a `/prompt` subcommand and return `(assistant_text, status)`.
fn prompt_report(input: &str) -> (String, String) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread runtime")
        .block_on(async move {
            let mut app = make_app();
            app.execute_slash_command(input);
            let text = app
                .messages
                .last()
                .map(|m| m.text_content())
                .unwrap_or_default();
            (text, app.status.clone())
        })
}

/// Build a validated tool-free custom agent (`max_steps: 1`) through the real
/// OASF record loader, ready to install into `App::custom_agent_defs`.
fn tool_free_custom_def(name: &str) -> CustomAgentDef {
    let record = OasfAgentRecord {
        name: name.to_string(),
        description: "Single-shot tool-free agent for the FR-009 test".to_string(),
        version: "1.0.0".to_string(),
        schema_version: "0.7.0".to_string(),
        authors: Vec::new(),
        created_at: None,
        skills: Vec::new(),
        domains: Vec::new(),
        locators: Vec::new(),
        modules: vec![OasfModule {
            module_type: RAGENT_MODULE_TYPE.to_string(),
            payload: json!({
                "system_prompt": "You answer in one step with no tools.",
                "mode": "primary",
                "max_steps": 1
            }),
        }],
    };
    let agent_info =
        record_to_agent_info(&record, Path::new("/tmp/tool-free-agent.json")).expect("valid agent");
    CustomAgentDef {
        record,
        source_path: std::path::PathBuf::from("/tmp/tool-free-agent.json"),
        agent_info,
        is_project_local: true,
    }
}

#[test]
fn test_is_tool_free_agent_predicate() {
    let builtins = ragent_agent::agent::builtin_agents();
    // Every built-in agent has tools (max_steps 1024 or None)...
    for agent in builtins.iter() {
        assert!(
            !is_tool_free_agent(agent),
            "built-in agent `{}` must not be tool-free",
            agent.name
        );
    }
    // ...while a max_steps <= 1 definition is tool-free.
    let def = tool_free_custom_def("unit-free");
    assert!(is_tool_free_agent(&def.agent_info));
    let mut one = (*def.agent_info).clone();
    one.max_steps = Some(0);
    assert!(is_tool_free_agent(&one));
    // None means the default unlimited budget: not tool-free.
    one.max_steps = None;
    assert!(!is_tool_free_agent(&one));
    one.max_steps = Some(2);
    assert!(!is_tool_free_agent(&one));
}

#[test]
fn test_effective_tool_defs_tool_free_agent_is_empty() {
    let def = tool_free_custom_def("unit-free-defs");
    let registry = ragent_agent::tool::create_default_registry();
    let defs = effective_tool_defs(&registry, &def.agent_info, false);
    assert!(
        defs.is_empty(),
        "FR-009: a tool-free agent must have an empty effective tool surface"
    );
}

#[test]
fn test_prompt_primary_tool_free_custom_agent_reports_no_tools() {
    let def = tool_free_custom_def("oneliner");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread runtime")
        .block_on(async move {
            let mut app = make_app();
            app.custom_agent_defs = vec![def];
            app.execute_slash_command("/prompt primary oneliner");
            let text = app
                .messages
                .last()
                .map(|m| m.text_content())
                .unwrap_or_default();

            // FR-009: the header summary states `(no tools)` instead of a
            // tool count.
            let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(
                flat.contains("(no tools)"),
                "FR-009: header must state `(no tools)` for a tool-free agent, got: {flat}"
            );
            // FR-009: the prompt body carries no `## Available Tools`
            // section at all.
            assert!(
                !text.contains("## Available Tools"),
                "FR-009: tool-free report must omit the tool reference, got: {}",
                text
            );
            // The status line names the tool-free case.
            assert!(
                app.status.contains("no tools"),
                "status must reflect the tool-free path, got: {}",
                app.status
            );
        });
}

#[test]
fn test_prompt_primary_tool_agent_still_shows_tools() {
    let (text, status) = prompt_report("/prompt primary general");
    // Contrast: a tool-carrying agent still renders the tool reference and a
    // positive tool count in the header.
    assert!(
        text.contains("## Available Tools"),
        "contrast: tool-carrying agent must render the tool reference, got: {text}"
    );
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        !flat.contains("(no tools)"),
        "contrast: tool-carrying agent must not be reported tool-free, got: {flat}"
    );
    assert!(
        status.contains(" tools)"),
        "contrast: status must report a tool count, got: {status}"
    );
}

#[test]
fn test_prompt_subagent_tool_free_custom_agent_reports_no_tools() {
    // The FR-009 path applies in subagent mode too (mode does not override
    // the tool-free gate).
    let def = tool_free_custom_def("oneliner-sub");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread runtime")
        .block_on(async move {
            let mut app = make_app();
            app.custom_agent_defs = vec![def];
            app.execute_slash_command("/prompt subagent oneliner-sub");
            let text = app
                .messages
                .last()
                .map(|m| m.text_content())
                .unwrap_or_default();
            let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(
                flat.contains("(no tools)"),
                "FR-009: subagent report for a tool-free agent must state `(no tools)`, got: {flat}"
            );
            assert!(
                !text.contains("## Available Tools"),
                "FR-009: subagent tool-free report must omit the tool reference, got: {}",
                text
            );
        });
}
