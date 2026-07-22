//! `ResearchSession` — the gathering orchestration engine.
//!
//! Combines the [`WebGatherer`] (T-014), [`LocalGatherer`] (T-016), and
//! [`LocalGatherer`] cross-referencing (T-018) into a single pass that
//! produces a fully-populated [`ResearchDocument`] ready for
//! [`ResearchManager::write_document`].
//!
//! This is the engine the TUI `/research create` slash command, the CLI
//! `ragent research create` sub-command, and the `POST /research` HTTP
//! endpoint all call (T-019, T-027, T-034, T-036).

use crate::analysis::{AnalysisEngine, AnalysisOutcome, AnalysisResult, build_source_bodies};
use crate::document::{ResearchDocument, mark_in_progress};
use crate::engine::{Critic, EngineConfig, IterativeEngine, SimpleCritic};
use crate::io::ResearchIo;
use crate::item::ResearchItem;
use crate::local_gatherer::{LocalGatherConfig, LocalGatherer, LocalTool};
use crate::manager::{ResearchError, ResearchManager, Result};
use crate::planner::{HeuristicPlanner, Planner};
use crate::research_name::ResearchName;
use crate::run_config::{Depth, OutputFormat};
use crate::source::{LocalSourceKind, Source};
use crate::web_gatherer::{
    DEFAULT_FETCH_CONCURRENCY, DEFAULT_MAX_WEB_RESULTS, GatherEvent, GatherObserver, WebGatherer,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

/// Forwards [`GatherEvent`]s from the [`WebGatherer`] into [`SessionEvent`]s
/// so the TUI/CLI can display why web sources were not captured.
struct GatherEventForwarder {
    observer: Arc<dyn SessionObserver>,
}

impl GatherObserver for GatherEventForwarder {
    fn on_event(&self, event: GatherEvent) {
        match event {
            GatherEvent::SearchFailed { error } => {
                self.observer
                    .on_event(SessionEvent::WebSearchFailed { error });
            }
            GatherEvent::FetchFailed { url, error } => {
                self.observer
                    .on_event(SessionEvent::WebFetchFailed { url, error });
            }
            GatherEvent::SearchReturnedNoHits => {
                self.observer.on_event(SessionEvent::WebSearchFailed {
                    error: "web search returned 0 hits".into(),
                });
            }
            GatherEvent::QueriesDecomposed { queries } => {
                // Forward immediately so the UI can render the decomposed
                // sub-queries as soon as they are generated, before the
                // parallel searches complete.
                self.observer
                    .on_event(SessionEvent::QueriesDecomposed { queries });
            }
            GatherEvent::SourceCaptured { url, title } => {
                // Forward inline so the UI shows each successfully retrieved
                // URL as it arrives, rather than only at the end of the
                // gather pass.
                self.observer
                    .on_event(SessionEvent::WebCaptured { url, title });
            }
        }
    }
}

/// Inputs the caller supplies to [`ResearchSession::run`].
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Free-form research topic — used to derive web queries and grep terms.
    ///
    /// When [`Self::from_url`] is set and `topic` is empty, the topic is
    /// derived from the fetched page body (cleaned via `readability-rs` in the
    /// `webfetch` tool) so the rest of the pipeline (query decomposition, local
    /// grep terms, synthesis) has a subject that reflects the page's actual
    /// content rather than its `<title>`. The full fetched page body is captured
    /// as the first web source regardless. When the cleaned body yields no
    /// usable topic the page title, then the URL, is used as a fallback.
    pub topic: String,
    /// Optional FR-019 extra sources directory.
    pub sources_dir: Option<PathBuf>,
    /// Optional FR-020 template file (resolved against `_templates/`).
    pub template: Option<String>,
    /// Maximum web sources to capture (default `5`).
    pub max_web_results: usize,
    /// Maximum in-project local sources to capture (default `10`).
    pub max_local_sources: usize,
    /// When `true`, skip the local-file scanning phase entirely.
    pub disable_local: bool,
    /// When `true`, skip the prior-spec cross-reference phase entirely.
    pub disable_specs: bool,
    /// `--from-url <URL>`: fetch the URL before gathering and use the returned
    /// page content as the research subject in place of an explicit topic.
    ///
    /// When set, the fetched page is captured as the primary web source and
    /// (when `topic` is empty) the page body is cleaned by the `readability-rs`
    /// extractor in the `webfetch` tool, from which a concise topic is derived
    /// for query decomposition, local-grep term derivation, and synthesis. The
    /// normal web-search phase still runs, using that derived topic, so
    /// additional related sources are gathered as usual.
    pub from_url: Option<String>,
    /// Maximum number of candidate pages to fetch concurrently during the
    /// web-gathering phase. Defaults to [`DEFAULT_FETCH_CONCURRENCY`] (10).
    /// Larger values reduce wall-clock latency when a search returns many
    /// hits, at the cost of more in-flight HTTP connections and memory.
    /// Override per-run with the `--fetch-concurrently N` CLI flag.
    pub fetch_concurrency: usize,
    /// Depth preset selected via `--depth`. When `None`, the engine behaves as
    /// `Depth::Standard` for budget purposes and remains single-pass.
    pub depth: Option<Depth>,
    /// Iteration override selected via `--iterations`. When `None`, the depth
    /// preset controls iteration count; the iterative branch is only taken
    /// when this is `Some` or depth is `Deep`.
    pub iterations: Option<u32>,
    /// Output artifact selected via `--format`.
    pub output_format: OutputFormat,
}

impl SessionConfig {
    /// Resolve the effective [`EngineConfig`] from `depth` + `iterations`.
    pub fn engine_config(&self) -> EngineConfig {
        let depth = self.depth.unwrap_or(Depth::Standard);
        depth.engine_config(self.iterations, depth == Depth::Deep)
    }

    /// Maximum web sources to capture for the selected depth/iteration combo.
    pub fn budget_web_results(&self) -> usize {
        let cfg = self.engine_config();
        (cfg.max_sources_per_question * 3).max(3)
    }

    /// Maximum local sources to capture for the selected depth.
    pub fn budget_local_sources(&self) -> usize {
        match self.depth.unwrap_or(Depth::Standard) {
            Depth::Shallow => 5,
            Depth::Standard => 10,
            Depth::Deep => 20,
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            topic: String::new(),
            sources_dir: None,
            template: None,
            max_web_results: DEFAULT_MAX_WEB_RESULTS,
            max_local_sources: 10,
            disable_local: true,
            disable_specs: true,
            from_url: None,
            fetch_concurrency: DEFAULT_FETCH_CONCURRENCY,
            depth: None,
            iterations: None,
            output_format: OutputFormat::Report,
        }
    }
}

/// Phases of a research session, in execution order. Surfaced via the
/// [`SessionEvent::Phase`] callback so the TUI log panel and the CLI JSON
/// emitter can show progress (T-027, T-035).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// Validating the supplied name and creating the item directory.
    Setup,
    /// Issuing web searches and fetching pages.
    Web,
    /// Scanning the project and any extra sources dir.
    Local,
    /// Cross-referencing prior specs.
    Specs,
    /// Synthesizing a structured analysis from gathered sources.
    Synthesize,
    /// Assembling the final `RESEARCH.md`.
    Assemble,
    /// Marking the item `Complete` and refreshing the index.
    Finalize,
}

impl SessionPhase {
    /// Human-readable label for log output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Web => "web",
            Self::Local => "local",
            Self::Specs => "specs",
            Self::Synthesize => "synthesize",
            Self::Assemble => "assemble",
            Self::Finalize => "finalize",
        }
    }
}

/// Progress event emitted as a research session runs. The TUI/CLI/HTTP
/// layers subscribe to this to render streaming progress.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A new phase has started.
    Phase {
        /// The phase that just started.
        phase: SessionPhase,
    },
    /// The web-gathering phase produced these focused sub-queries and will
    /// run each one in parallel.
    QueriesDecomposed {
        /// Sub-queries issued to the search tool.
        queries: Vec<String>,
    },
    /// The web-gathering phase captured a single source.
    WebCaptured {
        /// URL of the captured page.
        url: String,
        /// Page title (may be empty).
        title: String,
    },
    /// The `--from-url` primary page was fetched. Carries a short preview of
    /// the extracted article body so the UI can show what topic was derived
    /// from the page. Emitted once, immediately after the primary fetch
    /// succeeds, before the normal web-gathering phase runs.
    FromUrlBodyPreview {
        /// URL that was fetched.
        url: String,
        /// First ~200 characters of the cleaned page body used to derive the
        /// research topic.
        body_preview: String,
    },
    /// The local-gathering phase scored and captured a file.
    LocalCaptured {
        /// Project-relative path of the captured file.
        path: String,
        /// Relevance score from the keyword matcher.
        score: usize,
    },
    /// The session captured a prior spec as a cross-reference.
    SpecCaptured {
        /// Spec identifier.
        spec_id: String,
    },
    /// The web-gathering phase failed as a whole (search error, missing
    /// API key, network failure, etc.).
    WebSearchFailed {
        /// Human-readable error message.
        error: String,
    },
    /// A single candidate page could not be fetched.
    WebFetchFailed {
        /// URL that failed.
        url: String,
        /// Human-readable error message.
        error: String,
    },
    /// The synthesis phase finished (or fell back). Surfaces whether the
    /// final summary/findings came from an LLM or from the mechanical
    /// fallback so the UI can be transparent about it.
    SynthesizeResult {
        /// How the synthesis result was produced.
        outcome: SynthesizeOutcome,
        /// Optional human-readable detail (e.g. the LLM error message when
        /// the synthesis failed and the fallback was used).
        detail: Option<String>,
    },
    /// The research plan was updated with a new set of sub-questions.
    PlanUpdated {
        /// Sub-question texts in plan order.
        sub_questions: Vec<String>,
    },
    /// A sub-question changed status (e.g. pending → in_progress → answered).
    SubQuestionStatusChanged {
        /// Sub-question id.
        id: String,
        /// New status label (see [`SubQuestionStatus::as_str`](crate::state::SubQuestionStatus::as_str)).
        status: String,
    },
    /// A generic source fetch (web, local, or other) failed and was recorded
    /// in session state.
    SourceFailed {
        /// Optional source identifier (URL, path, or label). `None` when the
        /// failure is not tied to a single source.
        source: Option<String>,
        /// Human-readable error message.
        error: String,
    },
    /// The critic/evaluator finished an iteration.
    CriticResult {
        /// Evaluation score, if the critic produced one.
        score: Option<u32>,
        /// Short descriptions of any new evidence gaps.
        gaps: Vec<String>,
    },
    /// The verifier finished checking claims against sources.
    VerificationResult {
        /// `true` when every checked claim had source support.
        passed: bool,
        /// Human-readable issues for any failed checks.
        issues: Vec<String>,
    },
    /// A single iteration of the research loop completed.
    IterationCompleted {
        /// 1-based iteration number.
        iteration: u32,
        /// Evaluation score after this iteration, if known.
        score: Option<u32>,
    },
    /// Follow-up bridge queries were generated to close evidence gaps.
    FollowUpQueries {
        /// Queries to run in the next retrieval pass.
        queries: Vec<String>,
    },
    /// The session has finished and a fully-populated document was written.
    Done {
        /// Total number of sources captured.
        total_sources: usize,
    },
    /// Resolved run options, emitted once at the start of a session so that
    /// every observer (CLI JSON, TUI progress log, HTTP response) can confirm
    /// the output format and other flags that are in effect (FR-012).
    ConfigSnapshot {
        /// Output artifact selected via `--format`.
        output_format: String,
        /// Depth preset selected via `--depth`, if any.
        depth: Option<String>,
        /// Iteration override selected via `--iterations`, if any.
        iterations: Option<u32>,
        /// `--from-url` primary source, if any.
        from_url: Option<String>,
    },
}

