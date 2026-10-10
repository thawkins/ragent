//! Integration tests for the `execution_backend` config key and the backend
//! registry descriptors (spec `openhands` T-001; FR-001, FR-004, FR-019).

use ragent_config::{BackendConfig, Config, ExecutionBackend, ExecutionBackendKind};

#[test]
fn default_backend_is_local() {
    // FR-019: with no key configured, the active backend resolves to `local`.
    let config = Config::default();
    assert!(config.execution_backend.is_none());
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Local
    );
    assert!(!config.has_unknown_execution_backend());
}

#[test]
fn all_four_kinds_parse_and_round_trip() {
    // FR-001: the trait vocabulary covers local, docker, podman, and remote.
    for (label, expected) in [
        ("local", ExecutionBackendKind::Local),
        ("docker", ExecutionBackendKind::Docker),
        ("podman", ExecutionBackendKind::Podman),
        ("remote", ExecutionBackendKind::Remote),
    ] {
        let json = format!(r#"{{ "execution_backend": "{label}" }}"#);
        let config: Config = serde_json::from_str(&json).expect("parse config");
        assert_eq!(
            config.effective_execution_backend(),
            expected,
            "label {label} should resolve to {expected}"
        );
        assert_eq!(expected.as_str(), label);
        assert_eq!(expected.to_string(), label);
    }
}

#[test]
fn labels_are_case_insensitive_and_trimmed() {
    let config: Config =
        serde_json::from_str(r#"{ "execution_backend": "  PodMan " }"#).expect("parse config");
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Podman
    );
}

#[test]
fn object_form_carries_the_connection_descriptor() {
    // FR-004: the object spelling carries id, name, kind, and connection fields.
    let json = r#"
    {
      "execution_backend": {
        "id": "box",
        "name": "Docker sandbox",
        "kind": "docker",
        "image": "ghcr.io/example/sandbox:latest",
        "workspace": "."
      }
    }
    "#;
    let config: Config = serde_json::from_str(json).expect("parse config");
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Docker
    );
    match config.execution_backend.as_ref().expect("value present") {
        ExecutionBackend::Config(cfg) => {
            assert_eq!(cfg.id, "box");
            assert_eq!(cfg.display_name(), "Docker sandbox");
            assert_eq!(cfg.image.as_deref(), Some("ghcr.io/example/sandbox:latest"));
            assert_eq!(cfg.workspace.as_deref(), Some("."));
        }
        ExecutionBackend::Kind(_) => panic!("object form should parse as a descriptor"),
    }
}

#[test]
fn unknown_label_resolves_to_local_and_is_flagged() {
    // A typo must not silently select a more capable backend: resolution stays
    // on `local` (FR-019) and the config reports the unknown value.
    let config: Config =
        serde_json::from_str(r#"{ "execution_backend": "kubernetes" }"#).expect("parse config");
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Local
    );
    assert!(config.has_unknown_execution_backend());
}

#[test]
fn descriptor_without_kind_is_flagged_and_resolves_local() {
    let config: Config =
        serde_json::from_str(r#"{ "execution_backend": { "id": "mystery" } }"#).expect("parse");
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Local
    );
    assert!(config.has_unknown_execution_backend());
}

#[test]
fn overlay_backend_overrides_base() {
    let mut base = Config::default();
    base.execution_backend = Some(ExecutionBackend::from(ExecutionBackendKind::Local));

    let mut overlay = Config::default();
    overlay.execution_backend = Some(ExecutionBackend::from(ExecutionBackendKind::Podman));

    let merged = Config::merge(base, overlay);
    assert_eq!(
        merged.effective_execution_backend(),
        ExecutionBackendKind::Podman
    );
}

#[test]
fn absent_overlay_backend_preserves_base() {
    let mut base = Config::default();
    base.execution_backend = Some(ExecutionBackend::from(ExecutionBackendKind::Docker));

    let merged = Config::merge(base, Config::default());
    assert_eq!(
        merged.effective_execution_backend(),
        ExecutionBackendKind::Docker
    );
}

