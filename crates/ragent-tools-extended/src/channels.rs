//! Channel messaging tool (`send_channel_message`) - JCODEPLAN M7 (T-061).
//!
//! Posts notification messages to externally configured messaging channels.
//! Currently supported channel kinds:
//!
//! - **Telegram** - via the Bot API `sendMessage` endpoint.
//! - **Discord** - via channel webhooks (`POST <webhook_url>`).
//!
//! # Configuration
//!
//! Channels are registered in `ragent.json` under the `channels` key:
//!
//! ```json
//! {
//!   "channels": {
//!     "enabled": true,
//!     "telegram": {
//!       "bot_token": "env:TELEGRAM_BOT_TOKEN",
//!       "chat_id": "env:TELEGRAM_CHAT_ID"
//!     },
//!     "discord": {
//!       "webhook_url": "env:DISCORD_WEBHOOK_URL"
//!     }
//!   }
//! }
//! ```
//!
//! Any credential value may use the `env:VAR_NAME` indirection so secrets are
//! read from environment variables at send time instead of living inside the
//! config file.
//!
//! When `channels.enabled` is `false` (the default) the tool still answers
//! `status`/`list` actions but refuses to deliver messages; when no channels
//! are configured at all it degrades honestly with actionable guidance.

use anyhow::{Context, Result, bail};
use ragent_config::ChannelsConfig;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Tool name used by the LLM.
pub const SEND_CHANNEL_MESSAGE_TOOL_NAME: &str = "send_channel_message";

const MAX_MESSAGE_BYTES: usize = 4096;
const TELEGRAM_API_BASE: &str = "https://api.telegram.org";

/// Channel messaging tool.
///
/// Implements the `send_channel_message` tool, which delivers text messages
/// to the channels configured in `ragent.json`.
#[derive(Debug, Clone, Copy)]
pub struct SendChannelMessageTool;

/// Resolved (secret-materialised) Telegram settings.
struct TelegramResolved {
    bot_token: String,
    chat_id: String,
    base_url: String,
}

/// Refuse a config-supplied outbound target unless it is usable as a public
/// endpoint.
///
/// SEC-tools-extended-008 (SECTASKS T-066). `validate_url` blocks loopback and
/// private ranges outright, which is correct for agent-supplied URLs but would
/// also break the documented local-mock/test shape for these outbound sinks, so
/// a loopback literal is allowed here (the operator configured it) while every
/// other private, link-local, metadata, and rebinding host is refused.
fn refuse_non_public_target(label: &str, url: &str) -> Result<()> {
    if target_is_allowed(url) {
        return Ok(());
    }
    match crate::masterfetch::security::validate_url(url) {
        Ok(()) => Ok(()),
        Err(e) => bail!("{label} rejected: {e}"),
    }
}

/// Whether a config-supplied outbound target may be used.
///
/// `validate_url` blocks loopback and private ranges outright, which is correct
/// for agent-supplied URLs but would also break the documented local-mock/test
/// shape for these outbound sinks. A loopback literal is therefore allowed (the
/// operator configured it); every other private, link-local, metadata, and
/// rebinding host is refused (SEC-tools-extended-008, SECTASKS T-066).
#[must_use]
pub fn target_is_allowed(url: &str) -> bool {
    use crate::masterfetch::security::SecurityError;
    if matches!(
        crate::masterfetch::security::validate_url(url),
        Ok(()) | Err(SecurityError::Localhost(_))
    ) {
        return true;
    }
    // `validate_url` reports a loopback *literal* as a private range (only a
    // hostname like `localhost` produces `Localhost`), so an explicitly
    // configured loopback IP must be recognised separately.
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    }
}