/// Outcome of the synthesis phase, surfaced via
/// [`SessionEvent::SynthesizeResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesizeOutcome {
    /// An LLM produced a structured [`AnalysisResult`] and it was used.
    Llm,
    /// The LLM-backed engine returned empty content (e.g. parsing failed);
    /// the mechanical fallback supplied the summary/findings.
    FallbackEmpty,
    /// The LLM-backed engine returned an error (no key, network failure, …)
    /// and the mechanical fallback supplied the summary/findings.
    FallbackError,
    /// No LLM engine was wired in (`NoopAnalysisEngine`) and the mechanical
    /// fallback supplied the summary/findings.
    NoLlm,
}

impl SynthesizeOutcome {
    /// Short label for log output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::FallbackEmpty => "fallback-empty",
            Self::FallbackError => "fallback-error",
            Self::NoLlm => "no-llm",
        }
    }
}

/// Trait implemented by the TUI/CLI/HTTP callers to receive streaming
/// progress. The default [`NoopObserver`] discards all events.
pub trait SessionObserver: Send + Sync {
    /// Receive a progress event. Implementations should be cheap; the
    /// session calls this once per source.
    fn on_event(&self, event: SessionEvent);
}

/// Default observer that drops all events. Used when the caller doesn't
/// need progress streaming.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopObserver;

impl SessionObserver for NoopObserver {
    fn on_event(&self, _event: SessionEvent) {}
}

/// Orchestrates a single research session.
///
/// `ResearchSession` is cheap to clone (internally `Arc`s) so the TUI, CLI,
/// and HTTP layer can hold one instance per request and call
/// [`ResearchSession::run`] concurrently.
#[derive(Clone)]
pub struct ResearchSession {
    manager: ResearchManager,
    web: Option<WebGatherer>,
    local: Option<LocalGatherer>,
    analysis: Arc<dyn AnalysisEngine>,
    planner: Option<Arc<dyn Planner>>,
    critic: Option<Arc<dyn Critic>>,
}

impl std::fmt::Debug for ResearchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResearchSession")
            .field("research_root", &self.manager.root())
            .field("has_web", &self.web.is_some())
            .field("has_local", &self.local.is_some())
            .field("has_analysis", &!self.analysis_is_noop())
            .finish()
    }
}

impl ResearchSession {
    /// Returns `true` when the wired-in [`AnalysisEngine`] is the
    /// [`crate::analysis::NoopAnalysisEngine`] (no LLM analysis available).
    ///
    /// We compare `TypeId::of` of the concrete struct against the type-id
    /// of the value behind the trait object. The standard `Any::type_id`
    /// trick does not work here because `Any::type_id` on a trait object
    /// returns the *trait object's* `TypeId`, which is the same regardless
    /// of the underlying concrete type.
    fn analysis_is_noop(&self) -> bool {
        // `Arc<dyn AnalysisEngine>::as_ref()` gives `&dyn AnalysisEngine`,
        // which we can't directly query for its underlying type. So we use
        // a small discriminator that the constructors attach via a marker
        // method on the trait. `NoopAnalysisEngine` overrides it to return
        // `true`; every other implementation returns `false`.
        self.analysis.is_noop_marker()
    }

    /// Build a session over the given on-disk manager. Both web and local
    /// gatherers are optional; a session with neither is effectively a no-op
    /// (FR-006 graceful degradation).
    pub fn new(
        manager: ResearchManager,
        web: Option<WebGatherer>,
        local: Option<LocalGatherer>,
        analysis: Arc<dyn AnalysisEngine>,
    ) -> Self {
        Self {
            manager,
            web,
            local,
            analysis,
            planner: None,
            critic: None,
        }
    }

    /// Attach a planner for the iterative research branch.
    pub fn with_planner(mut self, planner: Arc<dyn Planner>) -> Self {
        self.planner = Some(planner);
        self
    }

    /// Attach a critic for the iterative research branch.
    pub fn with_critic(mut self, critic: Arc<dyn Critic>) -> Self {
        self.critic = Some(critic);
        self
    }

    /// Build a session backed only by a local tool (no web search).
    pub fn with_local_tool(
        manager: ResearchManager,
        local_tool: Arc<dyn LocalTool>,
        analysis: Arc<dyn AnalysisEngine>,
    ) -> Self {
        Self::new(
            manager,
            None,
            Some(LocalGatherer::new(local_tool)),
            analysis,
        )
    }
}

/// Minimum number of words a page title or body fragment must have to be
/// considered a usable research topic.
const MIN_TOPIC_WORDS: usize = 3;

/// Derive a concise but informative research topic from a `--from-url` page.
///
/// The `webfetch` tool already extracts a page title via `readability-rs`, so
/// the title is tried first — site names and common chrome prefixes are
/// stripped and glued-together words are split. When the title is available,
/// the first substantive sentence of the cleaned body is appended as a short
/// description (separated by an em dash) so the topic captures both *what* the
/// page is about and *why* it matters. If the title is missing or unusable,
/// the first substantive sentence of the body is used alone. Returns `None`
/// only when neither source yields usable text, so the caller can abort cleanly
/// instead of using a URL-only topic.
fn derive_topic_from_url_body(src_body: &str, src_title: &str, _src_url: &str) -> Option<String> {
    let title_topic = clean_site_title(src_title);
    let body_description = derive_topic_description(src_body, title_topic.as_deref());
    match (title_topic, body_description) {
        (Some(title), Some(desc)) => {
            let combined = format!("{} — {}", title, desc);
            Some(truncate_at_char_boundary(
                &combined,
                MAX_DERIVED_TOPIC_CHARS,
            ))
        }
        (Some(title), None) => Some(title),
        (None, Some(desc)) => Some(desc),
        (None, None) => {
            let body_topic = derive_topic_from_body(src_body);
            if body_topic.is_empty() {
                None
            } else {
                Some(body_topic)
            }
        }
    }
}

/// Maximum number of characters a derived topic may span.
const MAX_DERIVED_TOPIC_CHARS: usize = 240;

/// Maximum number of characters a body-derived description may span.
const MAX_BODY_DESCRIPTION_CHARS: usize = 140;

/// Pick the first substantive sentence from a cleaned page body to use as the
/// research topic. This is intentionally lightweight because the `webfetch` tool
/// already runs `readability-rs` to strip nav/cookie/footer chrome. If the
/// extractor could not isolate the article text and the tool fell back to
/// html2text, this helper skips link-only lines, headings of tables of contents,
/// update banners, and other common page noise.
fn derive_topic_from_body(cleaned_body: &str) -> String {
    let trimmed = cleaned_body.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    for raw in trimmed.split(['.', '?', '!', '\n']) {
        let fragment = raw.trim();
        if fragment.is_empty() {
            continue;
        }
        let cleaned = clean_topic_fragment(fragment);
        if cleaned.is_empty() {
            continue;
        }
        if is_topic_noise(&cleaned, fragment) {
            continue;
        }
        let word_count = cleaned.split_whitespace().count();
        // Headings ("# Introducing deep research") are valuable even when short.
        let is_heading = fragment.starts_with('#');
        if word_count >= 4 || (is_heading && word_count >= 3) {
            return truncate_at_char_boundary(&cleaned, MAX_DERIVED_TOPIC_CHARS);
        }
    }

    String::new()
}

/// Extract a short descriptive sentence from the cleaned page body to use as a
/// subtitle for a title-derived topic. The sentence must be substantive (at
/// least `MIN_BODY_DESCRIPTION_WORDS`), must not duplicate the page title, and
/// is truncated to [`MAX_BODY_DESCRIPTION_CHARS`].
fn derive_topic_description(cleaned_body: &str, title: Option<&str>) -> Option<String> {
    const MIN_BODY_DESCRIPTION_WORDS: usize = 6;

    let trimmed = cleaned_body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let title_normalized = title.map(|t| collapse_whitespace(t).to_lowercase());

    for raw in trimmed.split(['.', '?', '!', '\n']) {
        let fragment = raw.trim();
        if fragment.is_empty() {
            continue;
        }
        let cleaned = clean_topic_fragment(fragment);
        if cleaned.is_empty() {
            continue;
        }
        if is_topic_noise(&cleaned, fragment) {
            continue;
        }
        let word_count = cleaned.split_whitespace().count();
        if word_count < MIN_BODY_DESCRIPTION_WORDS {
            continue;
        }
        // Skip a sentence that is the same as (or contains/is contained by) the title.
        if let Some(ref t) = title_normalized {
            let cleaned_normalized = cleaned.to_lowercase();
            if t.starts_with(&cleaned_normalized)
                || cleaned_normalized.starts_with(t)
                || fuzzy_contains(t, &cleaned_normalized)
                || fuzzy_contains(&cleaned_normalized, t)
            {
                continue;
            }
        }
        return Some(truncate_at_char_boundary(
            &cleaned,
            MAX_BODY_DESCRIPTION_CHARS,
        ));
    }

    None
}

/// Return true when `needle` appears as a contiguous sequence of words inside
/// `haystack`, after normalising whitespace. Used to avoid appending a body
/// sentence that merely repeats the page title.
fn fuzzy_contains(haystack: &str, needle: &str) -> bool {
    let hay_words: Vec<&str> = haystack.split_whitespace().collect();
    let needle_words: Vec<&str> = needle.split_whitespace().collect();
    if needle_words.is_empty() || hay_words.len() < needle_words.len() {
        return false;
    }
    hay_words
        .windows(needle_words.len())
        .any(|window| window == needle_words.as_slice())
}

/// Strip markdown heading/list markers, split glued-together words, and remove
/// leading site-chrome tokens from a candidate topic fragment.
fn clean_topic_fragment(s: &str) -> String {
    let mut out = s.trim().to_string();
    while out.starts_with('#') {
        out = out.trim_start_matches('#').trim_start().to_string();
    }
    for prefix in ["* ", "- ", "+ "] {
        if let Some(rest) = out.strip_prefix(prefix) {
            out = rest.to_string();
        }
    }
    // Drop trailing markdown reference-link indices like "[12]".
    if let Some(idx) = out.rfind('[') {
        let tail = &out[idx..];
        if tail.ends_with(']') && tail.chars().filter(|c| c.is_ascii_digit()).count() > 0 {
            out.truncate(idx);
            out = out.trim_end().to_string();
        }
    }
    out = split_glued_words(&out);
    out = remove_topic_nav_prefixes(&out);
    collapse_whitespace(&out)
}

/// Characters used to separate article titles from site branding in HTML
/// `<title>` tags and OpenGraph metadata.
const TITLE_SEPARATORS: &[char] = &['|', '-', '—', '–', '/', '>', '»', '·'];

/// Clean a page title so it can be used directly as the research topic.
///
/// Splits the title on common separator characters, evaluates each segment
/// independently, and returns the longest usable segment. This handles both
/// leading site-brand tokens ("InfoQ Homepage Articles ...") and trailing
/// site names ("... | InfoQ"). Each segment has nav words, glued tokens, and
/// short/generic noise removed. Returns `None` when no segment is meaningful.
fn clean_site_title(title: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut best_words = 0;

    for raw_segment in title.split(TITLE_SEPARATORS) {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(cleaned) = clean_site_title_segment(segment) {
            let words = cleaned.split_whitespace().count();
            if words > best_words {
                best_words = words;
                best = Some(cleaned);
            }
        }
    }

    best
}

