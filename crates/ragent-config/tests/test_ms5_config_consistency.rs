//! ANTIPAT M5.4 regression tests for `ragent-config` consistency fixes.
//!
//! - M-3: `merge_project` treats `bash.denylist` and `dirs.denylist`
//!   symmetrically - both are stripped from the untrusted project overlay.
//! - M-7: `Config::default()` matches the serde defaults
//!   (`default_agent == "general"`, `activity_log == true`).
//! - L-4: the "at defaults" predicate is available under both names
//!   (`is_default` is the canonical alias of `is_empty`).

use ragent_config::Config;

/// A project overlay that adds both denylists must not widen the effective
/// policy: the project entries are dropped, the user (base) entries survive.
#[test]
fn merge_project_strips_both_denylists() {
    let mut base = Config::default();
    base.bash.denylist = vec!["rm -rf".to_string()];
    base.dirs.denylist = vec!["secrets/**".to_string()];

    let mut overlay = Config::default();
    overlay.bash.denylist = vec!["project-cmd".to_string()];
    overlay.dirs.denylist = vec!["project-secret/**".to_string()];

    let merged = Config::merge_project(base, overlay);

    // User entries survive; project entries are never admitted.
    assert_eq!(merged.bash.denylist, vec!["rm -rf".to_string()]);
    assert_eq!(merged.dirs.denylist, vec!["secrets/**".to_string()]);
    assert!(
        !merged.bash.denylist.contains(&"project-cmd".to_string()),
        "project bash.denylist must be stripped"
    );
    assert!(
        !merged
            .dirs
            .denylist
            .contains(&"project-secret/**".to_string()),
        "project dirs.denylist must be stripped (symmetric with bash.denylist)"
    );
}

/// The trusted merge path still unions both denylists - the symmetric strip is
/// confined to `merge_project`.
#[test]
fn trusted_merge_keeps_both_denylists() {
    let mut base = Config::default();
    base.bash.denylist = vec!["rm -rf".to_string()];
    base.dirs.denylist = vec!["secrets/**".to_string()];

    let mut overlay = Config::default();
    overlay.bash.denylist = vec!["curl evil".to_string()];
    overlay.dirs.denylist = vec!["private/**".to_string()];

    let merged = Config::merge(base, overlay);

    assert!(merged.bash.denylist.contains(&"curl evil".to_string()));
    assert!(merged.dirs.denylist.contains(&"private/**".to_string()));
}

/// ANTIPAT M-7: `Config::default()` must agree with the serde defaults.
#[test]
fn config_default_matches_serde_defaults() {
    let derived = Config::default();
    let parsed: Config = serde_json::from_str("{}").expect("empty object parses");

    assert_eq!(derived.default_agent, "general");
    assert_eq!(derived.default_agent, parsed.default_agent);
    assert!(derived.activity_log);
    assert_eq!(derived.activity_log, parsed.activity_log);
}

/// ANTIPAT L-4: `is_default` is the canonical alias of `is_empty`.
#[test]
fn at_defaults_predicate_has_both_names() {
    let cfg = ragent_config::TriggerConfig::default();
    assert_eq!(cfg.is_empty(), cfg.is_default());
    assert!(cfg.is_default(), "a default trigger config is at defaults");

    let cfg = ragent_config::SddConfig::default();
    assert_eq!(cfg.is_empty(), cfg.is_default());

    let cfg = ragent_config::PieGapConfig::default();
    assert_eq!(cfg.is_empty(), cfg.is_default());

    let cfg = ragent_config::ResearchConfig::default();
    assert_eq!(cfg.is_empty(), cfg.is_default());

    let cfg = ragent_config::ChannelsConfig::default();
    assert_eq!(cfg.is_empty(), cfg.is_default());

    let cfg = ragent_config::GmailConfig::default();
    assert_eq!(cfg.is_empty(), cfg.is_default());
}
