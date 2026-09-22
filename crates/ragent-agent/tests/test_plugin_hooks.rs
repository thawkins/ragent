//! Plugin hook bridge tests (spec `plugins` FR-033): plugin-contributed hooks
//! are mapped onto the session hook engine's triggers (accepting both ragent's
//! snake_case names and the Claude PascalCase names) and merged after the
//! user's configured hooks.

use std::path::PathBuf;

use ragent_agent::hooks::{HookConfig, HookTrigger, merge_hook_configs, plugin_hook_configs};
use ragent_plugins::PluginHook;

fn plugin_hook(trigger: &str, command: &str) -> PluginHook {
    PluginHook {
        plugin_id: "guard".to_string(),
        plugin_root: PathBuf::from("/plugins/guard"),
        trigger: trigger.to_string(),
        command: command.to_string(),
        timeout_secs: None,
        matcher: None,
    }
}

#[test]
fn hook_trigger_parses_both_ragent_and_claude_spellings() {
    assert_eq!(
        HookTrigger::parse("pre_tool_use"),
        Some(HookTrigger::PreToolUse)
    );
    assert_eq!(
        HookTrigger::parse("PreToolUse"),
        Some(HookTrigger::PreToolUse)
    );
    assert_eq!(
        HookTrigger::parse("PostToolUse"),
        Some(HookTrigger::PostToolUse)
    );
    assert_eq!(
        HookTrigger::parse("post-tool-use"),
        Some(HookTrigger::PostToolUse)
    );
    assert_eq!(
        HookTrigger::parse("on_session_start"),
        Some(HookTrigger::OnSessionStart)
    );
    // Claude's own event names map onto the closest ragent trigger.
    assert_eq!(
        HookTrigger::parse("UserPromptSubmit"),
        Some(HookTrigger::OnTurnStart)
    );
    assert_eq!(HookTrigger::parse("Stop"), Some(HookTrigger::OnSessionEnd));
    assert_eq!(
        HookTrigger::parse("PreCompact"),
        Some(HookTrigger::OnCompaction)
    );
    assert_eq!(HookTrigger::parse("not-a-trigger"), None);
}

#[test]
fn plugin_hook_configs_maps_triggers_and_applies_timeout() {
    let hooks = vec![
        plugin_hook("PreToolUse", "./guard.sh"),
        PluginHook {
            timeout_secs: Some(7),
            ..plugin_hook("SessionStart", "echo start")
        },
        plugin_hook("nonsense-event", "./never.sh"),
    ];
    let configs = plugin_hook_configs(&hooks);
    assert_eq!(configs.len(), 2, "unrecognised trigger dropped");
    assert_eq!(configs[0].trigger, HookTrigger::PreToolUse);
    assert_eq!(configs[0].command, "./guard.sh");
    assert_eq!(configs[0].timeout_secs, 30, "default timeout applied");
    assert_eq!(configs[1].trigger, HookTrigger::OnSessionStart);
    assert_eq!(configs[1].timeout_secs, 7);
}

#[test]
fn merge_hook_configs_runs_configured_hooks_first() {
    let configured = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: "configured.sh".to_string(),
        timeout_secs: 30,
        plugin_root: None,
        matcher: None,
    }];
    let merged = merge_hook_configs(&configured, &[plugin_hook("PreToolUse", "plugin.sh")]);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].command, "configured.sh");
    assert_eq!(merged[1].command, "plugin.sh");
}

#[test]
fn plugin_hook_carries_root_and_matcher_through_the_bridge() {
    let mut hook = plugin_hook(
        "PostToolUse",
        "bash \"${CLAUDE_PLUGIN_ROOT}/hooks/check.py\"",
    );
    hook.matcher = Some("Edit|Write|MultiEdit".to_string());
    let configs = plugin_hook_configs(&[hook]);
    assert_eq!(configs.len(), 1);
    assert_eq!(
        configs[0].plugin_root.as_deref(),
        Some(std::path::Path::new("/plugins/guard"))
    );
    assert_eq!(configs[0].matcher.as_deref(), Some("Edit|Write|MultiEdit"));
}

