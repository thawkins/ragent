//! Automation service configuration (spec `openhands` T-016; FR-013, FR-014,
//! FR-033).
//!
//! Loaded from the optional `"automation"` block in `ragent.json`. The block
//! declares each automation (a named, schedulable or webhook-triggered agent
//! run), its selected execution backend, and optional third-party dispatch
//! targets. `enabled: false` makes the whole subsystem inert: no scheduler tick,
//! no webhook ingress, no run history.
//!
//! ```jsonc
//! {
//!   "automation": {
//!     "enabled": true,             // master switch; default true
//!     "run_history_cap": 500,      // per-automation run records retained
//!     "scheduler_tick_secs": 30,   // scheduler tick interval
//!     "automations": [
//!       {
//!         "id": "on-issue",        // unique id (also the webhook URL segment)
//!         "agent": "general",      // agent to run
//!         "prompt": "Triage issue {{payload}}",
//!         "trigger": { "kind": "webhook" },        // or { "kind": "schedule", "schedule": "every 2m" }
//!         "backend": "local",      // execution backend the run is confined to
//!         "dispatch": [
//!           { "kind": "slack", "channel": "alerts", "token_env": "SLACK_BOT_TOKEN" }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! # Secret safety
//!
//! A dispatch target names the environment variable its credential is read from
//! (`token_env`); the credential value is never stored in the config file
//! (FR-035). A `slack`/`linear`/`notion` target without a `token_env` (or a
//! `github`/`webhook` target without a `url`) is inert, not an error.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Automation service configuration (FR-013, FR-014, FR-018, FR-033).
///
/// Every field uses `serde(default)` so a partial `automation` block overlays
/// the compiled defaults; the whole section is omitted from serialisation when
/// the value is `None` on the parent [`Config`](crate::Config).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutomationConfig {
    /// Master switch for the automation service. When `false`, the scheduler
    /// does not tick, the webhook ingress refuses every request, and no run
    /// history is written. Default: `true`.
    #[serde(default = "crate::config::default_true")]
    pub enabled: bool,
    /// Maximum number of run-history records retained per automation. Older
    /// records are pruned when the cap is exceeded. Default: 500.
    #[serde(default = "default_run_history_cap")]
    pub run_history_cap: usize,
    /// Scheduler tick interval in seconds. Due automations are evaluated once
    /// per tick. Default: 30.
    #[serde(default = "default_scheduler_tick_secs")]
    pub scheduler_tick_secs: u64,
    /// The registered automations, in declaration order. Duplicate ids are
    /// rejected when the service is built.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub automations: Vec<AutomationDefinition>,
    /// Optional override of the directory holding per-automation run output.
    /// When `None`, output is written under `<working_dir>/log/automation/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<PathBuf>,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            enabled: crate::config::default_true(),
            run_history_cap: default_run_history_cap(),
            scheduler_tick_secs: default_scheduler_tick_secs(),
            automations: Vec::new(),
            output_dir: None,
        }
    }
}

impl AutomationConfig {
    /// Returns `true` when the automation service is enabled (FR-013, FR-014).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Look up an automation definition by id (case-insensitive).
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AutomationDefinition> {
        self.automations
            .iter()
            .find(|a| a.id.eq_ignore_ascii_case(id))
    }

    /// Every enabled automation definition.
    pub fn enabled_definitions(&self) -> impl Iterator<Item = &AutomationDefinition> {
        self.automations.iter().filter(|a| a.enabled)
    }
}

/// A named, schedulable or webhook-triggered agent run (FR-013, FR-014).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutomationDefinition {
    /// Unique identifier; also the webhook URL segment (`/auto/<id>`).
    pub id: String,
    /// Display name (defaults to the id when omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The agent to run. Defaults to the configured `default_agent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The prompt template. `{{payload}}` is substituted with the trigger
    /// payload (the webhook JSON body, rendered as text). Defaults to an empty
    /// prompt.
    #[serde(default)]
    pub prompt: String,
    /// How the run is triggered.
    pub trigger: AutomationTriggerKind,
    /// The execution backend the run is confined to (FR-033). `None` resolves
    /// to the session's configured backend (which defaults to `local`,
    /// FR-019).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Third-party dispatch targets notified when the run reaches a terminal
    /// state. Empty means no dispatch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dispatch: Vec<DispatchTarget>,
    /// Whether the automation is active. Default: `true`.
    #[serde(default = "crate::config::default_true")]
    pub enabled: bool,
}

impl AutomationDefinition {
    /// The display name (`name`, falling back to the id).
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }

    /// The execution backend this run is confined to (`backend`, or `"local"`
    /// when unset) (FR-033).
    #[must_use]
    pub fn backend_label(&self) -> &str {
        self.backend.as_deref().unwrap_or("local")
    }
}

/// How an [`AutomationDefinition`] is triggered (FR-013, FR-014).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutomationTriggerKind {
    /// Fired by an inbound webhook POST to `/auto/<id>` (FR-013).
    Webhook,
    /// Fired when the scheduler reaches the due time (FR-014). Uses the same
    /// schedule grammar as the cron tool (`every 2m`, `at <ts>`, ...).
    Schedule {
        /// The schedule expression, parsed with `ragent_types::parse_schedule`.
        schedule: String,
    },
}

impl AutomationTriggerKind {
    /// The stable trigger label used in run-history records (`webhook` /
    /// `schedule`) (FR-018).
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::Schedule { .. } => "schedule",
        }
    }

    /// The schedule expression, when this is a schedule trigger.
    #[must_use]
    pub fn schedule(&self) -> Option<&str> {
        match self {
            Self::Schedule { schedule } => Some(schedule.as_str()),
            Self::Webhook => None,
        }
    }
}

/// A third-party dispatch target notified on run completion (FR-013).
///
/// The service renders a provider-shaped JSON payload for the kind and POSTs it
/// to the resolved URL. Credentials are named, never stored (FR-035).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchTarget {
    /// The dispatch kind (`slack`, `github`, `linear`, `notion`, or `webhook`
    /// for a generic JSON POST).
    pub kind: String,
    /// An explicit endpoint URL. Required for `webhook` and an override for
    /// `github` (a repository issues endpoint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The name of the environment variable holding the bearer token. The value
    /// is read from the environment at dispatch time and never persisted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
    /// A channel / repository / project identifier the provider payload names
    /// (e.g. a Slack channel, a GitHub `owner/repo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl DispatchTarget {
    /// The dispatch kind, lowercased for comparison.
    #[must_use]
    pub fn kind_lower(&self) -> String {
        self.kind.trim().to_ascii_lowercase()
    }
}

/// Default per-automation run-history cap.
const fn default_run_history_cap() -> usize {
    500
}

/// Default scheduler tick interval in seconds.
const fn default_scheduler_tick_secs() -> u64 {
    30
}
