//! Tests for the `send_channel_message` tool — JCODEPLAN M7 (T-061/T-062).
//!
//! Covers:
//! - Tool identity (name, permission category, description)
//! - Parameters schema (actions, message, channel targeting)
//! - Config parsing for the `channels` block
//! - `env:` secret indirection resolution
//! - Graceful degradation without config / when disabled
//! - Mocked Telegram Bot API delivery (axum test server)
//! - Mocked Discord webhook delivery (axum test server)

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::post;
use ragent_config::{ChannelsConfig, DiscordChannelConfig, TelegramChannelConfig};
use ragent_tools_extended::channels::{SEND_CHANNEL_MESSAGE_TOOL_NAME, SendChannelMessageTool};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Test context helpers
// ---------------------------------------------------------------------------

fn ctx_no_config() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::path::PathBuf::from("."),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

fn ctx_with_channels(channels: ChannelsConfig) -> ToolContext {
    let mut config = ragent_config::Config::default();
    config.channels = channels;
    ToolContext {
        config: Some(Arc::new(config)),
        ..ctx_no_config()
    }
}

// ---------------------------------------------------------------------------
// Tool identity
// ---------------------------------------------------------------------------

#[test]
fn test_tool_name() {
    let tool = SendChannelMessageTool::new();
    assert_eq!(tool.name(), SEND_CHANNEL_MESSAGE_TOOL_NAME);
    assert_eq!(tool.name(), "send_channel_message");
}

#[test]
fn test_permission_category_is_network_send() {
    let tool = SendChannelMessageTool::new();
    assert_eq!(tool.permission_category(), "network:send");
}

#[test]
fn test_description_mentions_channels() {
    let tool = SendChannelMessageTool::new();
    let desc = tool.description();
    assert!(desc.contains("Telegram"));
    assert!(desc.contains("Discord"));
    assert!(desc.contains("channels"));
}

// ---------------------------------------------------------------------------
// Parameters schema
// ---------------------------------------------------------------------------

#[test]
fn test_schema_has_actions_and_fields() {
    let tool = SendChannelMessageTool::new();
    let schema = tool.parameters_schema();
    let actions = schema
        .pointer("/properties/action/enum")
        .and_then(Value::as_array)
        .expect("action enum should exist");
    assert_eq!(actions.len(), 3);
    assert!(schema.pointer("/properties/message").is_some());
    assert!(schema.pointer("/properties/channel").is_some());
    let channel_enum = schema
        .pointer("/properties/channel/enum")
        .and_then(Value::as_array)
        .expect("channel enum should exist");
    assert!(channel_enum.contains(&json!("telegram")));
    assert!(channel_enum.contains(&json!("discord")));
    assert!(channel_enum.contains(&json!("all")));
}

// ---------------------------------------------------------------------------
// Config parsing
// ---------------------------------------------------------------------------

#[test]
fn test_channels_config_defaults() {
    let cfg = ChannelsConfig::default();
    assert!(!cfg.enabled);
    assert!(cfg.telegram.is_none());
    assert!(cfg.discord.is_none());
    assert!(cfg.is_empty());
}

#[test]
fn test_channels_config_parses_full_block() {
    let raw = json!({
        "channels": {
            "enabled": true,
            "telegram": {
                "bot_token": "env:UNSET_TEST_TG_TOKEN_XYZ",
                "chat_id": "-100123"
            },
            "discord": {
                "webhook_url": "https://discord.com/api/webhooks/123/abc"
            }
        }
    });
    let cfg: ragent_config::Config = serde_json::from_value(raw).expect("config should parse");
    assert!(cfg.channels.enabled);
    let tg = cfg
        .channels
        .telegram
        .as_ref()
        .expect("telegram should be present");
    assert_eq!(tg.bot_token.as_deref(), Some("env:UNSET_TEST_TG_TOKEN_XYZ"));
    assert_eq!(tg.chat_id.as_deref(), Some("-100123"));
    let dc = cfg
        .channels
        .discord
        .as_ref()
        .expect("discord should be present");
    assert_eq!(
        dc.webhook_url.as_deref(),
        Some("https://discord.com/api/webhooks/123/abc")
    );
    assert!(!cfg.channels.is_empty());
}

