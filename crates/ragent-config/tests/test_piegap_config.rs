//! Integration tests for the `piegap` config block (FR-016, FR-018 / T-021, T-022).
//!
//! These tests verify:
//! - All pie gap feature flags default to `false` (opt-in)
//! - The `piegap` block is omitted from serialized output when all flags are disabled
//! - Individual flags can be enabled and are serialized
//! - Merge uses OR semantics (enabling a flag in either config keeps it enabled)
//! - All defined flags exist and are functional
//! - Standalone compilation: each flag can be disabled independently

use ragent_config::{Config, PieGapConfig};

#[test]
fn default_piegap_config_has_all_flags_disabled() {
    let cfg = PieGapConfig::default();
    assert!(!cfg.triggers);
    assert!(!cfg.mcp_notifications);
    assert!(!cfg.inbox);
    assert!(!cfg.hooks);
    assert!(!cfg.archive);
    assert!(!cfg.bug_report);
    assert!(!cfg.templates);
    assert!(!cfg.goal);
    assert!(!cfg.web_ui);
    assert!(!cfg.undo);
    assert!(!cfg.session_naming);
    assert!(
        cfg.is_empty(),
        "default PieGapConfig should be empty when all flags are false"
    );
}

#[test]
fn top_level_config_defaults_piegap_when_absent() {
    let cfg: Config = serde_json::from_str("{}").expect("parse");
    assert_eq!(cfg.piegap, PieGapConfig::default());
    assert!(cfg.piegap.is_empty());
}

#[test]
fn top_level_config_parses_piegap_block() {
    let json = r#"{
        "piegap": {
            "triggers": true,
            "inbox": true,
            "goal": true
        }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert!(cfg.piegap.triggers);
    assert!(cfg.piegap.inbox);
    assert!(cfg.piegap.goal);
    // Unmentioned flags stay false
    assert!(!cfg.piegap.hooks);
    assert!(!cfg.piegap.archive);
    assert!(!cfg.piegap.web_ui);
    assert!(!cfg.piegap.is_empty());
}

#[test]
fn piegap_config_deserializes_empty_block() {
    let json = r#"{"piegap": {}}"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert!(cfg.piegap.is_empty());
    assert_eq!(cfg.piegap, PieGapConfig::default());
}

#[test]
fn piegap_config_roundtrip_preserves_enabled_flags() {
    let mut original = PieGapConfig::default();
    original.triggers = true;
    original.archive = true;
    original.session_naming = true;

    let json = serde_json::to_string(&original).unwrap();
    let restored: PieGapConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, original);
    assert!(restored.triggers);
    assert!(restored.archive);
    assert!(restored.session_naming);
}

#[test]
fn serialised_default_omits_piegap_block() {
    // When all flags are false, the `piegap` key should be omitted from the
    // serialised output (skip_serializing_if = "PieGapConfig::is_empty").
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("piegap").is_none(),
        "default config should omit empty piegap block, but found: {:?}",
        parsed.get("piegap")
    );
}

#[test]
fn serialised_config_includes_only_enabled_piegap_flags() {
    let mut config = Config::default();
    config.piegap.triggers = true;
    config.piegap.web_ui = true;

    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let piegap = &parsed["piegap"];
    assert_eq!(piegap["triggers"], serde_json::Value::Bool(true));
    assert_eq!(piegap["web_ui"], serde_json::Value::Bool(true));
    // Disabled flags should be omitted
    assert!(
        piegap.get("inbox").is_none(),
        "disabled inbox flag should be omitted"
    );
    assert!(
        piegap.get("hooks").is_none(),
        "disabled hooks flag should be omitted"
    );
}

#[test]
fn piegap_merge_or_semantics_preserves_base_enabled() {
    let mut base = PieGapConfig::default();
    base.triggers = true;
    base.archive = true;

    let overlay = PieGapConfig::default();
    base.merge(&overlay);

    assert!(base.triggers);
    assert!(base.archive);
    assert!(!base.inbox);
}

#[test]
fn piegap_merge_or_semantics_overlay_enables_flag() {
    let mut base = PieGapConfig::default();
    let mut overlay = PieGapConfig::default();
    overlay.inbox = true;
    overlay.goal = true;

    base.merge(&overlay);

    assert!(base.inbox);
    assert!(base.goal);
    assert!(!base.triggers);
}

#[test]
fn piegap_merge_or_semantics_both_enabled_stays_enabled() {
    let mut base = PieGapConfig::default();
    base.triggers = true;

    let mut overlay = PieGapConfig::default();
    overlay.triggers = true;

    base.merge(&overlay);

    assert!(base.triggers);
}

