//! Tests for the hooks lifecycle system.

use ragent_core::config::Config;
use ragent_core::hooks::{HookConfig, HookTrigger, fire_hooks};
use std::path::Path;

#[test]
fn test_hook_trigger_display() {
    assert_eq!(HookTrigger::OnSessionStart.to_string(), "on_session_start");
    assert_eq!(HookTrigger::OnSessionEnd.to_string(), "on_session_end");
    assert_eq!(HookTrigger::OnError.to_string(), "on_error");
    assert_eq!(
        HookTrigger::OnPermissionDenied.to_string(),
        "on_permission_denied"
    );
}

#[test]
fn test_hook_trigger_serde_round_trip() {
    let json = r#""on_session_start""#;
    let trigger: HookTrigger = serde_json::from_str(json).unwrap();
    assert_eq!(trigger, HookTrigger::OnSessionStart);

    let serialized = serde_json::to_string(&trigger).unwrap();
    assert_eq!(serialized, json);
}

#[test]
fn test_hook_config_deserialize() {
    let json = r#"{
        "trigger": "on_error",
        "command": "echo error"
    }"#;
    let hook: HookConfig = serde_json::from_str(json).unwrap();
    assert_eq!(hook.trigger, HookTrigger::OnError);
    assert_eq!(hook.command, "echo error");
    assert_eq!(hook.timeout_secs, 30); // default
}

#[test]
fn test_hook_config_timeout_override() {
    let json = r#"{
        "trigger": "on_session_end",
        "command": "echo done",
        "timeout_secs": 60
    }"#;
    let hook: HookConfig = serde_json::from_str(json).unwrap();
    assert_eq!(hook.timeout_secs, 60);
}

#[test]
fn test_config_hooks_field_default() {
    let config = Config::default();
    assert!(config.hooks.is_empty());
}

#[test]
fn test_config_hooks_deserialize() {
    let json = r#"{
        "hooks": [
            {
                "trigger": "on_session_start",
                "command": "echo started"
            },
            {
                "trigger": "on_error",
                "command": "echo error: $RAGENT_ERROR",
                "timeout_secs": 10
            }
        ]
    }"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.hooks.len(), 2);
    assert_eq!(config.hooks[0].trigger, HookTrigger::OnSessionStart);
    assert_eq!(config.hooks[1].trigger, HookTrigger::OnError);
    assert_eq!(config.hooks[1].timeout_secs, 10);
}

#[test]
fn test_config_merge_hooks_append() {
    let mut base = Config::default();
    base.hooks.push(HookConfig {
        trigger: HookTrigger::OnSessionStart,
        command: "echo base".to_string(),
        timeout_secs: 30,
    });

    let mut overlay = Config::default();
    overlay.hooks.push(HookConfig {
        trigger: HookTrigger::OnSessionEnd,
        command: "echo overlay".to_string(),
        timeout_secs: 30,
    });

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.hooks.len(), 2);
    assert_eq!(merged.hooks[0].trigger, HookTrigger::OnSessionStart);
    assert_eq!(merged.hooks[1].trigger, HookTrigger::OnSessionEnd);
}

#[test]
fn test_fire_hooks_empty_noop() {
    // Should not panic with an empty hooks list
    fire_hooks(&[], HookTrigger::OnSessionStart, Path::new("/tmp"), &[]);
}

#[test]
fn test_fire_hooks_no_matching_trigger_noop() {
    let hooks = vec![HookConfig {
        trigger: HookTrigger::OnSessionEnd,
        command: "echo end".to_string(),
        timeout_secs: 30,
    }];
    // Should return without spawning since trigger doesn't match
    fire_hooks(&hooks, HookTrigger::OnSessionStart, Path::new("/tmp"), &[]);
}

#[tokio::test]
async fn test_fire_hooks_runs_matching_command() {
    use tokio::time::{Duration, sleep};

    // Write to a temp file to verify command executed
    let output_path = std::env::temp_dir().join("ragent_hook_test_output.txt");
    let _ = std::fs::remove_file(&output_path);

    let cmd = format!("echo hook_ran > {}", output_path.display());
    let hooks = vec![HookConfig {
        trigger: HookTrigger::OnSessionStart,
        command: cmd,
        timeout_secs: 10,
    }];

    fire_hooks(&hooks, HookTrigger::OnSessionStart, Path::new("."), &[]);

    // Give the spawned task time to complete
    sleep(Duration::from_millis(500)).await;

    let contents = std::fs::read_to_string(&output_path).unwrap_or_default();
    assert!(
        contents.trim() == "hook_ran",
        "Expected 'hook_ran', got: {:?}",
        contents
    );
    let _ = std::fs::remove_file(&output_path);
}
