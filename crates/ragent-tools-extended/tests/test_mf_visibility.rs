//! Integration test verifying the `masterfetch` visibility switch hides/shows
//! all six `mf_*` tools via `effective_hidden_tools()` (T-038, FR-021, NFR-003).
//!
//! FR-021 requires that the `masterfetch` tool-visibility switch controls all
//! six masterfetch tools. When the switch is `true` (default), none of the
//! `mf_*` tools should appear in `effective_hidden_tools()`. When the switch
//! is `false`, all six should appear.
//!
//! This test also verifies:
//! - `tool_family_names("masterfetch")` returns all six tool names.
//! - The `masterfetch` switch defaults to `true`.
//! - The switch is included in `iter_switches()`.
//! - Config deserialisation honours an explicit `masterfetch: false`.
//! - Config merge preserves an explicitly-set `masterfetch` value.

use ragent_config::{Config, tool_family_names};

/// The six masterfetch tool names governed by the `masterfetch` visibility
/// switch.
const MF_TOOL_NAMES: &[&str] = &[
    "mf_fetch",
    "mf_crawl",
    "mf_search",
    "mf_screenshot",
    "mf_cache_clear",
    "mf_version",
];

// ---------------------------------------------------------------------------
// tool_family_names
// ---------------------------------------------------------------------------

#[test]
fn test_tool_family_names_masterfetch_returns_all_six() {
    let names = tool_family_names("masterfetch").expect("masterfetch family should exist");
    assert_eq!(
        names.len(),
        6,
        "masterfetch family should have exactly 6 tools"
    );
    for &expected in MF_TOOL_NAMES {
        assert!(
            names.contains(&expected),
            "masterfetch family should contain '{expected}'"
        );
    }
}

#[test]
fn test_tool_family_names_masterfetch_exact_set() {
    let names = tool_family_names("masterfetch").expect("masterfetch family should exist");
    let actual: std::collections::HashSet<&str> = names.iter().copied().collect();
    let expected: std::collections::HashSet<&str> = MF_TOOL_NAMES.iter().copied().collect();
    assert_eq!(
        actual, expected,
        "masterfetch family should be exactly the six mf_* tools"
    );
}

// ---------------------------------------------------------------------------
// Default value
// ---------------------------------------------------------------------------

#[test]
fn test_masterfetch_switch_defaults_true() {
    let config = Config::default();
    assert!(
        config.tool_visibility.masterfetch,
        "masterfetch switch should default to true"
    );
}

// ---------------------------------------------------------------------------
// iter_switches includes masterfetch
// ---------------------------------------------------------------------------

#[test]
fn test_iter_switches_includes_masterfetch() {
    let config = Config::default();
    let switches: std::collections::HashMap<&str, bool> =
        config.tool_visibility.iter_switches().collect();
    assert!(
        switches.contains_key("masterfetch"),
        "iter_switches should include 'masterfetch'"
    );
    assert!(
        *switches.get("masterfetch").unwrap(),
        "masterfetch should be true by default in iter_switches"
    );
}

// ---------------------------------------------------------------------------
// effective_hidden_tools: switch ON (default) → no mf_* tools hidden
// ---------------------------------------------------------------------------

#[test]
fn test_effective_hidden_tools_masterfetch_on_hides_no_mf_tools() {
    let config = Config::default();
    assert!(config.tool_visibility.masterfetch);

    let hidden = config.effective_hidden_tools();
    for &name in MF_TOOL_NAMES {
        assert!(
            !hidden.iter().any(|h| h == name),
            "tool '{name}' should NOT be hidden when masterfetch switch is on"
        );
    }
}

// ---------------------------------------------------------------------------
// effective_hidden_tools: switch OFF → all six mf_* tools hidden
// ---------------------------------------------------------------------------

#[test]
fn test_effective_hidden_tools_masterfetch_off_hides_all_six() {
    let mut config = Config::default();
    config.tool_visibility.masterfetch = false;

    let hidden = config.effective_hidden_tools();
    for &name in MF_TOOL_NAMES {
        assert!(
            hidden.iter().any(|h| h == name),
            "tool '{name}' should be hidden when masterfetch switch is off"
        );
    }
}