#[test]
fn overlay_typo_still_wins_over_base() {
    // The overlay's explicit (if invalid) value replaces the base value rather
    // than falling through, so the typo surfaces instead of silently keeping a
    // different backend.
    let mut base = Config::default();
    base.execution_backend = Some(ExecutionBackend::from(ExecutionBackendKind::Remote));

    let overlay: Config =
        serde_json::from_str(r#"{ "execution_backend": "dockerr" }"#).expect("parse");
    let merged = Config::merge(base, overlay);
    assert!(merged.has_unknown_execution_backend());
    assert_eq!(
        merged.effective_execution_backend(),
        ExecutionBackendKind::Local
    );
}

#[test]
fn backend_entries_merge_by_id() {
    let mut base = Config::default();
    base.backends = vec![
        BackendConfig {
            id: "one".to_string(),
            name: Some("First".to_string()),
            kind: "local".to_string(),
            ..BackendConfig::default()
        },
        BackendConfig {
            id: "two".to_string(),
            kind: "remote".to_string(),
            url: Some("http://127.0.0.1:3001".to_string()),
            ..BackendConfig::default()
        },
    ];

    let mut overlay = Config::default();
    overlay.backends = vec![BackendConfig {
        id: "one".to_string(),
        name: Some("Renamed".to_string()),
        kind: "podman".to_string(),
        ..BackendConfig::default()
    }];

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.backends.len(), 2, "duplicate ids must collapse");
    let one = merged
        .backends
        .iter()
        .find(|b| b.id == "one")
        .expect("one present");
    assert_eq!(one.display_name(), "Renamed");
    assert_eq!(one.kind_parsed(), Some(ExecutionBackendKind::Podman));
    let two = merged
        .backends
        .iter()
        .find(|b| b.id == "two")
        .expect("two preserved");
    assert_eq!(two.kind_parsed(), Some(ExecutionBackendKind::Remote));
}

#[test]
fn serialised_default_omits_the_backend_keys() {
    let json = serde_json::to_value(Config::default()).expect("serialise config");
    assert!(
        json.get("execution_backend").is_none(),
        "an unset backend must be omitted from ragent.json"
    );
    assert!(
        json.get("backends").is_none(),
        "an empty registry must be omitted from ragent.json"
    );
}

#[test]
fn debug_redacts_the_remote_api_key() {
    // FR-035 / ANTIPAT H-2: the descriptor's api_key is a secret; the redacting
    // Debug impl must not print it.
    let json = r#"
    {
      "execution_backend": {
        "id": "remote-a",
        "kind": "remote",
        "url": "http://127.0.0.1:3001",
        "api_key": "sk-do-not-print-me"
      }
    }
    "#;
    let config: Config = serde_json::from_str(json).expect("parse config");
    let rendered = format!("{config:?}");
    assert!(
        !rendered.contains("sk-do-not-print-me"),
        "the remote api_key must be redacted: {rendered}"
    );
}

#[test]
fn round_trip_preserves_a_remote_descriptor() {
    let mut config = Config::default();
    config.execution_backend = Some(ExecutionBackend::Config(BackendConfig {
        id: "remote-a".to_string(),
        name: Some("Remote A".to_string()),
        kind: "remote".to_string(),
        url: Some("http://127.0.0.1:3001".to_string()),
        api_key: Some("sk-secret".to_string()),
        ..BackendConfig::default()
    }));

    let json = serde_json::to_string(&config).expect("serialise config");
    let decoded: Config = serde_json::from_str(&json).expect("deserialise config");
    assert_eq!(
        decoded.effective_execution_backend(),
        ExecutionBackendKind::Remote
    );
    match decoded.execution_backend.as_ref().expect("value present") {
        ExecutionBackend::Config(cfg) => {
            assert_eq!(cfg.id, "remote-a");
            assert_eq!(cfg.url.as_deref(), Some("http://127.0.0.1:3001"));
            assert_eq!(cfg.api_key.as_deref(), Some("sk-secret"));
        }
        ExecutionBackend::Kind(_) => panic!("descriptor should survive a round trip"),
    }
}

#[test]
fn kind_helpers_are_consistent() {
    assert!(ExecutionBackendKind::Local.is_local());
    assert!(!ExecutionBackendKind::Docker.is_local());
    assert!(!ExecutionBackendKind::Podman.is_local());
    assert!(!ExecutionBackendKind::Remote.is_local());
    assert_eq!(
        ExecutionBackendKind::from_label("REMOTE"),
        Some(ExecutionBackendKind::Remote)
    );
    assert_eq!(ExecutionBackendKind::from_label("nope"), None);
}

// -- ACP client config (spec `openhands` T-007; FR-015, FR-022) ----------------

