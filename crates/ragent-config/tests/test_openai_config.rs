//! Integration tests for the `openai` config section and the OpenAI-compatible
//! surface's token gate (spec `openhands` T-010; FR-012, FR-024, FR-032).
//!
//! FR-032 requires the surface to refuse to start when it is enabled without a
//! configured bearer token; FR-024 requires every request to it to carry the
//! same bearer token as the native REST API. These tests exercise the config
//! helpers the startup gate reads:
//!
//! - `openai_surface_enabled()` reflects the `openai.enabled` flag;
//! - `openai_surface_token()` resolves `openai.token` and returns `None` when it
//!   is absent or blank (the refuse-to-start condition, FR-032);
//! - the untrusted project overlay cannot set the surface token.

use ragent_config::{Config, OpenAiConfig};

/// FR-032: a surface-enabled config with no token has no resolvable credential,
/// which is the refuse-to-start condition the server gates on.
#[test]
fn enabled_without_a_token_has_no_resolvable_token() {
    let config: Config =
        serde_json::from_str(r#"{ "openai": { "enabled": true } }"#).expect("parse config");
    assert!(config.openai_surface_enabled());
    assert_eq!(
        config.openai_surface_token(),
        None,
        "an enabled surface with no token must not resolve a credential (FR-032)"
    );
}

/// A blank token is treated as absent - the surface must still refuse to start
/// rather than run with an effectively empty credential.
#[test]
fn blank_token_is_not_a_credential() {
    let config: Config =
        serde_json::from_str(r#"{ "openai": { "enabled": true, "token": "   " } }"#)
            .expect("parse config");
    assert!(config.openai_surface_enabled());
    assert_eq!(config.openai_surface_token(), None);
}

/// FR-024: a stored `openai.token` makes the surface resolvable, so the server
/// starts it authenticated rather than refusing.
#[test]
fn configured_token_is_resolved() {
    let config: Config =
        serde_json::from_str(r#"{ "openai": { "enabled": true, "token": "sk-ragent-local" } }"#)
            .expect("parse config");
    assert!(config.openai_surface_enabled());
    assert_eq!(
        config.openai_surface_token().as_deref(),
        Some("sk-ragent-local")
    );
}

/// An absent `openai` section leaves the surface gating inert and resolves no
/// token (the endpoints remain reachable only under the native bearer check).
#[test]
fn absent_section_is_inert() {
    let config = Config::default();
    assert!(config.openai.is_none());
    assert!(!config.openai_surface_enabled());
    assert_eq!(config.openai_surface_token(), None);
}

/// FR-035/FR-024: a project-local overlay must not be able to set the surface
/// token (that would let repository content choose the credential guarding the
/// whole API), so the section is dropped by `merge_project`.
#[test]
fn project_overlay_cannot_set_the_surface_token() {
    let base = Config::default();
    let mut overlay = Config::default();
    overlay.openai = Some(OpenAiConfig {
        enabled: false,
        token: Some("attacker-chosen-token".to_string()),
    });

    let merged = Config::merge_project(base, overlay);
    assert!(
        merged.openai.is_none(),
        "the untrusted project overlay must not carry an `openai` section"
    );
}

/// The trusted merge path still carries the section: an overlay token wins over
/// the base, and an overlay that enables the surface enables it.
#[test]
fn trusted_merge_carries_the_section() {
    let mut base = Config::default();
    base.openai = Some(OpenAiConfig {
        enabled: false,
        token: Some("base-token".to_string()),
    });
    let overlay: Config =
        serde_json::from_str(r#"{ "openai": { "enabled": true, "token": "overlay-token" } }"#)
            .expect("parse config");

    let merged = Config::merge(base, overlay);
    let section = merged.openai.expect("section carried");
    assert!(section.enabled, "the overlay enables the surface");
    assert_eq!(section.token.as_deref(), Some("overlay-token"));
}