#[test]
fn hook_matcher_filters_by_tool_name_and_bash_guard() {
    let scoped = HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: "check.sh".to_string(),
        timeout_secs: 30,
        plugin_root: None,
        matcher: Some("Edit|Write|MultiEdit".to_string()),
    };
    assert!(scoped.matches_tool("Edit", r#"{"file_path":"a.rs"}"#));
    assert!(scoped.matches_tool("Write", r#"{"file_path":"a.rs"}"#));
    assert!(!scoped.matches_tool("Bash", r#"{"command":"ls"}"#));

    let bash = HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: "review.sh".to_string(),
        timeout_secs: 30,
        plugin_root: None,
        matcher: Some("Bash(git commit:*)".to_string()),
    };
    assert!(bash.matches_tool("Bash", r#"{"command":"git commit -m x"}"#));
    assert!(!bash.matches_tool("Bash", r#"{"command":"git push"}"#));
    assert!(!bash.matches_tool("Edit", r#"{"command":"git commit -m x"}"#));

    // No matcher matches every tool.
    let all = HookConfig {
        plugin_root: None,
        matcher: None,
        ..scoped.clone()
    };
    assert!(all.matches_tool("anything", "{}"));
}

#[tokio::test]
async fn post_tool_use_hook_additional_context_is_collected() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let script = dir.path().join("guidance.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s' '{\"hookSpecificOutput\":{\"additionalContext\":\"WARNING: yaml.load\"}}'\nexit 0\n",
    )
    .expect("write");
    let configs = vec![HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: format!("sh {}", script.display()),
        timeout_secs: 30,
        plugin_root: None,
        matcher: None,
    }];
    let result = ragent_agent::hooks::run_post_tool_use_hooks(
        &configs,
        dir.path(),
        "edit",
        r#"{"path":"a.py"}"#,
        r#"{"content":"ok"}"#,
        true,
        "sess-guidance",
        None,
    )
    .await;
    match result {
        ragent_agent::hooks::PostToolUseResult::Ok {
            additional_context, ..
        } => {
            assert_eq!(additional_context, vec!["WARNING: yaml.load".to_string()]);
        }
        other => panic!("expected Ok with guidance, got {other:?}"),
    }
}

#[tokio::test]
async fn stop_hook_blocking_collects_stdout_and_stderr_findings() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let script = dir.path().join("stop.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s' '{\"decision\":\"block\",\"reason\":\"hardcoded secret\"}'\nprintf '%s' 'plus stderr context' >&2\nexit 2\n",
    )
    .expect("write");
    let configs = vec![HookConfig {
        trigger: HookTrigger::OnSessionEnd,
        command: format!("sh {}", script.display()),
        timeout_secs: 30,
        plugin_root: None,
        matcher: None,
    }];
    let guidance = ragent_agent::hooks::run_stop_hooks(&configs, dir.path(), "sess-stop").await;
    assert_eq!(guidance, vec!["hardcoded secret".to_string()]);
}

#[tokio::test]
async fn stop_hook_without_findings_does_not_continue() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let script = dir.path().join("quiet.sh");
    std::fs::write(&script, "#!/bin/sh\ncat >/dev/null\nexit 0\n").expect("write");
    let configs = vec![HookConfig {
        trigger: HookTrigger::OnSessionEnd,
        command: format!("sh {}", script.display()),
        timeout_secs: 30,
        plugin_root: None,
        matcher: None,
    }];
    let guidance = ragent_agent::hooks::run_stop_hooks(&configs, dir.path(), "sess-stop").await;
    assert!(guidance.is_empty(), "a quiet Stop hook ends the turn");
}

#[tokio::test]
async fn stop_hook_additional_context_is_collected() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let script = dir.path().join("ctx.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\ncat >/dev/null\nprintf '%s' '{\"hookSpecificOutput\":{\"additionalContext\":\"review finding\"}}'\nexit 0\n",
    )
    .expect("write");
    let configs = vec![HookConfig {
        trigger: HookTrigger::OnSessionEnd,
        command: format!("sh {}", script.display()),
        timeout_secs: 30,
        plugin_root: None,
        matcher: None,
    }];
    let guidance = ragent_agent::hooks::run_stop_hooks(&configs, dir.path(), "sess-stop").await;
    assert_eq!(guidance, vec!["review finding".to_string()]);
}