#[test]
fn test_channels_config_round_trip_skips_empty() {
    let cfg = ragent_config::Config::default();
    let v = serde_json::to_value(&cfg).expect("serialise");
    assert!(v.get("channels").is_none(), "empty channels not serialised");
}

#[test]
fn test_channels_merge_overlay_wins() {
    let base = ChannelsConfig {
        discord: Some(DiscordChannelConfig {
            webhook_url: Some("https://base.example/webhook".into()),
        }),
        ..Default::default()
    };
    let overlay = ChannelsConfig {
        enabled: true,
        telegram: Some(TelegramChannelConfig {
            bot_token: Some("tok".into()),
            chat_id: Some("chat".into()),
            base_url: None,
        }),
        ..Default::default()
    };

    let mut base_cfg = ragent_config::Config::default();
    base_cfg.channels = base;
    let mut overlay_cfg = ragent_config::Config::default();
    overlay_cfg.channels = overlay;
    let merged = ragent_config::Config::merge(base_cfg, overlay_cfg);
    assert!(merged.channels.enabled);
    assert!(merged.channels.telegram.is_some());
    // Discord preserved from base.
    assert!(merged.channels.discord.is_some());
}

// ---------------------------------------------------------------------------
// Env indirection
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_secret_env_indirection() {
    use ragent_tools_extended::channels::resolve_secret;
    // Not set → None.
    assert!(resolve_secret(Some("env:DEFINITELY_UNSET_RAGENT_TEST_VAR")).is_none());
    // Plain value passes through.
    assert_eq!(
        resolve_secret(Some("plain-value")).as_deref(),
        Some("plain-value")
    );
    // Empty → None.
    assert!(resolve_secret(Some("")).is_none());
    assert!(resolve_secret(None).is_none());
}

// ---------------------------------------------------------------------------
// Graceful degradation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_status_without_config() {
    let tool = SendChannelMessageTool::new();
    let ctx = ctx_no_config();
    let out = tool
        .execute(json!({"action": "status"}), &ctx)
        .await
        .expect("status should not fail");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["enabled"], false);
    assert_eq!(meta["configured"], false);
    assert!(meta.get("next_action").is_some());
}

