//! Tests for plugin-contributed slash commands in the TUI (spec `plugins`
//! FR-031): prompt commands inject their body as a user turn, inline commands
//! report that they need the plugin host, and `/help` lists both.

use ragent_agent::plugin::PluginCommand;
use ragent_tui::App;

mod support;

use support::make_app;

fn prompt_command(trigger: &str, name: &str, prompt: &str) -> PluginCommand {
    PluginCommand {
        plugin_id: "commit-commands".to_string(),
        name: name.to_string(),
        trigger: trigger.to_string(),
        description: "Create a git commit".to_string(),
        prompt: Some(prompt.to_string()),
    }
}

fn inline_command(trigger: &str, name: &str) -> PluginCommand {
    PluginCommand {
        plugin_id: "commit-commands".to_string(),
        name: name.to_string(),
        trigger: trigger.to_string(),
        description: "Runs in the sandbox".to_string(),
        prompt: None,
    }
}

fn last_text(app: &App) -> String {
    app.messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

#[tokio::test]
async fn prompt_plugin_command_is_injected_as_a_user_turn() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    app.plugin_commands = vec![prompt_command(
        "commit",
        "commit",
        "Commit the staged changes.",
    )];

    app.execute_slash_command("/commit").await;

    // The invocation is shown as a user message and the command is handled by
    // the plugin path (not reported as an unknown command).
    assert_eq!(app.status, "plugin command /commit");
    let text = last_text(&app);
    assert!(
        text.contains("/commit"),
        "user message records the invocation, got: {text}"
    );
    assert!(
        !text.contains("Unknown command"),
        "plugin command must not fall through to the unknown-command arm"
    );
}

#[tokio::test]
async fn prompt_plugin_command_substitutes_arguments_and_uses_namespaced_trigger() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    // A namespaced trigger (a colliding bare name) is what the surface holds.
    app.plugin_commands = vec![prompt_command(
        "plugin:commit-commands:commit",
        "commit",
        "Message: $ARGUMENTS",
    )];

    app.execute_slash_command("/plugin:commit-commands:commit fix the bug")
        .await;

    assert_eq!(app.status, "plugin command /plugin:commit-commands:commit");
    let text = last_text(&app);
    assert!(
        text.contains("/plugin:commit-commands:commit fix the bug"),
        "the recorded user turn echoes the trigger and args, got: {text}"
    );
}

#[tokio::test]
async fn inline_plugin_command_reports_it_needs_the_plugin_host() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    app.plugin_commands = vec![inline_command("clean_gone", "clean_gone")];

    app.execute_slash_command("/clean_gone").await;

    assert_eq!(
        app.status,
        "plugin command /clean_gone requires the plugin host"
    );
    let text = last_text(&app);
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("is an inline plugin command"),
        "inline command explains it needs the plugin host, got: {text}"
    );
}

#[tokio::test]
async fn help_lists_plugin_commands() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    app.plugin_commands = vec![prompt_command("commit", "commit", "Commit.")];

    app.execute_slash_command("/help").await;

    let text = last_text(&app);
    assert!(
        text.contains("Plugin commands:"),
        "/help must add a plugin-command section, got: {text}"
    );
    assert!(text.contains("/commit"), "/help must list the command");
}
