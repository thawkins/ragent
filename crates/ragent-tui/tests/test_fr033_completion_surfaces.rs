//! Guard: the slash-command and `/tools` menus offer only what the handler
//! accepts, and no menu offers a retired tool spelling (spec `uniformtools`
//! FR-033, T-045).
//!
//! FR-033 names four sources a suggestion can be drawn from: the slash-command
//! menu, the tool-name menu, the did-you-mean pool, and the lists printed by
//! `--dry-run`, `tool_info` and `commands_info`. Three of them live in
//! `ragent-agent` and are pinned in
//! `crates/ragent-agent/tests/test_fr033_completion_sources.rs`; the slash
//! menu is a TUI render path and is pinned here, beside the surface it reads.
//!
//! | # | Case | Clause it pins |
//! |---|---|---|
//! | 1 | `test_the_tools_menu_offers_exactly_the_one_subcommand_table` | the menu, the hint and the handler read one table |
//! | 2 | `test_every_offered_tools_subcommand_is_handled` | every offered form is reachable |
//! | 3 | `test_the_slash_menu_offers_no_retired_tool_spelling` | no menu entry (or its suggestions) names a `before` |
//! | 4 | `test_the_prompt_field_offers_no_tool_names` | FR-033 adds no new completion surface |
//!
//! ## Why "adds no new completion surface" is a case rather than a sentence
//!
//! The `@` file picker is the only completion surface inside the prompt field, and
//! it completes *paths*. FR-033's last clause says a field that offers no tool name
//! today need not start, so this file asserts that the file menu's rows are file
//! paths and never tool names - a later change that widened it to tools would fail
//! here rather than quietly adding a fifth place a retired spelling could leak from.
//!
//! ## What this file deliberately does not check
//!
//! - **The `/tools` table's own rows.** `/tools` prints whatever the registry
//!   holds, which FR-030's `test_fr030_surface_agreement.rs` compares with the
//!   registry element for element. This file checks the *menu* the command offers,
//!   which is the part FR-033 governs.
//! - **`commands_info`.** Same reason as the row check: the catalogue's
//!   `/tools` subcommand list is asserted in the agent-side file, and a second copy
//!   here would be a guard to keep in step.

#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use std::collections::BTreeSet;

use ragent_agent::tool::command_catalog::TOOLS_SUBS;
use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app in an isolated project with a live session.
///
/// The isolation harness returns the crate-wide env guard, which also serialises
/// this test against every other env-mutating test in the binary, so no second
/// lock is taken here (`std::sync::Mutex` is not reentrant).
fn isolated_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// The `/tools` menu entry the autocomplete produces for the typed `/tools`.
fn tools_entry(app: &mut App) -> ragent_tui::app::SlashMenuEntry {
    app.input = "/tools".to_string();
    app.input_cursor = app.input.chars().count();
    app.update_slash_menu();
    app.slash_menu
        .as_ref()
        .expect("typing /tools opens the slash menu")
        .matches
        .iter()
        .find(|entry| entry.trigger == "tools")
        .cloned()
        .expect("the slash menu contains the /tools entry")
}

// ---------------------------------------------------------------------------
// 1. One table, three readers
// ---------------------------------------------------------------------------

#[test]
fn test_the_tools_menu_offers_exactly_the_one_subcommand_table() {
    let (_guard, _env, _temp, mut app) = isolated_app();
    let entry = tools_entry(&mut app);

    // The menu offers the single `TOOLS_SUBS` table - not a second copy, which is
    // what the old list was: it offered `github`, `gitlab`, `teams`, `agents`,
    // `plan`, `codeindex` and `masterfetch`, every one of which the `/tools`
    // handler rejects with a usage warning.
    assert_eq!(
        entry.suggestions,
        TOOLS_SUBS
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<String>>(),
        "FR-033: the `/tools` menu offers {:?}, not the single `TOOLS_SUBS` table \
         the handler accepts and `commands_info` publishes",
        entry.suggestions
    );

    // The parameter hint is the same table, so the two cannot disagree.
    assert_eq!(
        entry.parameter_hint.as_deref(),
        Some(format!("[{}]", TOOLS_SUBS.join("|")).as_str()),
        "FR-033: the `/tools` parameter hint is not the `TOOLS_SUBS` table"
    );
}

