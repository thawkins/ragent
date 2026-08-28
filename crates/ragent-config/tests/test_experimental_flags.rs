//! Tests for experimental flag merge semantics in `Config::merge`.
//!
//! Note: `std::env::set_var` is `unsafe` in Rust 2024; the workspace denies
//! `unsafe_code`, so this test target opts back in explicitly.

#![allow(unsafe_code)]
//!
//! Regression: `max_background_agents` and `background_agent_timeout` were not
//! propagated by `Config::merge`, so a value set in the global or project
//! `ragent.json` never replaced the compiled default of 8.

use ragent_config::Config;

#[test]
fn test_merge_overlay_max_background_agents_wins() {
    let base = Config::default();
    assert_eq!(base.experimental.max_background_agents, 8);

    let mut overlay = Config::default();
    overlay.experimental.max_background_agents = 32;

    let merged = Config::merge(base, overlay);
    assert_eq!(
        merged.experimental.max_background_agents, 32,
        "overlay max_background_agents=32 should win over base default 8"
    );
}

#[test]
fn test_merge_overlay_background_agent_timeout_wins() {
    let base = Config::default();
    assert_eq!(base.experimental.background_agent_timeout, 3600);

    let mut overlay = Config::default();
    overlay.experimental.background_agent_timeout = 7200;

    let merged = Config::merge(base, overlay);
    assert_eq!(
        merged.experimental.background_agent_timeout, 7200,
        "overlay background_agent_timeout=7200 should win over base default 3600"
    );
}

#[test]
fn test_merge_default_overlay_preserves_base() {
    // An overlay that leaves the fields at their defaults must not clobber a
    // base value that was already raised by a lower-precedence config file.
    let mut base = Config::default();
    base.experimental.max_background_agents = 32;
    base.experimental.background_agent_timeout = 7200;

    let overlay = Config::default();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.experimental.max_background_agents, 32);
    assert_eq!(merged.experimental.background_agent_timeout, 7200);
}

#[test]
fn test_experimental_round_trips_through_config_load() {
    let temp = tempfile::tempdir().expect("temp dir");
    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent dir");
    let path = ragent_dir.join("ragent.json");
    std::fs::write(
        &path,
        r#"{ "experimental": { "max_background_agents": 32 } }"#,
    )
    .expect("write config");

    unsafe { std::env::set_var("RAGENT_CONFIG", &path) };
    let config = Config::load().expect("load config");
    assert_eq!(config.experimental.max_background_agents, 32);
}