#[test]
fn config_merge_piegap_or_semantics() {
    let mut base = Config::default();
    base.piegap.triggers = true;
    base.piegap.hooks = true;

    let mut overlay = Config::default();
    overlay.piegap.hooks = true;
    overlay.piegap.inbox = true;
    overlay.piegap.web_ui = true;

    let merged = Config::merge(base, overlay);

    // Base flags preserved
    assert!(merged.piegap.triggers);
    // OR semantics: hooks enabled in both, stays enabled
    assert!(merged.piegap.hooks);
    // Overlay flags added
    assert!(merged.piegap.inbox);
    assert!(merged.piegap.web_ui);
    // Unmentioned flags stay false
    assert!(!merged.piegap.archive);
    assert!(!merged.piegap.goal);
}

#[test]
fn all_eleven_piegap_flags_exist() {
    // Verify all eleven gap feature flags are defined and accessible
    let mut cfg = PieGapConfig::default();

    // G-01: Dynamic trigger rules
    cfg.triggers = true;
    assert!(cfg.triggers);

    // G-02: MCP notification push events
    cfg.mcp_notifications = true;
    assert!(cfg.mcp_notifications);

    // G-03: Stateful loops + triage inbox
    cfg.inbox = true;
    assert!(cfg.inbox);

    // G-04: Lifecycle hooks
    cfg.hooks = true;
    assert!(cfg.hooks);

    // G-05: Portable session archive
    cfg.archive = true;
    assert!(cfg.archive);

    // G-06: Bug report generation
    cfg.bug_report = true;
    assert!(cfg.bug_report);

    // G-07: Reusable prompt templates
    cfg.templates = true;
    assert!(cfg.templates);

    // G-10: Goal-based autonomous stop hook
    cfg.goal = true;
    assert!(cfg.goal);

    // G-12: Browser-based web UI
    cfg.web_ui = true;
    assert!(cfg.web_ui);

    // G-13: /undo slash command
    cfg.undo = true;
    assert!(cfg.undo);

    // G-14: Session naming
    cfg.session_naming = true;
    assert!(cfg.session_naming);
}

#[test]
fn piegap_is_empty_false_when_single_flag_enabled() {
    let mut cfg = PieGapConfig::default();

    // Test each flag individually
    cfg.triggers = true;
    assert!(
        !cfg.is_empty(),
        "triggers=true should make is_empty() false"
    );
    cfg.triggers = false;

    cfg.mcp_notifications = true;
    assert!(
        !cfg.is_empty(),
        "mcp_notifications=true should make is_empty() false"
    );
    cfg.mcp_notifications = false;

    cfg.inbox = true;
    assert!(!cfg.is_empty(), "inbox=true should make is_empty() false");
    cfg.inbox = false;

    cfg.hooks = true;
    assert!(!cfg.is_empty(), "hooks=true should make is_empty() false");
    cfg.hooks = false;

    cfg.archive = true;
    assert!(!cfg.is_empty(), "archive=true should make is_empty() false");
    cfg.archive = false;

    cfg.bug_report = true;
    assert!(
        !cfg.is_empty(),
        "bug_report=true should make is_empty() false"
    );
    cfg.bug_report = false;

    cfg.templates = true;
    assert!(
        !cfg.is_empty(),
        "templates=true should make is_empty() false"
    );
    cfg.templates = false;

    cfg.goal = true;
    assert!(!cfg.is_empty(), "goal=true should make is_empty() false");
    cfg.goal = false;

    cfg.web_ui = true;
    assert!(!cfg.is_empty(), "web_ui=true should make is_empty() false");
    cfg.web_ui = false;

    cfg.undo = true;
    assert!(!cfg.is_empty(), "undo=true should make is_empty() false");
    cfg.undo = false;

    cfg.session_naming = true;
    assert!(
        !cfg.is_empty(),
        "session_naming=true should make is_empty() false"
    );
}

#[test]
fn piegap_merge_both_empty_stays_empty() {
    let mut base = PieGapConfig::default();
    let overlay = PieGapConfig::default();

    base.merge(&overlay);

    assert!(base.is_empty());
    assert!(overlay.is_empty());
}

