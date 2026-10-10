//! Locale message catalog with English fallback (spec `openhands` T-014;
//! FR-027).
//!
//! Covers:
//! - the `i18n` section is absent by default and omitted when serialised;
//! - it parses and round-trips `{ enabled, locale }`;
//! - a locale catalog overrides keys and falls back to the compiled English
//!   text for any key it does not translate;
//! - a malformed catalog is ignored and English is served;
//! - locale tags expand to file-name candidates (`fr_FR.UTF-8` -> `fr_fr`,
//!   `fr`);
//! - the merge precedence for the `i18n` section.

use std::sync::{Mutex, MutexGuard, OnceLock};

use ragent_config::{Config, I18nConfig, LocaleCatalog, MessageKey};

/// The i18n runtime state is process-global; one mutex serialises every test in
/// this binary against it.
fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Reset the global i18n state on drop so one test cannot leak into another.
struct ResetGuard;

impl Drop for ResetGuard {
    fn drop(&mut self) {
        ragent_config::i18n::reset();
    }
}

// ---------------------------------------------------------------------------
// Config shape (FR-027)
// ---------------------------------------------------------------------------

#[test]
fn i18n_absent_by_default_and_omitted_when_serialised() {
    let config = Config::default();
    assert!(config.i18n.is_none(), "i18n must be opt-in");
    let json = serde_json::to_value(&config).expect("serialise config");
    assert!(
        json.get("i18n").is_none(),
        "an unset i18n section must be omitted from ragent.json"
    );
}

#[test]
fn i18n_parses_and_round_trips() {
    let json = r#"{ "i18n": { "enabled": true, "locale": "fr" } }"#;
    let config: Config = serde_json::from_str(json).expect("parse i18n config");
    let section = config.i18n.as_ref().expect("section present");
    assert!(section.enabled);
    assert_eq!(section.locale, "fr");

    let round_trip = serde_json::to_string(&config).expect("serialise config");
    let decoded: Config = serde_json::from_str(&round_trip).expect("deserialise config");
    let decoded_section = decoded.i18n.as_ref().expect("section present");
    assert!(decoded_section.enabled);
    assert_eq!(decoded_section.locale, "fr");
}

#[test]
fn i18n_defaults_locale_to_en() {
    let section: I18nConfig = serde_json::from_str("{}").expect("parse empty section");
    assert!(!section.enabled);
    assert_eq!(section.locale, "en");
}

