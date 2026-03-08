use ragent_core::agent::resolve_agent;
use ragent_core::config::Config;

#[test]
fn test_agent_resolve_builtin() {
    let config = Config::default();
    let agent = resolve_agent("general", &config).unwrap();
    assert_eq!(agent.name, "general");
    assert_eq!(agent.description, "General-purpose coding agent");
    assert!(agent.model.is_some());
}

#[test]
fn test_agent_resolve_unknown_fails() {
    let config = Config::default();
    // Unknown agents get a fallback custom agent rather than an error
    let agent = resolve_agent("nonexistent_agent_xyz", &config).unwrap();
    assert_eq!(agent.name, "nonexistent_agent_xyz");
    assert!(agent.description.contains("Custom agent"));
}
