//! Integration tests for the `sdd` config block (FR-019 / T-035).

use ragent_config::{Config, SddConfig};

#[test]
fn default_sdd_config_has_all_flags_disabled() {
    let cfg = SddConfig::default();
    assert!(!cfg.clarification_markers);
    assert!(!cfg.quality_checklists);
    assert!(!cfg.constitution);
    assert!(!cfg.phase_minus_one_gates);
    assert!(!cfg.branch_per_spec);
    assert!(!cfg.research_artifacts);
    assert!(!cfg.data_model);
    assert!(!cfg.contracts);
    assert!(!cfg.quickstart);
    assert!(!cfg.test_first_ordering);
    assert!(!cfg.consistency_checks);
    assert!(!cfg.amendment_process);
    assert!(!cfg.feedback_loop);
    assert!(cfg.is_empty(), "default SddConfig should be empty");
}

#[test]
fn top_level_config_defaults_sdd_when_absent() {
    let cfg: Config = serde_json::from_str("{}").expect("parse");
    assert_eq!(cfg.sdd, SddConfig::default());
    assert!(cfg.sdd.is_empty());
}

#[test]
fn top_level_config_parses_sdd_block() {
    let json = r#"{
        "sdd": {
            "clarification_markers": true,
            "constitution": true,
            "consistency_checks": true
        }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert!(cfg.sdd.clarification_markers);
    assert!(cfg.sdd.constitution);
    assert!(cfg.sdd.consistency_checks);
    // Unmentioned flags stay false
    assert!(!cfg.sdd.quality_checklists);
    assert!(!cfg.sdd.data_model);
    assert!(!cfg.sdd.is_empty());
}

#[test]
fn sdd_config_deserializes_empty_block() {
    let json = r#"{"sdd": {}}"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert!(cfg.sdd.is_empty());
    assert_eq!(cfg.sdd, SddConfig::default());
}

#[test]
fn sdd_config_roundtrip_preserves_enabled_flags() {
    let mut original = SddConfig::default();
    original.constitution = true;
    original.feedback_loop = true;
    original.data_model = true;

    let json = serde_json::to_string(&original).unwrap();
    let restored: SddConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, original);
    assert!(restored.constitution);
    assert!(restored.feedback_loop);
    assert!(restored.data_model);
}

#[test]
fn serialised_default_omits_sdd_block() {
    // When all flags are false, the `sdd` key should be omitted from the
    // serialised output (skip_serializing_if = "SddConfig::is_empty").
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("sdd").is_none(),
        "default config should omit empty sdd block, but found: {:?}",
        parsed.get("sdd")
    );
}

#[test]
fn serialised_config_includes_only_enabled_sdd_flags() {
    let mut config = Config::default();
    config.sdd.constitution = true;
    config.sdd.quickstart = true;

    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let sdd = &parsed["sdd"];
    assert_eq!(sdd["constitution"], serde_json::Value::Bool(true));
    assert_eq!(sdd["quickstart"], serde_json::Value::Bool(true));
    // Disabled flags should be omitted
    assert!(
        sdd.get("data_model").is_none(),
        "disabled flag should be omitted from serialised output"
    );
    assert!(
        sdd.get("clarification_markers").is_none(),
        "disabled flag should be omitted from serialised output"
    );
}

#[test]
fn sdd_merge_or_semantics_preserves_base_enabled() {
    // Base has a flag enabled, overlay doesn't mention it → stays enabled.
    let mut base = SddConfig::default();
    base.constitution = true;

    let overlay = SddConfig::default();

    base.merge(&overlay);
    assert!(base.constitution, "base-enabled flag should survive merge");
}

#[test]
fn sdd_merge_or_semantics_overlay_enables_flag() {
    // Base is empty, overlay enables a flag → result enabled.
    let mut base = SddConfig::default();
    let mut overlay = SddConfig::default();
    overlay.data_model = true;

    base.merge(&overlay);
    assert!(base.data_model, "overlay-enabled flag should be merged in");
}

