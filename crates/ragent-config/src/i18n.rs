//! UI internationalisation: a locale message catalog with English fallback
//! (spec `openhands` T-014; FR-027).
//!
//! Where internationalisation is enabled with a configured locale, user-facing
//! strings are rendered from that locale's message catalog and fall back to the
//! built-in English text for any key the catalog does not translate.
//!
//! ## Design
//!
//! - The **English text is compiled in**. [`MessageKey::english`] is the single
//!   source of truth for every translatable string; a locale catalog only
//!   *overrides* keys, so a missing key can never render blank or as a raw key
//!   (FR-027), it renders the English text.
//! - A catalog is a JSON file `{"locale":"fr","messages":{"status.project":"..."}}`
//!   discovered under `<working_dir>/.ragent/locales/<locale>.json` (project,
//!   most specific) then `~/.config/ragent/locales/<locale>.json` (user-global).
//! - The feature is **opt-in**: the `i18n` section is absent by default and the
//!   runtime is inert, returning the compiled English text with no allocation.
//!   When the section is absent, a non-English ambient locale (`LANG`/
//!   `LC_ALL`/`LC_MESSAGES`) *with an installed catalog* auto-enables so
//!   `LANG=fr_FR.UTF-8 ragent` renders French without editing the config.
//!
//! ## Scope
//!
//! Only user-facing strings are routed through [`t`]: the status bar labels, the
//! `/help` header, and the catalogue-panel footer. Diagnostics and log lines stay
//! English (they are not user-facing presentation of the agent's state).

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// A translatable user-facing string.
///
/// The variant's [`MessageKey::english`] text is the compiled-in default; a
/// locale catalog overrides it by matching the stable [`MessageKey::key`]
/// string. Adding a key here means it is immediately translatable and already
/// has an English fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKey {
    /// Status-bar working-directory label (`Project: `).
    StatusProject,
    /// Status-bar git-branch label (`Branch: `).
    StatusBranch,
    /// Status-bar idle indicator (`Ready `).
    StatusReady,
    /// Status-bar model label (`Model: `).
    StatusModel,
    /// Status-bar thinking-level label (` Thinking:`).
    StatusThinking,
    /// Header line of the `/help` command output.
    HelpHeader,
    /// Footer key-hint line of the catalogue-browse panel.
    CatalogueFooter,
}

impl MessageKey {
    /// The stable catalog key for this message.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::StatusProject => "status.project",
            Self::StatusBranch => "status.branch",
            Self::StatusReady => "status.ready",
            Self::StatusModel => "status.model",
            Self::StatusThinking => "status.thinking",
            Self::HelpHeader => "help.header",
            Self::CatalogueFooter => "catalogue.footer",
        }
    }

    /// The compiled-in English text, used whenever no catalog overrides the key.
    #[must_use]
    pub const fn english(self) -> &'static str {
        match self {
            Self::StatusProject => "Project: ",
            Self::StatusBranch => "Branch: ",
            Self::StatusReady => "Ready ",
            Self::StatusModel => "Model: ",
            Self::StatusThinking => " Thinking:",
            Self::HelpHeader => "From: /help\nAvailable commands:",
            Self::CatalogueFooter => "Up/Down move  Enter install  c category  Esc close",
        }
    }

    /// Every key, in declaration order. Used by catalogue/consistency checks.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::StatusProject,
            Self::StatusBranch,
            Self::StatusReady,
            Self::StatusModel,
            Self::StatusThinking,
            Self::HelpHeader,
            Self::CatalogueFooter,
        ]
    }
}

/// The `i18n` section of `ragent.json` (FR-027).
///
/// ```jsonc
/// { "i18n": { "enabled": true, "locale": "fr" } }
/// ```
///
/// Opt-in: absent from the default config, so English is rendered and the
/// subsystem does no catalogue IO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nConfig {
    /// Whether user-facing strings are rendered from a locale catalog.
    #[serde(default)]
    pub enabled: bool,
    /// The configured locale (ISO-639 tag, optionally with a region/encoding
    /// suffix such as `fr_FR.UTF-8`). Defaults to `en`.
    #[serde(default = "default_locale")]
    pub locale: String,
}

/// Serde default for [`I18nConfig::locale`].
fn default_locale() -> String {
    "en".to_string()
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            locale: default_locale(),
        }
    }
}

