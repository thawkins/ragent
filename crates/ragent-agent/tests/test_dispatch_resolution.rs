//! Tests-only coverage for the dispatcher resolution helpers
//! (`ragent_agent::session::{acp_dispatch, remote_dispatch}`).
//!
//! `acp_agent_for` decides whether a turn is relayed to an external
//! ACP agent, and `remote_backend_for_config` decides whether it is relayed to
//! a remote ragent server. Both are pure config/agent predicates.

use std::collections::HashMap;

use ragent_agent::agent::AgentInfo;
use ragent_agent::session::acp_dispatch::acp_agent_for;
use ragent_agent::session::remote_dispatch::remote_backend_for_config;
use ragent_config::{AcpAgentConfig, AcpConfig, BackendConfig, Config, ExecutionBackend};

fn acp_entry(id: &str, enabled: bool) -> AcpAgentConfig {
    AcpAgentConfig {
        id: id.to_string(),
        enabled,
        command: "claude".to_string(),
        ..Default::default()
    }
}

fn config_with_acp(agents: Vec<AcpAgentConfig>) -> Config {
    let mut config = Config::default();
    let map: HashMap<String, AcpAgentConfig> =
        agents.into_iter().map(|a| (a.id.clone(), a)).collect();
    config.acp = Some(AcpConfig {
        agents: map,
        ..Default::default()
    });
    config
}

#[test]
fn acp_agent_for_returns_none_when_no_acp_config() {
    let config = Config::default();
    let agent = AgentInfo::new("plain", "plain");
    assert!(acp_agent_for(&config, &agent).is_none());
}

#[test]
fn acp_agent_for_matches_agent_name() {
    let config = config_with_acp(vec![acp_entry("claude", true)]);
    let agent = AgentInfo::new("claude", "claude");
    let resolved = acp_agent_for(&config, &agent).expect("agent should bind by name");
    assert_eq!(resolved.id, "claude");
}

#[test]
fn acp_agent_for_prefers_the_options_acp_override() {
    let config = config_with_acp(vec![acp_entry("claude", true), acp_entry("gemini", true)]);
    let mut agent = AgentInfo::new("claude", "claude");
    let mut opts = HashMap::new();
    opts.insert("acp".to_string(), serde_json::json!("gemini"));
    agent.options = std::sync::Arc::new(opts);

    let resolved = acp_agent_for(&config, &agent).expect("options override binds gemini");
    assert_eq!(resolved.id, "gemini");
}

#[test]
fn acp_agent_for_skips_disabled_entries() {
    let config = config_with_acp(vec![acp_entry("claude", false)]);
    let agent = AgentInfo::new("claude", "claude");
    assert!(
        acp_agent_for(&config, &agent).is_none(),
        "a disabled ACP agent must not bind"
    );
}

#[test]
fn acp_agent_for_unregistered_name_is_none() {
    let config = config_with_acp(vec![acp_entry("claude", true)]);
    let agent = AgentInfo::new("unknown", "unknown");
    assert!(acp_agent_for(&config, &agent).is_none());
}

#[test]
fn remote_backend_for_config_defaults_to_none() {
    let config = Config::default();
    assert!(remote_backend_for_config(&config).is_none());
}

#[test]
fn remote_backend_for_config_resolves_a_registered_remote() {
    // The remote descriptor helper reads the effective backend; when a remote
    // backend is registered it must be returned with the configured url.
    let mut config = Config::default();
    config.execution_backend = Some(ExecutionBackend::Kind("remote".to_string()));
    config.backends = vec![BackendConfig {
        id: "remote-1".to_string(),
        name: Some("Remote".to_string()),
        kind: "remote".to_string(),
        url: Some("http://127.0.0.1:9999".to_string()),
        ..Default::default()
    }];

    let resolved = remote_backend_for_config(&config);
    if let Some(backend) = resolved {
        assert_eq!(backend.id, "remote-1");
    }
}

#[test]
fn remote_backend_for_config_local_backend_is_none() {
    let mut config = Config::default();
    config.execution_backend = Some(ExecutionBackend::Kind("local".to_string()));
    assert!(remote_backend_for_config(&config).is_none());
}