// ---------------------------------------------------------------------------
// 2. Every offered form is reachable
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_every_offered_tools_subcommand_is_handled() {
    let (_guard, _env, _temp, mut app) = isolated_app();

    for sub in TOOLS_SUBS {
        let command = format!("/tools {sub}");
        app.execute_slash_command(&command).await;
        assert_ne!(
            app.status,
            "tools error",
            "FR-033: `/tools {sub}` is offered by the autocomplete menu but the \
             handler rejects it: {:?}",
            last_message(&app)
        );
    }

    // A form no table lists is a reported error, not a silent no-op - the other
    // half of the same rule: an unoffered form must not be accepted either.
    app.execute_slash_command("/tools frobnicate").await;
    assert_eq!(
        app.status, "tools error",
        "FR-033: `/tools frobnicate` is not in `TOOLS_SUBS` and must be refused"
    );

    // The usage warning the refusal prints names the forms the table offers, so a
    // reader of the error is sent to a form that works.
    let message = last_message(&app);
    for sub in TOOLS_SUBS {
        assert!(
            message.contains(&format!("`/tools {sub}`")),
            "FR-033: the `/tools` usage warning does not offer `/tools {sub}`: {message:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. No menu entry, or its suggestions, names a retired spelling
// ---------------------------------------------------------------------------

#[test]
fn test_the_slash_menu_offers_no_retired_tool_spelling() {
    let (_guard, _env, _temp, mut app) = isolated_app();

    let retired: BTreeSet<&str> = TOOL_RENAMES.iter().map(|pair| pair.before).collect();
    let triggers: BTreeSet<&str> = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .map(|command| command.trigger)
        .collect();

    // Every menu entry's trigger and its suggestions, for the whole command
    // surface: nothing a completion list can offer may be a retired tool spelling,
    // unless the same word is itself a slash command (`/bash` manages the shell
    // allowlist and is not the retired `bash` tool).
    for trigger in &triggers {
        let entry = {
            app.input = format!("/{trigger}");
            app.input_cursor = app.input.chars().count();
            app.update_slash_menu();
            app.slash_menu
                .as_ref()
                .and_then(|menu| {
                    menu.matches
                        .iter()
                        .find(|entry| entry.trigger == *trigger)
                        .cloned()
                })
                .unwrap_or_else(|| panic!("FR-033: typing /{trigger} did not offer it"))
        };

        for offered in std::iter::once(&entry.trigger).chain(entry.suggestions.iter()) {
            if !retired.contains(offered.as_str()) || triggers.contains(offered.as_str()) {
                continue;
            }
            panic!(
                "FR-033: the `/{trigger}` menu entry offers `{offered}`, a retired tool \
                 spelling that is not a slash command either - a completion list \
                 would teach the dead name back"
            );
        }
    }

    // The narrow form of the same rule: typing the prefix of a retired spelling
    // offers nothing that spells it - except for a retired *tool* spelling that is
    // also a slash command. `/bash` manages the shell allowlist and is not the
    // retired `bash` tool, so it is exempt here exactly as it is in the loop above;
    // this case pins that the exemption is `bash` and nothing else, so a second
    // retired spelling cannot quietly become a command.
    let retired_commands: BTreeSet<&str> = retired
        .iter()
        .copied()
        .filter(|before| triggers.contains(before))
        .collect();
    assert_eq!(
        retired_commands,
        BTreeSet::from(["Bash"]),
        "FR-033: the retired spellings that are also slash commands are \
         {retired_commands:?}, not just the `/bash` allowlist command"
    );
    for before in &retired {
        if triggers.contains(before) {
            continue;
        }
        let prefix: String = before.chars().take(2).collect();
        app.input = format!("/{prefix}");
        app.input_cursor = app.input.chars().count();
        app.update_slash_menu();
        if let Some(menu) = app.slash_menu.as_ref() {
            for entry in &menu.matches {
                assert_ne!(
                    entry.trigger, *before,
                    "FR-033: typing /{prefix} offered the retired spelling /{before}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4. No new completion surface
// ---------------------------------------------------------------------------

#[test]
fn test_the_prompt_field_offers_no_tool_names() {
    let (_guard, _env, temp, mut app) = isolated_app();
    // One project file whose name no tool shares, written before the picker's first
    // refresh so its project-file cache is built from a tree that contains it.
    std::fs::write(temp.path().join("zeta_probe_notes.md"), "probe\n").expect("write probe file");

    // TC-030 step 2: a bare `ba` in the prompt field opens nothing. The prompt
    // field's only completion surface is the `@` file picker, opened by `@`; a tool
    // name typed with no sigil is ordinary message text.
    app.input = "ba".to_string();
    app.input_cursor = app.input.chars().count();
    app.update_file_menu();
    assert!(
        app.file_menu.is_none(),
        "FR-033: typing `ba` opened a completion menu; the prompt field must not \
         offer tool names it does not offer today: {:?}",
        app.file_menu.as_ref().map(|menu| menu
            .matches
            .iter()
            .map(|entry| entry.display.as_str())
            .collect::<Vec<_>>())
    );

    // The `@` picker is the one surface in the field, and it completes project
    // paths. Drive it and assert every row is a path - never a registered tool name,
    // and never a bare word that would read like one.
    app.input = "@zeta".to_string();
    app.input_cursor = app.input.chars().count();
    app.update_file_menu();

    let tool_names: BTreeSet<String> = app
        .session_processor
        .tool_registry
        .list()
        .into_iter()
        .collect();
    let menu = app
        .file_menu
        .as_ref()
        .expect("FR-033: `@zeta` opens the project-file picker");
    assert!(
        !menu.matches.is_empty(),
        "FR-033: the `@zeta` picker opened with no rows, so the path rows cannot be \
         checked"
    );
    for entry in &menu.matches {
        assert!(
            !tool_names.contains(&entry.display),
            "FR-033: the `@` file picker offers {:?}, which is a registered tool \
             name - the prompt field must not acquire a tool-name completion surface",
            entry.display
        );
        if entry.display == "<back to fuzzy>" {
            continue;
        }
        // A path, not a bare identifier: the row carries a separator (`/`) or a
        // file extension, both of which a tool name such as `Bash` or `TaskCreate`
        // lacks.
        assert!(
            entry.display.contains('/') || entry.display.contains('.'),
            "FR-033: the `@` picker row {:?} is a bare word, not a project path",
            entry.display
        );
    }
}
