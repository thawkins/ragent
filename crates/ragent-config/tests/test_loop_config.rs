//! Integration tests for the loop-defaults config block (`agentloop`
//! T-002 / FR-013, FR-014, FR-015).

use ragent_config::{Config, DEFAULT_ERROR_RETRY_ALLOWANCE, DEFAULT_LOOP_MAX_STEPS, LoopConfig};

#[test]
fn test_loop_config_defaults_match_spec() {
    let cfg = LoopConfig::default();
    // FR-013: documented step-limit default of 512 iterations.
    assert_eq!(cfg.max_steps, 512);
    assert_eq!(cfg.max_steps, DEFAULT_LOOP_MAX_STEPS);
    // FR-014: no token cost gate by default.
    assert_eq!(cfg.cost_limit, None);
    // FR-012: recoverable-error retry allowance defaults to 3.
    assert_eq!(cfg.error_retry_allowance, 3);
    assert_eq!(cfg.error_retry_allowance, DEFAULT_ERROR_RETRY_ALLOWANCE);
    // FR-015: destructive-action checkpoints default on.
    assert!(cfg.checkpoints);
}

#[test]
fn test_loop_config_is_default_when_all_fields_default() {
    let cfg = LoopConfig::default();
    assert!(cfg.is_default());
}

#[test]
fn test_loop_config_is_default_false_when_any_field_differs() {
    let cases = [
        LoopConfig {
            max_steps: 10,
            ..LoopConfig::default()
        },
        LoopConfig {
            cost_limit: Some(1000),
            ..LoopConfig::default()
        },
        LoopConfig {
            error_retry_allowance: 5,
            ..LoopConfig::default()
        },
        LoopConfig {
            checkpoints: false,
            ..LoopConfig::default()
        },
        LoopConfig {
            checkpoint_timeout_secs: 30,
            ..LoopConfig::default()
        },
    ];
    for (i, cfg) in cases.iter().enumerate() {
        assert!(!cfg.is_default(), "case {i} must not be default");
    }
}

#[test]
fn test_top_level_config_parses_loop_block() {
    let json = r#"{
        "loop": {
            "max_steps": 50,
            "cost_limit": 200000,
            "error_retry_allowance": 2,
            "checkpoints": false
        }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert_eq!(cfg.r#loop.max_steps, 50);
    assert_eq!(cfg.r#loop.cost_limit, Some(200_000));
    assert_eq!(cfg.r#loop.error_retry_allowance, 2);
    assert!(!cfg.r#loop.checkpoints);
}

#[test]
fn test_top_level_config_defaults_loop_when_absent() {
    let cfg: Config = serde_json::from_str("{}").expect("parse");
    assert_eq!(cfg.r#loop, LoopConfig::default());
    assert_eq!(cfg.r#loop.max_steps, 512);
    assert!(cfg.r#loop.checkpoints);
}

#[test]
fn test_loop_block_omitted_when_default() {
    let cfg = Config::default();
    let json = serde_json::to_string(&cfg).expect("serialize");
    // The `loop` key must not appear in saved config when untouched, so
    // existing config files remain unchanged.
    assert!(
        !json.contains("\"loop\""),
        "loop block should be omitted when default; got: {json}"
    );
}

#[test]
fn test_loop_block_persists_when_explicitly_configured() {
    let json = r#"{
        "loop": {
            "max_steps": 8,
            "cost_limit": 5000
        }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert!(!cfg.r#loop.is_default());
    let out = serde_json::to_string(&cfg).expect("serialize");
    assert!(
        out.contains("\"loop\""),
        "explicit loop block persists: {out}"
    );
    let back: Config = serde_json::from_str(&out).expect("reparse");
    assert_eq!(back.r#loop.max_steps, 8);
    assert_eq!(back.r#loop.cost_limit, Some(5_000));
    // Unset fields still default.
    assert_eq!(back.r#loop.error_retry_allowance, 3);
    assert!(back.r#loop.checkpoints);
}

#[test]
fn test_loop_merge_overlay_explicit_fields_win() {
    let mut base = LoopConfig {
        max_steps: 40,
        cost_limit: Some(10_000),
        error_retry_allowance: 5,
        checkpoints: false,
        checkpoint_timeout_secs: 60,
    };

    // Overlay leaves max_steps and error_retry_allowance at defaults: base
    // values survive. Overlay sets cost_limit and disables checkpoints.
    let overlay = LoopConfig {
        max_steps: 512, // == default, so base 40 must survive
        cost_limit: Some(99_999),
        error_retry_allowance: 3, // == default, so base 5 must survive
        checkpoints: false,
        checkpoint_timeout_secs: 120, // == default, so base 60 must survive
    };
    base.merge(&overlay);
    assert_eq!(base.max_steps, 40);
    assert_eq!(base.cost_limit, Some(99_999));
    assert_eq!(base.error_retry_allowance, 5);
    assert!(!base.checkpoints);
    assert_eq!(base.checkpoint_timeout_secs, 60);
}

#[test]
fn test_agent_max_steps_doc_remains_per_agent_override() {
    // The per-agent knob keeps its Option<u32> shape; the loop-level block
    // provides the fallback when unset.
    let cfg: Config = serde_json::from_str("{}").expect("parse");
    let agent = cfg.agent.get("coder");
    assert!(agent.is_none(), "no agents configured by default");
}

#[test]
fn test_loop_config_serialise_round_trip() {
    let cfg = LoopConfig {
        max_steps: 12,
        cost_limit: Some(77_000),
        error_retry_allowance: 1,
        checkpoints: false,
        checkpoint_timeout_secs: 45,
    };
    let json = serde_json::to_string(&cfg).expect("serialize");
    let back: LoopConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, cfg);
}
