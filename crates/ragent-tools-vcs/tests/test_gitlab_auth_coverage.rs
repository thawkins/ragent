//! Coverage tests for `gitlab::auth` - the layered config/token resolution
//! (`load_config`, `load_token_checked`), database persistence
//! (`save_config`/`delete_config`), and the legacy file migration.
//!
//! A private in-memory [`StorageBackend`] stands in for the encrypted store so
//! no database is touched; environment variables and the legacy `~/.ragent/`
//! files are redirected at a temp directory under a process-wide lock. No
//! network is used.

use std::path::PathBuf;
use std::sync::Mutex;

use ragent_tools_vcs::gitlab::auth::{
    GitLabConfig, delete_config, load_config, load_token_checked, migrate_legacy_files,
    save_config, save_token,
};
use ragent_tools_vcs::storage::StorageBackend;

/// Serialises every test in this binary that mutates the environment or the
/// process-global token cache.
static LOCK: Mutex<()> = Mutex::new(());

/// A minimal in-memory credential/settings store.
#[derive(Default)]
struct MemStore {
    token: Mutex<Option<String>>,
    setting: Mutex<Option<String>>,
}

impl StorageBackend for MemStore {
    fn get_provider_auth(&self, _provider_id: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn set_provider_auth(&self, _provider_id: &str, api_key: &str) -> anyhow::Result<()> {
        *self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(api_key.to_string());
        Ok(())
    }

    fn delete_provider_auth(&self, _provider_id: &str) -> anyhow::Result<()> {
        *self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }

    fn get_setting(&self, _key: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .setting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn set_setting(&self, _key: &str, value: &str) -> anyhow::Result<()> {
        *self
            .setting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(value.to_string());
        Ok(())
    }

    fn delete_setting(&self, _key: &str) -> anyhow::Result<()> {
        *self
            .setting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }
}

/// RAII guard restoring the named environment variables on drop.
struct EnvRestore(Vec<(&'static str, Option<String>)>);

impl EnvRestore {
    fn apply(changes: &[(&'static str, Option<String>)]) -> Self {
        let mut saved = Vec::new();
        for (name, value) in changes {
            let previous = std::env::var(name).ok();
            // SAFETY: test-only; a process-wide lock serialises all callers.
            #[allow(unsafe_code)]
            unsafe {
                match value.clone() {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
            saved.push((*name, previous));
        }
        Self(saved)
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (name, previous) in self.0.drain(..) {
            // SAFETY: test-only; see `apply`.
            #[allow(unsafe_code)]
            unsafe {
                match previous {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

/// Run `f` with `HOME`/`XDG_CONFIG_HOME` pointed at a fresh temp dir and the
/// GitLab env layer cleared, all under the process lock.
fn with_isolated_env<T>(f: impl FnOnce(&PathBuf) -> T) -> T {
    let _lock = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let home = tempfile::tempdir().expect("tempdir");
    let home_str = home.path().to_string_lossy().to_string();
    let _restore = EnvRestore::apply(&[
        ("HOME", Some(home_str.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{home_str}/.config"))),
        ("GITLAB_TOKEN", None),
        ("GITLAB_URL", None),
        ("GITLAB_USERNAME", None),
    ]);
    f(&home.path().to_path_buf())
}

#[test]
fn test_save_and_load_config_round_trip_via_store() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        let cfg = GitLabConfig {
            instance_url: "https://gitlab.example.com".to_string(),
            username: "alice".to_string(),
        };
        save_config(&store, &cfg).expect("save_config");
        let loaded = load_config(&store).expect("config present");
        assert_eq!(loaded.instance_url, "https://gitlab.example.com");
        assert_eq!(loaded.username, "alice");
    });
}

#[test]
fn test_delete_config_clears_store() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        save_config(
            &store,
            &GitLabConfig {
                instance_url: "https://gl".to_string(),
                username: "bob".to_string(),
            },
        )
        .expect("save");
        delete_config(&store).expect("delete");
        assert!(load_config(&store).is_none(), "config must be gone");
    });
}

#[test]
fn test_load_config_env_vars_win() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        // Store holds a stale config; env must override both fields.
        save_config(
            &store,
            &GitLabConfig {
                instance_url: "https://stored.example".to_string(),
                username: "stored".to_string(),
            },
        )
        .expect("seed");
        let _env = EnvRestore::apply(&[
            ("GITLAB_URL", Some("https://env.example".to_string())),
            ("GITLAB_USERNAME", Some("envuser".to_string())),
        ]);
        let cfg = load_config(&store).expect("env config");
        assert_eq!(cfg.instance_url, "https://env.example");
        assert_eq!(cfg.username, "envuser");
    });
}

#[test]
fn test_load_config_only_url_env_fills_from_store() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        save_config(
            &store,
            &GitLabConfig {
                instance_url: "https://stored.example".to_string(),
                username: "storeduser".to_string(),
            },
        )
        .expect("seed");
        // Only the URL env var set: username comes from the store.
        let _env =
            EnvRestore::apply(&[("GITLAB_URL", Some("https://urlonly.example".to_string()))]);
        let cfg = load_config(&store).expect("merged config");
        assert_eq!(cfg.instance_url, "https://urlonly.example");
        assert_eq!(cfg.username, "storeduser");
    });
}

#[test]
fn test_load_config_none_when_only_one_field_present() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        // Only a URL, no username anywhere: the filter drops the partial config.
        let _env = EnvRestore::apply(&[("GITLAB_URL", Some("https://lonely.example".to_string()))]);
        assert!(
            load_config(&store).is_none(),
            "a partial config (no username) must not be returned"
        );
    });
}