/// A loaded locale catalog: a locale id plus a `key -> translation` map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleCatalog {
    /// The locale this catalog targets (`fr`, `de`, ...).
    #[serde(default)]
    pub locale: String,
    /// Translated messages keyed by [`MessageKey::key`].
    #[serde(default)]
    pub messages: std::collections::HashMap<String, String>,
}

impl LocaleCatalog {
    /// Parse a catalog from JSON bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not valid UTF-8 JSON of the
    /// `{ "locale": ..., "messages": { ... } }` shape.
    pub fn from_json(raw: &str) -> anyhow::Result<Self> {
        serde_json::from_str(raw)
            .map_err(|e| anyhow::anyhow!("failed to parse locale catalog: {e}"))
    }

    /// The translation for `key`, or `None` when this catalog does not override
    /// it (the caller falls back to English).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(String::as_str)
    }
}

// -- Runtime state -----------------------------------------------------------

/// Process-wide "i18n active" flag. Read on the hot render path before taking
/// the catalogue lock so the default (disabled) path never locks.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// The resolved runtime locale plus the loaded catalogue (or `None`).
struct CatalogState {
    locale: String,
    catalog: Option<LocaleCatalog>,
}

/// Process-wide catalogue slot.
static STATE: OnceLock<RwLock<CatalogState>> = OnceLock::new();

/// Access the catalogue slot, initialising it to the English default.
fn state() -> &'static RwLock<CatalogState> {
    STATE.get_or_init(|| {
        RwLock::new(CatalogState {
            locale: default_locale(),
            catalog: None,
        })
    })
}

/// Whether internationalisation is currently active.
#[must_use]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// The resolved runtime locale (defaults to `en`).
#[must_use]
pub fn active_locale() -> String {
    state()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .locale
        .clone()
}

/// Whether a non-English catalogue is loaded.
#[must_use]
pub fn catalog_loaded() -> bool {
    state()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .catalog
        .is_some()
}

/// Render a user-facing string for the active locale (FR-027).
///
/// Returns the catalogue translation when one is present, otherwise the
/// compiled English text. The default (disabled) path borrows the English
/// constant and allocates nothing.
#[must_use]
pub fn t(key: MessageKey) -> Cow<'static, str> {
    if !is_enabled() {
        return Cow::Borrowed(key.english());
    }
    let guard = state()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(catalog) = guard.catalog.as_ref() {
        if let Some(text) = catalog.get(key.key()) {
            return Cow::Owned(text.to_string());
        }
    }
    Cow::Borrowed(key.english())
}

// -- Discovery ---------------------------------------------------------------

/// Candidate locale directories, most specific first: the project
/// `.ragent/locales/` then the user-global `~/.config/ragent/locales/`.
#[must_use]
pub fn locale_dirs(working_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![working_dir.join(".ragent").join("locales")];
    if let Some(global) = Config::global_state_dir() {
        dirs.push(global.join("locales"));
    }
    dirs
}

/// Expand a locale tag into the file-name candidates to probe, most specific
/// first (`fr_FR.UTF-8` -> `fr_fr`, `fr`).
#[must_use]
pub fn locale_candidates(locale: &str) -> Vec<String> {
    let trimmed = locale.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    // Drop any encoding suffix (`fr_FR.UTF-8` -> `fr_FR`).
    let base = trimmed.split('.').next().unwrap_or(trimmed);
    let mut out = vec![base.to_ascii_lowercase()];
    for sep in ['_', '-'] {
        if let Some((lang, _)) = base.split_once(sep) {
            let lang = lang.to_ascii_lowercase();
            if !out.contains(&lang) {
                out.push(lang);
            }
        }
    }
    out
}

/// The ambient locale from the environment (`LC_ALL`, `LC_MESSAGES`, `LANG`),
/// or `None` when unset or set to a non-translatable placeholder (`C`,
/// `POSIX`).
#[must_use]
pub fn env_locale() -> Option<String> {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let lower = value.to_ascii_lowercase();
            if lower == "c" || lower == "posix" {
                continue;
            }
            return Some(value.to_string());
        }
    }
    None
}