#[test]
fn i18n_overlay_wins_wholesale() {
    let mut base = Config::default();
    base.i18n = Some(I18nConfig {
        enabled: false,
        locale: "en".to_string(),
    });

    let overlay: Config =
        serde_json::from_str(r#"{ "i18n": { "enabled": true, "locale": "de" } }"#).expect("parse");
    let merged = Config::merge(base, overlay);
    let section = merged.i18n.as_ref().expect("section present");
    assert!(section.enabled, "the overlay section must win");
    assert_eq!(section.locale, "de");

    // An absent overlay preserves the base section.
    let mut base = Config::default();
    base.i18n = Some(I18nConfig {
        enabled: true,
        locale: "fr".to_string(),
    });
    let merged = Config::merge(base, Config::default());
    assert_eq!(merged.i18n.as_ref().map(|s| s.locale.as_str()), Some("fr"));
}

// ---------------------------------------------------------------------------
// English fallback (FR-027)
// ---------------------------------------------------------------------------

#[test]
fn disabled_i18n_serves_the_compiled_english_text() {
    let _guard = test_lock();
    let _reset = ResetGuard;
    ragent_config::i18n::reset();

    assert!(!ragent_config::i18n::is_enabled());
    assert!(!ragent_config::i18n::catalog_loaded());
    for key in MessageKey::all() {
        assert_eq!(
            ragent_config::i18n::t(*key).as_ref(),
            key.english(),
            "key {} must fall back to English while i18n is off",
            key.key()
        );
    }
}

#[test]
fn locale_catalog_overrides_keys_and_falls_back_to_english() {
    let _guard = test_lock();
    let _reset = ResetGuard;

    let temp = tempfile::tempdir().expect("tempdir");
    let locales = temp.path().join(".ragent").join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    // A partial catalog: `status.project` is translated, `status.branch` is not.
    std::fs::write(
        locales.join("zz.json"),
        r#"{"locale":"zz","messages":{"status.project":"PROJET: "}}"#,
    )
    .expect("write catalog");

    ragent_config::i18n::apply(true, "zz", temp.path());
    assert!(ragent_config::i18n::is_enabled());
    assert!(ragent_config::i18n::catalog_loaded());
    assert_eq!(ragent_config::i18n::active_locale(), "zz");

    // Translated key renders from the catalog.
    assert_eq!(
        ragent_config::i18n::t(MessageKey::StatusProject).as_ref(),
        "PROJET: "
    );
    // Untranslated key falls back to the English text - never a raw key or blank.
    assert_eq!(
        ragent_config::i18n::t(MessageKey::StatusBranch).as_ref(),
        MessageKey::StatusBranch.english()
    );
    assert!(!ragent_config::i18n::t(MessageKey::StatusBranch).is_empty());
}

#[test]
fn malformed_catalog_is_ignored_and_english_is_served() {
    let _guard = test_lock();
    let _reset = ResetGuard;

    let temp = tempfile::tempdir().expect("tempdir");
    let locales = temp.path().join(".ragent").join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    std::fs::write(locales.join("zz.json"), "{ not json").expect("write catalog");

    ragent_config::i18n::apply(true, "zz", temp.path());
    assert!(
        !ragent_config::i18n::catalog_loaded(),
        "a malformed catalog must not be adopted"
    );
    assert_eq!(
        ragent_config::i18n::t(MessageKey::StatusProject).as_ref(),
        MessageKey::StatusProject.english()
    );
}

#[test]
fn catalog_parses_from_json() {
    let catalog =
        LocaleCatalog::from_json(r#"{"locale":"fr","messages":{"help.header":"From: /help"}}"#)
            .expect("parse catalog");
    assert_eq!(catalog.locale, "fr");
    assert_eq!(catalog.get("help.header"), Some("From: /help"));
    assert_eq!(catalog.get("missing"), None);
}

// ---------------------------------------------------------------------------
// Locale candidates + discovery
// ---------------------------------------------------------------------------

#[test]
fn locale_tags_expand_most_specific_first() {
    assert_eq!(
        ragent_config::i18n::locale_candidates("fr_FR.UTF-8"),
        vec!["fr_fr".to_string(), "fr".to_string()]
    );
    assert_eq!(
        ragent_config::i18n::locale_candidates("de"),
        vec!["de".to_string()]
    );
    assert_eq!(
        ragent_config::i18n::locale_candidates("en-US"),
        vec!["en-us".to_string(), "en".to_string()]
    );
    assert_eq!(
        ragent_config::i18n::locale_candidates("  "),
        Vec::<String>::new()
    );
}

#[test]
fn catalog_discovery_walks_the_locale_dirs() {
    let temp = tempfile::tempdir().expect("tempdir");
    let locales = temp.path().join(".ragent").join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    // Only the language fallback (`fr`) exists, not the region-specific (`fr_fr`).
    std::fs::write(locales.join("fr.json"), r#"{"locale":"fr","messages":{}}"#)
        .expect("write catalog");

    assert!(ragent_config::i18n::catalog_exists(
        "fr_FR.UTF-8",
        temp.path()
    ));
    assert!(!ragent_config::i18n::catalog_exists("zz", temp.path()));
}

#[test]
fn project_catalog_wins_over_global() {
    let _guard = test_lock();
    let _reset = ResetGuard;

    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join(".ragent").join("locales");
    std::fs::create_dir_all(&project).expect("create project locales dir");
    std::fs::write(
        project.join("zz.json"),
        r#"{"locale":"zz","messages":{"status.project":"PROJECT-WINS: "}}"#,
    )
    .expect("write project catalog");

    ragent_config::i18n::apply(true, "zz", temp.path());
    assert_eq!(
        ragent_config::i18n::t(MessageKey::StatusProject).as_ref(),
        "PROJECT-WINS: "
    );
}

#[test]
fn message_keys_are_unique_and_round_trip() {
    let mut keys: Vec<&str> = MessageKey::all().iter().map(|k| k.key()).collect();
    keys.sort_unstable();
    let count = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), count, "message keys must be unique");
    for key in MessageKey::all() {
        assert!(
            !key.english().is_empty(),
            "every key needs a non-empty English fallback"
        );
    }
}