#[test]
fn piegap_merge_all_flags_from_overlay() {
    let mut base = PieGapConfig::default();
    let mut overlay = PieGapConfig::default();
    overlay.triggers = true;
    overlay.mcp_notifications = true;
    overlay.inbox = true;
    overlay.hooks = true;
    overlay.archive = true;
    overlay.bug_report = true;
    overlay.templates = true;
    overlay.goal = true;
    overlay.web_ui = true;
    overlay.undo = true;
    overlay.session_naming = true;

    base.merge(&overlay);

    assert!(base.triggers);
    assert!(base.mcp_notifications);
    assert!(base.inbox);
    assert!(base.hooks);
    assert!(base.archive);
    assert!(base.bug_report);
    assert!(base.templates);
    assert!(base.goal);
    assert!(base.web_ui);
    assert!(base.undo);
    assert!(base.session_naming);
    assert!(!base.is_empty());
}

#[test]
fn piegap_merge_preserves_base_flags_when_overlay_empty() {
    let mut base = PieGapConfig::default();
    base.triggers = true;
    base.inbox = true;
    base.web_ui = true;

    let overlay = PieGapConfig::default();
    base.merge(&overlay);

    assert!(base.triggers);
    assert!(base.inbox);
    assert!(base.web_ui);
    assert!(!base.hooks);
    assert!(!base.archive);
}

#[test]
fn standalone_compilation_each_flag_independent() {
    // FR-016: Verify each flag can be enabled/disabled independently
    // without compile-time or runtime dependencies on other flags.

    // Test 1: Only triggers enabled
    let cfg1 = PieGapConfig {
        triggers: true,
        ..PieGapConfig::default()
    };
    assert!(cfg1.triggers);
    assert!(!cfg1.inbox);
    assert!(!cfg1.hooks);

    // Test 2: Only hooks enabled
    let cfg2 = PieGapConfig {
        hooks: true,
        ..PieGapConfig::default()
    };
    assert!(cfg2.hooks);
    assert!(!cfg2.triggers);
    assert!(!cfg2.inbox);

    // Test 3: Only inbox enabled
    let cfg3 = PieGapConfig {
        inbox: true,
        ..PieGapConfig::default()
    };
    assert!(cfg3.inbox);
    assert!(!cfg3.triggers);
    assert!(!cfg3.hooks);

    // Test 4: Only archive enabled
    let cfg4 = PieGapConfig {
        archive: true,
        ..PieGapConfig::default()
    };
    assert!(cfg4.archive);
    assert!(!cfg4.triggers);
    assert!(!cfg4.inbox);

    // Test 5: Only goal enabled
    let cfg5 = PieGapConfig {
        goal: true,
        ..PieGapConfig::default()
    };
    assert!(cfg5.goal);
    assert!(!cfg5.triggers);
    assert!(!cfg5.inbox);

    // Test 6: Only web_ui enabled
    let cfg6 = PieGapConfig {
        web_ui: true,
        ..PieGapConfig::default()
    };
    assert!(cfg6.web_ui);
    assert!(!cfg6.triggers);
    assert!(!cfg6.inbox);

    // Test 7: Only undo enabled
    let cfg7 = PieGapConfig {
        undo: true,
        ..PieGapConfig::default()
    };
    assert!(cfg7.undo);
    assert!(!cfg7.triggers);
    assert!(!cfg7.inbox);

    // Test 8: Only session_naming enabled
    let cfg8 = PieGapConfig {
        session_naming: true,
        ..PieGapConfig::default()
    };
    assert!(cfg8.session_naming);
    assert!(!cfg8.triggers);
    assert!(!cfg8.inbox);

    // Test 9: Multiple non-adjacent flags enabled
    let cfg9 = PieGapConfig {
        triggers: true,
        archive: true,
        web_ui: true,
        ..PieGapConfig::default()
    };
    assert!(cfg9.triggers);
    assert!(cfg9.archive);
    assert!(cfg9.web_ui);
    assert!(!cfg9.inbox);
    assert!(!cfg9.hooks);
    assert!(!cfg9.goal);
}

#[test]
fn piegap_with_sdd_coexist_independently() {
    // FR-017: Verify piegap features don't interfere with SDD features
    let mut config = Config::default();

    // Enable some SDD flags
    config.sdd.constitution = true;
    config.sdd.clarification_markers = true;

    // Enable some piegap flags
    config.piegap.triggers = true;
    config.piegap.inbox = true;

    // Verify both sets of flags are independent
    assert!(config.sdd.constitution);
    assert!(config.sdd.clarification_markers);
    assert!(!config.sdd.quality_checklists);

    assert!(config.piegap.triggers);
    assert!(config.piegap.inbox);
    assert!(!config.piegap.hooks);

    // Serialize and verify both blocks appear
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(parsed.get("sdd").is_some());
    assert!(parsed.get("piegap").is_some());
    assert_eq!(parsed["sdd"]["constitution"], serde_json::Value::Bool(true));
    assert_eq!(parsed["piegap"]["triggers"], serde_json::Value::Bool(true));
}
