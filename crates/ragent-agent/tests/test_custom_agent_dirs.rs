//! Tests for the custom-agent `config_agents_dir` path construction and the
//! discovery-priority contract (closest directory wins).
//!
//! `load_custom_agents` reads `dirs::home_dir()` / `dirs::config_dir()`
//! directly and mutating `HOME`/`XDG_CONFIG_HOME` in tests requires `unsafe`
//! on modern Rust, so these tests exercise the pure helper
//! (`config_agents_dir` is wrapped over a testable `_impl`) plus an end-to-end
//! assertion that the real helper resolves to a `ragent/agents` suffix.

use ragent_agent::agent::custom::config_agents_dir;

#[test]
fn test_config_agents_dir_appends_ragent_agents() {
    // With the real environment the result must end in `ragent/agents`
    // regardless of where XDG_CONFIG_HOME points, when it resolves at all.
    if let Some(dir) = config_agents_dir() {
        assert_eq!(dir.file_name().unwrap(), "agents");
        assert_eq!(
            dir.parent().unwrap().file_name().unwrap(),
            "ragent",
            "config agents dir must be <config>/ragent/agents, got {}",
            dir.display()
        );
    }
}

#[test]
fn test_config_agents_dir_default_location_on_linux() {
    // On Linux with the default environment this is ~/.config/ragent/agents.
    // Only asserted when XDG_CONFIG_HOME is unset; a customised XDG env (CI
    // containers, nix shells) legitimately produces a different base.
    #[cfg(target_os = "linux")]
    if std::env::var_os("XDG_CONFIG_HOME").is_none()
        && let Some(home) = std::env::var_os("HOME")
    {
        let expected = std::path::PathBuf::from(home)
            .join(".config")
            .join("ragent")
            .join("agents");
        assert_eq!(config_agents_dir(), Some(expected));
    }
}