#[test]
fn test_effective_hidden_tools_masterfetch_off_hides_exactly_six_mf_tools() {
    let mut config = Config::default();
    config.tool_visibility.masterfetch = false;

    let hidden = config.effective_hidden_tools();
    let mf_hidden: Vec<&String> = hidden.iter().filter(|h| h.starts_with("mf_")).collect();
    assert_eq!(
        mf_hidden.len(),
        6,
        "exactly 6 mf_* tools should be hidden, got: {mf_hidden:?}"
    );
}

// ---------------------------------------------------------------------------
// effective_hidden_tools: switch OFF hides the correct six names
// ---------------------------------------------------------------------------

#[test]
fn test_effective_hidden_tools_masterfetch_off_correct_names() {
    let mut config = Config::default();
    config.tool_visibility.masterfetch = false;

    let hidden = config.effective_hidden_tools();
    let hidden_set: std::collections::HashSet<&str> = hidden.iter().map(String::as_str).collect();

    for &name in MF_TOOL_NAMES {
        assert!(
            hidden_set.contains(name),
            "hidden set should contain '{name}'"
        );
    }
}

// ---------------------------------------------------------------------------
// Config deserialisation: explicit masterfetch: false
// ---------------------------------------------------------------------------

#[test]
fn test_config_parses_masterfetch_false() {
    let config: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "masterfetch": false
            }
        }"#,
    )
    .expect("config should parse");

    assert!(
        !config.tool_visibility.masterfetch,
        "masterfetch should be false when explicitly set to false"
    );
    assert!(
        config.tool_visibility.specified.masterfetch,
        "masterfetch specified flag should be true when explicitly set"
    );
}

#[test]
fn test_config_parses_masterfetch_true() {
    let config: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "masterfetch": true
            }
        }"#,
    )
    .expect("config should parse");

    assert!(config.tool_visibility.masterfetch);
    assert!(config.tool_visibility.specified.masterfetch);
}

#[test]
fn test_config_defaults_masterfetch_true_when_absent() {
    let config: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "office": true
            }
        }"#,
    )
    .expect("config should parse");

    assert!(
        config.tool_visibility.masterfetch,
        "masterfetch should default to true when absent from JSON"
    );
    assert!(
        !config.tool_visibility.specified.masterfetch,
        "masterfetch specified flag should be false when absent from JSON"
    );
}

// ---------------------------------------------------------------------------
// effective_hidden_tools after deserialisation
// ---------------------------------------------------------------------------

#[test]
fn test_effective_hidden_tools_after_parse_masterfetch_false() {
    let config: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "masterfetch": false
            }
        }"#,
    )
    .expect("config should parse");

    let hidden = config.effective_hidden_tools();
    for &name in MF_TOOL_NAMES {
        assert!(
            hidden.iter().any(|h| h == name),
            "tool '{name}' should be hidden after parsing masterfetch: false"
        );
    }
}

#[test]
fn test_effective_hidden_tools_after_parse_masterfetch_true() {
    let config: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "masterfetch": true
            }
        }"#,
    )
    .expect("config should parse");

    let hidden = config.effective_hidden_tools();
    for &name in MF_TOOL_NAMES {
        assert!(
            !hidden.iter().any(|h| h == name),
            "tool '{name}' should NOT be hidden after parsing masterfetch: true"
        );
    }
}

// ---------------------------------------------------------------------------
// Config merge: masterfetch
// ---------------------------------------------------------------------------

#[test]
fn test_merge_preserves_explicit_masterfetch_false() {
    let base = Config::default();

    let overlay: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "masterfetch": false
            }
        }"#,
    )
    .expect("overlay should parse");

    let merged = Config::merge(base, overlay);
    assert!(
        !merged.tool_visibility.masterfetch,
        "merged config should have masterfetch=false from overlay"
    );
}

#[test]
fn test_merge_preserves_explicit_masterfetch_true() {
    let mut base = Config::default();
    base.tool_visibility.masterfetch = false;

    let overlay: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "masterfetch": true
            }
        }"#,
    )
    .expect("overlay should parse");

    let merged = Config::merge(base, overlay);
    assert!(
        merged.tool_visibility.masterfetch,
        "merged config should have masterfetch=true from overlay"
    );
}

#[test]
fn test_merge_preserves_base_masterfetch_when_overlay_unspecified() {
    let mut base = Config::default();
    base.tool_visibility.masterfetch = false;
    base.tool_visibility.specified.masterfetch = true;

    let overlay: Config = serde_json::from_str(
        r#"{
            "tool_visibility": {
                "office": true
            }
        }"#,
    )
    .expect("overlay should parse");

    let merged = Config::merge(base, overlay);
    assert!(
        !merged.tool_visibility.masterfetch,
        "merged config should preserve base masterfetch=false when overlay doesn't specify it"
    );
}

