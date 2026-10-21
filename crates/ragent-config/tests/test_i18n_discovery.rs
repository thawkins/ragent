//! Coverage for the i18n discovery/wiring surface: locale-tag expansion,
//! locale directory resolution, the ambient-locale reader, `resolve`, `persist`,
//! and `catalog_exists`.
//!
//! The i18n runtime state and the process environment are process-global, so
//! the tests are serialised and redirect `XDG_CONFIG_HOME` at a temp root.

#![allow(unsafe_code)]

use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use ragent_config::{Config, MessageKey};

fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct ResetGuard;
impl Drop for ResetGuard {
    fn drop(&mut self) {
        ragent_config::i18n::reset();
    }
}

struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
    temp: tempfile::TempDir,
}

impl EnvGuard {
    fn root(&self) -> &Path {
        self.temp.path()
    }
}

impl EnvGuard {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let keys = ["XDG_CONFIG_HOME", "LC_ALL", "LC_MESSAGES", "LANG"];
        let saved = keys.iter().map(|k| (*k, env::var(k).ok())).collect();
        // SAFETY: Rust 2024 makes `set_var` unsafe; this target opts in and
        // runs its tests serially.
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp.path().join("config"));
            env::remove_var("LC_ALL");
            env::remove_var("LC_MESSAGES");
            env::remove_var("LANG");
        }
        Self { saved, temp }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            match v {
                // SAFETY: see `EnvGuard::new`.
                Some(val) => unsafe { env::set_var(k, val) },
                None => unsafe { env::remove_var(k) },
            }
        }
    }
}

#[test]
#[serial_test::serial]
fn test_locale_candidates_expands_tags() {
    assert_eq!(
        ragent_config::i18n::locale_candidates("fr_FR.UTF-8"),
        vec!["fr_fr".to_string(), "fr".to_string()]
    );
    assert_eq!(
        ragent_config::i18n::locale_candidates("zh-Hans"),
        vec!["zh-hans".to_string(), "zh".to_string()]
    );
    // Pure language tag has a single candidate (no duplicate).
    assert_eq!(
        ragent_config::i18n::locale_candidates("de"),
        vec!["de".to_string()]
    );
    assert_eq!(ragent_config::i18n::locale_candidates("   ").len(), 0);
    assert_eq!(ragent_config::i18n::locale_candidates("").len(), 0);
}

#[test]
#[serial_test::serial]
fn test_locale_dirs_prefers_project_then_global() {
    let _guard = test_lock();
    let env = EnvGuard::new();
    let workdir = PathBuf::from("/tmp/zz_locale_probe");
    let dirs = ragent_config::i18n::locale_dirs(&workdir);
    assert_eq!(dirs[0], workdir.join(".ragent").join("locales"));
    // The global dir is `<global_state_dir>/locales`.
    let global = ragent_config::Config::global_state_dir().expect("global state dir");
    assert!(dirs.contains(&global.join("locales")));
    let _ = &env;
}

#[test]
#[serial_test::serial]
fn test_env_locale_reads_and_skips_placeholders() {
    let _guard = test_lock();
    let _env = EnvGuard::new();

    assert_eq!(ragent_config::i18n::env_locale(), None);

    // LC_ALL is the highest-priority variable.
    unsafe { env::set_var("LC_ALL", "fr_FR.UTF-8") };
    assert_eq!(
        ragent_config::i18n::env_locale().as_deref(),
        Some("fr_FR.UTF-8")
    );

    // A `C`/`POSIX` placeholder is skipped in favour of the next variable.
    unsafe { env::set_var("LC_ALL", "C") };
    unsafe { env::set_var("LANG", "de_DE.UTF-8") };
    assert_eq!(
        ragent_config::i18n::env_locale().as_deref(),
        Some("de_DE.UTF-8")
    );
    unsafe { env::set_var("LANG", "POSIX") };
    assert_eq!(ragent_config::i18n::env_locale(), None);
    // A blank value is also skipped.
    unsafe { env::set_var("LANG", "  ") };
    assert_eq!(ragent_config::i18n::env_locale(), None);
}

