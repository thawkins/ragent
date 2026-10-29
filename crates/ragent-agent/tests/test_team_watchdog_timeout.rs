//! FR-013 / T-022: `TeamSettings.watchdog_timeout_secs` sets how long a live
//! teammate may go without progress before the watchdog fails it, and the
//! watchdog's tick interval is derived from that same value.
//!
//! The tests pin the three surfaces the setting owns:
//!
//! 1. the declared default (300 seconds, the operator remedy for F-09);
//! 2. the round trip through `config.json` and the `0` fallback, so an
//!    operator's value is what the manager sees;
//! 3. the derivation itself - [`TeamManager::watchdog_timeout`] seeds from the
//!    team's settings at construction, and the tick interval the watchdog uses
//!    is half that window capped at one minute, so widening the window never
//!    widens the detection latency.
//!
//! The watchdog's *verdict* semantics (which member is eligible, what it
//! publishes) are covered by `test_watchdog_status_branches.rs`; this file
//! covers the value those verdicts are measured against.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::team::manager::TeamManager;
use ragent_agent::team::{
    DEFAULT_WATCHDOG_TIMEOUT_SECS, TeamSettings, TeamStore, watchdog_check_interval,
};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::{stub_processor, team_manager};

/// The declared default is 300 seconds (FR-013; plan question Q-3).
#[test]
fn test_watchdog_timeout_defaults_to_300_seconds() {
    assert_eq!(
        DEFAULT_WATCHDOG_TIMEOUT_SECS, 300,
        "FR-013: the documented no-progress window is 300 seconds"
    );
    assert_eq!(
        TeamSettings::default().watchdog_timeout_secs,
        300,
        "FR-013: a team created without the field gets the 300-second default"
    );
}

/// A configured value survives a save/load cycle, and `0` resolves to the
/// default rather than arming a zero-length window.
#[test]
fn test_watchdog_timeout_round_trips_and_zero_falls_back() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let store = TeamStore::create("rt-team", "lead-1", &workspace, true).expect("create team");
    let team_dir = store.dir.clone();
    drop(store);

    // A config written by hand, as an operator would: only the one field.
    let write = |value: serde_json::Value| {
        let mut json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(team_dir.join("config.json")).unwrap())
                .expect("parse config");
        json["settings"]["watchdog_timeout_secs"] = value;
        std::fs::write(
            team_dir.join("config.json"),
            serde_json::to_string_pretty(&json).expect("serialise"),
        )
        .expect("write config");
    };

    write(serde_json::json!(45));
    let loaded = TeamStore::load(&team_dir).expect("load team");
    assert_eq!(
        loaded.config.settings.watchdog_timeout_secs, 45,
        "FR-013: a configured window must survive a load"
    );
    assert_eq!(
        loaded.config.settings.effective_watchdog_timeout_secs(),
        45,
        "a positive window is used as written"
    );

    write(serde_json::json!(0));
    let loaded = TeamStore::load(&team_dir).expect("load team");
    assert_eq!(
        loaded.config.settings.effective_watchdog_timeout_secs(),
        DEFAULT_WATCHDOG_TIMEOUT_SECS,
        "0 means 'use the default', so a zeroed field can never arm a zero window"
    );
}

/// An absent field deserialises to the default, so an older `config.json` that
/// predates this setting keeps the documented window.
#[test]
fn test_missing_setting_field_deserialises_to_the_default() {
    let raw = r#"{
        "max_teammates": 4,
        "require_plan_approval": false,
        "auto_claim_tasks": true
    }"#;
    let settings: TeamSettings = serde_json::from_str(raw).expect("parse settings");
    assert_eq!(
        settings.watchdog_timeout_secs, DEFAULT_WATCHDOG_TIMEOUT_SECS,
        "a config written before T-022 must still get the 300-second window"
    );
}

/// `TeamManager::new` seeds its in-process timeout from the team's settings, so
/// the configured window is what the watchdog measures against.
#[test]
fn test_manager_seeds_timeout_from_team_settings() {
    let (manager, _tmp, _bus) = team_manager("wd-seed", "lead-1", true);
    // The shared fixture writes a default-settings team.
    assert_eq!(
        manager.watchdog_timeout,
        Duration::from_secs(DEFAULT_WATCHDOG_TIMEOUT_SECS),
        "a default team must arm the 300-second window"
    );
    drop(manager);

    // A team whose config names a window gets exactly that window.
    let bus = Arc::new(ragent_agent::event::EventBus::new(16));
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let store = TeamStore::create("wd-seed-2", "lead-1", &workspace, true).expect("create team");
    let team_dir = store.dir.clone();
    drop(store);
    let mut loaded = TeamStore::load(&team_dir).expect("load team");
    loaded.config.settings.watchdog_timeout_secs = 90;
    loaded.save().expect("save settings");

    let manager = TeamManager::new(
        "wd-seed-2",
        "lead-1",
        team_dir,
        stub_processor(bus.clone()),
        bus,
    );
    assert_eq!(
        manager.watchdog_timeout,
        Duration::from_secs(90),
        "FR-013: the configured window is the one the watchdog uses"
    );
}

/// The tick interval is derived from the window: half of it, capped at one
/// minute so a wide operator window does not also widen detection latency.
#[test]
fn test_tick_interval_is_half_the_window_capped_at_a_minute() {
    for (window_secs, expected_secs) in [(10u64, 5u64), (40, 20), (120, 30), (300, 30), (3600, 30)]
    {
        assert_eq!(
            watchdog_check_interval(Duration::from_secs(window_secs)),
            Duration::from_secs(expected_secs),
            "a {window_secs}s window must tick every {expected_secs}s"
        );
    }
}