#[test]
fn sdd_merge_or_semantics_both_enabled_stays_enabled() {
    let mut base = SddConfig::default();
    base.feedback_loop = true;

    let mut overlay = SddConfig::default();
    overlay.feedback_loop = true;

    base.merge(&overlay);
    assert!(base.feedback_loop);
}

#[test]
fn config_merge_sdd_or_semantics() {
    // Full Config::merge should propagate OR semantics for the sdd block.
    let mut base = Config::default();
    base.sdd.constitution = true;
    base.sdd.consistency_checks = true;

    let overlay: Config = serde_json::from_str(
        r#"{
            "sdd": {
                "constitution": true,
                "data_model": true
            }
        }"#,
    )
    .unwrap();

    let merged = Config::merge(base, overlay);
    assert!(merged.sdd.constitution);
    assert!(
        merged.sdd.consistency_checks,
        "base flag should survive merge"
    );
    assert!(merged.sdd.data_model, "overlay flag should be merged in");
    assert!(!merged.sdd.quickstart, "unmentioned flag should stay false");
}

#[test]
fn all_thirteen_sdd_flags_exist() {
    // Ensure every SDD capability from the spec has a corresponding flag.
    let mut cfg = SddConfig::default();
    cfg.clarification_markers = true; // FR-002
    cfg.quality_checklists = true; // FR-006
    cfg.constitution = true; // FR-007
    cfg.phase_minus_one_gates = true; // FR-008
    cfg.branch_per_spec = true; // FR-009
    cfg.research_artifacts = true; // FR-010
    cfg.data_model = true; // FR-011
    cfg.contracts = true; // FR-012
    cfg.quickstart = true; // FR-013
    cfg.test_first_ordering = true; // FR-014
    cfg.consistency_checks = true; // FR-015
    cfg.amendment_process = true; // FR-016
    cfg.feedback_loop = true; // FR-017
    assert!(!cfg.is_empty(), "all flags enabled should not be empty");
}
// ── Edge-case tests for SddConfig (T-041, NFR-004) ──────────────────────────

#[test]
fn sdd_is_empty_false_when_single_flag_enabled() {
    let mut cfg = SddConfig::default();
    cfg.constitution = true;
    assert!(!cfg.is_empty(), "one flag true should make is_empty false");

    cfg = SddConfig::default();
    cfg.feedback_loop = true;
    assert!(
        !cfg.is_empty(),
        "feedback_loop flag alone should make is_empty false"
    );

    cfg = SddConfig::default();
    cfg.clarification_markers = true;
    assert!(
        !cfg.is_empty(),
        "clarification_markers flag alone should make is_empty false"
    );
}

#[test]
fn sdd_merge_both_empty_stays_empty() {
    let mut base = SddConfig::default();
    let overlay = SddConfig::default();
    base.merge(&overlay);
    assert!(
        base.is_empty(),
        "merging two empty configs should stay empty"
    );
    assert_eq!(base, SddConfig::default());
}

#[test]
fn sdd_merge_all_flags_from_overlay() {
    let mut base = SddConfig::default();
    let mut overlay = SddConfig::default();
    overlay.constitution = true;
    overlay.quickstart = true;
    overlay.consistency_checks = true;

    base.merge(&overlay);
    assert!(base.constitution);
    assert!(base.quickstart);
    assert!(base.consistency_checks);
    assert!(!base.is_empty());
}

#[test]
fn sdd_merge_preserves_base_flags_when_overlay_empty() {
    let mut base = SddConfig::default();
    base.data_model = true;
    base.contracts = true;
    let overlay = SddConfig::default();

    base.merge(&overlay);
    assert!(
        base.data_model,
        "base flag should survive merge with empty overlay"
    );
    assert!(base.contracts);
    assert!(!base.is_empty());
}