#[test]
#[serial_test::serial]
fn test_resolve_uses_section_then_env_then_default() {
    let _guard = test_lock();
    let _env = EnvGuard::new();
    let workdir = Path::new("/tmp/zz_resolve_probe");

    // No section, no ambient locale -> disabled English.
    let cfg = Config::default();
    let (enabled, locale) = ragent_config::i18n::resolve(&cfg, workdir);
    assert!(!enabled);
    assert_eq!(locale, "en");

    // Section present with an explicit non-English locale.
    let mut cfg = Config::default();
    cfg.i18n = Some(ragent_config::I18nConfig {
        enabled: true,
        locale: "fr".to_string(),
    });
    let (enabled, locale) = ragent_config::i18n::resolve(&cfg, workdir);
    assert!(enabled);
    assert_eq!(locale, "fr");

    // Section present but English/unset -> falls back to the ambient locale.
    unsafe { env::set_var("LANG", "es_ES.UTF-8") };
    let mut cfg = Config::default();
    cfg.i18n = Some(ragent_config::I18nConfig {
        enabled: false,
        locale: String::new(),
    });
    let (enabled, locale) = ragent_config::i18n::resolve(&cfg, workdir);
    assert!(!enabled);
    assert_eq!(locale, "es_ES.UTF-8");

    // No section, but a non-English ambient locale with NO installed catalog
    // stays off, and the locale falls back to the default English tag.
    let cfg = Config::default();
    let (enabled, locale) = ragent_config::i18n::resolve(&cfg, workdir);
    assert!(!enabled);
    assert_eq!(locale, "en");
}

#[test]
#[serial_test::serial]
fn test_resolve_auto_enables_with_installed_catalog() {
    let _guard = test_lock();
    let _env = EnvGuard::new();
    let temp = tempfile::tempdir().expect("tempdir");
    let locales = temp.path().join(".ragent").join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    std::fs::write(
        locales.join("zz.json"),
        r#"{"locale":"zz","messages":{"status.project":"P: "}}"#,
    )
    .expect("write catalog");

    unsafe { env::set_var("LANG", "zz_ZZ.UTF-8") };
    let cfg = Config::default();
    let (enabled, locale) = ragent_config::i18n::resolve(&cfg, temp.path());
    assert!(enabled, "a non-English locale with a catalog auto-enables");
    assert_eq!(locale, "zz_ZZ.UTF-8");
    assert!(ragent_config::i18n::catalog_exists(&locale, temp.path()));
}

#[test]
#[serial_test::serial]
fn test_catalog_exists_reflects_files() {
    let _guard = test_lock();
    let _env = EnvGuard::new();
    let temp = tempfile::tempdir().expect("tempdir");
    assert!(!ragent_config::i18n::catalog_exists("zz", temp.path()));

    let locales = temp.path().join(".ragent").join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    std::fs::write(
        locales.join("zz.json"),
        r#"{"locale":"zz","messages":{"status.project":"P: "}}"#,
    )
    .expect("write catalog");
    assert!(ragent_config::i18n::catalog_exists("zz", temp.path()));
    assert!(ragent_config::i18n::catalog_exists(
        "zz_ZZ.UTF-8",
        temp.path()
    ));
    assert!(!ragent_config::i18n::catalog_exists("", temp.path()));
}

#[test]
#[serial_test::serial]
fn test_apply_and_translate_via_catalog() {
    let _guard = test_lock();
    let _reset = ResetGuard;
    let _env = EnvGuard::new();
    let temp = tempfile::tempdir().expect("tempdir");
    let locales = temp.path().join(".ragent").join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    std::fs::write(
        locales.join("zz.json"),
        r#"{"locale":"zz","messages":{"status.project":"TRADUIT"}}"#,
    )
    .expect("write catalog");

    ragent_config::i18n::apply(true, "zz", temp.path());
    assert!(ragent_config::i18n::is_enabled());
    assert!(ragent_config::i18n::catalog_loaded());
    assert_eq!(ragent_config::i18n::active_locale(), "zz");
    assert_eq!(
        ragent_config::i18n::t(MessageKey::StatusProject).as_ref(),
        "TRADUIT"
    );
}

#[test]
#[serial_test::serial]
fn test_persist_writes_i18n_section() {
    let _guard = test_lock();
    let _reset = ResetGuard;
    let env_guard = EnvGuard::new();
    let original_cwd = env::current_dir().expect("cwd");
    env::set_current_dir(env_guard.root()).expect("chdir");

    ragent_config::i18n::persist(Some(true), Some("fr")).expect("persist i18n");

    let raw = std::fs::read_to_string(".ragent/ragent.json").expect("read config");
    let root: serde_json::Value = serde_json::from_str(&raw).expect("parse config");
    assert_eq!(root["i18n"]["enabled"], serde_json::json!(true));
    assert_eq!(root["i18n"]["locale"], serde_json::json!("fr"));

    env::set_current_dir(&original_cwd).ok();
    let _ = &env_guard;
}