/// Clean a single title segment: strip leading site nav words, drop a leading
/// site-brand token when it is followed by a nav word, split glued tokens such
/// as "HomepageArticlesLarge", and discard empty or short results.
fn clean_site_title_segment(title: &str) -> Option<String> {
    let mut out = title.trim().to_string();
    if out.is_empty() {
        return None;
    }
    out = collapse_whitespace(&out);
    out = split_glued_words(&out);
    out = remove_topic_nav_prefixes(&out);

    // Drop a leading site-brand token when it is immediately followed by a
    // nav word ("InfoQ Homepage Articles ..." -> "Articles ..." -> etc.).
    let words: Vec<String> = out.split_whitespace().map(str::to_string).collect();
    let mut i = 0;
    while i < words.len() {
        let lower = words[i].to_lowercase();
        if TOPIC_NAV_PREFIXES.contains(&lower.as_str()) {
            i += 1;
            continue;
        }
        if i + 1 < words.len() && words[i].starts_with(|c: char| c.is_uppercase()) {
            let next_lower = words[i + 1].to_lowercase();
            if TOPIC_NAV_PREFIXES.contains(&next_lower.as_str()) {
                i += 1;
                continue;
            }
        }
        break;
    }
    let kept: Vec<&str> = words[i..].iter().map(|s| s.as_str()).collect();
    out = kept.join(" ");

    out = collapse_whitespace(&out);
    if out.is_empty() || is_topic_noise(&out, &out) {
        return None;
    }
    if out.split_whitespace().count() < MIN_TOPIC_WORDS {
        return None;
    }
    Some(truncate_at_char_boundary(&out, MAX_DERIVED_TOPIC_CHARS))
}

/// Tokens that, when they appear at the start of a title or fragment, indicate
/// site navigation chrome rather than article content.
const TOPIC_NAV_PREFIXES: &[&str] = &[
    "home",
    "homepage",
    "articles",
    "about",
    "contact",
    "login",
    "sign in",
    "sign up",
    "menu",
    "search",
    "subscribe",
    "share",
    "sitemap",
    "rss",
    "feed",
    "privacy",
    "terms",
];

/// Remove leading nav/site tokens from a topic candidate, one pass at a time,
/// so strings like "InfoQ Homepage Articles Large Concept Models..." collapse
/// to "Large Concept Models...".
fn remove_topic_nav_prefixes(s: &str) -> String {
    let mut out = s.trim().to_string();
    loop {
        let lower = out.to_lowercase();
        let mut changed = false;
        for prefix in TOPIC_NAV_PREFIXES {
            if lower.starts_with(prefix) {
                let rest = &out[prefix.len()..]
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .trim_start();
                if rest.len() < out.len() {
                    out = rest.to_string();
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    out
}

/// Common page-chrome phrases that should never become a research topic.
const TOPIC_NOISE_KEYWORDS: &[&str] = &[
    "skip to main content",
    "skip to content",
    "skip navigation",
    "cookie",
    "subscribe",
    "newsletter",
    "sign in",
    "sign up",
    "log in",
    "login",
    "loading",
    "share",
    "all rights reserved",
    "footer",
    "update:",
    "table of contents",
    "try chatgpt",
    "jump to content",
    "copyright",
    "©",
    "posted",
    "updated",
    "min read",
    "minutes to read",
    "listen to this article",
    "your browser does not support",
    "audio element",
    "key takeaways",
    "like key takeaways",
];

/// Return true when a fragment is clearly page chrome rather than article prose.
fn is_topic_noise(cleaned: &str, original: &str) -> bool {
    let lower = cleaned.to_lowercase();
    // Whole-phrase chrome strings anywhere in the fragment.
    for kw in TOPIC_NOISE_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }
    // Leading nav words (Home, Articles, Login, ...) are almost always chrome.
    if let Some(first) = lower.split_whitespace().next()
        && TOPIC_NAV_PREFIXES.contains(&first)
    {
        return true;
    }
    // Markdown reference-link lines like "[Skip to main content][1]" or
    // "* [Foundation(opens in a new window)][7]" are nav links, not topics.
    if original.contains("][") || original.contains("](") {
        let stripped = remove_markdown_links(original);
        let remaining_words = stripped.split_whitespace().count();
        if remaining_words < 3 {
            return true;
        }
    }
    false
}

/// Split glued-together words such as "HomepageArticlesLarge" or "AIReasoning"
/// into separate tokens so topic derivation and nav-prefix removal work on the
/// individual words. Uses character-boundary heuristics instead of regex
/// look-around, which the `regex` crate does not support.
///
/// Acronyms that end with a lowercase plural suffix (e.g. "LCMs", "APIs") are
/// kept intact rather than split into "LC Ms".
fn split_glued_words(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + chars.len());
    for (i, c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let after_next = chars.get(i + 2).copied();
            if should_split_topic_words(prev, *c, next, after_next) {
                out.push(' ');
            }
        }
        out.push(*c);
    }
    out
}

/// Decide whether to insert a word boundary between `prev` and `curr`.
fn should_split_topic_words(
    prev: char,
    curr: char,
    next: Option<char>,
    after_next: Option<char>,
) -> bool {
    // "eA..." or "5A..." when the uppercase letter starts a new word.
    if (prev.is_lowercase() || prev.is_ascii_digit())
        && curr.is_uppercase()
        && next.is_some_and(|n| n.is_lowercase())
    {
        return true;
    }
    // "AIReasoning" -> split before the R because it starts a capitalised word.
    // Do not split pluralised acronyms such as "LCMs" or "APIs": the split is
    // only inserted when at least two lowercase letters follow the uppercase
    // letter, i.e. a real word rather than a trailing "s" or a space/punctuation.
    if prev.is_uppercase()
        && curr.is_uppercase()
        && next.is_some_and(|n| n.is_lowercase())
        && after_next.is_some_and(|n| n.is_lowercase())
    {
        return true;
    }
    false
}

/// Collapse runs of whitespace into a single space and trim the result.
fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove markdown reference and inline links, leaving only the surrounding text.
fn remove_markdown_links(s: &str) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?s)\[[^\]]+\](?:\[[^\]]*\]|\([^)]+\))").unwrap()
    });
    RE.replace_all(s, "").into_owned()
}

/// Truncate `s` to at most `max_chars` characters on a UTF-8 char boundary.
fn truncate_at_char_boundary(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take(max_chars)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    s[..end].to_string()
}

