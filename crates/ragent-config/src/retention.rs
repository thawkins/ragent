//! Session-message retention configuration.
//!
//! These types define the `storage` section of ragent's configuration file.
//! Retention is **opt-in**: with no `storage` section, no message is ever
//! expired, and the transcript grows without bound.

use serde::{Deserialize, Serialize};

/// Default number of days a message is kept once retention is enabled.
pub const DEFAULT_MESSAGE_DAYS: u32 = 14;

/// Top-level storage configuration.
///
/// Corresponds to the `storage` key in `ragent.json`.
///
/// ```json
/// { "storage": { "retention": { "enabled": true, "message_days": 14 } } }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Message-age retention settings.
    #[serde(default)]
    pub retention: RetentionConfig,
}

impl StorageConfig {
    /// `true` when the section is at its default state (retention disabled).
    ///
    /// Used by `Config` serialisation to omit the `storage` key while the
    /// feature is disabled, so a never-toggled config file stays unchanged in
    /// intent.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.retention.is_default()
    }
}

/// Message-age retention settings.
///
/// When enabled, every message whose `created_at` is older than
/// [`RetentionConfig::message_days`] is deleted from the session store. The
/// session rows themselves are kept, so an old conversation remains listed
/// and resumable with an empty transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Whether age-based message expiry runs at startup. Default: `false`.
    #[serde(default)]
    pub enabled: bool,
    /// Days a message is retained before expiry. Default: 14.
    ///
    /// Must be at least 1; a zero-day window would delete every message on
    /// every start, so the loader rejects it rather than silently emptying the
    /// store.
    #[serde(default = "default_message_days")]
    pub message_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            message_days: DEFAULT_MESSAGE_DAYS,
        }
    }
}

impl RetentionConfig {
    /// `true` when the section is at its default state (disabled, 14 days).
    #[must_use]
    pub fn is_default(&self) -> bool {
        !self.enabled && self.message_days == DEFAULT_MESSAGE_DAYS
    }

    /// Validate the retention settings and return a list of any problems.
    ///
    /// A zero-day window would delete every message on every start, so it is
    /// rejected rather than silently emptying the store.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        if self.message_days == 0 {
            vec![
                "storage.retention.message_days must be >= 1 (a zero-day window would \
                 delete every message on every start)"
                    .to_string(),
            ]
        } else {
            Vec::new()
        }
    }
}

fn default_message_days() -> u32 {
    DEFAULT_MESSAGE_DAYS
}