/// Find the first existing catalog for `locale` across [`locale_dirs`] and
/// parse it. Project-local wins over user-global.
fn find_catalog(locale: &str, working_dir: &Path) -> Option<(PathBuf, LocaleCatalog)> {
    let dirs = locale_dirs(working_dir);
    for candidate in locale_candidates(locale) {
        let file = format!("{candidate}.json");
        for dir in &dirs {
            let path = dir.join(&file);
            if !path.is_file() {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(raw) => match LocaleCatalog::from_json(&raw) {
                    Ok(catalog) => return Some((path, catalog)),
                    Err(e) => {
                        tracing::warn!(path = %path.display(), error = %e, "i18n: ignoring malformed locale catalog");
                    }
                },
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "i18n: failed to read locale catalog");
                }
            }
        }
    }
    None
}

/// Whether a parseable catalog exists for `locale` under the search path.
#[must_use]
pub fn catalog_exists(locale: &str, working_dir: &Path) -> bool {
    find_catalog(locale, working_dir).is_some()
}

// -- Wiring ------------------------------------------------------------------

/// Resolve the effective `(enabled, locale)` for a loaded [`Config`].
///
/// - `i18n` section present: `enabled` is used verbatim; the locale is the
///   configured value, or the ambient environment locale when the configured
///   value is unset/English.
/// - `i18n` section absent: the subsystem stays off unless a non-English ambient
///   locale has an installed catalog, in which case it auto-enables so a
///   `LANG=fr_FR.UTF-8` launch renders French without editing the config.
#[must_use]
pub fn resolve(config: &Config, working_dir: &Path) -> (bool, String) {
    match config.i18n.as_ref() {
        Some(section) => {
            let configured = section.locale.trim();
            let locale = if configured.is_empty() || configured.eq_ignore_ascii_case("en") {
                env_locale().unwrap_or_else(default_locale)
            } else {
                configured.to_string()
            };
            (section.enabled, locale)
        }
        None => match env_locale() {
            Some(locale)
                if !locale.eq_ignore_ascii_case("en") && catalog_exists(&locale, working_dir) =>
            {
                (true, locale)
            }
            _ => (false, default_locale()),
        },
    }
}

/// Apply an explicit `(enabled, locale)` pair, loading the catalog from
/// `working_dir` (FR-027).
///
/// Disabling clears any loaded catalog so the compiled English text is served.
/// This is the single mutation point: [`sync_from_config`], [`persist`], and the
/// TUI `/i18n` arms all funnel through it so the runtime state cannot drift.
pub fn apply(enabled: bool, locale: &str, working_dir: &Path) {
    let catalog = if enabled {
        find_catalog(locale, working_dir).map(|(_, catalog)| catalog)
    } else {
        None
    };
    if enabled && catalog.is_none() && !locale.eq_ignore_ascii_case("en") {
        tracing::warn!(
            locale,
            "i18n: no message catalog found for the configured locale; falling back to English"
        );
    }
    ENABLED.store(enabled, Ordering::Relaxed);
    let mut guard = state()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.locale = locale.to_string();
    guard.catalog = catalog;
}

/// Apply the effective `(enabled, locale)` for `config`, discovering catalogs
/// relative to the current working directory.
pub fn sync_from_config(config: &Config) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (enabled, locale) = resolve(config, &cwd);
    apply(enabled, &locale, &cwd);
}

/// Persist the requested `i18n` settings to the loaded config source and apply
/// them to the runtime.
///
/// `None` for a field leaves that field unchanged.
///
/// # Errors
///
/// Propagates config load and write failures rather than silently discarding
/// the change.
pub fn persist(enabled: Option<bool>, locale: Option<&str>) -> anyhow::Result<()> {
    let mut config = Config::load()?;
    let mut section = config.i18n.clone().unwrap_or_default();
    if let Some(enabled) = enabled {
        section.enabled = enabled;
    }
    if let Some(locale) = locale {
        section.locale = locale.trim().to_string();
    }
    config.i18n = Some(section);
    config.save_to_source()?;
    // `save_to_source` already dropped the M-025 load cache, so this re-reads
    // the freshly written file and applies the new state.
    sync_from_config(&config);
    Ok(())
}

/// Reset the runtime to the disabled English default (used by tests).
#[doc(hidden)]
pub fn reset() {
    apply(false, "en", Path::new("."));
}