impl ResearchSession {
    /// Run a complete research session end-to-end. The flow is:
    ///
    /// 1. Validate name + emit the setup phase.
    /// 2. If `--from-url` is provided, fetch the primary page *before* creating
    ///    the on-disk item; derive the topic from the page body when no explicit
    ///    topic was supplied. A fetch failure here aborts the session and leaves
    ///    no research folder or `RESEARCH.md` behind.
    /// 3. Create the on-disk item (if absent) using the resolved topic.
    /// 4. Mark the item `InProgress` and load the optional template.
    /// 5. Run web-gathering (T-014, T-015).
    /// 6. Run local-gathering (T-016, T-017, T-018).
    /// 7. Cross-reference prior specs (T-018).
    /// 8. Assemble `RESEARCH.md` (T-020, T-021, T-022).
    /// 9. Persist + mark `Complete` (T-012, T-013).
    pub async fn run(
        &self,
        name_str: &str,
        title: &str,
        config: &SessionConfig,
        observer: Arc<dyn SessionObserver>,
    ) -> Result<RunOutcome> {
        let name = ResearchName::try_new(name_str).map_err(ResearchError::InvalidName)?;
        let project_root = project_root_for(self.manager.root()).to_path_buf();

        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Setup,
        });

        // Confirm resolved options up front so callers can verify the expected
        // output format and other flags before any expensive work runs.
        observer.on_event(SessionEvent::ConfigSnapshot {
            output_format: config.output_format.as_str().to_string(),
            depth: config.depth.map(|d| d.as_str().to_string()),
            iterations: config.iterations,
            from_url: config.from_url.clone(),
        });

        // ── --from-url pre-step ──────────────────────────────────────────
        //
        // Fetch the primary page up front and capture it as the first web
        // source. If no explicit topic was provided, derive the topic from the
        // page's *body content* (not its `<title>`): the body is cleaned via the
        // `readability-rs` extractor inside the `webfetch` tool, so nav bars,
        // cookie notices, and other boilerplate are already removed. The title
        // is used as the primary topic signal, and a short descriptive sentence
        // from the body is appended when available so the topic is informative
        // rather than just the page headline. The page title and URL are only
        // used as fallbacks when the body yields no usable text.
        //
        // When the caller supplied no explicit title (or supplied the raw URL
        // because `--from-url` was used without a topic), the cleaned page title
        // replaces the URL in the rendered `RESEARCH.md` header and frontmatter.
        //
        // Crucially, this fetch happens *before* the on-disk item is created so
        // an inaccessible primary URL aborts the session without leaving an empty
        // research folder or skeleton `RESEARCH.md` behind.
        let mut topic = config.topic.clone();
        let mut sources = Vec::new();
        let mut web_queries = Vec::new();
        let mut item_title = title.to_string();
        if let Some(url) = config.from_url.as_deref() {
            let Some(web) = &self.web else {
                return Err(ResearchError::FromUrlFetchFailed {
                    url: url.to_string(),
                    message: "web gathering is disabled; cannot fetch --from-url".to_string(),
                });
            };
            match web.fetch_url_as_source(url).await {
                Ok((src, _page)) => {
                    let src_url = match &src {
                        Source::Web { url, .. } => url.clone(),
                        _ => url.to_string(),
                    };
                    let src_title = match &src {
                        Source::Web { title, .. } => title.clone(),
                        _ => String::new(),
                    };
                    let src_body = match &src {
                        Source::Web { body, .. } => body.clone(),
                        _ => String::new(),
                    };
                    // Emit a short preview of the fetched body so the UI can
                    // show what content was used to derive the topic. Take the
                    // first ~200 characters of the cleaned body, with the
                    // fenced-codeblock markers added by
                    // `fence_captured_body` stripped out.
                    let preview_src: String = src_body
                        .lines()
                        .filter(|l| !l.trim_start().starts_with("```"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let body_preview: String = preview_src.chars().take(200).collect();
                    observer.on_event(SessionEvent::FromUrlBodyPreview {
                        url: src_url.clone(),
                        body_preview,
                    });
                    observer.on_event(SessionEvent::WebCaptured {
                        url: src_url.clone(),
                        title: src_title.clone(),
                    });
                    if topic.trim().is_empty() {
                        match derive_topic_from_url_body(&src_body, &src_title, &src_url) {
                            Some(derived) => {
                                topic = derived;
                                tracing::info!(
                                    url = %src_url,
                                    derived_topic = %topic,
                                    "research: --from-url derived topic from fetched page body"
                                );
                            }
                            None => {
                                let message = format!(
                                    "fetched page body for '{}' contained no usable article text to derive a topic",
                                    src_url
                                );
                                observer.on_event(SessionEvent::WebFetchFailed {
                                    url: src_url.clone(),
                                    error: message.clone(),
                                });
                                return Err(ResearchError::FromUrlNoUsableBody {
                                    url: src_url.clone(),
                                });
                            }
                        }
                    }
                    // Use the cleaned page title as the item title when the
                    // caller only supplied the raw URL (or no title at all).
                    if (item_title.is_empty()
                        || item_title == src_url
                        || item_title.starts_with("http://")
                        || item_title.starts_with("https://"))
                        && let Some(clean_title) = clean_site_title(&src_title)
                    {
                        item_title = clean_title;
                    }
                    sources.push(src);
                    web_queries.push(url.to_string());
                }
                Err(e) => {
                    observer.on_event(SessionEvent::WebFetchFailed {
                        url: url.to_string(),
                        error: e.to_string(),
                    });
                    return Err(ResearchError::FromUrlFetchFailed {
                        url: url.to_string(),
                        message: e.to_string(),
                    });
                }
            }
        }

        // ── Create / load the on-disk item ──────────────────────────────
        let item_exists = ResearchIo::item_exists(self.manager.root(), &name).await;
        let mut item = if item_exists {
            self.manager.show(name_str).await?
        } else {
            self.manager
                .create_with_format(name_str, &item_title, &topic, config.output_format)
                .await?
        };
        mark_in_progress(&mut item);
        self.manager.start_gathering(name_str).await?;
        let template_body = load_template(self.manager.root(), config.template.as_deref()).await;

        // If we didn't have an explicit topic and no from-url was supplied, fall
        // back to whatever topic is stored on the pre-existing item.
        if topic.trim().is_empty() && config.from_url.is_none() {
            topic = item.topic.clone();
        }

        // ── Decide single-pass vs. iterative engine ─────────────────────
        let engine_cfg = config.engine_config();
        let use_iterative = config.iterations.is_some() || config.depth == Some(Depth::Deep);

        if use_iterative && engine_cfg.max_iterations > 1 {
            observer.on_event(SessionEvent::Phase {
                phase: SessionPhase::Web,
            });
            match self
                .run_iterative_pass(&topic, config, observer.clone())
                .await
            {
                Ok((iter_sources, iter_queries, iterations)) => {
                    web_queries.extend(iter_queries);
                    sources.extend(iter_sources);
                    tracing::info!(
                        name = %name,
                        iterations = iterations,
                        sources = sources.len(),
                        "research: iterative pass complete"
                    );
                }
                Err(e) => {
                    observer.on_event(SessionEvent::WebSearchFailed {
                        error: e.to_string(),
                    });
                    tracing::warn!(error = %e, "research: iterative pass failed; continuing with remaining sources");
                }
            }
        } else {
            // ── Web phase ───────────────────────────────────────��───────────
            observer.on_event(SessionEvent::Phase {
                phase: SessionPhase::Web,
            });
            if let Some(web) = &self.web {
                let web_budget = config.max_web_results.max(config.budget_web_results());
                // Apply the per-run fetch-concurrency override. `WebGatherer` is
                // cheap to clone (Arc pair) so we rebuild a copy rather than
                // mutating the shared instance.
                let web = web.clone().with_fetch_concurrency(config.fetch_concurrency);
                let forwarder = GatherEventForwarder {
                    observer: observer.clone(),
                };
                match web
                    .gather_with_observer(&topic, web_budget, Some(&forwarder))
                    .await
                {
                    Ok(result) => {
                        let gathered_queries = result.queries;
                        if !gathered_queries.is_empty() {
                            web_queries.extend(gathered_queries);
                        }
                        sources.extend(result.sources);
                    }
                    Err(e) => {
                        observer.on_event(SessionEvent::WebSearchFailed {
                            error: e.to_string(),
                        });
                        tracing::warn!(error = %e, "research: web phase failed; continuing");
                    }
                }
            }
        }

        // ── Local phase ───────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Local,
        });
        let local_budget = config.max_local_sources.max(config.budget_local_sources());
        let local_gathered = if config.disable_local {
            tracing::info!(
                name = %name,
                "research: local phase skipped (--no-local)"
            );
            Vec::new()
        } else if let Some(local) = &self.local {
            let cfg = LocalGatherConfig {
                max_local_sources: local_budget,
                skip_specs: config.disable_specs,
                ..LocalGatherConfig::default()
            };
            match local
                .gather(&project_root, &topic, config.sources_dir.as_deref(), &cfg)
                .await
            {
                Ok(s) => {
                    for src in &s {
                        if let Source::Local {
                            path, relevance, ..
                        } = src
                        {
                            let score = relevance
                                .split_whitespace()
                                .next()
                                .and_then(|n| n.parse::<usize>().ok())
                                .unwrap_or(1);
                            observer.on_event(SessionEvent::LocalCaptured {
                                path: path.clone(),
                                score,
                            });
                        }
                    }
                    s
                }
                Err(e) => {
                    tracing::warn!(error = %e, "research: local phase failed; continuing");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        sources.extend(local_gathered);

        // ── Spec phase ────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Specs,
        });
        if config.disable_specs {
            tracing::info!(
                name = %name,
                "research: spec phase skipped (--no-specs)"
            );
        }
        let spec_sources: Vec<Source> = if config.disable_specs {
            Vec::new()
        } else {
            sources
                .iter()
                .filter(|s| matches!(s, Source::Spec { .. }))
                .cloned()
                .collect()
        };
        for src in &spec_sources {
            if let Source::Spec { spec_id, .. } = src {
                observer.on_event(SessionEvent::SpecCaptured {
                    spec_id: spec_id.clone(),
                });
            }
        }

        // ── Synthesize ─────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Synthesize,
        });
        // Decide which fallback path we'll take *before* calling the engine
        // so we can attribute the resulting summary correctly in the UI.
        let has_llm_engine = !self.analysis_is_noop();
        let (analysis, synth_outcome, synth_detail) =
            match self.synthesize(&name, &topic, &sources).await {
                Ok((result, outcome)) => {
                    // Map the engine's AnalysisOutcome to the user-facing
                    // SynthesizeOutcome. When no LLM engine is wired in
                    // (NoopAnalysisEngine), the default analyze_with_outcome
                    // returns AnalysisOutcome::Llm, but we override to NoLlm
                    // so the UI is transparent about the provenance.
                    let synth = if !has_llm_engine {
                        SynthesizeOutcome::NoLlm
                    } else {
                        match outcome {
                            AnalysisOutcome::Llm => SynthesizeOutcome::Llm,
                            AnalysisOutcome::FallbackEmpty => SynthesizeOutcome::FallbackEmpty,
                            AnalysisOutcome::FallbackError => SynthesizeOutcome::FallbackError,
                        }
                    };
                    (result, synth, None)
                }
                Err(e) => {
                    // Log at error level (not warn) so it's visible by default
                    // — synthesis failures are the reason RESEARCH.md ends up
                    // looking skeletal, and the user needs to know.
                    tracing::error!(
                        error = %e,
                        "research: synthesis failed; falling back to mechanical summary"
                    );
                    (
                        AnalysisResult::default(),
                        SynthesizeOutcome::FallbackError,
                        Some(e.to_string()),
                    )
                }
            };
        observer.on_event(SessionEvent::SynthesizeResult {
            outcome: synth_outcome,
            detail: synth_detail,
        });
        // ── Assemble ──────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Assemble,
        });
        let mut item_with_sources = ResearchItem::new(name.clone(), &item_title, &topic);
        item_with_sources.set_queries(web_queries.clone());
        // Only set output_format when it is not the default report so the
        // frontmatter stays minimal for the common case.
        if config.output_format != OutputFormat::Report {
            item_with_sources.output_format = Some(config.output_format.as_str().to_string());
        }
        for s in &sources {
            item_with_sources.add_source(s.clone());
        }
        let llm_produced_summary = !analysis.summary.is_empty()
            || !analysis.findings.is_empty()
            || !analysis.cross_references.is_empty()
            || !analysis.open_questions.is_empty();
        let doc = ResearchDocument {
            item: item_with_sources,
            summary: if analysis.summary.is_empty() {
                default_summary(&sources, &topic)
            } else {
                analysis.summary
            },
            findings: if analysis.findings.is_empty() {
                // FR-011 / T-010: the analysis engine guarantees non-empty
                // findings via the mechanical fallback (see
                // `mechanical_fallback_findings`), so this branch is a
                // defense-in-depth safety net rather than the primary path.
                // It only triggers if a custom `AnalysisEngine`
                // implementation returns `Ok` with empty findings AND the
                // `Llm` outcome (the built-in `LlmAnalysisEngine` never
                // does). `default_findings` keeps RESEARCH.md usable.
                default_findings(&sources, &topic)
            } else {
                analysis.findings
            },
            cross_references: if analysis.cross_references.is_empty() {
                cross_references_from(&sources)
            } else {
                analysis.cross_references
            },
            open_questions: if analysis.open_questions.is_empty() {
                if llm_produced_summary {
                    Vec::new()
                } else {
                    // Surface suggested open questions from the mechanical
                    // fallback so the section is never empty when no LLM
                    // analysis was available.
                    default_open_questions(&sources, &topic)
                }
            } else {
                analysis.open_questions
            },
            template_body,
            decomposed_queries: web_queries.clone(),
            output_format: config.output_format,
        };
        let assembled = self.manager.write_document(&doc).await?;
        // ── Finalize ──────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Finalize,
        });
        self.manager.complete_gathering(name_str).await?;

        let total_sources = sources.len();
        observer.on_event(SessionEvent::Done { total_sources });

        info!(
            name = %name,
            total = total_sources,
            "research: session complete"
        );

        Ok(RunOutcome {
            research_name: name.to_string(),
            sources,
            document: assembled,
            web_queries,
        })
    }
}

impl ResearchSession {
    /// Run the iterative research engine for multi-iteration passes.
    ///
    /// Returns the gathered sources, the sub-questions/queries that drove the
    /// engine, and the number of iterations completed.
    async fn run_iterative_pass(
        &self,
        topic: &str,
        config: &SessionConfig,
        observer: Arc<dyn SessionObserver>,
    ) -> Result<(Vec<Source>, Vec<String>, u32)> {
        let planner = self
            .planner
            .clone()
            .unwrap_or_else(|| Arc::new(HeuristicPlanner::new()));
        let critic = self
            .critic
            .clone()
            .unwrap_or_else(|| Arc::new(SimpleCritic));
        let engine = IterativeEngine::new(
            planner,
            self.web.clone(),
            self.analysis.clone(),
            critic,
            config.engine_config(),
        );
        let state = engine
            .run(topic, observer)
            .await
            .map_err(|e| ResearchError::EngineRunFailed(e.to_string()))?;
        let queries: Vec<String> = state
            .plan
            .sub_questions
            .iter()
            .map(|s| s.question.clone())
            .collect();
        Ok((state.sources, queries, state.iteration_count))
    }
}