// ---------------------------------------------------------------------------
// effective_hidden_tools: interaction with other switches
// ---------------------------------------------------------------------------

#[test]
fn test_effective_hidden_tools_masterfetch_off_with_other_switches_off() {
    let mut config = Config::default();
    config.tool_visibility.masterfetch = false;
    config.tool_visibility.github = false;
    config.tool_visibility.teams = false;

    let hidden = config.effective_hidden_tools();
    // All six mf_* tools should be hidden.
    for &name in MF_TOOL_NAMES {
        assert!(
            hidden.iter().any(|h| h == name),
            "mf tool '{name}' should be hidden"
        );
    }
    // GitHub tools should also be hidden.
    assert!(hidden.iter().any(|h| h == "github_list_issues"));
    // Teams tools should also be hidden.
    assert!(hidden.iter().any(|h| h == "team_create"));
}

#[test]
fn test_effective_hidden_tools_masterfetch_off_does_not_affect_other_families() {
    let mut config = Config::default();
    config.tool_visibility.masterfetch = false;
    // All other switches are at their defaults (false for most, true for codeindex).

    let hidden = config.effective_hidden_tools();
    // No GitHub tools should be hidden (github defaults to false, so they ARE hidden).
    // Wait — github defaults to false! So github tools ARE hidden by default.
    // Let's check that codeindex tools are NOT hidden (codeindex defaults to true).
    assert!(
        !hidden.iter().any(|h| h == "codeindex_search"),
        "codeindex tools should not be hidden when codeindex switch is on"
    );
    // And mf_* tools ARE hidden (masterfetch is off).
    assert!(hidden.iter().any(|h| h == "mf_fetch"));
}

// ---------------------------------------------------------------------------
// Serialisation round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_serialise_round_trip_masterfetch() {
    let mut config = Config::default();
    config.tool_visibility.masterfetch = false;
    config.tool_visibility.specified.masterfetch = true;

    let json = serde_json::to_string(&config).expect("should serialise");
    assert!(
        json.contains("\"masterfetch\":false"),
        "serialised config should contain masterfetch:false, got: {json}"
    );

    let parsed: Config = serde_json::from_str(&json).expect("should deserialise");
    assert!(
        !parsed.tool_visibility.masterfetch,
        "round-trip should preserve masterfetch=false"
    );
}

#[test]
fn test_serialise_omits_masterfetch_when_not_specified() {
    let config = Config::default();
    // masterfetch specified flag is false by default.

    let json = serde_json::to_string(&config).expect("should serialise");
    assert!(
        !json.contains("masterfetch"),
        "serialised config should omit masterfetch when not explicitly set, got: {json}"
    );
}

// ---------------------------------------------------------------------------
// All six tools registered in the extended registry
// ---------------------------------------------------------------------------

#[test]
fn test_all_six_mf_tools_registered_in_registry() {
    use ragent_tools_extended::create_extended_registry;

    let registry = create_extended_registry();
    let definitions = registry.definitions();
    let registered_names: std::collections::HashSet<String> =
        definitions.iter().map(|d| d.name.clone()).collect();

    for &name in MF_TOOL_NAMES {
        assert!(
            registered_names.contains(name),
            "tool '{name}' should be registered in create_extended_registry()"
        );
    }
}

// ---------------------------------------------------------------------------
// effective_hidden_tools combined with registry: hidden tools match registered
// ---------------------------------------------------------------------------

#[test]
fn test_hidden_mf_tools_are_subset_of_registered_tools() {
    use ragent_tools_extended::create_extended_registry;

    let registry = create_extended_registry();
    let registered_names: std::collections::HashSet<String> = registry
        .definitions()
        .iter()
        .map(|d| d.name.clone())
        .collect();

    let mut config = Config::default();
    config.tool_visibility.masterfetch = false;
    let hidden = config.effective_hidden_tools();

    // Every hidden mf_* tool should be a registered tool.
    for &name in MF_TOOL_NAMES {
        assert!(
            hidden.iter().any(|h| h == name),
            "tool '{name}' should be hidden"
        );
        assert!(
            registered_names.contains(name),
            "tool '{name}' should be registered"
        );
    }
}
