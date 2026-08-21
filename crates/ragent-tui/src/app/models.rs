//! Model selection, thinking-level, and provider management for the TUI.
use std::sync::Arc;
use std::sync::atomic::Ordering;

use pulldown_cmark::{Options, Parser, html};

use ragent_agent::team::TeamManager;
use ragent_agent::{
    agent::{AgentInfo, ModelRef},
    event::Event,
    provider::ModelInfo,
    storage::Storage,
};
use ragent_team::team::TeamStore;
use ragent_types::{ThinkingConfig, ThinkingLevel};

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{
    App, ConfiguredProvider, FileMenuEntry, FileMenuState, LogLevel, ModelPickerEntry,
    PROVIDER_LIST, ProviderSetupStep, ProviderSource,
};

// Helpers
use crate::app::helpers::{sanitize_for_display, try_extract_research_code_block};

// Re-export status types from theme

impl App {
    pub(crate) fn is_ascii_table_line(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }
        trimmed.contains('│')
            || (trimmed.contains('─')
                && trimmed
                    .chars()
                    .all(|c| matches!(c, '─' | '┬' | '┼' | '┴' | ' ')))
    }

    pub(crate) fn table_row_cells(line: &str) -> Vec<String> {
        line.split('│').map(|c| c.trim().to_string()).collect()
    }

    pub(crate) fn table_border(widths: &[usize]) -> String {
        let mut out = String::from("+");
        for w in widths {
            out.push_str(&"-".repeat(*w + 2));
            out.push('+');
        }
        out
    }

    /// Normalize ASCII table rendering: collapse repeated border rows, ensure
    /// each table has a top border, and emit clean aligned rows for display.
    pub fn normalize_ascii_tables(&self, rendered: &str) -> String {
        let lines: Vec<&str> = rendered.lines().collect();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;

        while i < lines.len() {
            if !Self::is_ascii_table_line(lines[i]) {
                out.push(lines[i].to_string());
                i += 1;
                continue;
            }

            let start = i;
            while i < lines.len() && Self::is_ascii_table_line(lines[i]) {
                i += 1;
            }
            let block = &lines[start..i];

            let mut rows: Vec<Vec<String>> = Vec::new();
            let mut separators: Vec<bool> = Vec::new();
            let mut col_count = 0usize;
            for line in block {
                let trimmed = line.trim();
                if trimmed.contains('│') {
                    let cells = Self::table_row_cells(trimmed);
                    col_count = col_count.max(cells.len());
                    rows.push(cells);
                    separators.push(false);
                } else {
                    separators.push(true);
                    rows.push(Vec::new());
                }
            }
            if col_count == 0 {
                out.extend(block.iter().map(|s| s.to_string()));
                continue;
            }

            let mut widths = vec![0usize; col_count];
            for row in &rows {
                for (idx, cell) in row.iter().enumerate() {
                    widths[idx] = widths[idx].max(cell.chars().count());
                }
            }

            let border = Self::table_border(&widths);
            let mut wrote_top = false;
            for (idx, row) in rows.iter().enumerate() {
                if separators[idx] {
                    if !wrote_top {
                        out.push(border.clone());
                        wrote_top = true;
                    } else {
                        out.push(border.clone());
                    }
                    continue;
                }
                if !wrote_top {
                    out.push(border.clone());
                    wrote_top = true;
                }
                let mut line = String::from("|");
                for col in 0..col_count {
                    let cell = row.get(col).cloned().unwrap_or_default();
                    let pad = widths[col].saturating_sub(cell.chars().count());
                    line.push(' ');
                    line.push_str(&cell);
                    line.push_str(&" ".repeat(pad));
                    line.push(' ');
                    line.push('|');
                }
                out.push(line);
            }
            if wrote_top {
                out.push(border);
            }
        }
        out.join("\n")
    }

    /// Return pre-rendered research text (code block or progress log) if the
    /// input should bypass the markdown pipeline, otherwise `None`.
    fn bypass_research_text(text: &str) -> Option<String> {
        if let Some(research) = try_extract_research_code_block(text) {
            return Some(research);
        }
        if text.starts_with("🔬 Research Progress") {
            return Some(sanitize_for_display(text));
        }
        None
    }

    /// Render a markdown string to ASCII for the chat log, preserving research
    /// slash-command preformatted blocks and passing through plain runtime text
    /// unchanged. Otherwise runs the markdown→HTML→text pipeline and normalizes
    /// ASCII tables.
    pub fn render_markdown_to_ascii(&mut self, text: &str) -> String {
        if let Some(bypassed) = Self::bypass_research_text(text) {
            return bypassed;
        }
        // Only convert markdown-like slash output; preserve plain runtime text.
        if !text.starts_with("From: /") {
            return sanitize_for_display(text);
        }

        self.render_markdown_pipeline(text)
    }

    /// Render a markdown string unconditionally through the markdown→HTML→text
    /// pipeline.
    ///
    /// Unlike [`render_markdown_to_ascii`], this does NOT require a `From: /`
    /// prefix.  Use this for standalone messages that contain markdown formatting
    /// (e.g. `agent_complete` summaries with headers, bullet points, bold text)
    /// so they are rendered with proper visual structure instead of appearing
    /// as a wall of plain text.
    ///
    /// Research code blocks and research progress lines are still bypassed.
    pub fn render_markdown_unconditionally(&mut self, text: &str) -> String {
        if let Some(bypassed) = Self::bypass_research_text(text) {
            return bypassed;
        }

        self.render_markdown_pipeline(text)
    }

    /// Shared markdown→HTML→text rendering pipeline with caching and panic
    /// isolation.
    fn render_markdown_pipeline(&mut self, text: &str) -> String {
        // Check cache using FNV-1a hash of input.
        let hash = {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in text.as_bytes() {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            h
        };
        if let Some(cached) = self.md_render_cache.get(&hash) {
            return cached.clone();
        }
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(text, opts);
        let mut html_buf = String::new();
        html::push_html(&mut html_buf, parser);

        // html2text may panic on malformed HTML (word-wrapper subtraction
        // overflow); run it on a dedicated thread so any panic unwinds only
        // that thread, never the UI thread (which installs the panic hook).
        let rendered = {
            let html_owned = html_buf;
            match std::thread::Builder::new()
                .name("md-html2text".to_string())
                .spawn(move || html2text::from_read(html_owned.as_bytes(), 120))
                .map_err(|e| e.to_string())
                .and_then(|h| h.join().map_err(|_| "html2text panicked".to_string()))
            {
                Ok(Ok(text)) => sanitize_for_display(&text),
                _ => {
                    // Fallback to sanitized text when markdown conversion panics or fails.
                    sanitize_for_display(text)
                }
            }
        };
        let cleaned = rendered
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<&str>>()
            .join("\n");
        let result = self.normalize_ascii_tables(&cleaned);

        // Limit cache size to avoid unbounded growth.
        if self.md_render_cache.len() >= 256 {
            self.md_render_cache.clear(); // LRU handles eviction
        }
        self.md_render_cache.put(hash, result.clone());
        result
    }

    pub(crate) fn add_to_history(&mut self, text: String) {
        // Don't add empty or duplicate entries
        if text.is_empty() || self.input_history.last() == Some(&text) {
            return;
        }
        self.input_history.push(text);
        // Trim to 100 entries
        if self.input_history.len() > 100 {
            self.input_history.remove(0);
        }
        // Mark dirty; the main loop will flush after the debounce window.
        self.history_dirty = true;
        if self.history_save_deadline.is_none() {
            self.history_save_deadline =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        }
        self.history_index = None;
        self.history_draft.clear();
    }

    pub(crate) fn selected_model_context_window(&self) -> Option<usize> {
        let model = self.selected_model.as_deref()?;
        let (provider_id, model_id) = model.split_once('/')?;
        // Try the static registry first (works for providers with hardcoded default_models).
        self.provider_registry
            .resolve_model(provider_id, model_id)
            .map(|m| m.context_window)
            .filter(|w| *w > 0)
            // Fall back to the cached context window stored during model selection
            // (required for dynamically discovered models like ollama/ollama_cloud).
            .or(self.selected_model_ctx_window.filter(|w| *w > 0))
    }

    pub(crate) fn parse_thinking_level_setting(value: &str) -> Option<ThinkingLevel> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(ThinkingLevel::Auto),
            "off" => Some(ThinkingLevel::Off),
            "low" => Some(ThinkingLevel::Low),
            "medium" => Some(ThinkingLevel::Medium),
            "high" => Some(ThinkingLevel::High),
            _ => None,
        }
    }

    pub(crate) fn thinking_level_setting_value(level: ThinkingLevel) -> &'static str {
        match level {
            ThinkingLevel::Auto => "auto",
            ThinkingLevel::Off => "off",
            ThinkingLevel::Low => "low",
            ThinkingLevel::Medium => "medium",
            ThinkingLevel::High => "high",
        }
    }

    pub(crate) fn thinking_level_display(level: ThinkingLevel) -> &'static str {
        Self::thinking_level_setting_value(level)
    }

    pub(crate) fn thinking_level_is_explicit(storage: &Storage) -> bool {
        storage
            .get_setting("thinking_level_explicit")
            .ok()
            .flatten()
            .is_some_and(|value| value == "1")
    }

    pub(crate) fn load_persisted_thinking_level(storage: &Storage) -> Option<ThinkingLevel> {
        let level = storage
            .get_setting("thinking_level")
            .ok()
            .flatten()
            .and_then(|s| Self::parse_thinking_level_setting(&s))?;

        if level == ThinkingLevel::Auto && !Self::thinking_level_is_explicit(storage) {
            return None;
        }

        Some(level)
    }

    /// Return the short display label for a [`ThinkingLevel`] (e.g. `"Med"`).
    pub(crate) fn thinking_level_short(level: ThinkingLevel) -> &'static str {
        match level {
            ThinkingLevel::Auto => "Auto",
            ThinkingLevel::Off => "Off",
            ThinkingLevel::Low => "Low",
            ThinkingLevel::Medium => "Med",
            ThinkingLevel::High => "High",
        }
    }

    pub(crate) fn format_thinking_levels(levels: &[ThinkingLevel]) -> String {
        if levels.is_empty() {
            "—".to_string()
        } else {
            levels
                .iter()
                .map(|level| Self::thinking_level_short(*level))
                .collect::<Vec<_>>()
                .join("/")
        }
    }

    pub(crate) fn default_thinking_level_for_entry(entry: &ModelPickerEntry) -> ThinkingLevel {
        if let Some(thinking) = &entry.thinking_config {
            return if thinking.is_effective_enabled() {
                thinking.level
            } else {
                ThinkingLevel::Off
            };
        }

        if entry.thinking_levels.contains(&ThinkingLevel::Off) {
            ThinkingLevel::Off
        } else {
            entry
                .thinking_levels
                .first()
                .copied()
                .unwrap_or(ThinkingLevel::Off)
        }
    }

    pub(crate) fn thinking_config_for_level(level: ThinkingLevel) -> ThinkingConfig {
        if level == ThinkingLevel::Off {
            ThinkingConfig::off()
        } else {
            ThinkingConfig::new(level)
        }
    }

    pub(crate) fn explicit_selected_thinking_config(&self) -> Option<ThinkingConfig> {
        self.selected_thinking_level
            .map(Self::thinking_config_for_level)
    }

    pub(crate) fn effective_thinking_config_for_entry(entry: &ModelPickerEntry) -> ThinkingConfig {
        entry.thinking_config.clone().unwrap_or_else(|| {
            Self::thinking_config_for_level(Self::default_thinking_level_for_entry(entry))
        })
    }

    pub(crate) fn model_entry_for_ref(&self, model_ref: &ModelRef) -> Option<ModelPickerEntry> {
        self.resolved_model_entries_for_provider(&model_ref.provider_id)
            .into_iter()
            .find(|entry| entry.id == model_ref.model_id)
    }

    pub(crate) fn effective_thinking_config_for_agent(
        &self,
        agent: &AgentInfo,
    ) -> Option<ThinkingConfig> {
        self.explicit_selected_thinking_config()
            .or_else(|| agent.thinking.clone())
            .or_else(|| {
                agent
                    .model
                    .as_ref()
                    .and_then(|model_ref| self.model_entry_for_ref(model_ref))
                    .map(|entry| Self::effective_thinking_config_for_entry(&entry))
            })
    }

    pub(crate) fn effective_thinking_level_for_agent(
        &self,
        agent: &AgentInfo,
    ) -> Option<ThinkingLevel> {
        self.effective_thinking_config_for_agent(agent)
            .map(|config| config.level)
    }

    pub(crate) fn persist_selected_thinking_level(&mut self, level: ThinkingLevel) {
        self.selected_thinking_level = Some(level);
        let _ = self
            .storage
            .set_setting("thinking_level", Self::thinking_level_setting_value(level));
        let _ = self.storage.set_setting("thinking_level_explicit", "1");
    }

    pub(crate) fn apply_selected_model_and_thinking(&self, agent: &mut AgentInfo) {
        if (!agent.model_pinned || agent.model.is_none())
            && let Some(ref model_str) = self.selected_model
            && let Some((provider, model)) = model_str.split_once('/')
        {
            agent.model = Some(ModelRef {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            });
        }

        // If still no model, fall back to the first available provider/model.
        if agent.model.is_none() {
            if let Some(model_ref) =
                ragent_agent::agent::resolve_default_model(agent, &self.provider_registry)
            {
                agent.model = Some(model_ref);
            }
        }

        if let Some(thinking) = self.effective_thinking_config_for_agent(agent) {
            agent.thinking = Some(thinking);
        }
    }

    pub(crate) fn active_model_entry(&self) -> Option<ModelPickerEntry> {
        let model_ref = self.selected_model.as_deref()?;
        let (provider_id, model_id) = model_ref.split_once('/')?;
        self.resolved_model_entries_for_provider(provider_id)
            .into_iter()
            .find(|entry| entry.id == model_id)
    }

    pub(crate) fn active_thinking_levels(&self) -> Vec<ThinkingLevel> {
        self.active_model_entry()
            .map(|entry| entry.thinking_levels)
            .unwrap_or_default()
    }

    pub(crate) fn finalize_model_selection(
        &mut self,
        provider_id: String,
        provider_name: String,
        entry: &ModelPickerEntry,
        thinking_level: ThinkingLevel,
    ) -> String {
        let model_value = format!("{}/{}", provider_id, entry.id);
        let _ = self.storage.set_setting("selected_model", &model_value);
        let _ = self.storage.set_setting("preferred_provider", &provider_id);
        let _ = self.storage.set_setting(
            "selected_model_ctx_window",
            &entry.context_window.to_string(),
        );
        // Persist the chosen model per-provider so it can be restored later (FR-003).
        let _ = self
            .storage
            .set_setting(&format!("provider_{}_last_model", provider_id), &entry.id);
        self.selected_model = Some(model_value);
        self.selected_model_ctx_window = Some(entry.context_window);
        self.persist_selected_thinking_level(thinking_level);
        self.configured_provider = Some(ConfiguredProvider {
            id: provider_id,
            name: provider_name,
            source: ProviderSource::Database,
        });
        entry.name.clone()
    }

    /// Restore the previously-selected model for the given provider from
    /// persistent storage. Returns the restored entry, or `None` if no model
    /// was persisted or the persisted model is no longer advertised by the
    /// provider (in which case the stale setting is pruned).
    pub fn try_restore_provider_model(
        &mut self,
        provider_id: &str,
        provider_name: &str,
    ) -> Option<ModelPickerEntry> {
        let last_model_key = format!("provider_{}_last_model", provider_id);
        let persisted = self.storage.get_setting(&last_model_key).ok().flatten();
        let model_id = match persisted {
            Some(ref mid) if !mid.is_empty() => mid.clone(),
            _ => return None,
        };

        let models = self.models_for_provider(provider_id);
        let lower_model_id = model_id.to_lowercase();

        if let Some(entry) = models
            .iter()
            .find(|e| e.id.to_lowercase() == lower_model_id)
        {
            // Model still available — restore it without showing the picker (FR-003).
            let model_value = format!("{}/{}", provider_id, entry.id);
            let _ = self.storage.set_setting("selected_model", &model_value);
            let _ = self.storage.set_setting("preferred_provider", provider_id);
            let _ = self.storage.set_setting(
                "selected_model_ctx_window",
                &entry.context_window.to_string(),
            );
            // Re-persist with the correct casing from the current model list.
            let _ = self.storage.set_setting(&last_model_key, &entry.id);
            self.selected_model = Some(model_value);
            self.selected_model_ctx_window = Some(entry.context_window);
            let default_level = Self::default_thinking_level_for_entry(entry);
            self.persist_selected_thinking_level(default_level);
            self.configured_provider = Some(ConfiguredProvider {
                id: provider_id.to_string(),
                name: provider_name.to_string(),
                source: ProviderSource::Database,
            });
            return Some(entry.clone());
        }

        // Stale model — prune the persisted key (FR-004).
        let _ = self.storage.delete_setting(&last_model_key);
        self.status = format!(
            "Previous model `{}` is no longer available — please choose a new one",
            model_id
        );
        None
    }

    /// Backfill `selected_model_ctx_window` from cached/default provider model
    /// metadata so the UI does not block on provider discovery.
    ///
    /// During startup this intentionally avoids synchronous model discovery
    /// (`sync_discover_models`), which can block for 5+ seconds when a
    /// provider endpoint is slow or unreachable.  Only cached or default
    /// model metadata is consulted; the context window is refreshed later
    /// when the user opens the model picker or sends a message.
    pub fn backfill_model_ctx_window(&mut self) {
        let model = match self.selected_model.as_deref() {
            Some(m) => m.to_string(),
            None => return,
        };
        let Some((provider_id, model_id)) = model.split_once('/') else {
            return;
        };
        let previous_context_window = self.selected_model_ctx_window;

        // Use only cached/default metadata — never block on network discovery.
        let cached = self.cached_model_entries(provider_id);
        let models = if !cached.is_empty() {
            cached
        } else {
            // Fall back to the provider's static default model list.
            self.provider_registry
                .get(provider_id)
                .map(|provider| self.picker_entries_from_models(provider.default_models()))
                .unwrap_or_default()
        };
        if let Some(entry) = models.iter().find(|e| e.id == model_id) {
            if entry.context_window > 0 && previous_context_window != Some(entry.context_window) {
                self.selected_model_ctx_window = Some(entry.context_window);
                let _ = self.storage.set_setting(
                    "selected_model_ctx_window",
                    &entry.context_window.to_string(),
                );
                tracing::info!(
                    model = %model,
                    previous_context_window = ?previous_context_window,
                    context_window = entry.context_window,
                    "Refreshed context window for selected model"
                );
            }
        }
    }

    pub(crate) fn provider_api_key(&self, provider_id: &str) -> Option<String> {
        let from_storage = || {
            self.storage
                .get_provider_auth(provider_id)
                .ok()
                .flatten()
                .filter(|key| !key.is_empty())
        };

        match provider_id {
            "anthropic" => from_storage().or_else(|| {
                std::env::var("ANTHROPIC_API_KEY")
                    .ok()
                    .filter(|key| !key.is_empty())
            }),
            "gemini" => from_storage()
                .or_else(|| {
                    std::env::var("GEMINI_API_KEY")
                        .ok()
                        .filter(|key| !key.is_empty())
                })
                .or_else(|| {
                    std::env::var("GOOGLE_API_KEY")
                        .ok()
                        .filter(|key| !key.is_empty())
                }),
            "huggingface" => from_storage()
                .or_else(|| std::env::var("HF_TOKEN").ok().filter(|key| !key.is_empty()))
                .or_else(|| {
                    std::env::var("HUGGING_FACE_HUB_TOKEN")
                        .ok()
                        .filter(|key| !key.is_empty())
                }),
            "xai" => from_storage().or_else(|| {
                std::env::var("XAI_API_KEY")
                    .ok()
                    .filter(|key| !key.is_empty())
            }),
            "ollama_cloud" => self.ollama_cloud_api_key(),
            "azure_foundry" => from_storage().or_else(|| {
                std::env::var("AZURE_AI_FOUNDRY_API_KEY")
                    .ok()
                    .filter(|key| !key.is_empty())
            }),
            _ => from_storage(),
        }
    }

    pub(crate) fn calculate_cost_tier(
        &self,
        cost_input: f64,
        cost_output: f64,
        baseline_cost: f64,
        request_multiplier: Option<f64>,
    ) -> (String, String) {
        // If we have a Copilot-style request multiplier, use it directly
        if let Some(mult) = request_multiplier {
            let tier = if mult == 0.0 {
                "Included".to_string()
            } else if mult <= 0.33 {
                "Low".to_string()
            } else if mult <= 1.0 {
                "Standard".to_string()
            } else if mult <= 3.0 {
                "High".to_string()
            } else {
                "Premium".to_string()
            };

            let multiplier_str = if mult == 0.0 {
                "0x".to_string()
            } else if (mult - mult.round()).abs() < 0.001 {
                format!("{:.0}x", mult)
            } else if mult < 1.0 {
                format!("{:.2}x", mult)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
                    + "x"
            } else {
                format!("{:.1}x", mult)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
                    + "x"
            };

            return (tier, multiplier_str);
        }

        // Standard per-token cost calculation
        let avg_cost = f64::midpoint(cost_input, cost_output);

        let tier = if avg_cost == 0.0 {
            "Free".to_string()
        } else if avg_cost <= 0.001 {
            "Low".to_string()
        } else if avg_cost <= 0.01 {
            "Medium".to_string()
        } else if avg_cost <= 0.1 {
            "High".to_string()
        } else {
            "Premium".to_string()
        };

        let multiplier = if baseline_cost > 0.0 {
            let factor = avg_cost / baseline_cost;
            // Round to 1 decimal place for display
            if factor < 0.01 {
                "0x".to_string()
            } else if factor < 1.0 {
                format!("{:.1}x", factor)
            } else if (factor - factor.round()).abs() < 0.01 {
                format!("{:.0}x", factor)
            } else {
                format!("{:.1}x", factor)
            }
        } else {
            "0x".to_string()
        };

        (tier, multiplier)
    }

    /// Build the default HuggingFace model picker entries. Currently unused
    /// because the model picker now uses the generic provider-based flow, but
    /// retained as a documented helper in case a provider-specific default list
    /// is reintroduced.
    #[allow(dead_code)]
    pub(crate) fn hf_default_model_entries(&self) -> Vec<ModelPickerEntry> {
        self.provider_registry
            .get("huggingface")
            .map(|p| self.picker_entries_from_models(p.default_models()))
            .unwrap_or_default()
    }

    /// Detect the first configured provider from persisted credentials/env.
    /// Returns `None` when no provider has been set up.
    ///
    /// This is the hot path used at startup and on provider refresh.  It first
    /// runs a cheap pass that never spawns a subprocess (env vars, `apps.json`,
    /// database), and only falls back to the Copilot `gh auth token` CLI
    /// subprocess when nothing cheaper was found — so provider detection never
    /// blocks startup on a cold keyring / slow `gh`.
    pub fn detect_provider(storage: &Storage) -> Option<ConfiguredProvider> {
        // Fast pass: cheap sources only (no `gh` subprocess).
        let mut fast = Self::get_configured_providers_impl(storage, true);
        if !fast.is_empty() {
            return Some(fast.remove(0));
        }
        // Slow fallback: include Copilot discovery via the `gh` CLI subprocess,
        // only reached when no provider was found cheaply.
        Self::get_configured_providers_impl(storage, false)
            .into_iter()
            .next()
    }

    /// Enumerate all configured providers from the database, honouring explicit
    /// `provider_{id}_disabled` opt-out flags set via `/provider reset`.
    pub fn get_configured_providers(storage: &Storage) -> Vec<ConfiguredProvider> {
        Self::get_configured_providers_impl(storage, false)
    }

    /// Shared implementation of configured-provider enumeration.
    ///
    /// When `defer_gh_cli` is `true`, the Copilot provider is only detected from
    /// cheap sources (env var, IDE `apps.json`, database) and the `gh auth token`
    /// CLI subprocess is skipped.  Used by [`detect_provider`] on its fast path to
    /// avoid spawning a subprocess during startup.
    fn get_configured_providers_impl(
        storage: &Storage,
        defer_gh_cli: bool,
    ) -> Vec<ConfiguredProvider> {
        // Helper: returns true when the user has explicitly reset this provider.
        let is_disabled = |pid: &str| -> bool {
            storage
                .get_setting(&format!("provider_{pid}_disabled"))
                .ok()
                .flatten()
                .is_some()
        };

        let mut configured: Vec<ConfiguredProvider> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        let mut push = |id: &str, name: &str, source: ProviderSource| {
            if seen.insert(id.to_string()) {
                configured.push(ConfiguredProvider {
                    id: id.to_string(),
                    name: name.to_string(),
                    source,
                });
            }
        };

        // 1. Check for an explicit user preference first — this overrides auto-discovery
        //    so that e.g. selecting Ollama doesn't get overwritten by Copilot IDE tokens.
        if let Ok(Some(preferred)) = storage.get_setting("preferred_provider") {
            if !preferred.is_empty()
                && !is_disabled(&preferred)
                && let Some(&(pid, pname)) = PROVIDER_LIST.iter().find(|(id, _)| *id == preferred)
            {
                push(pid, pname, ProviderSource::Database);
            }
        }

        // 2. Check each provider in PROVIDER_LIST order (env vars, then auto-discovery)
        for &(pid, pname) in PROVIDER_LIST {
            if is_disabled(pid) {
                continue;
            }
            let found = match pid {
                "anthropic" => std::env::var("ANTHROPIC_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|_| ProviderSource::EnvVar),
                "openai" => std::env::var("OPENAI_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|_| ProviderSource::EnvVar),
                "gemini" => std::env::var("GEMINI_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .or_else(|| {
                        std::env::var("GOOGLE_API_KEY")
                            .ok()
                            .filter(|k| !k.is_empty())
                    })
                    .map(|_| ProviderSource::EnvVar),
                "huggingface" => std::env::var("HF_TOKEN")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .or_else(|| {
                        std::env::var("HUGGING_FACE_HUB_TOKEN")
                            .ok()
                            .filter(|k| !k.is_empty())
                    })
                    .map(|_| ProviderSource::EnvVar),
                "xai" => std::env::var("XAI_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|_| ProviderSource::EnvVar),
                "bedrock" => {
                    // Bedrock is "configured" when AWS static credentials are present
                    // (access key + secret). Profile-based auth is handled at request
                    // time, so we only surface it here when env-var creds exist.
                    let has_access = std::env::var("AWS_ACCESS_KEY_ID")
                        .ok()
                        .as_ref()
                        .is_some_and(|k| !k.is_empty());
                    let has_secret = std::env::var("AWS_SECRET_ACCESS_KEY")
                        .ok()
                        .as_ref()
                        .is_some_and(|k| !k.is_empty());
                    if has_access && has_secret {
                        Some(ProviderSource::EnvVar)
                    } else {
                        None
                    }
                }
                "generic_openai" => {
                    if let Ok(key) = std::env::var("GENERIC_OPENAI_API_KEY") {
                        if !key.is_empty() {
                            Some(ProviderSource::EnvVar)
                        } else {
                            None
                        }
                    } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                        if !key.is_empty() {
                            Some(ProviderSource::EnvVar)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                "copilot" => {
                    if let Ok(key) = std::env::var("GITHUB_COPILOT_TOKEN") {
                        if !key.is_empty() {
                            Some(ProviderSource::EnvVar)
                        } else if ragent_agent::provider::copilot::find_copilot_token().is_some() {
                            Some(ProviderSource::AutoDiscovered)
                        } else if !defer_gh_cli
                            && ragent_agent::provider::copilot::find_gh_cli_token().is_some()
                        {
                            Some(ProviderSource::AutoDiscovered)
                        } else {
                            None
                        }
                    } else if ragent_agent::provider::copilot::find_copilot_token().is_some() {
                        Some(ProviderSource::AutoDiscovered)
                    } else if !defer_gh_cli
                        && ragent_agent::provider::copilot::find_gh_cli_token().is_some()
                    {
                        Some(ProviderSource::AutoDiscovered)
                    } else {
                        None
                    }
                }
                "ollama" => std::env::var("OLLAMA_HOST")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|_| ProviderSource::EnvVar),
                "ollama_cloud" => std::env::var("OLLAMA_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|_| ProviderSource::EnvVar),
                "azure_foundry" => std::env::var("AZURE_AI_FOUNDRY_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|_| ProviderSource::EnvVar),
                "azure_resource" => {
                    // Azure Resource is "configured" when azureresources.json exists
                    let config_path = dirs::home_dir()
                        .map(|h| h.join(".config").join("ragent").join("azureresources.json"))
                        .filter(|p| p.exists())
                        .or_else(|| {
                            let p = std::path::PathBuf::from(".ragent").join("azureresources.json");
                            if p.exists() { Some(p) } else { None }
                        });
                    config_path.map(|_| ProviderSource::Database)
                }
                _ => None,
            };
            if let Some(source) = found {
                push(pid, pname, source);
            }
        }

        // 3. Check database for any stored provider auth (for providers not already found)
        for (pid, pname) in PROVIDER_LIST {
            if is_disabled(pid) {
                continue;
            }
            if let Ok(Some(_key)) = storage.get_provider_auth(pid) {
                push(pid, pname, ProviderSource::Database);
            }
        }

        configured
    }

    /// Enumerate all configured providers **excluding** the router virtual provider.
    ///
    /// This is the list used by the router setup flow to populate the multi-selection
    /// palette; the router must never route to itself (FR-004, FR-024).
    pub fn get_configured_providers_for_router(storage: &Storage) -> Vec<ConfiguredProvider> {
        Self::get_configured_providers(storage)
            .into_iter()
            .filter(|p| p.id != "router")
            .collect()
    }

    /// Detect whether the router virtual provider has been enabled by reading
    /// the [`RouterProvider`] state from the provider registry, if present.
    #[allow(dead_code)]
    pub(crate) fn is_router_enabled(
        provider_registry: &ragent_llm::provider::ProviderRegistry,
    ) -> bool {
        provider_registry
            .get_as_any("router")
            .and_then(|p| p.downcast_ref::<ragent_llm::providers::router::RouterProvider>())
            .map(|rp| rp.is_enabled())
            .unwrap_or(false)
    }

    /// Re-detect the configured provider and update `configured_provider`.
    pub(crate) fn refresh_provider(&mut self) {
        self.configured_provider = Self::detect_provider(&self.storage);
    }

    /// Make the Model Router the active provider/model selection.
    ///
    /// Persists `selected_model`, `preferred_provider`, and the configured
    /// provider so the router remains active across restarts (FR-049).
    pub(crate) fn select_router_as_active(&mut self) {
        let model_value = "router/router".to_string();
        let _ = self.storage.set_setting("selected_model", &model_value);
        let _ = self.storage.set_setting("preferred_provider", "router");
        let _ = self.storage.delete_setting("selected_thinking_level");

        self.selected_model = Some(model_value);
        self.selected_model_ctx_window = None;
        self.selected_thinking_level = None;
        self.configured_provider = Some(ConfiguredProvider {
            id: "router".to_string(),
            name: "Model Router".to_string(),
            source: ProviderSource::Database,
        });
    }

    /// Restore router state when the persisted active model is the router.
    ///
    /// If `selected_model` is `router/router`, this sets the configured
    /// provider to the router and synchronises the in-memory
    /// `router_enabled` flag and the provider-registry `RouterProvider`
    /// state from the saved `provider.router` configuration (FR-049).
    pub fn restore_router_state(&mut self) {
        let is_router = self
            .selected_model
            .as_deref()
            .map(|s| s == "router/router")
            .unwrap_or(false);
        if !is_router {
            return;
        }

        self.configured_provider = Some(ConfiguredProvider {
            id: "router".to_string(),
            name: "Model Router".to_string(),
            source: ProviderSource::Database,
        });

        if let Some(raw_config) = self.load_raw_router_config() {
            self.router_enabled = raw_config.enabled;
            if let Some(router_provider) = self
                .provider_registry
                .get_as_any("router")
                .and_then(|p| p.downcast_ref::<ragent_llm::providers::router::RouterProvider>())
            {
                router_provider.reload_config(raw_config);
            }
        } else {
            // A previous session selected the router but no `provider.router`
            // block is present; keep it selected but leave it disabled until
            // a cluster is configured.
            self.router_enabled = false;
        }
    }

    /// Paste the supplied text into the active provider-setup dialog field,
    /// stripping carriage returns and routing to the currently focused key or
    /// endpoint input.
    pub fn paste_text_into_provider_setup(&mut self, text: &str) {
        let clean: String = text.chars().filter(|&c| c != '\r').collect();
        let Some(step) = self.provider_setup.as_mut() else {
            return;
        };
        if let ProviderSetupStep::EnterKey {
            key_field,
            endpoint_field,
            active_field,
            ..
        } = step
        {
            let target = if *active_field == 1 {
                endpoint_field
            } else {
                key_field
            };
            target.insert_str(&clean);
        } else if let ProviderSetupStep::GitLabSetup {
            url_input,
            url_cursor,
            token_input,
            token_cursor,
            active_field,
            ..
        } = step
        {
            if *active_field == 0 {
                let insert_pos = url_input
                    .char_indices()
                    .nth(*url_cursor)
                    .map(|(byte, _)| byte)
                    .unwrap_or_else(|| url_input.len());
                url_input.insert_str(insert_pos, &clean);
                *url_cursor += clean.chars().count();
            } else {
                let insert_pos = token_input
                    .char_indices()
                    .nth(*token_cursor)
                    .map(|(byte, _)| byte)
                    .unwrap_or_else(|| token_input.len());
                token_input.insert_str(insert_pos, &clean);
                *token_cursor += clean.chars().count();
            }
        } else if let ProviderSetupStep::TelemetrySetup {
            endpoint_field,
            interval_field,
            timeout_field,
            port_field,
            active_field,
            ..
        } = step
        {
            let target = match *active_field {
                0 => endpoint_field,
                2 => interval_field,
                3 => timeout_field,
                4 => port_field,
                _ => return,
            };
            target.insert_str(&clean);
        }
    }

    pub(crate) fn paste_provider_setup_from_clipboard(&mut self) {
        if let Some(text) = Self::get_clipboard() {
            self.paste_text_into_provider_setup(&text);
        }
    }

    pub(crate) fn ensure_team_manager_for_team(
        &mut self,
        team_name: &str,
        known_team_dir: Option<std::path::PathBuf>,
    ) {
        self.ensure_team_manager_for_team_inner(team_name, known_team_dir, false);
    }

    pub(crate) fn ensure_team_manager_for_team_inner(
        &mut self,
        team_name: &str,
        known_team_dir: Option<std::path::PathBuf>,
        reconcile: bool,
    ) {
        if self.session_processor.team_manager.get().is_some() {
            return;
        }
        let Some(lead_session_id) = self.session_id.clone() else {
            return;
        };

        let team_dir = if let Some(dir) = known_team_dir {
            dir
        } else {
            let working_dir = std::env::current_dir().unwrap_or_default();
            match TeamStore::load_by_name(team_name, &working_dir) {
                Ok(store) => store.dir,
                Err(e) => {
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        format!("TeamManager init skipped: cannot load team '{team_name}': {e}"),
                    );
                    return;
                }
            }
        };

        // Parse the currently selected model so teammates inherit it in the reconcile loop.
        let active_model: Option<ragent_agent::agent::ModelRef> =
            self.selected_model.as_deref().and_then(|s| {
                s.split_once('/')
                    .map(|(pid, mid)| ragent_agent::agent::ModelRef {
                        provider_id: pid.to_string(),
                        model_id: mid.to_string(),
                    })
            });

        let mut manager = TeamManager::new(
            team_name.to_string(),
            lead_session_id,
            team_dir,
            self.session_processor.clone(),
            self.event_bus.clone(),
        );
        manager.active_model = active_model;
        let manager = Arc::new(manager);

        if self
            .session_processor
            .team_manager
            .set(manager.clone())
            .is_ok()
        {
            self.push_log_no_agent(
                LogLevel::Info,
                format!("TeamManager initialised for team '{team_name}'"),
            );
            // Only reconcile when explicitly requested (i.e. when the team was seeded
            // via the LLM tool path and members may be queued in Spawning state).
            if reconcile {
                manager.clone().reconcile_spawning_members();
            }
            // M6-T1: start the teammate watchdog so hung teammates are
            // detected and marked Failed without waiting for team_wait.
            manager.clone().start_watchdog();
        }
    }

    pub(crate) fn overlay_model_config(
        &self,
        config: &ragent_agent::Config,
        mut model: ragent_agent::provider::ModelInfo,
    ) -> ragent_agent::provider::ModelInfo {
        if let Some(provider_config) = config.provider.get(&model.provider_id) {
            if model.thinking_config.is_none() {
                model.thinking_config = provider_config.thinking.clone();
            }

            if let Some(model_config) = provider_config.models.get(&model.id) {
                if let Some(name) = &model_config.name {
                    model.name = name.clone();
                }
                if let Some(cost) = &model_config.cost {
                    model.cost = ragent_config::Cost {
                        input: cost.input,
                        output: cost.output,
                    };
                }
                if let Some(capabilities) = &model_config.capabilities {
                    model.capabilities = ragent_config::Capabilities {
                        reasoning: capabilities.reasoning,
                        streaming: capabilities.streaming,
                        vision: capabilities.vision,
                        tool_use: capabilities.tool_use,
                        thinking_levels: capabilities.thinking_levels.clone(),
                    };
                }
                if let Some(thinking) = &model_config.thinking {
                    model.thinking_config = Some(thinking.clone());
                }
            }
        }

        if model.thinking_config.is_none() {
            model.thinking_config = Some(ragent_agent::agent::default_thinking_config_for_levels(
                &model.capabilities.thinking_levels,
            ));
        }

        model
    }

    pub(crate) fn model_to_picker_entry(
        &self,
        m: ragent_agent::provider::ModelInfo,
        baseline_cost: f64,
    ) -> ModelPickerEntry {
        let (cost_tier, cost_multiplier) = self.calculate_cost_tier(
            m.cost.input,
            m.cost.output,
            baseline_cost,
            m.request_multiplier,
        );
        ModelPickerEntry {
            id: m.id,
            name: m.name,
            context_window: m.context_window,
            max_output: m.max_output,
            cost_input: m.cost.input,
            cost_output: m.cost.output,
            reasoning: m.capabilities.reasoning,
            vision: m.capabilities.vision,
            tool_use: m.capabilities.tool_use,
            thinking_levels: m.capabilities.thinking_levels,
            thinking_config: m.thinking_config,
            cost_tier,
            cost_multiplier,
        }
    }

    pub(crate) fn picker_entries_from_models(
        &self,
        models: Vec<ragent_agent::provider::ModelInfo>,
    ) -> Vec<ModelPickerEntry> {
        let config = self.current_config();
        let models: Vec<_> = models
            .into_iter()
            .map(|model| self.overlay_model_config(&config, model))
            .collect();
        let baseline_cost = models
            .iter()
            .map(|m| f64::midpoint(m.cost.input, m.cost.output))
            .filter(|c| *c > 0.0)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.001);

        let mut entries: Vec<ModelPickerEntry> = models
            .into_iter()
            .map(|m| self.model_to_picker_entry(m, baseline_cost))
            .collect();

        // Sort alphabetically by display name (case-insensitive), falling back to
        // the model id for stable ordering when display names are identical.
        entries.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.to_lowercase().cmp(&b.id.to_lowercase()))
        });

        entries
    }

    pub(crate) fn cache_discovered_models(
        &self,
        provider_id: &str,
        models: &[ragent_agent::provider::ModelInfo],
    ) {
        if let Ok(models_json) = serde_json::to_string(models) {
            let _ = self
                .storage
                .set_discovered_models(provider_id, &models_json);
        }
    }

    pub(crate) fn cached_model_entries(&self, provider_id: &str) -> Vec<ModelPickerEntry> {
        self.storage
            .get_discovered_models(provider_id)
            .ok()
            .flatten()
            .and_then(|models_json| {
                serde_json::from_str::<Vec<ragent_agent::provider::ModelInfo>>(&models_json).ok()
            })
            .map(|models| self.picker_entries_from_models(models))
            .unwrap_or_default()
    }

    pub(crate) fn selected_model_fallback_entries(
        &self,
        provider_id: &str,
    ) -> Vec<ModelPickerEntry> {
        let Some(model_ref) = self.selected_model.as_deref() else {
            return Vec::new();
        };
        let Some((selected_provider, model_id)) = model_ref.split_once('/') else {
            return Vec::new();
        };
        if selected_provider != provider_id {
            return Vec::new();
        }

        vec![ModelPickerEntry {
            id: model_id.to_string(),
            name: model_id.to_string(),
            context_window: self.selected_model_ctx_window.unwrap_or(0),
            max_output: None,
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: false,
            vision: false,
            tool_use: true,
            thinking_levels: Vec::new(),
            thinking_config: None,
            cost_tier: "Unknown".to_string(),
            cost_multiplier: "?".to_string(),
        }]
    }

    pub(crate) fn sync_discover_models(&self, provider_id: &str) -> Vec<ModelInfo> {
        let provider = match self.provider_registry.get(provider_id) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => return Vec::new(),
        };
        // `block_in_place` is only permitted on the multi-threaded Tokio
        // scheduler. Tests run on the current-thread scheduler (or outside a
        // runtime), so skip synchronous discovery there to avoid a runtime
        // panic. The caller falls back to cached/default model entries.
        if handle.runtime_flavor() != tokio::runtime::RuntimeFlavor::MultiThread {
            return Vec::new();
        }
        match tokio::task::block_in_place(|| handle.block_on(provider.discover_models())) {
            Ok(models) => {
                self.cache_discovered_models(provider_id, &models);
                models
            }
            Err(e) => {
                tracing::warn!(provider = %provider_id, error = %e, "Synchronous model discovery failed");
                Vec::new()
            }
        }
    }

    pub(crate) fn resolved_model_entries_for_provider(
        &self,
        provider_id: &str,
    ) -> Vec<ModelPickerEntry> {
        let default_entries = || {
            self.provider_registry
                .get(provider_id)
                .map(|provider| self.picker_entries_from_models(provider.default_models()))
                .unwrap_or_default()
        };

        match provider_id {
            "ollama" | "ollama_cloud" => {
                let cached = self.cached_model_entries(provider_id);
                if !cached.is_empty() {
                    cached
                } else {
                    let discovered = self.sync_discover_models(provider_id);
                    if !discovered.is_empty() {
                        self.picker_entries_from_models(discovered)
                    } else {
                        self.selected_model_fallback_entries(provider_id)
                    }
                }
            }
            "huggingface" => {
                let cached = self.cached_model_entries("huggingface");
                if !cached.is_empty() {
                    cached
                } else if self.provider_api_key("huggingface").is_some() {
                    // A token is configured but (a) no models are cached and
                    // (b) synchronous discovery returned nothing. Return an
                    // empty list so the picker shows "no models" instead of
                    // stale hard-coded defaults.
                    let discovered = self.sync_discover_models("huggingface");
                    if !discovered.is_empty() {
                        self.picker_entries_from_models(discovered)
                    } else {
                        Vec::new()
                    }
                } else {
                    // No token and no cached/discovered models. Do not fall
                    // back to a hard-coded catalog; models must be discovered.
                    Vec::new()
                }
            }
            "azure_foundry" => {
                let cached = self.cached_model_entries("azure_foundry");
                if !cached.is_empty() {
                    cached
                } else if self.provider_api_key("azure_foundry").is_some() {
                    let discovered = self.sync_discover_models("azure_foundry");
                    if !discovered.is_empty() {
                        self.picker_entries_from_models(discovered)
                    } else {
                        default_entries()
                    }
                } else {
                    default_entries()
                }
            }
            "anthropic" | "gemini" | "copilot" | "xai" | "bedrock" => {
                let cached = self.cached_model_entries(provider_id);
                if !cached.is_empty() {
                    cached
                } else {
                    let discovered = self.sync_discover_models(provider_id);
                    if !discovered.is_empty() {
                        self.picker_entries_from_models(discovered)
                    } else {
                        default_entries()
                    }
                }
            }
            _ => {
                let cached = self.cached_model_entries(provider_id);
                if !cached.is_empty() {
                    cached
                } else {
                    default_entries()
                }
            }
        }
    }

    /// Return the list of model entries available for the given provider,
    /// preferring cached entries, falling back to resolved defaults, and
    /// finally to a fallback reflecting the currently selected model.
    pub fn models_for_provider(&self, provider_id: &str) -> Vec<ModelPickerEntry> {
        let mut models = self.resolved_model_entries_for_provider(provider_id);
        let cached = self.cached_model_entries(provider_id);
        if !cached.is_empty() {
            models = cached;
        }

        if models.is_empty() {
            return self.selected_model_fallback_entries(provider_id);
        }

        models
    }

    /// Kick off async model discovery for the given provider, emitting
    /// `ProviderLoadingStarted` / `ProviderLoadingFinished` events so the UI
    /// can show a spinner and populate the picker when results arrive.
    pub(crate) fn start_model_discovery(&self, provider_id: String, provider_name: String) {
        let registry = self.provider_registry.clone();
        let event_bus = self.event_bus.clone();
        event_bus.publish(Event::ProviderLoadingStarted {
            provider_id: provider_id.clone(),
            provider_name: provider_name.clone(),
        });
        tokio::spawn(async move {
            let result = registry.get(&provider_id).map(|p| p.discover_models());
            let (models, error) = match result {
                Some(fut) => match fut.await {
                    Ok(m) => (
                        m.into_iter()
                            .map(|model| serde_json::to_value(model).unwrap_or_default())
                            .collect(),
                        None,
                    ),
                    Err(e) => (Vec::new(), Some(format!("{e:#}"))),
                },
                None => (
                    Vec::new(),
                    Some(format!("Provider '{}' is not registered", provider_id)),
                ),
            };
            event_bus.publish(Event::ProviderLoadingFinished {
                provider_id,
                provider_name,
                models,
                error,
            });
        });
    }

    /// Build the human-readable `"Provider / model [thinking: Level]"` label
    /// shown in the status bar, or `None` when no provider/model is configured.
    ///
    /// When the active model belongs to the router virtual provider, the label
    /// reads `"Model Router ({downstream}) / {tier}"` once a request has been
    /// routed, showing the actual downstream model and the selected bucket.
    /// Before the first routing decision (or when the router is not active)
    /// it falls back to `"Model Router / router"`.
    /// If the router is enabled but a concrete model is still selected, the
    /// label falls back to that provider so the displayed name matches the
    /// provider actually handling the request.
    pub fn provider_model_label(&self) -> Option<String> {
        let model_ref = self.active_model_ref_string()?;
        let (provider_id, model_id) = model_ref
            .split_once('/')
            .unwrap_or((&model_ref, &model_ref));

        let thinking = self
            .active_model_entry()
            .filter(|entry| !entry.thinking_levels.is_empty())
            .and(self.effective_thinking_level_for_agent(&self.agent_info))
            .map(|level| format!(" [thinking: {}]", Self::thinking_level_display(level)))
            .unwrap_or_default();

        if provider_id == "router" {
            let label = match (&self.router_current_model, &self.router_current_tier) {
                (Some(model), Some(tier)) => {
                    format!("Model Router ({}) / {}", model, tier)
                }
                _ => "Model Router / router".to_string(),
            };
            return Some(format!("{}{}", label, thinking));
        }

        let provider_name = self
            .configured_provider
            .as_ref()
            .filter(|p| p.id == provider_id)
            .map(|p| p.name.clone())
            .or_else(|| {
                self.provider_registry
                    .get(provider_id)
                    .map(|p| p.name().to_string())
            })
            .unwrap_or_else(|| provider_id.to_string());
        Some(format!("{} / {}{}", provider_name, model_id, thinking))
    }

    pub(crate) fn active_model_ref_string(&self) -> Option<String> {
        self.selected_model.clone().or_else(|| {
            self.agent_info
                .model
                .as_ref()
                .map(|model| format!("{}/{}", model.provider_id, model.model_id))
        })
    }

    pub(crate) fn active_model_metadata_report(&self) -> Option<String> {
        let model_ref = self.active_model_ref_string()?;
        let (provider_id, model_id) = model_ref.split_once('/').unwrap_or((&model_ref, ""));
        let provider_name = self
            .configured_provider
            .as_ref()
            .filter(|provider| provider.id == provider_id)
            .map(|provider| provider.name.clone())
            .or_else(|| {
                self.provider_registry
                    .get(provider_id)
                    .map(|provider| provider.name().to_string())
            })
            .unwrap_or_else(|| provider_id.to_string());

        let mut report = format!(
            "From: /model show\n\n# Active Model\n\n- **Provider:** {} (`{}`)\n- **Model ID:** `{}`\n- **Model Ref:** `{}`\n",
            provider_name, provider_id, model_id, model_ref
        );

        if let Some(entry) = self
            .resolved_model_entries_for_provider(provider_id)
            .into_iter()
            .find(|entry| entry.id == model_id)
        {
            let max_output = entry
                .max_output
                .map(|value| value.to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            report.push_str(&format!(
                "\n## Capabilities\n\n- **Reasoning:** {}\n- **Vision:** {}\n- **Tool use:** {}\n\n## Limits\n\n- **Context window:** {} tokens\n- **Max output:** {}\n\n## Cost\n\n- **Input:** ${:.2} / 1M tokens\n- **Output:** ${:.2} / 1M tokens\n- **Tier:** {}\n- **Relative multiplier:** {}\n",
                if entry.reasoning { "Yes" } else { "No" },
                if entry.vision { "Yes" } else { "No" },
                if entry.tool_use { "Yes" } else { "No" },
                entry.context_window,
                max_output,
                entry.cost_input,
                entry.cost_output,
                entry.cost_tier,
                entry.cost_multiplier,
            ));
            report.push_str(&format!(
                "\n## Thinking\n\n- **Current level:** {}\n- **Supported levels:** {}\n",
                self.effective_thinking_level_for_agent(&self.agent_info)
                    .map(Self::thinking_level_display)
                    .unwrap_or("unknown"),
                Self::format_thinking_levels(&entry.thinking_levels),
            ));

            if entry.name != entry.id {
                report.push_str(&format!("\n- **Display name:** {}\n", entry.name));
            }
        } else {
            report.push_str("\n_Metadata could not be resolved from the provider registry._\n");
            if let Some(context_window) = self.selected_model_context_window() {
                report.push_str(&format!(
                    "\n- **Cached context window:** {} tokens\n",
                    context_window
                ));
            }
        }

        Some(report)
    }

    /// Render a detailed `/provider show` report for the router virtual provider,
    /// including each tier and the provider/model pairs assigned to it (FR-010).
    pub fn router_config_report(
        &self,
        _registry: &ragent_llm::provider::ProviderRegistry,
    ) -> String {
        let router_config = self.load_raw_router_config().unwrap_or_default();

        let mut report = String::from(
            "From: /provider show\n\n# Provider: Model Router\n\n- **ID:** `router`\n- **Name:** Model Router\n",
        );
        report.push_str(&format!(
            "- **Enabled:** {}\n\n",
            if router_config.enabled { "yes" } else { "no" }
        ));

        report.push_str("## Tier Mappings\n\n");
        for tier in ragent_llm::providers::router_config::Tier::all() {
            report.push_str(&format!("### {}\n\n", tier));
            let tier_config = router_config.tier_config(*tier);
            if tier_config.models.is_empty() {
                report.push_str("_No models assigned._\n\n");
            } else {
                for entry in &tier_config.models {
                    let provider_name = self
                        .provider_registry
                        .get(&entry.provider)
                        .map(|p| p.name().to_string())
                        .unwrap_or_else(|| entry.provider.clone());
                    report.push_str(&format!(
                        "- `{}` via **{}** (`{}`)\n",
                        entry.model, provider_name, entry.provider
                    ));
                }
                report.push('\n');
            }
        }

        report
    }

    /// Load the raw `provider.router` block from the on-disk `ragent.json`
    /// without being affected by `ProviderConfig` catch-all deserialisation.
    pub(crate) fn load_raw_router_config(
        &self,
    ) -> Option<ragent_llm::providers::router_config::RouterConfig> {
        for path in &self.config_paths {
            if !path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(path).unwrap_or_default();
            let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
            let router_value = json.get("provider")?.get("router")?;
            let router_config = serde_json::from_value::<
                ragent_llm::providers::router_config::RouterConfig,
            >(router_value.clone())
            .ok()?;
            return Some(router_config);
        }
        None
    }

    /// Build the [`ProviderSetupStep::SetupRouter`] state, seeded from the
    /// persisted `provider.router` configuration when one exists.
    ///
    /// When a saved router cluster is present, its tiers (and the provider
    /// ids referenced by them) are loaded into the draft so the user can see
    /// and edit their existing configuration instead of starting from an
    /// empty panel. When no configuration has been saved yet, a fresh empty
    /// draft is used so first-time setup still presents four empty buckets.
    ///
    /// `selected_provider_ids` is pre-populated with the concrete providers
    /// (from `providers`) that are referenced by any saved tier, so the left
    /// palette reflects existing cluster membership. Providers that are no
    /// longer configured are not pre-checked.
    pub(crate) fn seeded_router_setup_step(
        &self,
        providers: Vec<ConfiguredProvider>,
    ) -> ProviderSetupStep {
        use ragent_llm::providers::router_config::{RouterConfig, Tier};

        let (draft_config, selected_provider_ids) = if let Some(saved) =
            self.load_raw_router_config()
        {
            let provider_ids: std::collections::HashSet<String> =
                providers.iter().map(|p| p.id.clone()).collect();
            let mut selected: Vec<String> = Vec::new();
            for tier in saved.tiers.values() {
                for entry in &tier.models {
                    if provider_ids.contains(&entry.provider) && !selected.contains(&entry.provider)
                    {
                        selected.push(entry.provider.clone());
                    }
                }
            }
            (saved, selected)
        } else {
            (
                RouterConfig {
                    enabled: false,
                    tiers: std::collections::HashMap::new(),
                    ..RouterConfig::default()
                },
                Vec::new(),
            )
        };

        ProviderSetupStep::SetupRouter {
            providers,
            selected_provider_ids,
            selected_provider_index: 0,
            draft_config,
            active_bucket: Tier::Simple,
            active_bucket_index: 0,
            left_pane_focused: true,
            error: None,
        }
    }

    /// Persist the supplied draft router cluster to `ragent.json`.
    ///
    /// The existing classifier weights, boundary thresholds, context window, and
    /// timeout settings are preserved when a prior `provider.router` block
    /// exists (FR-026). The resulting `provider.router` value is the serialised
    /// [`RouterConfig`] itself, matching the format expected by `/router reload`
    /// (FR-008, FR-009, FR-014).
    pub(crate) fn save_router_config(
        &self,
        draft: &ragent_llm::providers::router_config::RouterConfig,
    ) -> Result<(), String> {
        use std::collections::HashMap;

        let config_path = self.config_paths.first().cloned().unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join(".ragent")
                .join("ragent.json")
        });

        // Seed the saved config from any existing router block so we preserve
        // weights, boundaries, and other manually-edited fields.
        let mut saved = self.load_raw_router_config().unwrap_or_default();
        saved.enabled = true;
        saved.tiers = HashMap::new();
        for (key, tier) in &draft.tiers {
            if !tier.models.is_empty() {
                saved.tiers.insert(key.clone(), tier.clone());
            }
        }

        let router_value =
            serde_json::to_value(&saved).map_err(|e| format!("serialise router config: {e}"))?;

        crate::app::state::atomic_config_update(&config_path, |json| {
            json["provider"]["router"] = router_value.clone();
            Ok(())
        })
        .map_err(|e| format!("save router config: {e}"))?;

        // Keep the in-memory provider registry in sync so the status bar and
        // subsequent routing reflect the new configuration immediately.
        if let Some(router_provider) = self
            .provider_registry
            .get_as_any("router")
            .and_then(|p| p.downcast_ref::<ragent_llm::providers::router::RouterProvider>())
        {
            router_provider.reload_config(saved.clone());
        }

        Ok(())
    }

    /// Return a human-readable cost estimate for a provider/model pair, if the
    /// registry advertises pricing information for the model.
    pub(crate) fn estimate_entry_cost(&self, provider_id: &str, model_id: &str) -> Option<String> {
        let model = self
            .provider_registry
            .resolve_model(provider_id, model_id)?;
        let avg = f64::midpoint(model.cost.input, model.cost.output);
        if avg <= 0.0 {
            return None;
        }
        Some(format!("~${:.2}/M", avg))
    }

    /// Resolve a router tier entry `(provider_id, model_id)` into a fully
    /// populated [`ModelPickerEntry`] so the router setup UI can display the
    /// model's properties (context window, features, thinking levels, cost
    /// tier) alongside the assignment.
    ///
    /// Lookup first tries cached/discovered entries (which retain the richest
    /// metadata, including cost multipliers for Copilot), then falls back to the
    /// provider registry's static default catalog via [`ProviderRegistry::resolve_model`].
    /// Returns `None` when neither source advertises the model, in which case
    /// the caller renders a minimal `provider / model` label.
    pub(crate) fn router_model_picker_entry(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Option<ModelPickerEntry> {
        // Prefer cached/discovered entries: they include provider-specific
        // metadata such as Copilot premium-request multipliers.
        let cached = self.models_for_provider(provider_id);
        if let Some(entry) = cached
            .iter()
            .find(|e| e.id == model_id || e.id.split_once('@').map(|(b, _)| b) == Some(model_id))
        {
            return Some(entry.clone());
        }

        // Fall back to the registry's static default-model catalog.
        let model = self
            .provider_registry
            .resolve_model(provider_id, model_id)?;
        let avg_cost = f64::midpoint(model.cost.input, model.cost.output);
        Some(self.model_to_picker_entry(model, avg_cost))
    }

    /// Render a detailed `/provider show` report for a single configured
    /// provider: id, name, source, configured models, and thinking settings.
    pub(crate) fn provider_config_report(&self, prov: &ConfiguredProvider) -> String {
        let mut report = format!(
            "From: /provider show\n\n# Provider: {}\n\n- **ID:** `{}`\n- **Name:** {}\n- **Source:** {}\n",
            prov.name,
            prov.id,
            prov.name,
            match prov.source {
                ProviderSource::EnvVar => "Environment variable",
                ProviderSource::Database => "Database (stored credential)",
                ProviderSource::AutoDiscovered => "Auto-discovered",
            }
        );

        let config = self.current_config();
        let mut has_config_section = false;
        if let Some(provider_config) = config.provider.get(&prov.id) {
            has_config_section = true;
            report.push_str("\n## Configuration (ragent.json)\n\n");
            if !provider_config.env.is_empty() {
                report.push_str(&format!(
                    "- **Env vars:** {}\n",
                    provider_config.env.join(", ")
                ));
            }
            if let Some(ref api) = provider_config.api {
                if let Some(ref url) = api.base_url {
                    report.push_str(&format!("- **API base URL:** {}\n", url));
                }
                if !api.headers.is_empty() {
                    report.push_str("- **Headers:**\n");
                    for (k, v) in &api.headers {
                        report.push_str(&format!("  - `{}`: `{}`\n", k, v));
                    }
                }
            }
            if let Some(ref thinking) = provider_config.thinking {
                report.push_str(&format!("- **Thinking:** {:?}\n", thinking));
            }
            if !provider_config.models.is_empty() {
                report.push_str("\n**Models:**\n\n");
                for (id, cfg) in &provider_config.models {
                    report.push_str(&format!("- `{}`", id));
                    if let Some(ref n) = cfg.name {
                        report.push_str(&format!(" ({})", n));
                    }
                    report.push_str("\n");
                }
            }
            if !provider_config.options.is_empty() {
                report.push_str("\n**Options:**\n\n");
                for (k, v) in &provider_config.options {
                    report.push_str(&format!("- `{}`: {}\n", k, v));
                }
            }
        }
        // Also show stored endpoint from settings (for generic_openai / azure_foundry)
        if prov.id == "generic_openai" || prov.id == "azure_foundry" {
            if let Ok(Some(stored_endpoint)) =
                self.storage.get_setting(&format!("{}_api_base", prov.id))
            {
                if !stored_endpoint.is_empty() {
                    if !has_config_section {
                        report.push_str("\n## Configuration\n\n");
                        has_config_section = true;
                    }
                    report.push_str(&format!("- **Stored endpoint:** {}\n", stored_endpoint));
                }
            }
        }
        if !has_config_section {
            report.push_str("\n_No custom configuration in ragent.json_\n");
        }
        let models = self.models_for_provider(&prov.id);
        if !models.is_empty() {
            report.push_str("\n## Available Models\n\n");
            for m in &models {
                report.push_str(&format!(
                    "- `{}` (ctx: {} tokens{})\n",
                    m.id,
                    m.context_window,
                    if m.name != m.id {
                        format!(", {}", m.name)
                    } else {
                        String::new()
                    }
                ));
            }
        } else {
            report.push_str("\n_No models available for this provider._\n");
        }

        report
    }

    /// Summarize accumulated LLM request stats (input/output tokens, samples,
    /// and estimated USD cost) grouped by provider. Returns `None` when no
    /// requests have been recorded.
    pub(crate) fn cost_summary(&self) -> Option<String> {
        if self.llm_request_stats.is_empty() {
            return None;
        }

        #[derive(Default)]
        struct ProviderCost {
            input_tokens: u64,
            output_tokens: u64,
            samples: usize,
            cost_usd: f64,
        }

        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;
        let mut total_cost = 0.0f64;
        let mut by_provider: std::collections::HashMap<String, ProviderCost> =
            std::collections::HashMap::new();

        for sample in &self.llm_request_stats {
            let (provider_id, model_id) = sample
                .model_ref
                .split_once('/')
                .unwrap_or((&sample.model_ref, ""));
            let model = self.provider_registry.resolve_model(provider_id, model_id);
            let cost = model
                .map(|m| {
                    (sample.input_tokens as f64 * m.cost.input / 1_000_000.0)
                        + (sample.output_tokens as f64 * m.cost.output / 1_000_000.0)
                })
                .unwrap_or(0.0);

            total_input_tokens += sample.input_tokens;
            total_output_tokens += sample.output_tokens;
            total_cost += cost;

            let entry = by_provider.entry(provider_id.to_string()).or_default();
            entry.input_tokens += sample.input_tokens;
            entry.output_tokens += sample.output_tokens;
            entry.samples += 1;
            entry.cost_usd += cost;
        }

        let session_duration = self
            .session_id
            .as_deref()
            .and_then(|sid| {
                self.session_processor
                    .session_manager
                    .get_session(sid)
                    .ok()
                    .flatten()
            })
            .map(|session| chrono::Utc::now() - session.created_at)
            .map(|duration| {
                let seconds = duration.num_seconds().max(0);
                let hours = seconds / 3600;
                let minutes = (seconds % 3600) / 60;
                let secs = seconds % 60;
                if hours > 0 {
                    format!("{hours}h {minutes}m {secs}s")
                } else if minutes > 0 {
                    format!("{minutes}m {secs}s")
                } else {
                    format!("{secs}s")
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let mut providers: Vec<_> = by_provider.into_iter().collect();
        providers.sort_by(|a, b| a.0.cmp(&b.0));

        let mut out = String::from("From: /cost\n");
        out.push_str(&format!("Samples: {}\n", self.llm_request_stats.len()));
        out.push_str(&format!("Session duration: {}\n", session_duration));
        out.push_str(&format!(
            "Total tokens: {} input / {} output\n",
            total_input_tokens, total_output_tokens
        ));
        out.push_str(&format!("Estimated cost: ${:.6}\n", total_cost));
        if !providers.is_empty() {
            out.push_str("\nBy provider:\n");
            for (provider, summary) in providers {
                out.push_str(&format!(
                    "  - {}: ${:.6} ({} in / {} out, {} samples)\n",
                    provider,
                    summary.cost_usd,
                    summary.input_tokens,
                    summary.output_tokens,
                    summary.samples
                ));
            }
        }

        Some(out)
    }

    /// Spawn an async provider health check (pinging the configured endpoint)
    /// and store the result in `provider_health` (`0` = unknown, `1` = up,
    /// `2` = down). Resolves the Copilot token via env/IDE/`gh`/DB first.
    ///
    /// The Copilot token resolution (which may spawn `gh auth token` as a
    /// subprocess) is performed **inside** the spawned async task so it does
    /// not block the TUI render loop during startup.
    pub(crate) fn check_provider_health(&mut self) {
        let provider = match &self.configured_provider {
            Some(p) => p.clone(),
            None => {
                self.provider_health.store(0, Ordering::Relaxed);
                return;
            }
        };
        self.provider_health.store(0, Ordering::Relaxed);
        let health = self.provider_health.clone();
        let storage = self.storage.clone();

        tokio::spawn(async move {
            // Resolve the copilot token inside the async task so the `gh`
            // CLI subprocess call does not block the TUI startup.
            let copilot_token = if provider.id == "copilot" {
                let storage = storage.clone();
                let db_lookup = move || {
                    storage
                        .get_provider_auth("copilot")
                        .ok()
                        .flatten()
                        .filter(|k| !k.is_empty())
                };
                ragent_agent::provider::copilot::resolve_copilot_github_token(Some(&db_lookup))
            } else {
                None
            };

            let available = match provider.id.as_str() {
                "ollama" => ragent_agent::provider::ollama::list_ollama_models(None)
                    .await
                    .is_ok(),
                "copilot" => {
                    if let Some(token) = copilot_token {
                        ragent_agent::provider::copilot::check_copilot_health(&token).await
                    } else {
                        false
                    }
                }
                // For API-key providers we trust the key is present
                _ => true,
            };

            health.store(if available { 1 } else { 2 }, Ordering::Relaxed);
        });
    }

    /// Return the last known provider health state, or `None` if no check
    /// has completed yet.
    pub(crate) fn provider_health_status(&self) -> Option<bool> {
        match self.provider_health.load(Ordering::Relaxed) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }

    pub(crate) fn tool_visibility_switches(&self) -> [(&'static str, bool); 9] {
        [
            ("office", self.tool_visibility.office),
            ("github", self.tool_visibility.github),
            ("gitlab", self.tool_visibility.gitlab),
            ("teams", self.tool_visibility.teams),
            ("agents", self.tool_visibility.agents),
            ("plan", self.tool_visibility.plan),
            ("codeindex", self.tool_visibility.codeindex),
            ("masterfetch", self.tool_visibility.masterfetch),
            ("browser", self.tool_visibility.browser),
        ]
    }

    pub(crate) fn tool_visibility_state(&self, switch: &str) -> Option<bool> {
        self.tool_visibility_switches()
            .into_iter()
            .find_map(|(name, enabled)| (name == switch).then_some(enabled))
    }

    pub(crate) fn set_tool_visibility_state(&mut self, switch: &str, enabled: bool) -> bool {
        match switch {
            "office" => {
                self.tool_visibility.office = enabled;
                self.tool_visibility.specified.office = true;
            }
            "github" => {
                self.tool_visibility.github = enabled;
                self.tool_visibility.specified.github = true;
            }
            "gitlab" => {
                self.tool_visibility.gitlab = enabled;
                self.tool_visibility.specified.gitlab = true;
            }
            "teams" => {
                self.tool_visibility.teams = enabled;
                self.tool_visibility.specified.teams = true;
            }
            "agents" => {
                self.tool_visibility.agents = enabled;
                self.tool_visibility.specified.agents = true;
            }
            "plan" => {
                self.tool_visibility.plan = enabled;
                self.tool_visibility.specified.plan = true;
            }
            "codeindex" => self.tool_visibility.set_codeindex(enabled),
            "masterfetch" => {
                self.tool_visibility.masterfetch = enabled;
                self.tool_visibility.specified.masterfetch = true;
            }
            "browser" => {
                self.tool_visibility.browser = enabled;
                self.tool_visibility.specified.browser = true;
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn render_tool_visibility_table(&self) -> String {
        let mut output = String::from(
            "From: /tools\nTool Family Visibility\n\n```text\nfamily     state\n------     -----\n",
        );
        for (name, enabled) in self.tool_visibility_switches() {
            output.push_str(&format!(
                "{name:<10} {}\n",
                if enabled { "on" } else { "off" }
            ));
        }
        output.push_str("```\n\n");

        // List all currently visible tools from the registry.
        let defs = self.session_processor.tool_registry.definitions();
        if defs.is_empty() {
            output.push_str("No tools are currently visible.\n");
        } else {
            output.push_str(&format!(
                "Visible Tools ({} total):\n\n```text\n{:<24} description\n{:<24} -----------\n",
                defs.len(),
                "name",
                "----"
            ));
            for def in defs {
                let desc = ragent_types::truncate_bytes(&def.description, 60);
                output.push_str(&format!("{:<24} {}\n", def.name, desc));
            }
            output.push_str("```\n");
        }

        output
    }

    pub(crate) fn sync_tool_visibility_from_config(&mut self, cfg: &ragent_agent::Config) {
        self.tool_visibility = cfg.tool_visibility.clone();
        let hidden = cfg.effective_hidden_tools();
        self.session_processor.tool_registry.set_hidden(&hidden);
    }

    pub(crate) fn populate_directory_menu(
        &mut self,
        dir_rel: &std::path::Path,
        filter: Option<&str>,
    ) {
        let wd = std::env::current_dir().unwrap_or_default();
        let abs = wd.join(dir_rel);
        let mut entries: Vec<FileMenuEntry> = Vec::new();
        let filter_lower = filter
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase());

        if abs.is_dir() {
            // Read the directory contents from disk (sorted)
            if let Ok(rd) = std::fs::read_dir(&abs) {
                let mut sorted: Vec<_> = rd.filter_map(|e| e.ok()).collect();
                sorted.sort_by_key(|e| e.file_name());
                for entry in sorted {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Skip hidden
                    if name.starts_with('.') && !self.file_menu_show_hidden {
                        continue;
                    }
                    if let Some(ref f) = filter_lower
                        && !name.to_lowercase().contains(f)
                    {
                        continue;
                    }
                    let path_abs = entry.path();
                    let is_dir = path_abs.is_dir();
                    let rel = path_abs
                        .strip_prefix(&wd)
                        .unwrap_or(&path_abs)
                        .to_path_buf();
                    let display = if is_dir {
                        format!("{}/", rel.to_string_lossy())
                    } else {
                        rel.to_string_lossy().to_string()
                    };
                    entries.push(FileMenuEntry {
                        display,
                        path: rel,
                        is_dir,
                    });
                }
            }

            // Add parent entry if not at project root
            if !dir_rel.as_os_str().is_empty() {
                let parent = dir_rel.parent().unwrap_or(std::path::Path::new(""));
                let parent_display = if parent.as_os_str().is_empty() {
                    "../".to_string()
                } else {
                    format!("{}/", parent.to_string_lossy())
                };
                entries.insert(
                    0,
                    FileMenuEntry {
                        display: parent_display,
                        path: parent.to_path_buf(),
                        is_dir: true,
                    },
                );
            }

            // Add explicit "back to fuzzy search" action.
            entries.insert(
                0,
                FileMenuEntry {
                    display: "<back to fuzzy>".to_string(),
                    path: std::path::PathBuf::new(),
                    is_dir: true,
                },
            );
        }

        if entries.is_empty() {
            self.file_menu = None;
        } else {
            self.file_menu = Some(FileMenuState {
                selected: 0,
                matches: entries,
                scroll_offset: 0,
                query: filter.unwrap_or_default().to_string(),
                current_dir: Some(dir_rel.to_path_buf()),
            });
        }
    }

    pub(crate) fn set_profile_panel_enabled(&mut self, enabled: bool) {
        let profiler = ragent_agent::session::profiler::agent_loop_profiler();
        profiler.set_enabled(enabled);
        self.show_profile = enabled;
        if enabled {
            // Entering profile mode: dismiss the other side panels so only one
            // occupies the side column (FR-012 mutual-exclusion policy).
            self.show_log = false;
            self.show_tasks_panel = false;
            self.show_memory = false;
            self.show_telemetry = false;
        }
        self.status = if enabled {
            "profile panel visible".to_string()
        } else {
            "profile panel hidden".to_string()
        };
        self.push_log_no_agent(
            LogLevel::Info,
            if enabled {
                "Agent loop profiler enabled".to_string()
            } else {
                "Agent loop profiler disabled".to_string()
            },
        );
    }
}