impl ResearchSession {
    /// Read captured source bodies from disk and run the analysis engine,
    /// returning the [`AnalysisResult`] paired with an [`AnalysisOutcome`]
    /// so the caller can surface `SynthesizeOutcome::FallbackEmpty` when
    /// the LLM produced malformed output (FR-005 / T-005).
    async fn synthesize(
        &self,
        name: &ResearchName,
        topic: &str,
        sources: &[Source],
    ) -> anyhow::Result<(AnalysisResult, AnalysisOutcome)> {
        // Prefer the inline `body` field on each source — it's the captured
        // text from the gatherer and is always populated for fresh sessions.
        // Fall back to reading the on-disk supporting file for items loaded
        // from disk that predate the body field.
        let research_root = self.manager.root().to_path_buf();
        let name = name.clone();
        let sources = sources.to_vec();
        let bodies = tokio::task::spawn_blocking(move || {
            build_source_bodies(&sources, |src| -> Option<String> {
                if let Some(inline) = src.body()
                    && !inline.is_empty()
                {
                    return Some(inline.to_string());
                }
                match src {
                    Source::Web { body_path, .. }
                    | Source::Local { body_path, .. }
                    | Source::Other { body_path, .. } => {
                        let path = ResearchIo::item_dir(&research_root, &name).join(body_path);
                        match std::fs::read_to_string(&path) {
                            Ok(body) => Some(body),
                            Err(e) => {
                                tracing::warn!(
                                    path = %path.display(),
                                    error = %e,
                                    "research: could not read supporting file for synthesis"
                                );
                                None
                            }
                        }
                    }
                    Source::Spec { relevance, .. } => Some(relevance.clone()),
                }
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("synthesis body loading failed: {e}"))?;
        self.analysis.analyze_with_outcome(topic, &bodies).await
    }
}

/// What [`ResearchSession::run`] returns to the caller.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// The validated research name.
    pub research_name: String,
    /// Every captured source (web + local + spec).
    pub sources: Vec<Source>,
    /// The fully assembled document that was written to disk.
    pub document: crate::document::AssembledDocument,
    /// Sub-queries used by the web-gathering phase. Empty when web gathering
    /// was disabled or no decomposer was configured.
    pub web_queries: Vec<String>,
}

// ── Free helpers ─────────────────────────────────────────────────────────

/// Compute the project root from the `research/` root (its parent).
fn project_root_for(research_root: &Path) -> &Path {
    research_root.parent().unwrap_or(research_root)
}

/// Load a FR-020 template body from `_templates/<name>.md` if it exists.
/// Returns `None` when no template was requested, or when the file does
/// not exist.
async fn load_template(research_root: &Path, template: Option<&str>) -> Option<String> {
    let name = template?;
    let path = ResearchIo::template_path(research_root, name);
    match tokio::fs::read_to_string(&path).await {
        Ok(body) => Some(body),
        Err(e) => {
            tracing::warn!(
                template = %name,
                path = %path.display(),
                error = %e,
                "research: template not loaded"
            );
            None
        }
    }
}

fn default_summary(sources: &[Source], topic: &str) -> String {
    let web = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .collect::<Vec<_>>();
    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .collect::<Vec<_>>();
    let specs = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .collect::<Vec<_>>();
    let total = sources.len();

    if sources.is_empty() {
        return format!(
            "No sources were captured for '{topic}'. Re-run with a more specific topic or after enabling the relevant tools."
        );
    }

    let mut out = format!(
        "Gathered {total} source(s) for '{topic}' ({w} web, {l} local, {s} spec).",
        w = web.len(),
        l = local.len(),
        s = specs.len(),
        topic = topic,
        total = total,
    );

    // Web: name the top 3 by title so the reader knows what was actually pulled in.
    if !web.is_empty() {
        out.push_str("\n\n**Web sources:** ");
        let titles: Vec<String> = web
            .iter()
            .filter_map(|s| match s {
                Source::Web { title, url, .. } if !title.is_empty() => Some(title.clone()),
                Source::Web { url, .. } => Some(url.clone()),
                _ => None,
            })
            .take(3)
            .collect();
        out.push_str(&titles.join("; "));
        if web.len() > 3 {
            out.push_str(&format!(" (and {} more)", web.len() - 3));
        }
        out.push('.');
    }

    // Local: name the top 3 paths so the reader knows which files were pulled in.
    if !local.is_empty() {
        out.push_str("\n\n**Local files:** ");
        let paths: Vec<String> = local
            .iter()
            .filter_map(|s| match s {
                Source::Local { path, .. } => Some(path.clone()),
                _ => None,
            })
            .take(3)
            .collect();
        out.push_str(&paths.join("; "));
        if local.len() > 3 {
            out.push_str(&format!(" (and {} more)", local.len() - 3));
        }
        out.push('.');
    }

    // Specs: name each spec so the reader sees which prior specs informed this research.
    if !specs.is_empty() {
        out.push_str("\n\n**Prior specs cross-referenced:** ");
        let ids: Vec<String> = specs
            .iter()
            .filter_map(|s| match s {
                Source::Spec { spec_id, .. } => Some(spec_id.clone()),
                _ => None,
            })
            .collect();
        out.push_str(&ids.join(", "));
        out.push('.');
    }

    out.push_str(
        "\n\n_No LLM analysis was applied to these sources — the section above is a mechanical digest. Re-run with a configured model for a synthesized analysis._",
    );
    out
}

fn default_findings(sources: &[Source], topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let web: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .collect();
    let local: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .collect();
    let specs: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .collect();

    // Per-web-source finding. The reader gets the title and a 240-char
    // excerpt so the finding stands on its own without opening the
    // supporting file.
    for (idx, src) in web.iter().enumerate() {
        if let Source::Web {
            published_at: None,
            title,
            url,
            body,
            ..
        } = src
        {
            let label = if title.is_empty() {
                url.as_str()
            } else {
                title.as_str()
            };
            let excerpt = body_excerpt(body, 240);
            let observation = if excerpt.is_empty() {
                format!(
                    "The web source **{label}** from <{url}> was captured, but no body text was returned by the fetch. [#{n}]",
                    n = idx + 1,
                )
            } else {
                format!(
                    "The web source **{label}** from <{url}> states: \"{excerpt}\" [#{n}]",
                    n = idx + 1,
                )
            };
            let previous = if idx > 0 {
                format!(
                    "This finding follows and reinforces the web-source thread begun in Finding {}.",
                    idx
                )
            } else {
                "No direct dependencies.".to_string()
            };
            let headline = crate::document::make_headline_from_observation(&observation);
            let finding = format!(
                "**Headline:** {headline}\n\n**Observation:** {observation}\n\n**Analysis:** This evidence relates directly to the topic '{topic}', providing public context that can be compared against project-local material.\n\n**Cross-reference / Dependencies:** {previous}\n\n**Implication:** The source should be treated as background unless it is corroborated by an in-project reference or a later finding; if no corroboration exists, flag it as an open question.",
                headline = headline,
                observation = observation,
                topic = topic,
                previous = previous,
            );
            out.push(finding);
        }
    }

    // Per-local-source findings.
    let local_offset = web.len();
    for (idx, src) in local.iter().enumerate() {
        if let Source::Local {
            path,
            relevance,
            body,
            ..
        } = src
        {
            let excerpt = body_excerpt(body, 240);
            let observation = if excerpt.is_empty() {
                format!(
                    "The in-project file `{path}` was matched as relevant (`{relevance}`), but no excerpt was captured. [#{n}]",
                    n = local_offset + idx + 1,
                )
            } else {
                format!(
                    "The in-project file `{path}` (relevance: `{relevance}`) contains the following excerpt: \"{excerpt}\" [#{n}]",
                    n = local_offset + idx + 1,
                )
            };
            let sibling_idx = if idx > 0 {
                Some(local_offset + idx)
            } else {
                None
            };
            let web_idx = if !web.is_empty() { Some(1usize) } else { None };
            let dependencies = match (sibling_idx, web_idx) {
                (Some(s), Some(_)) => format!(
                    "This finding is related to Finding {sibling} (the previous local match) and builds on Finding 1 (the first web source) by grounding public information in project code.",
                    sibling = s,
                ),
                (Some(s), None) => format!(
                    "This finding depends on Finding {sibling}, which established the first local match in this sequence.",
                    sibling = s,
                ),
                                  (None, Some(_)) => "This finding is the first local match; it can be cross-checked against Finding 1 (the first web source).".to_string(),                (None, None) => "No direct dependencies.".to_string(),
            };
            let headline = crate::document::make_headline_from_observation(&observation);
            let finding = format!(
                "**Headline:** {headline}\n\n**Observation:** {observation}\n\n**Analysis:** This in-project evidence shows how '{topic}' touches the current codebase and is the strongest signal of immediate relevance.\n\n**Cross-reference / Dependencies:** {dependencies}\n\n**Implication:** The referenced path is a concrete place to start implementation or further investigation; consider opening it as a cross-reference and verifying the excerpt against the latest source.",
                headline = headline,
                observation = observation,
                topic = topic,
                dependencies = dependencies,
            );
            out.push(finding);
        }
    }

    // Per-spec findings.
    let spec_offset = web.len() + local.len();
    for (idx, src) in specs.iter().enumerate() {
        if let Source::Spec {
            spec_id, relevance, ..
        } = src
        {
            let note = if relevance.is_empty() {
                format!("see specs/{spec_id}/SPEC.md")
            } else {
                relevance.clone()
            };
            let first_local = if local_offset > 0 {
                Some(local_offset + 1)
            } else {
                None
            };
            let first_web = if !web.is_empty() { Some(1usize) } else { None };
            let dependencies = match (first_local, first_web) {
                (Some(l), Some(_)) => format!(
                    "This finding connects the prior specification to the in-project evidence in Finding {l} and the web background in Finding 1; treat it as the bridge between design intent and current code.",
                    l = l,
                ),
                (Some(l), None) => format!(
                    "This finding depends on Finding {l}, which identified the in-project material that implements (or should implement) this spec.",
                    l = l,
                ),
                (None, Some(_)) => "This finding is related to Finding 1 (web background) but no local implementation has been matched yet.".to_string(),
                (None, None) => "No direct dependencies.".to_string(),
            };
            let finding = format!(
                "**Headline:** Prior spec `{spec_id}` is relevant to '{topic}' ({note})\n\n**Observation:** Prior spec `{spec_id}` is relevant to '{topic}' ({note}) [#{n}].\n\n**Analysis:** This specification establishes requirements or decisions that pre-date the current research, and should constrain or guide any conclusions drawn from newer sources.\n\n**Cross-reference / Dependencies:** {dependencies}\n\n**Implication:** Before acting on later findings, verify that the project still honours this spec; conflicts between this spec and newer evidence should be escalated as an open question.",
                n = spec_offset + idx + 1,
                spec_id = spec_id,
                topic = topic,
                note = note,
                dependencies = dependencies,
            );
            out.push(finding);
        }
    }

    if sources.is_empty() {
        out.push(format!(
            "**Headline:** No sources captured\n\n**Observation:** No sources were captured for '{topic}'.\n\n**Analysis:** Without captured web pages, local files, or prior specs, the research cannot yet support a substantive conclusion.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** Consider re-running with a more specific topic, or run inside a project with relevant files and specs so gathering has something to work with."
        ));
    }
    out
}

/// Build a per-source bullet title + short excerpt suitable for embedding
/// in the Findings section when no LLM analysis is available. Returns an
/// empty string when the body is empty / unavailable.
fn body_excerpt(body: &str, max_chars: usize) -> String {
    // Strip the "Excerpt — N keyword match(es)" header that the local
    // gatherer prepends so we don't double-print it in the Findings section.
    let stripped = body
        .strip_prefix("Excerpt —")
        .map(|rest| rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '\n'))
        .unwrap_or(body);
    // Collapse whitespace so the excerpt fits on one logical line.
    let collapsed: String = stripped
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let collapsed = collapsed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let mut out: String = collapsed.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

fn default_open_questions(sources: &[Source], topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let web = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .count();
    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    let spec = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    if sources.is_empty() {
        out.push(format!(
            "Why was nothing captured for '{topic}' — was a tool unavailable, the topic too narrow, or the search query off?"
        ));
    } else {
        if web == 0 {
            out.push("No web sources were captured — was `websearch` unavailable, or does the topic lack good public references?".into());
        }
        if local == 0 {
            out.push(
                "No in-project files matched — is there a code path or doc the topic should touch that grep did not surface?"
                    .into(),
            );
        }
        if spec == 0 {
            out.push(
                "No prior specs were cross-referenced — is the topic genuinely new, or were existing specs filtered out by the keyword match?"
                    .into(),
            );
        }
        out.push(format!(
            "Re-run `/research {topic}` with a configured LLM to produce an LLM-synthesized analysis instead of this mechanical digest."
        ));
    }
    out
}

fn cross_references_from(sources: &[Source]) -> Vec<crate::document::CrossReference> {
    sources
        .iter()
        .filter_map(|s| match s {
            Source::Local {
                path,
                relevance,
                kind,
                ..
            } => Some(crate::document::CrossReference {
                path: path.clone(),
                relevance: format_with_kind(relevance, *kind),
            }),
            _ => None,
        })
        .collect()
}

fn format_with_kind(relevance: &str, kind: LocalSourceKind) -> String {
    match kind {
        LocalSourceKind::InProject => relevance.to_string(),
        LocalSourceKind::Extra => format!("{relevance} (from --sources-dir)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_gatherer::{GrepMatch, LocalTool};
    use crate::web_gatherer::{
        HeuristicQueryDecomposer, WebFetchTool, WebFetchedPage, WebSearchHit, WebSearchTool,
    };
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

    struct FakeSearch {
        hits: Vec<WebSearchHit>,
    }
    #[async_trait]
    impl WebSearchTool for FakeSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(self.hits.clone())
        }
    }
    struct FakeFetch {
        pages: HashMap<String, WebFetchedPage>,
    }
    #[async_trait]
    impl WebFetchTool for FakeFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no page"))
        }
    }

