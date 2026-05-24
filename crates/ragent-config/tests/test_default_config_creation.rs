//! Tests for default config creation when no config exists.
//!
//! These tests mutate process-global state (env vars, current dir) and must
//! run serially to avoid cross-test contamination.

/// Sets XDG_CONFIG_HOME to a temp directory and returns the old value (if any).
fn with_temp_config_home(temp: &tempfile::TempDir) -> Option<String> {
    let original = std::env::var("XDG_CONFIG_HOME").ok();
    let fake = temp.path().join("fake_config");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &fake);
    }
    original
}

fn restore_config_home(original: Option<String>) {
    unsafe {
        if let Some(v) = original {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
}

/// When no global or project config exists, `Config::load` should create a
/// default `ragent.json` at the project level (`.ragent/ragent.json`).
#[test]
#[serial_test::serial]
fn test_creates_default_project_config_when_none_exists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ragent_dir = temp.path().join(".ragent");
    let project_config = ragent_dir.join("ragent.json");

    assert!(!project_config.exists(), "config should not exist pre-test");

    let original_cwd = std::env::current_dir().expect("cwd");
    let original_ragent_config = std::env::var("RAGENT_CONFIG").ok();
    let original_ragent_content = std::env::var("RAGENT_CONFIG_CONTENT").ok();
    let original_xdg = with_temp_config_home(&temp);

    std::env::set_current_dir(temp.path()).expect("chdir");
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::remove_var("RAGENT_CONFIG_CONTENT");
    }

    let config = ragent_config::Config::load().expect("config load should succeed");

    assert!(
        project_config.exists(),
        "default project config should have been created at {}",
        project_config.display()
    );

    let found = config.config_paths.iter().any(|p| {
        p.file_name().is_some_and(|f| f == "ragent.json")
            && p.parent()
                .is_some_and(|parent| parent.file_name().is_some_and(|f| f == ".ragent"))
    });
    assert!(
        found,
        "config_paths should contain the project-level ragent.json"
    );

    let content = std::fs::read_to_string(&project_config).expect("read created config");
    let roundtrip: ragent_config::Config =
        serde_json::from_str(&content).expect("created config should be valid JSON");
    // Config::default() sets default_agent to "" (empty string) via the
    // standard Default derive; the "general" fallback only applies during
    // serde deserialization when the field is absent.
    assert_eq!(
        roundtrip.default_agent, "",
        "default_agent from Config::default() is empty"
    );
    assert!(
        roundtrip.provider.is_empty(),
        "default provider map should be empty"
    );
    assert!(
        roundtrip.permission.is_empty(),
        "default permission list should be empty"
    );

    // Restore environment.
    std::env::set_current_dir(original_cwd).expect("restore cwd");
    unsafe {
        if let Some(v) = original_ragent_config {
            std::env::set_var("RAGENT_CONFIG", v);
        }
        if let Some(v) = original_ragent_content {
            std::env::set_var("RAGENT_CONFIG_CONTENT", v);
        }
    }
    restore_config_home(original_xdg);
}

/// When a project config already exists, `Config::load` should NOT overwrite it
/// with a fresh default.
#[test]
#[serial_test::serial]
fn test_does_not_overwrite_existing_project_config() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ragent_dir = temp.path().join(".ragent");
    let project_config = ragent_dir.join("ragent.json");

    std::fs::create_dir_all(&ragent_dir).expect("mkdir");
    std::fs::write(&project_config, r#"{"default_agent":"architect"}"#)
        .expect("write pre-existing config");

    let original_cwd = std::env::current_dir().expect("cwd");
    let original_ragent_config = std::env::var("RAGENT_CONFIG").ok();
    let original_ragent_content = std::env::var("RAGENT_CONFIG_CONTENT").ok();
    let original_xdg = with_temp_config_home(&temp);

    std::env::set_current_dir(temp.path()).expect("chdir");
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::remove_var("RAGENT_CONFIG_CONTENT");
    }

    let config = ragent_config::Config::load().expect("config load should succeed");

    assert_eq!(
        config.default_agent, "architect",
        "existing config should not be overwritten"
    );

    let content = std::fs::read_to_string(&project_config).expect("read existing config");
    assert!(
        content.contains("architect"),
        "original config file content should be preserved"
    );

    // Restore environment.
    std::env::set_current_dir(original_cwd).expect("restore cwd");
    unsafe {
        if let Some(v) = original_ragent_config {
            std::env::set_var("RAGENT_CONFIG", v);
        }
        if let Some(v) = original_ragent_content {
            std::env::set_var("RAGENT_CONFIG_CONTENT", v);
        }
    }
    restore_config_home(original_xdg);
}

/// When a global config exists but no project config exists,
/// `Config::load` should NOT create a default project config.
#[test]
#[serial_test::serial]
fn test_does_not_create_default_when_global_config_exists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let fake_config = temp.path().join("fake_config");
    let global_dir = fake_config.join("ragent");
    let global_config = global_dir.join("ragent.json");

    // Pre-create a global config inside our fake XDG_CONFIG_HOME.
    std::fs::create_dir_all(&global_dir).expect("mkdir global");
    std::fs::write(&global_config, r#"{"default_agent":"coder"}"#).expect("write global config");

    let project_dir = temp.path().join("project");
    let project_config = project_dir.join(".ragent").join("ragent.json");
    std::fs::create_dir_all(&project_dir.join(".ragent")).expect("mkdir project .ragent");

    let original_xdg = with_temp_config_home(&temp);
    let original_cwd = std::env::current_dir().expect("cwd");
    let original_ragent_config = std::env::var("RAGENT_CONFIG").ok();
    let original_ragent_content = std::env::var("RAGENT_CONFIG_CONTENT").ok();

    std::env::set_current_dir(&project_dir).expect("chdir");
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::remove_var("RAGENT_CONFIG_CONTENT");
    }

    let config = ragent_config::Config::load().expect("config load should succeed");

    assert_eq!(
        config.default_agent, "coder",
        "global config should be loaded"
    );

    assert!(
        !project_config.exists(),
        "default project config should NOT be created when global config exists"
    );

    // Restore environment.
    std::env::set_current_dir(original_cwd).expect("restore cwd");
    unsafe {
        if let Some(v) = original_ragent_config {
            std::env::set_var("RAGENT_CONFIG", v);
        }
        if let Some(v) = original_ragent_content {
            std::env::set_var("RAGENT_CONFIG_CONTENT", v);
        }
    }
    restore_config_home(original_xdg);
}
