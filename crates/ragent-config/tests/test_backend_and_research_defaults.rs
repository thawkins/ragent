//! Coverage for `Config` accessors that resolve backend descriptors and for
//! the `is_default` aliases of the research sub-configs.

use ragent_config::Config;
use ragent_config::config::{
    ResearchEvaluateConfig, ResearchModelsConfig, ResearchSupervisorConfig,
};

fn parse(json: &str) -> Config {
    serde_json::from_str(json).expect("parse config")
}

#[test]
fn test_effective_backend_config_inline_and_label_forms() {
    // Inline descriptor is returned as-is.
    let cfg =
        parse(r#"{ "execution_backend": { "id": "box", "kind": "podman", "image": "img:1" } }"#);
    let backend = cfg.effective_backend_config().expect("inline descriptor");
    assert_eq!(backend.id, "box");
    assert_eq!(backend.image.as_deref(), Some("img:1"));
    assert_eq!(backend.display_name(), "box");

    // A bare label resolves against the registry by kind.
    let cfg = parse(
        r#"{ "execution_backend": "podman",
             "backends": [ { "id": "reg", "kind": "podman", "image": "img:2" } ] }"#,
    );
    let backend = cfg.effective_backend_config().expect("registry lookup");
    assert_eq!(backend.id, "reg");

    // An object naming a registered entry by id (no kind) resolves against the
    // registry entry whose kind matches via `kind_parsed`.
    let cfg = parse(
        r#"{ "execution_backend": "docker",
             "backends": [ { "id": "d", "kind": "docker", "image": "img:3" } ] }"#,
    );
    assert_eq!(
        cfg.effective_backend_config().map(|b| b.id.as_str()),
        Some("d")
    );

    // An unknown label resolves to no configured entry.
    let cfg = parse(r#"{ "execution_backend": "nonsense" }"#);
    assert!(cfg.effective_backend_config().is_none());
    assert!(cfg.has_unknown_execution_backend());

    // Nothing configured -> None.
    assert!(Config::default().effective_backend_config().is_none());
}

#[test]
fn test_backend_config_helpers() {
    let cfg = parse(r#"{ "backends": [ { "id": "b", "name": "Friendly", "kind": "PODMAN" } ] }"#);
    let backend = &cfg.backends[0];
    // Explicit name wins; kind parses case-insensitively.
    assert_eq!(backend.display_name(), "Friendly");
    assert_eq!(
        backend.kind_parsed(),
        Some(ragent_config::ExecutionBackendKind::Podman)
    );

    let unnamed = parse(r#"{ "backends": [ { "id": "raw", "kind": "local" } ] }"#);
    assert_eq!(unnamed.backends[0].display_name(), "raw");
    assert!(unnamed.backends[0].kind_parsed().is_some());
}

#[test]
fn test_execution_backend_kind_helpers() {
    use ragent_config::ExecutionBackendKind;
    assert!(ExecutionBackendKind::Local.is_local());
    assert!(!ExecutionBackendKind::Docker.is_local());
    assert_eq!(
        ExecutionBackendKind::from_label(" DOCKER "),
        Some(ExecutionBackendKind::Docker)
    );
    assert_eq!(ExecutionBackendKind::from_label("bogus"), None);
    assert_eq!(ExecutionBackendKind::Podman.as_str(), "podman");
    assert_eq!(ExecutionBackendKind::Remote.to_string(), "remote");
}

#[test]
fn test_execution_backend_enum_accessors() {
    use ragent_config::{ExecutionBackend, ExecutionBackendKind};

    let bare: ExecutionBackend = serde_json::from_str(r#""podman""#).expect("parse bare label");
    assert_eq!(bare.kind(), Some(ExecutionBackendKind::Podman));
    assert_eq!(bare.label(), "podman");
    assert_eq!(bare.id(), None);

    let descriptor: ExecutionBackend =
        serde_json::from_str(r#"{ "id": "box", "kind": "remote", "url": "http://host" }"#)
            .expect("parse descriptor");
    assert_eq!(descriptor.id(), Some("box"));
    assert_eq!(descriptor.label(), "remote");
    assert_eq!(descriptor.kind(), Some(ExecutionBackendKind::Remote));

    // `From<ExecutionBackendKind>` emits the canonical lowercase label.
    let from_kind: ExecutionBackend = ExecutionBackendKind::Docker.into();
    assert_eq!(from_kind.label(), "docker");
}

#[test]
fn test_research_subconfig_is_default_aliases() {
    let supervisor = ResearchSupervisorConfig::default();
    assert!(supervisor.is_empty());
    assert!(supervisor.is_default());

    let models = ResearchModelsConfig::default();
    assert!(models.is_empty());
    assert!(models.is_default());

    let evaluate = ResearchEvaluateConfig::default();
    assert!(evaluate.is_empty());
    assert!(evaluate.is_default());

    let mut evaluate = ResearchEvaluateConfig::default();
    evaluate.merge(&ResearchEvaluateConfig { enabled: true });
    assert!(!evaluate.is_empty());
    assert!(!evaluate.is_default());
    assert!(evaluate.enabled);
}