#[test]
fn test_load_token_checked_env_wins_over_store() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        save_token(&store, "glpat-stored").expect("seed");
        let _env = EnvRestore::apply(&[("GITLAB_TOKEN", Some("glpat-env".to_string()))]);
        assert_eq!(
            load_token_checked(&store).expect("ok").as_deref(),
            Some("glpat-env")
        );
    });
}

#[test]
fn test_load_token_checked_blank_env_falls_through_to_store() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        save_token(&store, "glpat-store-value").expect("seed");
        // A present-but-blank env token must not shadow the store.
        let _env = EnvRestore::apply(&[("GITLAB_TOKEN", Some("   ".to_string()))]);
        assert_eq!(
            load_token_checked(&store).expect("ok").as_deref(),
            Some("glpat-store-value")
        );
    });
}

#[test]
fn test_load_token_checked_none_without_any_layer() {
    with_isolated_env(|_home| {
        let store = MemStore::default();
        assert_eq!(load_token_checked(&store).expect("ok"), None);
    });
}

#[test]
fn test_migrate_legacy_files_imports_and_deletes() {
    with_isolated_env(|home| {
        let legacy = home.join(".ragent");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("gitlab_token"), "glpat-legacy\n").unwrap();
        std::fs::write(
            legacy.join("gitlab_config.json"),
            r#"{"instance_url":"https://legacy.example","username":"legacyuser"}"#,
        )
        .unwrap();

        let store = MemStore::default();
        migrate_legacy_files(&store);

        assert_eq!(
            store.get_provider_auth("gitlab").unwrap().as_deref(),
            Some("glpat-legacy"),
            "legacy token must be imported"
        );
        assert!(
            !legacy.join("gitlab_token").exists(),
            "legacy token file must be removed"
        );
        let cfg = load_config(&store).expect("imported config");
        assert_eq!(cfg.instance_url, "https://legacy.example");
        assert!(
            !legacy.join("gitlab_config.json").exists(),
            "legacy config file must be removed"
        );
    });
}

#[test]
fn test_migrate_legacy_files_is_noop_when_store_already_populated() {
    with_isolated_env(|home| {
        let legacy = home.join(".ragent");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("gitlab_token"), "glpat-legacy\n").unwrap();

        let store = MemStore::default();
        save_token(&store, "glpat-existing").expect("seed");
        migrate_legacy_files(&store);

        assert_eq!(
            store.get_provider_auth("gitlab").unwrap().as_deref(),
            Some("glpat-existing"),
            "an existing stored token must not be overwritten"
        );
        // The legacy file remains because there was nothing to import.
        assert!(legacy.join("gitlab_token").exists());
    });
}

#[test]
fn test_migrate_legacy_files_ignores_blank_and_missing_files() {
    with_isolated_env(|home| {
        let legacy = home.join(".ragent");
        std::fs::create_dir_all(&legacy).unwrap();
        // A blank token file, and no config file at all.
        std::fs::write(legacy.join("gitlab_token"), "   \n").unwrap();

        let store = MemStore::default();
        migrate_legacy_files(&store);

        assert_eq!(store.get_provider_auth("gitlab").unwrap(), None);
        assert!(legacy.join("gitlab_token").exists(), "blank file untouched");
    });
}