/// Whether a resolved credential has a plausible shape.
///
/// Tokens, PATs, webhook URLs, chat ids, and client ids never contain ASCII
/// whitespace or control characters. A value that does indicates the
/// `env:VAR` indirection resolved to something that is not credential material
/// (e.g. it points at `$HOME` or a multi-line value), so it is rejected rather
/// than forwarded verbatim into an authenticated request (audit T-110).
#[must_use]
pub fn is_plausible_secret(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && !trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Resolve a config credential value. Values prefixed with `env:` are read
/// from the named environment variable at send time.
///
/// An env-sourced value whose shape is not plausible credential material is
/// rejected (see [`is_plausible_secret`]) and a warning is logged naming the
/// variable - never its value.
pub fn resolve_secret(config_value: Option<&str>) -> Option<String> {
    let value = config_value?;
    if let Some(var) = value.strip_prefix("env:") {
        let var = var.trim();
        match std::env::var(var) {
            Ok(resolved) if is_plausible_secret(&resolved) => Some(resolved),
            Ok(_) => {
                tracing::warn!(
                    var = %var,
                    "env-sourced credential has an implausible shape; ignoring"
                );
                None
            }
            Err(_) => None,
        }
    } else if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

impl SendChannelMessageTool {
    /// Create a new `send_channel_message` tool instance.
    pub const fn new() -> Self {
        Self
    }

    fn channels_config(ctx: &ToolContext) -> Option<ChannelsConfig> {
        ctx.config.as_ref().map(|c| c.channels.clone())
    }

    fn resolve_telegram(channels: &ChannelsConfig) -> Option<TelegramResolved> {
        let tg = channels.telegram.as_ref()?;
        let bot_token = resolve_secret(tg.bot_token.as_deref())?;
        let chat_id = resolve_secret(tg.chat_id.as_deref())?;
        let base_url = tg
            .base_url
            .as_deref()
            .filter(|u| !u.is_empty())
            .unwrap_or(TELEGRAM_API_BASE)
            .trim_end_matches('/')
            .to_string();
        Some(TelegramResolved {
            bot_token,
            chat_id,
            base_url,
        })
    }

    fn resolve_discord(channels: &ChannelsConfig) -> Option<String> {
        resolve_secret(channels.discord.as_ref()?.webhook_url.as_deref())
    }

    /// Build the status payload describing configured channels.
    ///
    /// Never includes secret material - only booleans describing whether each
    /// channel is fully configured.
    fn status_payload(ctx: &ToolContext) -> Value {
        let Some(channels) = Self::channels_config(ctx) else {
            return json!({
                "enabled": false,
                "configured": false,
                "channels": [],
                "next_action": "Add a \"channels\" block to ragent.json (see tool description)."
            });
        };

        let mut list = Vec::new();
        if let Some(tg) = channels.telegram.as_ref() {
            list.push(json!({
                "kind": "telegram",
                "configured": Self::resolve_telegram(&channels).is_some(),
                "has_bot_token": resolve_secret(tg.bot_token.as_deref()).is_some(),
                "has_chat_id": resolve_secret(tg.chat_id.as_deref()).is_some(),
            }));
        }
        if let Some(dc) = channels.discord.as_ref() {
            list.push(json!({
                "kind": "discord",
                "configured": resolve_secret(dc.webhook_url.as_deref()).is_some(),
                "has_webhook_url": resolve_secret(dc.webhook_url.as_deref()).is_some(),
            }));
        }
        json!({
            "enabled": channels.enabled,
            "configured": !list.is_empty(),
            "channels": list,
        })
    }

    async fn send_telegram(resolved: &TelegramResolved, message: &str) -> Result<String> {
        // SEC-tools-extended-008 (SECTASKS T-066): `channels.telegram.base_url`
        // is config-supplied and was interpolated into a POST target with no
        // SSRF check, so a config could point the tool at an internal service.
        // A loopback target is the documented local-mock/test shape, so it is
        // permitted when the user has explicitly pointed the config there;
        // every other private/metadata host is refused.
        refuse_non_public_target("telegram base_url", &resolved.base_url)?;
        let url = format!(
            "{}/bot{}/sendMessage",
            resolved.base_url, resolved.bot_token
        );
        // ANTIPAT M5.9 / 3.2: reuse the shared client singleton (connection
        // pool + TLS session cache) instead of building a fresh client per send.
        let client =
            crate::masterfetch::http::shared_client().context("Failed to build HTTP client")?;

        let resp = client
            .post(&url)
            .json(&json!({
                "chat_id": resolved.chat_id,
                "text": message,
            }))
            .send()
            .await
            .with_context(|| "Failed to reach Telegram Bot API".to_string())?;

        let status = resp.status();
        // ANTIPAT M5.9 / 3.5: bound the response body before parsing instead of
        // an unbounded `resp.json()`.
        let body: Value = crate::masterfetch::http::read_body_capped_lossy(
            resp,
            crate::masterfetch::http::MAX_SMALL_BODY_BYTES,
        )
        .await
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
        if !status.is_success() || body.get("ok").and_then(Value::as_bool) == Some(false) {
            let description = body
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            bail!("Telegram send failed (HTTP {status}): {description}");
        }
        let message_id = body
            .pointer("/result/message_id")
            .and_then(Value::as_i64)
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Ok(format!("telegram message_id={message_id}"))
    }

    async fn send_discord(webhook_url: &str, message: &str) -> Result<String> {
        // SEC-tools-extended-00x (ANTIPAT 3.1): `channels.discord.webhook_url`
        // is config-supplied, and this sibling of `send_telegram` had no
        // public-target check, so a config could point the tool at an internal
        // service. Same policy as Telegram: a loopback target is permitted
        // (documented local-mock/test shape); every other private, link-local,
        // metadata, and rebinding host is refused.
        refuse_non_public_target("discord webhook_url", webhook_url)?;
        // ANTIPAT M5.9 / 3.2: reuse the shared client singleton instead of
        // building a fresh client per send.
        let client =
            crate::masterfetch::http::shared_client().context("Failed to build HTTP client")?;

        let resp = client
            .post(webhook_url)
            .json(&json!({ "content": message }))
            .send()
            .await
            .context("Failed to reach Discord webhook URL")?;

        // Discord webhooks return 204 No Content (or 200 with the message
        // object when ?wait=true) on success.
        if !resp.status().is_success() {
            let status = resp.status();
            // ANTIPAT 4.1: cap the error body (a hostile upstream can answer
            // with an arbitrarily large one).
            let body = crate::masterfetch::http::read_body_capped(
                resp,
                crate::masterfetch::http::MAX_SMALL_BODY_BYTES,
            )
            .await
            .unwrap_or_default();
            bail!("Discord webhook send failed (HTTP {status}): {body}");
        }
        Ok("discord webhook delivered".to_string())
    }
}

impl Default for SendChannelMessageTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for SendChannelMessageTool {
    fn name(&self) -> &'static str {
        SEND_CHANNEL_MESSAGE_TOOL_NAME
    }

    fn description(&self) -> &'static str {
        "Send a message to a configured external messaging channel (Telegram bot \
             or Discord webhook). Channels are configured in ragent.json under the \
             'channels' block; set channels.enabled=true to allow delivery. REQUIRED \
             parameter: 'action' (string enum send/list/status, default send). For \
             action='send', 'message' (string, max 4096 bytes) is required. Optional: \
             'channel' (telegram/discord/all) to target a specific channel instead of \
             the first configured one. Use action='list' to see configured channels and \
             action='status' to check readiness. Common gotcha: no action parameter \
             defaults to send, but 'message' is still required for that action."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform (default: send)",
                    "enum": ["send", "list", "status"]
                },
                "message": {
                    "type": "string",
                    "description": "Message text to deliver (required for send; max 4096 bytes)"
                },
                "channel": {
                    "type": "string",
                    "description": "Channel targeting: a specific kind (telegram/discord), 'all' for every configured channel, or omit for the first configured channel",
                    "enum": ["telegram", "discord", "all"]
                }
            },
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "network:send"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"].as_str().unwrap_or("send");

        match action {
            "status" | "list" => {
                let payload = Self::status_payload(ctx);
                let content = match action {
                    "list" => {
                        let channels = payload["channels"].as_array();
                        match channels {
                            Some(list) if !list.is_empty() => {
                                let kinds: Vec<String> = list
                                    .iter()
                                    .map(|c| {
                                        let kind =
                                            c["kind"].as_str().unwrap_or("unknown").to_string();
                                        let configured = c["configured"].as_bool().unwrap_or(false);
                                        format!("{kind} (configured={configured})")
                                    })
                                    .collect();
                                format!("Configured channels: {}", kinds.join(", "))
                            }
                            _ => "No channels configured. Add a \"channels\" block to ragent.json."
                                .to_string(),
                        }
                    }
                    _ => {
                        let enabled = payload["enabled"].as_bool().unwrap_or(false);
                        let configured = payload["configured"].as_bool().unwrap_or(false);
                        format!(
                            "channels.enabled={enabled}, configured={configured} - {}",
                            serde_json::to_string_pretty(&payload["channels"]).unwrap_or_default()
                        )
                    }
                };
                Ok(ToolOutput {
                    content,
                    metadata: Some(payload),
                })
            }
            "send" => {
                let message = input["message"]
                    .as_str()
                    .context("Missing required 'message' parameter for send action")?;
                if message.len() > MAX_MESSAGE_BYTES {
                    bail!(
                        "Message too long: {} bytes (max {MAX_MESSAGE_BYTES})",
                        message.len()
                    );
                }

                let Some(channels) = Self::channels_config(ctx) else {
                    bail!(
                        "No channels configured. Add a \"channels\" block to ragent.json \
                         (telegram.bot_token/chat_id and/or discord.webhook_url) and set \
                         channels.enabled=true."
                    );
                };
                if !channels.enabled {
                    bail!(
                        "Channel messaging is disabled. Set \"channels.enabled\": true in \
                         ragent.json to allow message delivery."
                    );
                }

                let target = input["channel"].as_str().unwrap_or("auto");
                let send_telegram = matches!(target, "auto" | "all" | "telegram");
                let send_discord = matches!(target, "all" | "discord")
                    || (target == "auto" && Self::resolve_telegram(&channels).is_none());

                let mut results: Vec<String> = Vec::new();
                let mut failures: Vec<String> = Vec::new();

                if send_telegram {
                    match Self::resolve_telegram(&channels) {
                        Some(resolved) => match Self::send_telegram(&resolved, message).await {
                            Ok(detail) => results.push(detail),
                            Err(e) => failures.push(format!("telegram: {e}")),
                        },
                        None => {
                            if matches!(target, "telegram") {
                                bail!(
                                    "Telegram channel is not fully configured (bot_token and chat_id required)."
                                );
                            }
                        }
                    }
                }
                if send_discord {
                    match Self::resolve_discord(&channels) {
                        Some(webhook_url) => {
                            match Self::send_discord(&webhook_url, message).await {
                                Ok(detail) => results.push(detail),
                                Err(e) => failures.push(format!("discord: {e}")),
                            }
                        }
                        None => {
                            if matches!(target, "discord") {
                                bail!(
                                    "Discord channel is not fully configured (webhook_url required)."
                                );
                            }
                        }
                    }
                }

                if results.is_empty() && failures.is_empty() {
                    bail!(
                        "No channels configured. Add telegram and/or discord settings under \
                         the \"channels\" block in ragent.json."
                    );
                }

                let mut content = String::new();
                for r in &results {
                    content.push_str(&format!("[ok] {r}\n"));
                }
                for f in &failures {
                    content.push_str(&format!("[fail] {f}\n"));
                }

                if results.is_empty() {
                    bail!("All channel deliveries failed: {}", failures.join("; "));
                }

                Ok(ToolOutput {
                    content: content.trim_end().to_string(),
                    metadata: Some(json!({
                        "action": "send",
                        "delivered": results.len(),
                        "failed": failures,
                    })),
                })
            }
            other => bail!("Unknown action: '{other}'. Supported actions: send, list, status"),
        }
    }
}