#[test]
fn acp_absent_by_default_and_inert() {
    // FR-015: with no `acp` block the subsystem is inert and no ACP agent is
    // offered.
    let config = Config::default();
    assert!(config.acp.is_none());
    assert!(!config.acp_enabled());
    assert!(config.enabled_acp_agents().is_empty());
    assert!(config.resolve_acp_agent(None).is_none());
}

#[test]
fn acp_parses_agents_and_omits_the_key_when_default() {
    // FR-015: the `acp` block round-trips and is omitted from serialised output
    // when unset, so a default config does not gain the key.
    let json = r#"
    {
      "acp": {
        "default_agent": "claude",
        "agents": {
          "claude": {
            "id": "claude",
            "name": "Claude Code",
            "command": "claude-code-acp",
            "args": ["--stdio"]
          }
        }
      }
    }
    "#;
    let config: Config = serde_json::from_str(json).expect("parse acp config");
    assert!(config.acp_enabled());
    let (id, agent) = config.enabled_acp_agents()[0];
    assert_eq!(id, "claude");
    assert_eq!(agent.display_name(), "Claude Code");
    assert_eq!(agent.command, "claude-code-acp");
    assert_eq!(agent.args, vec!["--stdio".to_string()]);
    assert!(agent.enabled);
    assert_eq!(agent.turn_timeout_secs, 300);

    let default_json = serde_json::to_string(&Config::default()).expect("serialise default");
    assert!(
        !default_json.contains("\"acp\""),
        "the default config must not emit the acp key: {default_json}"
    );
}

#[test]
fn disabled_acp_agents_are_skipped() {
    // FR-015/FR-022: a disabled entry is not offered and does not make the
    // subsystem "enabled".
    let json = r#"
    {
      "acp": {
        "agents": {
          "retired": { "id": "retired", "command": "old-acp", "enabled": false }
        }
      }
    }
    "#;
    let config: Config = serde_json::from_str(json).expect("parse acp config");
    assert!(!config.acp_enabled());
    assert!(config.enabled_acp_agents().is_empty());
    assert!(config.resolve_acp_agent(Some("retired")).is_none());
}

#[test]
fn resolve_acp_agent_prefers_selection_then_default_then_first() {
    // FR-022: selection order is explicit id -> default_agent -> first enabled.
    let json = r#"
    {
      "acp": {
        "default_agent": "beta",
        "agents": {
          "alpha": { "id": "alpha", "command": "a" },
          "beta":  { "id": "beta",  "command": "b" }
        }
      }
    }
    "#;
    let config: Config = serde_json::from_str(json).expect("parse acp config");

    assert_eq!(
        config
            .resolve_acp_agent(Some("alpha"))
            .map(|a| a.id.as_str()),
        Some("alpha")
    );
    assert_eq!(
        config.resolve_acp_agent(None).map(|a| a.id.as_str()),
        Some("beta"),
        "default_agent wins when nothing is selected"
    );
    assert_eq!(
        config
            .resolve_acp_agent(Some("missing"))
            .map(|a| a.id.as_str()),
        Some("beta"),
        "an unknown selection falls back to the default"
    );
}

#[test]
fn acp_overlay_merges_by_id_and_wins_on_default() {
    // FR-015: a project-level `acp` block extends the base registry rather than
    // discarding it; a same-id entry replaces the base one.
    let mut base = Config::default();
    base.acp = Some(ragent_config::AcpConfig {
        default_agent: None,
        server_enabled: false,
        server_agent: None,
        agents: std::collections::HashMap::from([(
            "alpha".to_string(),
            ragent_config::AcpAgentConfig {
                id: "alpha".to_string(),
                command: "base-alpha".to_string(),
                ..ragent_config::AcpAgentConfig::default()
            },
        )]),
    });

    let overlay: Config = serde_json::from_str(
        r#"
        {
          "acp": {
            "default_agent": "beta",
            "agents": {
              "alpha": { "id": "alpha", "command": "override-alpha" },
              "beta":  { "id": "beta",  "command": "beta-cmd" }
            }
          }
        }
        "#,
    )
    .expect("parse overlay");

    let merged = Config::merge_project(base, overlay);
    let agents = merged.enabled_acp_agents();
    assert_eq!(agents.len(), 2, "overlay extends rather than replaces");
    let alpha = merged
        .acp
        .as_ref()
        .unwrap()
        .agents
        .get("alpha")
        .expect("alpha present");
    assert_eq!(alpha.command, "override-alpha");
    assert_eq!(
        merged.acp.as_ref().and_then(|a| a.default_agent.as_deref()),
        Some("beta")
    );
}
