//! Trigger system configuration (spec `piegap` FR-002, FR-003).
//!
//! Configuration for the dynamic trigger rule system — poll interval, feature
//! gate, and maximum rules per session. Also defines the MCP notification
//! injection mode (`inject_summary` / `inject_and_run`) used by the MCP
//! notification push-event adapter (FR-003).
//!
//! Read from the `trigger` block in `ragent.json` following the standard
//! configuration discovery pattern (FR-018).

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the dynamic trigger rule system.
///
/// Loaded from the `"trigger"` block in `ragent.json`. All fields default
/// sensibly so the feature works out-of-the-box without explicit configuration.
///
/// ```jsonc
/// {
///   "trigger": {
///     "enabled": true,
///     "poll_interval_secs": 30,
///     "max_rules": 32
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    /// Master feature gate for the trigger system. When `false`, all trigger
    /// functionality no-ops cleanly (FR-016). Default: `true`.
    #[serde(default = "default_trigger_enabled")]
    pub enabled: bool,
    /// Interval at which dynamic trigger rules poll their conditions.
    /// Default: 30 seconds (FR-002 "configurable interval").
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// Maximum number of dynamic trigger rules allowed per session. Prevents
    /// unbounded rule accumulation. Default: 32.
    #[serde(default = "default_max_rules")]
    pub max_rules: usize,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            enabled: default_trigger_enabled(),
            poll_interval_secs: default_poll_interval_secs(),
            max_rules: default_max_rules(),
        }
    }
}

impl TriggerConfig {
    /// Returns the poll interval as a `Duration`.
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }

    /// Returns `true` if the trigger system is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns `true` when the config is at default values (nothing configured).
    pub fn is_empty(&self) -> bool {
        self.enabled == default_trigger_enabled()
            && self.poll_interval_secs == default_poll_interval_secs()
            && self.max_rules == default_max_rules()
    }
}

fn default_trigger_enabled() -> bool {
    true
}

fn default_poll_interval_secs() -> u64 {
    30
}

fn default_max_rules() -> usize {
    32
}

// ── MCP notification injection mode (FR-003) ─────────────────────────────

/// How an MCP server's push notifications should be injected into the parent
/// session (FR-003).
///
/// Configured per MCP server via the `notification` field in `McpServerConfig`.
/// When an MCP server pushes a notification frame, the adapter normalizes it
/// into a trigger envelope and routes it through the trigger runtime. The
/// injection mode determines what happens after dedup/cycle suppression
/// allows the envelope through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpNotificationMode {
    /// Notifications from this server are ignored (default).
    #[default]
    None,
    /// Inject a bounded summary into the parent chat without a model call
    /// (FR-003 `inject_summary`).
    InjectSummary,
    /// Inject a prompt and run one model turn in the parent's full tool
    /// context (FR-003 `inject_and_run`).
    InjectAndRun,
}

impl McpNotificationMode {
    /// Returns `true` when this mode is `None` (no notification handling).
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns `true` when this mode is `InjectSummary`.
    #[must_use]
    pub fn is_inject_summary(&self) -> bool {
        matches!(self, Self::InjectSummary)
    }

    /// Returns `true` when this mode is `InjectAndRun`.
    #[must_use]
    pub fn is_inject_and_run(&self) -> bool {
        matches!(self, Self::InjectAndRun)
    }
}