#[tokio::test]
async fn test_send_without_config_fails_honestly() {
    let tool = SendChannelMessageTool::new();
    let ctx = ctx_no_config();
    let err = tool
        .execute(json!({"action": "send", "message": "hi"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("channels"));
}

#[tokio::test]
async fn test_send_when_disabled_fails() {
    let tool = SendChannelMessageTool::new();
    let channels = ChannelsConfig {
        discord: Some(DiscordChannelConfig {
            webhook_url: Some("https://discord.com/api/webhooks/1/x".into()),
        }),
        ..Default::default()
    };
    let ctx = ctx_with_channels(channels);
    let err = tool
        .execute(json!({"action": "send", "message": "hi"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("disabled"));
}

#[tokio::test]
async fn test_send_requires_message() {
    let tool = SendChannelMessageTool::new();
    let ctx = ctx_no_config();
    let err = tool
        .execute(json!({"action": "send"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("message"));
}

#[tokio::test]
async fn test_unknown_action_fails() {
    let tool = SendChannelMessageTool::new();
    let ctx = ctx_no_config();
    let err = tool
        .execute(json!({"action": "explode"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Unknown action"));
}

// ---------------------------------------------------------------------------
// Mocked backends
// ---------------------------------------------------------------------------

/// Spawn a mock server that records request bodies and returns canned
/// responses. Returns (base_url, captured_bodies).
async fn spawn_mock_server(telegram: bool) -> (String, Arc<Mutex<Vec<(String, Value)>>>) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured2 = Arc::clone(&captured);

    let app = if telegram {
        Router::new().route(
            "/bot{token}/sendMessage",
            post(
                move |path: axum::extract::Path<String>, body: axum::Json<Value>| {
                    let captured = Arc::clone(&captured2);
                    async move {
                        captured
                            .lock()
                            .unwrap()
                            .push((format!("/bot{}/sendMessage", path.0), body.0));
                        axum::Json(json!({
                            "ok": true,
                            "result": { "message_id": 42 }
                        }))
                    }
                },
            ),
        )
    } else {
        Router::new().route(
            "/api/webhooks/{id}/{token}",
            post(
                move |path: axum::extract::Path<(String, String)>, body: axum::Json<Value>| {
                    let captured = Arc::clone(&captured2);
                    async move {
                        captured
                            .lock()
                            .unwrap()
                            .push((format!("/api/webhooks/{}/{}", path.0.0, path.0.1), body.0));
                        (axum::http::StatusCode::NO_CONTENT, "")
                    }
                },
            ),
        )
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (format!("http://{addr}"), captured)
}

#[tokio::test]
async fn test_send_telegram_via_mock() {
    let (base_url, captured) = spawn_mock_server(true).await;
    let channels = ChannelsConfig {
        enabled: true,
        telegram: Some(TelegramChannelConfig {
            bot_token: Some("test-token".into()),
            chat_id: Some("chat-1".into()),
            base_url: Some(base_url),
        }),
        discord: None,
    };
    let ctx = ctx_with_channels(channels);
    let tool = SendChannelMessageTool::new();
    let out = tool
        .execute(
            json!({"action": "send", "message": "deployed", "channel": "telegram"}),
            &ctx,
        )
        .await
        .expect("send should succeed");
    assert!(out.content.contains("telegram"));
    assert!(out.content.contains("42"));

    let captured = captured.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].0, "/bottest-token/sendMessage");
    assert_eq!(captured[0].1["chat_id"], "chat-1");
    assert_eq!(captured[0].1["text"], "deployed");
}

#[tokio::test]
async fn test_send_discord_via_mock() {
    let (base_url, captured) = spawn_mock_server(false).await;
    let channels = ChannelsConfig {
        enabled: true,
        telegram: None,
        discord: Some(DiscordChannelConfig {
            webhook_url: Some(format!("{base_url}/api/webhooks/999/tok")),
        }),
    };
    let ctx = ctx_with_channels(channels);
    let tool = SendChannelMessageTool::new();
    let out = tool
        .execute(json!({"action": "send", "message": "hello discord"}), &ctx)
        .await
        .expect("send should succeed");
    assert!(out.content.contains("discord"));

    let captured = captured.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].0, "/api/webhooks/999/tok");
    assert_eq!(captured[0].1["content"], "hello discord");
}

#[tokio::test]
async fn test_send_all_channels_fanout() {
    let (tg_base, tg_captured) = spawn_mock_server(true).await;
    let (dc_base, dc_captured) = spawn_mock_server(false).await;
    let channels = ChannelsConfig {
        enabled: true,
        telegram: Some(TelegramChannelConfig {
            bot_token: Some("t".into()),
            chat_id: Some("c".into()),
            base_url: Some(tg_base),
        }),
        discord: Some(DiscordChannelConfig {
            webhook_url: Some(format!("{dc_base}/api/webhooks/1/x")),
        }),
    };
    let ctx = ctx_with_channels(channels);
    let tool = SendChannelMessageTool::new();
    let out = tool
        .execute(
            json!({"action": "send", "message": "both", "channel": "all"}),
            &ctx,
        )
        .await
        .expect("send should succeed");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["delivered"], 2);
    assert_eq!(tg_captured.lock().unwrap().len(), 1);
    assert_eq!(dc_captured.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_send_targeting_missing_specific_channel_fails() {
    let tool = SendChannelMessageTool::new();
    let channels = ChannelsConfig {
        enabled: true,
        telegram: None,
        discord: Some(DiscordChannelConfig {
            webhook_url: Some("https://discord.com/api/webhooks/1/x".into()),
        }),
    };
    let ctx = ctx_with_channels(channels);
    let err = tool
        .execute(
            json!({"action": "send", "message": "hi", "channel": "telegram"}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Telegram"));
}

#[tokio::test]
async fn test_message_length_cap() {
    let tool = SendChannelMessageTool::new();
    let channels = ChannelsConfig {
        enabled: true,
        telegram: Some(TelegramChannelConfig {
            bot_token: Some("t".into()),
            chat_id: Some("c".into()),
            base_url: None,
        }),
        discord: None,
    };
    let ctx = ctx_with_channels(channels);
    let long = "x".repeat(5000);
    let err = tool
        .execute(json!({"action": "send", "message": long}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("too long"));
}

#[test]
fn test_registered_in_extended_registry() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(registry.contains("send_channel_message"));
}