    struct FakeLocal {
        files: HashMap<PathBuf, String>,
    }
    #[async_trait]
    impl LocalTool for FakeLocal {
        async fn glob(&self, _root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
            let ext = pattern.rsplit('.').next().unwrap_or("");
            Ok(self
                .files
                .keys()
                .filter(|p| p.extension().map(|e| e == ext).unwrap_or(false))
                .cloned()
                .collect())
        }
        async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
            let body = self.files.get(path).cloned().unwrap_or_default();
            let mut out = Vec::new();
            for (i, line) in body.lines().enumerate() {
                let l = line.to_lowercase();
                if terms.iter().any(|t| l.contains(t)) {
                    out.push(GrepMatch {
                        line: i + 1,
                        text: line.to_string(),
                    });
                }
            }
            Ok(out)
        }
        async fn read(&self, path: &Path) -> anyhow::Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing"))
        }
        async fn list_specs(&self, _root: &Path) -> anyhow::Result<Vec<String>> {
            Ok(Vec::new())
        }
        async fn spec_title(&self, _root: &Path, _id: &str) -> anyhow::Result<String> {
            Ok(String::new())
        }
    }

    #[derive(Default)]
    struct CollectObserver {
        events: Mutex<Vec<SessionEvent>>,
    }
    impl SessionObserver for CollectObserver {
        fn on_event(&self, event: SessionEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[tokio::test]
    async fn session_runs_end_to_end_and_writes_document() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();
        // Seed a single in-project file that contains a topic word.
        let f = tmp.path().join("notes.md");
        tokio::fs::write(&f, "Rust async programming is great.")
            .await
            .unwrap();

        let manager = ResearchManager::new(&research_root);
        let web = WebGatherer::new(
            Arc::new(FakeSearch {
                hits: vec![WebSearchHit {
                    url: "https://example.com".into(),
                    title: "Example".into(),
                    snippet: "snippet".into(),
                    matched_query: "".into(),
                }],
            }),
            Arc::new(FakeFetch {
                pages: HashMap::from([(
                    "https://example.com".into(),
                    WebFetchedPage {
                        published_at: None,
                        url: "https://example.com".into(),
                        title: "Example".into(),
                        body: "body".into(),
                    },
                )]),
            }),
        );
        let local_tool = Arc::new(FakeLocal {
            files: HashMap::from([(f.clone(), "Rust async programming is great.".into())]),
        });
        let local = LocalGatherer::new(local_tool);

        let session = ResearchSession::new(
            manager,
            Some(web),
            Some(local),
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: "Rust async".into(),
            ..SessionConfig::default()
        };
        let observer = Arc::new(CollectObserver::default());
        let outcome = session
            .run("rust-async", "Rust Async", &cfg, observer.clone())
            .await
            .unwrap();
        assert_eq!(outcome.research_name, "rust-async");
        assert_eq!(outcome.web_queries, vec!["Rust async".to_string()]);
        assert!(!outcome.sources.is_empty());
        // Document should exist on disk.
        let p = research_root.join("rust-async/RESEARCH.md");
        assert!(p.is_file());
        let body = tokio::fs::read_to_string(&p).await.unwrap();
        assert!(body.contains("Rust Async"));
        // INDEX.md should exist.
        assert!(research_root.join("INDEX.md").is_file());
        // Observer should have received at least a Phase(Setup), Phase(Web), etc.
        let events = observer.events.lock().unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Web
            }
        )));
    }

    #[tokio::test]
    async fn session_forwards_web_search_errors_to_observer() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();

        struct AlwaysFailSearch;
        #[async_trait]
        impl crate::web_gatherer::WebSearchTool for AlwaysFailSearch {
            async fn search(
                &self,
                _: &str,
                _: usize,
            ) -> anyhow::Result<Vec<crate::web_gatherer::WebSearchHit>> {
                anyhow::bail!("api key missing")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl crate::web_gatherer::WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<crate::web_gatherer::WebFetchedPage> {
                Ok(crate::web_gatherer::WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: "b".into(),
                })
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web =
            crate::web_gatherer::WebGatherer::new(Arc::new(AlwaysFailSearch), Arc::new(OkFetch));
        let session = ResearchSession::new(
            manager,
            Some(web),
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: "topic".into(),
            ..SessionConfig::default()
        };
        let observer = Arc::new(CollectObserver::default());
        let outcome = session
            .run("err", "Error", &cfg, observer.clone())
            .await
            .unwrap();
        assert_eq!(outcome.sources.len(), 0);
        let events = observer.events.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                SessionEvent::WebSearchFailed { error } if error.contains("api key missing")
            )),
            "expected WebSearchFailed event, got {:?}",
            *events
        );
    }

    #[tokio::test]
    async fn session_handles_missing_web_gatherer() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();
        let manager = ResearchManager::new(&research_root);
        let session = ResearchSession::new(
            manager,
            None,
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: "topic".into(),
            ..SessionConfig::default()
        };
        let outcome = session
            .run("rust-async", "Rust Async", &cfg, Arc::new(NoopObserver))
            .await
            .unwrap();
        assert_eq!(outcome.sources.len(), 0);
        assert!(outcome.web_queries.is_empty(), "no web gatherer configured");
    }
    #[tokio::test]
    async fn session_persists_decomposed_queries_in_research_md() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();

        struct RecordingSearch;
        #[async_trait]
        impl WebSearchTool for RecordingSearch {
            async fn search(
                &self,
                _query: &str,
                _max_results: usize,
            ) -> anyhow::Result<Vec<WebSearchHit>> {
                Ok(vec![WebSearchHit {
                    url: "https://example.com".into(),
                    title: "Example".into(),
                    snippet: "".into(),
                    matched_query: "".into(),
                }])
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Example".into(),
                    body: "body".into(),
                })
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web = WebGatherer::new(Arc::new(RecordingSearch), Arc::new(OkFetch))
            .with_decomposer(Arc::new(HeuristicQueryDecomposer));
        let session = ResearchSession::new(
            manager,
            Some(web),
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: "Rust async and Tokio runtime".into(),
            max_web_results: 5,
            ..SessionConfig::default()
        };
        let outcome = session
            .run("decomp-test", "Decomp Test", &cfg, Arc::new(NoopObserver))
            .await
            .unwrap();

        assert_eq!(
            outcome.web_queries,
            vec![
                "Rust async",
                "Tokio runtime",
                "Rust async and Tokio runtime"
            ]
        );

        let body = tokio::fs::read_to_string(research_root.join("decomp-test/RESEARCH.md"))
            .await
            .unwrap();
        assert!(body.contains("## Search Queries"));
        assert!(body.contains("- Rust async"));
        assert!(body.contains("- Tokio runtime"));
        assert!(body.contains("queries:"));
    }

    #[tokio::test]
    async fn session_rejects_invalid_name() {
        let tmp = TempDir::new().unwrap();
        let manager = ResearchManager::new(tmp.path());
        let session = ResearchSession::new(
            manager,
            None,
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig::default();
        let err = session
            .run("AB", "t", &cfg, Arc::new(NoopObserver))
            .await
            .unwrap_err();
        assert!(matches!(err, ResearchError::InvalidName(_)));
    }

    #[tokio::test]
    async fn from_url_fetches_page_and_derives_topic_when_topic_is_empty() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();

        struct NoSearch;
        #[async_trait]
        impl WebSearchTool for NoSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                // The web-search phase still runs; returning no hits is fine
                // — we only need to prove the --from-url source was captured.
                Ok(Vec::new())
            }
        }
        struct PageFetch;
        #[async_trait]
        impl WebFetchTool for PageFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Rust Async Programming Guide".into(),
                    body: "Long-form article about Rust async/await idioms. \
                           Tokio is the most popular runtime and provides a \
                           multi-threaded scheduler for async tasks."
                        .into(),
                })
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
        let session = ResearchSession::new(
            manager,
            Some(web),
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: String::new(),
            from_url: Some("https://example.com/guide".into()),
            ..SessionConfig::default()
        };
        let observer = Arc::new(CollectObserver::default());
        let outcome = session
            .run("from-url-test", "From URL", &cfg, observer.clone())
            .await
            .unwrap();

        // The fetched URL must be captured as the primary web source.
        let web_sources: Vec<&Source> = outcome
            .sources
            .iter()
            .filter(|s| matches!(s, Source::Web { .. }))
            .collect();
        assert!(
            web_sources.iter().any(|s| matches!(
                s,
                Source::Web { url, title, body, .. }
                if url == "https://example.com/guide"
                    && title == "Rust Async Programming Guide"
                    && body.contains("Long-form article")
            )),
            "expected the --from-url page as a web source, got {:?}",
            outcome.sources
        );

        // The URL must appear in the decomposed-queries list.
        assert!(
            outcome
                .web_queries
                .iter()
                .any(|q| q == "https://example.com/guide"),
            "expected the --from-url URL in web_queries, got {:?}",
            outcome.web_queries
        );

        // The research document should reference the topic derived from
        // the fetched page body (not the page title). The body's first
        // substantive sentence is "Long-form article about Rust async/await
        // idioms.", which must appear in RESEARCH.md.
        let body = tokio::fs::read_to_string(research_root.join("from-url-test/RESEARCH.md"))
            .await
            .unwrap();
        assert!(
            body.contains("Long-form article about Rust async/await idioms"),
            "RESEARCH.md should reference the topic derived from the fetched page body, not the title: {body}"
        );

        // The WebCaptured event for the --from-url source must have fired.
        let events = observer.events.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                SessionEvent::WebCaptured { url, title }
                    if url == "https://example.com/guide"
                        && title == "Rust Async Programming Guide"
            )),
            "expected WebCaptured for --from-url, got {:?}",
            *events
        );
    }

    #[tokio::test]
    async fn from_url_derives_topic_from_body_not_title_when_body_has_boilerplate() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();

        struct NoSearch;
        #[async_trait]
        impl WebSearchTool for NoSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                Ok(Vec::new())
            }
        }
        struct PageFetch;
        #[async_trait]
        impl WebFetchTool for PageFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                // The page title is a generic site name, and the body is
                // dominated by nav/cookie/share boilerplate with the real
                // article content in the middle. The derived topic must
                // come from the article content, not the title.
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Example Site".into(),
                    body: "Home About Contact Login\n\n\
                           Accept all cookies We use cookies on this site.\n\n\
                           The Rust async model maps asynchronous operations \
                           onto lightweight futures that a runtime polls to \
                           completion. This article walks through how Tokio \
                           schedules those futures onto worker threads.\n\n\
                           Read more Subscribe Newsletter\n\n\
                           © 2024 Example Corp. All rights reserved."
                        .into(),
                })
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
        let session = ResearchSession::new(
            manager,
            Some(web),
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: String::new(),
            from_url: Some("https://example.com/article".into()),
            ..SessionConfig::default()
        };
        let outcome = session
            .run(
                "body-topic-test",
                "Body Topic",
                &cfg,
                Arc::new(NoopObserver),
            )
            .await
            .unwrap();

        // The topic must be derived from the article sentence, not the
        // "Example Site" title or the nav/cookie boilerplate.
        let body = tokio::fs::read_to_string(research_root.join("body-topic-test/RESEARCH.md"))
            .await
            .unwrap();
        assert!(
            body.contains("Rust async model maps asynchronous operations"),
            "RESEARCH.md should reference the topic derived from the cleaned page body: {body}"
        );
        // The title-derived topic ("Example Site") must NOT have been used
        // as the research topic. The References Index still legitimately
        // cites the source by its page title, so we only check the topic
        // line in the frontmatter / summary, not the whole document.
        let topic_line = body
            .lines()
            .find(|l| l.starts_with("topic:"))
            .or_else(|| body.lines().find(|l| l.starts_with("# ")))
            .unwrap_or("");
        assert!(
            !topic_line.contains("Example Site"),
            "research topic should not be the generic page title: {topic_line}"
        );

        // Sanity: the source was still captured.
        assert!(
            outcome.sources.iter().any(
                |s| matches!(s, Source::Web { url, .. } if url == "https://example.com/article")
            ),
            "the --from-url page should be captured as a source"
        );
    }

    #[tokio::test]
    async fn from_url_falls_back_to_title_when_body_is_pure_boilerplate() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();

        struct NoSearch;
        #[async_trait]
        impl WebSearchTool for NoSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                Ok(Vec::new())
            }
        }
        struct PageFetch;
        #[async_trait]
        impl WebFetchTool for PageFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Meaningful Page Title".into(),
                    body: "Home About Contact\n\nLogin Sign up\n\n© 2024 Example Corp.".into(),
                })
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
        let session = ResearchSession::new(
            manager,
            Some(web),
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: String::new(),
            from_url: Some("https://example.com/boilerplate".into()),
            ..SessionConfig::default()
        };
        let outcome = session
            .run("fallback-test", "Fallback", &cfg, Arc::new(NoopObserver))
            .await
            .unwrap();
        let body = tokio::fs::read_to_string(research_root.join("fallback-test/RESEARCH.md"))
            .await
            .unwrap();
        assert!(
            body.contains("Meaningful Page Title"),
            "RESEARCH.md should fall back to the page title when the cleaned body is empty: {body}"
        );
        let _ = outcome;
    }

    #[tokio::test]
    async fn from_url_keeps_explicit_topic_when_both_are_supplied() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();

        struct NoSearch;
        #[async_trait]
        impl WebSearchTool for NoSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                Ok(Vec::new())
            }
        }
        struct PageFetch;
        #[async_trait]
        impl WebFetchTool for PageFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Fetched Page Title".into(),
                    body: "body text".into(),
                })
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
        let session = ResearchSession::new(
            manager,
            Some(web),
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: "Custom Topic".into(),
            from_url: Some("https://example.com/page".into()),
            ..SessionConfig::default()
        };
        let outcome = session
            .run("both-test", "Both", &cfg, Arc::new(NoopObserver))
            .await
            .unwrap();

        // The explicit topic must win — the derived-topic branch only fires
        // when topic is empty.
        assert!(
            outcome
                .sources
                .iter()
                .any(|s| matches!(s, Source::Web { url, .. } if url == "https://example.com/page")),
            "the --from-url page should still be captured as a source"
        );
        let body = tokio::fs::read_to_string(research_root.join("both-test/RESEARCH.md"))
            .await
            .unwrap();
        assert!(
            body.contains("Custom Topic"),
            "explicit topic should be used, not the fetched page title: {body}"
        );
    }

    #[tokio::test]
    async fn from_url_records_web_fetch_failed_when_fetch_errors() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();

        struct NoSearch;
        #[async_trait]
        impl WebSearchTool for NoSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                Ok(Vec::new())
            }
        }
        struct FailFetch;
        #[async_trait]
        impl WebFetchTool for FailFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                anyhow::bail!("network down")
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(FailFetch));
        let session = ResearchSession::new(
            manager,
            Some(web),
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let cfg = SessionConfig {
            topic: String::new(),
            from_url: Some("https://example.com/x".into()),
            ..SessionConfig::default()
        };
        let observer = Arc::new(CollectObserver::default());
        let err = session
            .run("fail-test", "Fail", &cfg, observer.clone())
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                ResearchError::FromUrlFetchFailed { ref url, ref message }
                    if url == "https://example.com/x" && message.contains("network down")
            ),
            "expected FromUrlFetchFailed, got {err:?}"
        );
        // A WebFetchFailed progress event is also surfaced to the observer.
        {
            let events = observer.events.lock().unwrap();
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    SessionEvent::WebFetchFailed { url, error }
                        if url == "https://example.com/x" && error.contains("network down")
                )),
                "expected WebFetchFailed for --from-url, got {:?}",
                *events
            );
        }
        // No on-disk item is created when the primary URL fails.
        assert!(
            !ResearchIo::item_exists(
                research_root.as_path(),
                &ResearchName::try_new("fail-test").unwrap()
            )
            .await,
            "research folder should not be created when --from-url fails"
        );
    }

    #[tokio::test]
    async fn session_skips_local_phase_when_disable_local_is_true() {
        use crate::local_gatherer::{LocalGatherer, LocalTool};
        use std::path::PathBuf;
        use std::sync::Arc;

        /// Minimal `LocalTool` that would otherwise emit one local source.
        #[derive(Default)]
        struct SingleLocalTool;
        #[async_trait::async_trait]
        // NOTE: intentional duplication — see DUPPLAN.md Milestone J.
        // Trait impls for different mock types; cannot be deduplicated.
        impl LocalTool for SingleLocalTool {
            async fn glob(&self, _root: &Path, _pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
                Ok(Vec::new())
            }
            async fn grep(
                &self,
                _path: &Path,
                _terms: &[String],
            ) -> anyhow::Result<Vec<crate::local_gatherer::GrepMatch>> {
                Ok(Vec::new())
            }
            async fn read(&self, _path: &Path) -> anyhow::Result<String> {
                Ok(String::new())
            }
            async fn list_specs(&self, _root: &Path) -> anyhow::Result<Vec<String>> {
                Ok(Vec::new())
            }
            async fn spec_title(&self, _root: &Path, _spec_id: &str) -> anyhow::Result<String> {
                Ok(String::new())
            }
        }

        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();
        let manager = ResearchManager::new(&research_root);
        let local = LocalGatherer::new(Arc::new(SingleLocalTool));
        let session = ResearchSession::new(
            manager,
            None,
            Some(local),
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let observer = Arc::new(CollectObserver::default());
        let cfg = SessionConfig {
            topic: "anything".into(),
            disable_local: true,
            ..SessionConfig::default()
        };
        let outcome = session
            .run("rust-async", "Rust Async", &cfg, observer.clone())
            .await
            .unwrap();
        let local_count = outcome
            .sources
            .iter()
            .filter(|s| matches!(s, Source::Local { .. }))
            .count();
        assert_eq!(local_count, 0, "--no-local must produce zero local sources");
        let spec_count = outcome
            .sources
            .iter()
            .filter(|s| matches!(s, Source::Spec { .. }))
            .count();
        assert_eq!(
            spec_count, 0,
            "spec sources must not appear when --no-local is set"
        );
        // The Local phase event should still have been emitted so the
        // progress log makes the skip observable.
        let events = observer.events.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                SessionEvent::Phase {
                    phase: SessionPhase::Local
                }
            )),
            "Local phase event should fire even when skipped"
        );
    }

    #[tokio::test]
    async fn session_skips_spec_phase_when_disable_specs_is_true() {
        use crate::local_gatherer::{LocalGatherer, LocalTool};
        use std::path::PathBuf;
        use std::sync::Arc;

        /// LocalTool that emits one `Source::Spec` via list_specs/spec_title
        /// but no regular local files. This is the only path through which
        /// spec sources enter the session, so it exercises the disable_specs
        /// gate at the gatherer boundary.
        #[derive(Default)]
        struct SpecOnlyTool;
        #[async_trait::async_trait]
        impl LocalTool for SpecOnlyTool {
            async fn glob(&self, _root: &Path, _pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
                Ok(Vec::new())
            }
            async fn grep(
                &self,
                _path: &Path,
                _terms: &[String],
            ) -> anyhow::Result<Vec<crate::local_gatherer::GrepMatch>> {
                Ok(Vec::new())
            }
            async fn read(&self, _path: &Path) -> anyhow::Result<String> {
                Ok(String::new())
            }
            async fn list_specs(&self, _root: &Path) -> anyhow::Result<Vec<String>> {
                Ok(vec!["some-spec".into()])
            }
            async fn spec_title(&self, _root: &Path, _spec_id: &str) -> anyhow::Result<String> {
                Ok("Some spec title".into())
            }
        }

        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();
        let manager = ResearchManager::new(&research_root);
        let local = LocalGatherer::new(Arc::new(SpecOnlyTool));
        let session = ResearchSession::new(
            manager,
            None,
            Some(local),
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
        let observer = Arc::new(CollectObserver::default());
        let cfg = SessionConfig {
            topic: "topic".into(),
            disable_specs: true,
            ..SessionConfig::default()
        };
        let outcome = session
            .run("rust-async", "Rust Async", &cfg, observer.clone())
            .await
            .unwrap();
        let spec_count = outcome
            .sources
            .iter()
            .filter(|s| matches!(s, Source::Spec { .. }))
            .count();
        assert_eq!(spec_count, 0, "--no-specs must suppress spec sources");
        // The Specs phase event should still fire so the UI shows the skip.
        let events = observer.events.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                SessionEvent::Phase {
                    phase: SessionPhase::Specs
                }
            )),
            "Specs phase event should fire even when skipped"
        );
    }
    #[test]
    fn default_summary_counts_each_source_type() {
        let s = vec![
            Source::Web {
                published_at: None,
                url: "u".into(),
                title: "t".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-01.md"),
                relevance: String::new(),
                body: String::new(),
            },
            Source::Local {
                path: "x.md".into(),
                kind: LocalSourceKind::InProject,
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/local-01.md"),
                relevance: "r".into(),
                body: String::new(),
            },
        ];
        let out = default_summary(&s, "topic");
        assert!(out.contains("2 source(s)"));
        assert!(out.contains("1 web"));
        assert!(out.contains("1 local"));
        // Mechanical fallback must be transparent about its provenance.
        assert!(out.contains("No LLM analysis was applied"));
    }

    #[test]
    fn default_summary_names_web_titles_and_local_paths() {
        let s = vec![
            Source::Web {
                published_at: None,
                url: "https://a".into(),
                title: "Article A".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-01.md"),
                relevance: String::new(),
                body: String::new(),
            },
            Source::Web {
                published_at: None,
                url: "https://b".into(),
                title: "Article B".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-02.md"),
                relevance: String::new(),
                body: String::new(),
            },
            Source::Local {
                path: "src/lib.rs".into(),
                kind: LocalSourceKind::InProject,
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/local-01.md"),
                relevance: "anchor".into(),
                body: String::new(),
            },
        ];
        let out = default_summary(&s, "topic");
        assert!(out.contains("**Web sources:**"));
        assert!(out.contains("Article A"));
        assert!(out.contains("Article B"));
        assert!(out.contains("**Local files:**"));
        assert!(out.contains("src/lib.rs"));
    }

    #[test]
    fn default_summary_handles_empty_source_list() {
        let out = default_summary(&[], "topic");
        assert!(out.contains("No sources were captured"));
        assert!(!out.contains("No LLM analysis"));
    }

    #[test]
    fn default_findings_handles_zero_sources() {
        let out = default_findings(&[], "x");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("No sources"));
        assert!(out[0].contains("**Headline:**"));
        assert!(out[0].contains("**Observation:**"));
        assert!(out[0].contains("No direct dependencies."));
    }

    #[test]
    fn default_findings_include_source_citation_marker() {
        let s = vec![Source::Web {
            published_at: None,
            url: "https://a".into(),
            title: "Article A".into(),
            captured_at: chrono::Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "Body of article A — talks about cargo workspaces and lockfiles.".into(),
        }];
        let out = default_findings(&s, "topic");
        assert_eq!(out.len(), 1);
        assert!(
            out[0].contains("[#1]"),
            "mechanical finding should cite its source: {}",
            out[0]
        );
    }

    #[test]
    fn default_findings_emits_per_source_with_excerpts() {
        let s = vec![
            Source::Web {
                published_at: None,
                url: "https://a".into(),
                title: "Article A".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-01.md"),
                relevance: String::new(),
                body: "Body of article A — talks about cargo workspaces and lockfiles.".into(),
            },
            Source::Local {
                path: "src/lib.rs".into(),
                kind: LocalSourceKind::InProject,
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/local-01.md"),
                relevance: "anchor file".into(),
                body: "Excerpt — 2 keyword match(es)\n\n▶    1: fn main() { }".into(),
            },
            Source::Spec {
                spec_id: "foo".into(),
                captured_at: chrono::Utc::now(),
                relevance: "Foo spec".into(),
            },
        ];
        let out = default_findings(&s, "topic");
        // One finding per source.
        assert_eq!(out.len(), 3, "expected 3 findings, got {:?}", out);
        // Each finding uses the five-paragraph structure (Headline + four required).
        for f in &out {
            assert!(
                f.contains("**Headline:**"),
                "missing Headline paragraph: {}",
                f
            );
            assert!(
                f.contains("**Observation:**"),
                "missing Observation paragraph: {}",
                f
            );
            assert!(
                f.contains("**Analysis:**"),
                "missing Analysis paragraph: {}",
                f
            );
            assert!(
                f.contains("**Cross-reference / Dependencies:**"),
                "missing Cross-reference paragraph: {}",
                f
            );
            assert!(
                f.contains("**Implication:**"),
                "missing Implication paragraph: {}",
                f
            );
        }
        // Web finding carries the title and excerpt.
        assert!(out[0].contains("Article A"));
        assert!(out[0].contains("cargo workspaces"));
        // Local finding carries the relevance note and excerpt, and references the web finding.
        assert!(out[1].contains("src/lib.rs"));
        assert!(out[1].contains("anchor file"));
        assert!(out[1].contains("Finding 1"));
        // Spec finding carries the id and references the local finding.
        assert!(out[2].contains("foo"));
        assert!(out[2].contains("Finding 2"));
    }

    #[test]
    fn default_findings_falls_back_to_metadata_when_body_is_empty() {
        let s = vec![Source::Web {
            published_at: None,
            url: "https://a".into(),
            title: "Empty Page".into(),
            captured_at: chrono::Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: String::new(),
        }];
        let out = default_findings(&s, "topic");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("Empty Page"));
        assert!(out[0].contains("no body text was returned"));
        assert!(out[0].contains("**Headline:**"));
        assert!(out[0].contains("**Observation:**"));
        assert!(out[0].contains("No direct dependencies."));
    }

    #[test]
    fn default_open_questions_suggests_re_run_with_llm() {
        let s = vec![Source::Spec {
            spec_id: "x".into(),
            captured_at: chrono::Utc::now(),
            relevance: String::new(),
        }];
        let out = default_open_questions(&s, "topic");
        assert!(out.iter().any(|q| q.contains("No web sources")));
        assert!(out.iter().any(|q| q.contains("No in-project files")));
        // Always suggest a re-run when no LLM analysis was applied.
        assert!(out.iter().any(|q| q.contains("Re-run")));
    }

    #[test]
    fn default_open_questions_handles_empty_source_list() {
        let out = default_open_questions(&[], "topic");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("Why was nothing captured"));
    }

    #[tokio::test]
    async fn synthesize_result_event_emitted_when_no_llm() {
        use crate::analysis::NoopAnalysisEngine;
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();
        let manager = ResearchManager::new(&research_root);
        let session = ResearchSession::new(manager, None, None, Arc::new(NoopAnalysisEngine));
        let observer = Arc::new(CollectObserver::default());
        let cfg = SessionConfig {
            topic: "topic".into(),
            ..SessionConfig::default()
        };
        session
            .run("rust-async", "Rust Async", &cfg, observer.clone())
            .await
            .unwrap();
        let events = observer.events.lock().unwrap();
        let synth = events
            .iter()
            .find_map(|e| match e {
                SessionEvent::SynthesizeResult { outcome, .. } => Some(*outcome),
                _ => None,
            })
            .expect("SynthesizeResult event should be emitted");
        assert_eq!(synth, SynthesizeOutcome::NoLlm);
    }

    #[test]
    fn engine_config_defaults_to_standard_single_pass() {
        let cfg = SessionConfig::default();
        let ec = cfg.engine_config();
        assert_eq!(ec.max_iterations, 3);
        assert_eq!(ec.max_sources_per_question, 3);
        assert!(!ec.force_deeper);
    }

    #[test]
    fn engine_config_deep_forces_deeper_and_more_iterations() {
        let cfg = SessionConfig {
            depth: Some(Depth::Deep),
            ..SessionConfig::default()
        };
        let ec = cfg.engine_config();
        assert_eq!(ec.max_iterations, 5);
        assert!(ec.force_deeper);
    }

    #[test]
    fn engine_config_explicit_iterations_override() {
        let cfg = SessionConfig {
            depth: Some(Depth::Shallow),
            iterations: Some(7),
            ..SessionConfig::default()
        };
        let ec = cfg.engine_config();
        assert_eq!(ec.max_iterations, 7);
    }

    #[test]
    fn budget_web_results_scales_with_depth() {
        let shallow = SessionConfig {
            depth: Some(Depth::Shallow),
            ..SessionConfig::default()
        };
        let deep = SessionConfig {
            depth: Some(Depth::Deep),
            ..SessionConfig::default()
        };
        assert_eq!(shallow.budget_web_results(), 6);
        assert_eq!(deep.budget_web_results(), 15);
    }

    #[test]
    fn budget_local_sources_matches_depth_preset() {
        let shallow = SessionConfig {
            depth: Some(Depth::Shallow),
            ..SessionConfig::default()
        };
        let standard = SessionConfig {
            depth: Some(Depth::Standard),
            ..SessionConfig::default()
        };
        let deep = SessionConfig {
            depth: Some(Depth::Deep),
            ..SessionConfig::default()
        };
        assert_eq!(shallow.budget_local_sources(), 5);
        assert_eq!(standard.budget_local_sources(), 10);
        assert_eq!(deep.budget_local_sources(), 20);
    }

    #[test]
    fn use_iterative_only_when_iterations_or_deep() {
        let none = SessionConfig::default();
        let shallow = SessionConfig {
            depth: Some(Depth::Shallow),
            ..SessionConfig::default()
        };
        let standard = SessionConfig {
            depth: Some(Depth::Standard),
            ..SessionConfig::default()
        };
        let deep = SessionConfig {
            depth: Some(Depth::Deep),
            ..SessionConfig::default()
        };
        let iterations = SessionConfig {
            iterations: Some(2),
            ..SessionConfig::default()
        };
        assert!(none.iterations.is_none() && none.depth != Some(Depth::Deep));
        assert!(shallow.iterations.is_none() && shallow.depth != Some(Depth::Deep));
        assert!(standard.iterations.is_none() && standard.depth != Some(Depth::Deep));
        assert!(deep.iterations.is_some() || deep.depth == Some(Depth::Deep));
        assert!(iterations.iterations.is_some() || iterations.depth == Some(Depth::Deep));
    }

    // ── topic-derivation unit tests ──────────��──────────────────────────────

    #[test]
    fn split_glued_words_splits_camel_case_and_acronyms() {
        assert_eq!(
            split_glued_words("HomepageArticlesLarge"),
            "Homepage Articles Large"
        );
        assert_eq!(split_glued_words("AIReasoning"), "AI Reasoning");
        assert_eq!(split_glued_words("RustAsync"), "Rust Async");
        // Lower/upper transitions only, not inside acronyms.
        assert_eq!(split_glued_words("URL"), "URL");
    }

    #[test]
    fn clean_site_title_strips_site_brand_and_nav_prefixes() {
        assert_eq!(
            clean_site_title(
                "InfoQ HomepageArticlesLarge Concept Models: a Paradigm Shift in AI Reasoning"
            ),
            Some("Large Concept Models: a Paradigm Shift in AI Reasoning".into())
        );
        assert_eq!(
            clean_site_title("Medium | Articles | Rust Async Patterns"),
            Some("Rust Async Patterns".into())
        );
        assert_eq!(
            clean_site_title("Large Concept Models: a Paradigm Shift in AI Reasoning | InfoQ"),
            Some("Large Concept Models: a Paradigm Shift in AI Reasoning".into())
        );
    }

    #[test]
    fn clean_site_title_rejects_generic_or_short_titles() {
        assert_eq!(clean_site_title("Example Site"), None);
        assert_eq!(clean_site_title("Home"), None);
        assert_eq!(clean_site_title("Articles"), None);
        assert_eq!(clean_site_title("OK"), None);
    }

    #[test]
    fn derive_topic_prefers_cleaned_title_over_body() {
        let body = "Home About Contact\n\nThe actual article begins here with useful content."; // 10 words - kept
        let title = "InfoQ HomepageArticlesLarge Concept Models: a Paradigm Shift in AI Reasoning";
        assert_eq!(
            derive_topic_from_url_body(body, title, "https://example.com/article"),
            Some("Large Concept Models: a Paradigm Shift in AI Reasoning — The actual article begins here with useful content".into())
        );
    }

    #[test]
    fn derive_topic_appends_description_and_skips_title_duplicate() {
        let body = "Large Concept Models: a Paradigm Shift in AI Reasoning are introduced in this article. They move generation from tokens to concepts, improving reasoning and explainability.";
        let title = "InfoQ HomepageArticlesLarge Concept Models: a Paradigm Shift in AI Reasoning";
        let topic = derive_topic_from_url_body(body, title, "https://example.com/article");
        assert!(
            topic
                .as_deref()
                .unwrap_or("")
                .starts_with("Large Concept Models: a Paradigm Shift in AI Reasoning — They move"),
            "expected title + body description, got {:?}",
            topic
        );
    }

    #[test]
    fn derive_topic_falls_back_to_body_when_title_is_generic() {
        let body = "The Rust async model maps asynchronous operations onto lightweight futures.";
        let title = "Example Site";
        let topic = derive_topic_from_url_body(body, title, "https://example.com/article");
        assert!(
            topic
                .as_deref()
                .unwrap_or("")
                .contains("Rust async model maps asynchronous operations"),
            "expected body-derived topic, got {:?}",
            topic
        );
    }

    #[test]
    fn derive_topic_description_truncates_long_sentences() {
        let body = "This is an extremely long introductory sentence that goes on and on and on in order to test that the derived topic description is truncated to a reasonable length without breaking in the middle of a word.";
        let title = "Some Article Title";
        let topic = derive_topic_from_url_body(body, title, "https://example.com/article").unwrap();
        assert!(
            topic.len() <= MAX_DERIVED_TOPIC_CHARS,
            "topic too long: {}",
            topic.len()
        );
        assert!(
            topic.starts_with("Some Article Title —"),
            "topic: {}",
            topic
        );
    }

    #[test]
    fn fuzzy_contains_detects_subsequence() {
        assert!(fuzzy_contains(
            "large concept models shift ai reasoning",
            "concept models shift"
        ));
        assert!(!fuzzy_contains(
            "large concept models",
            "concept models shift"
        ));
    }
}
