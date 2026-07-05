//! Regression tests for `/tools <switch> on|off`.

use std::sync::{Mutex, OnceLock};

#[path = "support/mod.rs"]
mod support;

struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn enter_temp_config_dir() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    temp
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn test_slash_tools_toggle_persists_and_updates_hidden_registry() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    app.execute_slash_command("/tools codeindex off");

    assert!(!app.tool_visibility.codeindex);
    assert_eq!(app.status, "tools: codeindex off");
    assert!(
        app.messages
            .last()
            .expect("message")
            .text_content()
            .contains("`codeindex` visibility is now **off**")
    );
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    let cfg = ragent_agent::Config::load().expect("load saved config");
    assert!(!cfg.tool_visibility.codeindex);
}

#[test]
fn test_slash_codeindex_off_updates_visibility_and_config() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.code_index_enabled = true;
    app.tool_visibility.codeindex = true;

    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    app.execute_slash_command("/codeindex off");

    assert!(!app.code_index_enabled);
    assert!(!app.tool_visibility.codeindex);
    assert_eq!(app.status, "codeindex: off");
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    let cfg = ragent_agent::Config::load().expect("load saved config");
    assert!(!cfg.code_index.enabled);
    assert!(!cfg.tool_visibility.codeindex);
}
