//! CLI helpers for the `ragent research …` sub-commands (T-034, T-035).
//!
//! Provides parse, build-help, and JSON progress-emit helpers used by
//! `src/main.rs` to dispatch the `ragent research <subcommand>` family.

use crate::research_name::ResearchNameError;

/// Parsed `ragent research <subcommand>` arguments.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchCliCommand {
    /// `ragent research help` — show the help table.
    Help,
    /// `ragent research create <name> [topic] [--from-url <URL>] [--from-file <PATH>] [--iterations N] [--depth shallow|standard|deep] [--tier light|full|dissertation] [--format report|executive-summary|comparison-table|source-bibliography|imrad] [--sources-dir <path>] [--template <name>] [--fetch-concurrently N] [--use-local] [--use-specs] [--use-low-relevance] [--no-papers]` — run a gathering session.
    Create {
        /// Validated research name (or raw string if validation hasn't run).
        name: String,
        /// Free-form topic description. Optional when `--from-url` or
        /// `--from-file` is supplied; in that case the fetched/extracted
        /// content becomes the research subject.
        topic: String,
        /// `--from-url <URL>`: fetch one or more URLs and use their content as
        /// research subjects in place of (or alongside) an explicit topic.
        /// Each fetched page is captured as a primary web source; the normal
        /// web-search phase still runs using the derived topic. Repeat the
        /// flag to seed multiple pages.
        from_urls: Vec<String>,
        /// `--from-file <PATH>`: extract one or more local documents and use their
        /// content as research subjects in place of (or alongside) an explicit
        /// topic. The extracted content from each file is captured as the
        /// primary `Source::Other`; the normal web-search phase still runs using
        /// the derived topic. Repeat the flag to seed multiple files. If any
        /// referenced file is a PDF, PDF web sources are automatically enabled
        /// for the gather phase.
        from_files: Vec<String>,
        /// Optional FR-010 `--iterations N` override.
        iterations: Option<u32>,
        /// Optional FR-011 `--depth shallow|standard|deep`.
        depth: Option<String>,
        /// Optional FR-001 `--tier light|full|dissertation`.
        tier: Option<String>,
        /// Optional FR-012 `--format <artifact>`.
        format: Option<String>,
        /// Optional FR-019 `--sources-dir <path>`.
        sources_dir: Option<String>,
        /// Optional FR-020 `--template <name>`.
        template: Option<String>,
        /// `--fetch-concurrently N` — override the maximum number of candidate
        /// pages fetched in parallel during the web-gathering phase. The
        /// default is `ragent_research::DEFAULT_FETCH_CONCURRENCY` (10); `0`
        /// is clamped up to `1`. Larger values reduce wall-clock latency when
        /// a search returns many hits, at the cost of more in-flight HTTP
        /// connections.
        fetch_concurrency: Option<usize>,
        /// `--local-concurrently N` — override the maximum number of local
        /// candidate scoring/spec-scan tasks that run in parallel. The
        /// default is `ragent_research::DEFAULT_LOCAL_CONCURRENCY` (8); `0`
        /// is clamped up to `1`. Larger values reduce wall-clock latency on
        /// large projects at the cost of more in-flight file handles.
        local_concurrency: Option<usize>,
        /// `--use-local` — enable the local-file scanning phase.
        use_local: bool,
        /// `--use-specs` — enable the prior-spec cross-reference phase.
        use_specs: bool,
        /// `--use-low-relevance` — keep web sources that would otherwise be
        /// filtered out as low-relevance. By default the web-gathering phase
        /// discards sources whose query-match ratio falls below the "Low"
        /// threshold; this flag disables that filter so every fetched page is
        /// retained regardless of relevance score.
        use_low_relevance: bool,
        /// `--no-papers` — disable scholarly search engines (e.g. OpenAlex)
        /// during the web-gathering phase. When set, hits from scholarly
        /// backends are filtered out so only general web search results are
        /// captured.
        no_papers: bool,
        /// `--use-pdf` — allow PDF documents returned by web search or
        /// `--from-url` to be captured as sources. By default PDF web sources
        /// are skipped because they require extra extraction time and are often
        /// paywalled or large.
        use_pdf: bool,
        /// `--fetch-timeout-secs N` — override the per-page fetch timeout.
        /// Pages that take longer than this are treated as a fetch failure so
        /// one slow URL cannot stall the whole gather pass. The default is
        /// 30 seconds.
        fetch_timeout_secs: Option<u64>,
        /// `--web-phase-timeout-secs N` / `--web-time N` — optional
        /// wall-clock timeout for the entire web-gathering phase (Milestone
        /// H-001). When the deadline passes, everything gathered so far is
        /// ingested and the run proceeds to analysis/synthesis. `0` disables
        /// the timeout.
        web_phase_timeout_secs: Option<u64>,
        /// `--local-phase-timeout-secs N` — optional wall-clock timeout for the
        /// entire local-gathering phase (Milestone H-001). When set, the phase
        /// is aborted if it exceeds `N` seconds and a diagnostic is emitted so
        /// a slow filesystem scan cannot stall the session. When `None`, no
        /// phase-level timeout is applied.
        local_phase_timeout_secs: Option<u64>,
        /// `--search-max-retries N` — maximum retry attempts for a failed
        /// sub-query search (Milestone H-002). Defaults to 2. `0` disables
        /// retries.
        search_max_retries: Option<u32>,
        /// `--search-retry-base-delay-ms N` — base delay in milliseconds for
        /// the first search-retry backoff (Milestone H-002). Subsequent
        /// retries double this value. Defaults to 200 ms.
        search_retry_base_delay_ms: Option<u64>,
        /// `--search-circuit-breaker-threshold N` — number of consecutive
        /// search-tool failures after which the circuit-breaker opens
        /// (Milestone H-003). `0` disables the circuit-breaker. Defaults to 3.
        search_circuit_breaker_threshold: Option<u32>,
    },
    /// `ragent research continue <name> [message]` — resume an in-progress item (T-012).
    Continue {
        /// Research name.
        name: String,
        /// Optional follow-up requirement to add to the plan (T-014).
        message: Option<String>,
    },
    /// `ragent research list` — list every item.
    List {
        /// `--all` includes archived items.
        all: bool,
    },
    /// `ragent research open <name>` — print the absolute path of `RESEARCH.md`.
    Open {
        /// Research name.
        name: String,
    },
    /// `ragent research search <query>` — full-text search.
    Search {
        /// Search query string.
        query: String,
    },
    /// `ragent research show <name>` — print metadata.
    Show {
        /// Research name.
        name: String,
    },
    /// `ragent research delete <name>` — remove the item.
    Delete {
        /// Research name.
        name: String,
        /// `--yes` to skip confirmation.
        yes: bool,
    },
    /// `ragent research archive <name>` — mark as archived.
    Archive {
        /// Research name.
        name: String,
    },
    /// `ragent research cluster <name> [--force]` — extract the top 10 concepts
    /// from the web-source documents under `research/<name>/sources/`.
    Cluster {
        /// Research name.
        name: String,
        /// `--force` to overwrite an existing `CONCEPTS.md`.
        force: bool,
    },
    /// Unknown / malformed sub-command — preserves the raw input for an
    /// error message.
    Unknown(String),
}

impl ResearchCliCommand {
    /// Parse `ragent research …` arguments. The first positional argument is
    /// the subcommand; remaining arguments are subcommand-specific.
    #[must_use]
    pub fn parse(args: &str) -> Self {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return Self::Help;
        }
        let mut parts = trimmed.split_whitespace();
        let sub = parts.next().unwrap_or("");
        let rest: Vec<&str> = parts.collect();
        match sub {
            "help" | "-h" | "--help" => Self::Help,
            "create" => Self::parse_create(&rest),
            "continue" => Self::parse_continue(&rest),
            "list" | "ls" => {
                let all = rest.contains(&"--all");
                Self::List { all }
            }
            "open" => Self::parse_open(&rest),
            "search" => {
                let query = rest.join(" ");
                Self::Search { query }
            }
            "show" => Self::parse_show(&rest),
            "delete" | "rm" => Self::parse_delete(&rest),
            "archive" => Self::Archive {
                name: rest.join(" "),
            },
            "cluster" => Self::parse_cluster(&rest),
            other => {
                // Treat as `create <name> <topic…>` if it looks like a name.
                let name = other.to_string();
                let topic = rest.join(" ");
                if topic.is_empty() {
                    Self::Unknown(name)
                } else {
                    Self::Create {
                        name,
                        topic,
                        from_urls: Vec::new(),
                        from_files: Vec::new(),
                        iterations: None,
                        depth: None,
                        format: None,
                        sources_dir: None,
                        template: None,
                        fetch_concurrency: None,
                        use_local: false,
                        use_specs: false,
                        use_low_relevance: false,
                        no_papers: false,
                        use_pdf: false,
                        fetch_timeout_secs: None,
                        local_concurrency: None,
                        web_phase_timeout_secs: None,
                        local_phase_timeout_secs: None,
                        search_max_retries: None,
                        search_retry_base_delay_ms: None,
                        search_circuit_breaker_threshold: None,
                        tier: None,
                    }
                }
            }
        }
    }

    fn parse_create(rest: &[&str]) -> Self {
        let mut i = 0;
        let mut name: Option<String> = None;
        let mut topic_words: Vec<&str> = Vec::new();
        let mut from_urls: Vec<String> = Vec::new();
        let mut from_files: Vec<String> = Vec::new();
        let mut iterations: Option<u32> = None;
        let mut depth: Option<String> = None;
        let mut tier: Option<String> = None;
        let mut format: Option<String> = None;
        let mut sources_dir: Option<String> = None;
        let mut template: Option<String> = None;
        let mut fetch_concurrency: Option<usize> = None;
        let mut fetch_timeout_secs: Option<u64> = None;
        let mut local_concurrency: Option<usize> = None;
        let mut web_phase_timeout_secs: Option<u64> = None;
        let mut local_phase_timeout_secs: Option<u64> = None;
        let mut search_max_retries: Option<u32> = None;
        let mut search_retry_base_delay_ms: Option<u64> = None;
        let mut search_circuit_breaker_threshold: Option<u32> = None;
        let mut use_local = false;
        let mut use_specs = false;
        let mut use_low_relevance = false;
        let mut no_papers = false;
        let mut use_pdf = false;

        while i < rest.len() {
            let arg = rest[i];
            match arg {
                "--from-url"
                | "--from-file"
                | "--iterations"
                | "--depth"
                | "--tier"
                | "--format"
                | "--sources-dir"
                | "--template"
                | "--fetch-concurrently"
                | "--local-concurrently"
                | "--fetch-timeout-secs"
                | "--web-phase-timeout-secs"
                | "--web-time"
                | "--local-phase-timeout-secs"
                | "--search-max-retries"
                | "--search-retry-base-delay-ms"
                | "--search-circuit-breaker-threshold" => {
                    // Move to the value (if present); the final i += 1 below
                    // will then step past it.
                    i += 1;
                    if let Some(v) = rest.get(i) {
                        match arg {
                            "--from-url" => from_urls.push((*v).to_string()),
                            "--from-file" => from_files.push((*v).to_string()),
                            "--iterations" => iterations = v.parse().ok(),
                            "--depth" => depth = Some((*v).to_string()),
                            "--tier" => tier = Some((*v).to_string()),
                            "--format" => format = Some((*v).to_string()),
                            "--sources-dir" => sources_dir = Some((*v).to_string()),
                            "--template" => template = Some((*v).to_string()),
                            "--fetch-concurrently" => fetch_concurrency = v.parse().ok(),
                            "--local-concurrently" => local_concurrency = v.parse().ok(),
                            "--fetch-timeout-secs" => fetch_timeout_secs = v.parse().ok(),
                            "--web-phase-timeout-secs" | "--web-time" => {
                                web_phase_timeout_secs = v.parse().ok();
                            }
                            "--local-phase-timeout-secs" => {
                                local_phase_timeout_secs = v.parse().ok();
                            }
                            "--search-max-retries" => search_max_retries = v.parse().ok(),
                            "--search-retry-base-delay-ms" => {
                                search_retry_base_delay_ms = v.parse().ok();
                            }
                            "--search-circuit-breaker-threshold" => {
                                search_circuit_breaker_threshold = v.parse().ok();
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                "--use-local" => use_local = true,
                "--use-specs" => use_specs = true,
                "--use-low-relevance" => use_low_relevance = true,
                "--no-papers" => no_papers = true,
                "--use-pdf" => use_pdf = true,
                _ => {
                    if name.is_none() {
                        name = Some(arg.to_string());
                    } else {
                        topic_words.push(arg);
                    }
                }
            }
            i += 1;
        }
        let Some(name) = name else {
            return Self::Unknown("create".to_string());
        };
        let topic = topic_words.join(" ");
        Self::Create {
            name,
            topic,
            from_urls,
            from_files,
            iterations,
            depth,
            tier,
            format,
            sources_dir,
            template,
            fetch_concurrency,
            use_local,
            use_specs,
            use_low_relevance,
            no_papers,
            use_pdf,
            fetch_timeout_secs,
            local_concurrency,
            web_phase_timeout_secs,
            local_phase_timeout_secs,
            search_max_retries,
            search_retry_base_delay_ms,
            search_circuit_breaker_threshold,
        }
    }

    fn is_flag(arg: &str) -> bool {
        arg.starts_with('-')
    }

    fn first_positional(rest: &[&str]) -> Option<String> {
        rest.iter()
            .find(|a| !Self::is_flag(a))
            .map(std::string::ToString::to_string)
    }

    fn parse_continue(rest: &[&str]) -> Self {
        let name = Self::first_positional(rest).unwrap_or_default();
        if name.is_empty() {
            return Self::Unknown("continue".to_string());
        }
        let message = rest.iter().skip_while(|a| **a != name).nth(1).map(|_| {
            rest.iter()
                .skip_while(|a| **a != name)
                .skip(1)
                .copied()
                .collect::<Vec<_>>()
                .join(" ")
        });
        let message = message.filter(|m| !m.is_empty());
        Self::Continue { name, message }
    }
    fn parse_open(rest: &[&str]) -> Self {
        match Self::first_positional(rest) {
            Some(name) => Self::Open { name },
            None => Self::Unknown("open".to_string()),
        }
    }

    fn parse_show(rest: &[&str]) -> Self {
        match Self::first_positional(rest) {
            Some(name) => Self::Show { name },
            None => Self::Unknown("show".to_string()),
        }
    }

    fn parse_delete(rest: &[&str]) -> Self {
        let yes = rest.contains(&"--yes") || rest.contains(&"-y");
        match Self::first_positional(rest) {
            Some(name) => Self::Delete { name, yes },
            None => Self::Unknown("delete".to_string()),
        }
    }

    fn parse_cluster(rest: &[&str]) -> Self {
        let force = rest.contains(&"--force");
        match Self::first_positional(rest) {
            Some(name) => Self::Cluster { name, force },
            None => Self::Unknown("cluster".to_string()),
        }
    }

    /// Build the static help message shown by `ragent research help`.
    #[must_use]
    pub const fn build_help_message() -> &'static str {
        "ragent research — manage research items under research/\n\
               \n\
               USAGE:\n\
                 ragent research <SUBCOMMAND> [ARGS]\n\
               \n\
               SUBCOMMANDS:\n\
                                   create <name> [topic] [--from-url <URL>] [--iterations N] [--depth shallow|standard|deep] [--tier light|full|dissertation]\n\
                                         [--format report|executive-summary|comparison-table|source-bibliography|imrad]\n\
                                         [--sources-dir <path>] [--template <name>]                                          [--fetch-concurrently N] [--use-local] [--use-specs] [--use-low-relevance] [--use-pdf] [--no-papers]
                                          [--web-time <secs>] [--web-phase-timeout-secs <secs>]\n\
                                         Run an information-gathering session and write RESEARCH.md.\n\
                                           --from-url            Fetch one or more URLs and use their content as the research subject\n\
                                                                 in place of (or alongside) an explicit topic. Each page is captured\n\
                                                                 as a primary source; web search still runs. Repeat the flag to seed\n\
                                                                 multiple pages.\n\
                                           --from-file           Extract one or more local documents and use their content as the research subject\n\
                                                                 in place of (or alongside) an explicit topic. Each file is captured\n\
                                                                 as a primary source; web search still runs. Repeat the flag to seed\n\
                                                                 multiple files. PDF files automatically enable PDF web sources.\n\
                                         --iterations          Override the default maximum number of iterations.\n\
                                         --depth               Choose a preset: shallow, standard, or deep (default: standard).\n\
                                         --tier                Choose a research tier: light, full, or dissertation (default: full).\n\
                                         --format              Select the output artifact format. Values: report, executive-summary, comparison-table, source-bibliography, imrad (default: report).\n\
                                         --fetch-concurrently  Override the maximum number of candidate pages fetched\n\
                                                               in parallel during the web-gathering phase (default 10).\n\
                                         --web-time            Wall-clock timeout in seconds for the whole web-gathering\n\
                                                               phase (alias of --web-phase-timeout-secs; default 60). When\n\
                                                               the deadline passes, everything gathered so far is ingested\n\
                                                               and the run continues to analysis/synthesis. `0` disables.\n\
                                         --use-local           Enable local-file scanning (in-project + extras).\n\
                                         --use-specs           Enable prior-spec cross-referencing.\n\
                                         --use-low-relevance   Keep low-relevance web sources instead of filtering them out.\n\
                                         --use-pdf            Allow PDF documents from web search or --from-url to be captured.\\
                                           --no-papers           Disable scholarly search engines (e.g. OpenAlex) during web gathering.\n\
                   continue <name> [message] Resume an in-progress research item.\n\
                   list [--all]                  List every research item.\n\
                 open <name>                   Print the absolute path of RESEARCH.md.\n\
                 search <query>                Full-text search across all RESEARCH.md.\n\
                 show <name>                   Print metadata for a single item.\n\
                 delete <name> [--yes]         Remove a research item (prompts unless --yes).\n\
                 archive <name>                Mark a research item as archived.\n\
                 cluster <name> [--force]      Extract top 10 concepts from research/<name>/sources/.\n\
                 help                          Show this message.\n\
               \n\
               Output: by default `ragent research` emits machine-readable JSON lines\n\
               prefixed with `ragent-research:` so callers can pipe the output\n\
               through `jq` or other tools."
    }
    /// `true` if this is a usage-error variant (i.e. parse succeeded but the
    /// caller passed wrong args).
    #[must_use]
    pub const fn is_usage_error(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

/// Render a [`SessionEvent`] as a pure JSON string (no CLI prefix).
///
/// This is the shared serialization core used by both the CLI line renderer
/// ([`render_session_event_json`]) and the HTTP SSE handler. The result is
/// always a valid JSON object of the form `{"kind": "...", "payload": {...}}`.
#[must_use]
pub fn session_event_json(event: &crate::session::SessionEvent) -> String {
    use crate::session::{AnalysisEvent, SessionEvent, SynthesisEvent};
    let (kind, payload) = match event {
        SessionEvent::Phase { phase } => ("phase", serde_json::json!({ "phase": phase.as_str() })),
        SessionEvent::QueriesDecomposed { queries } => {
            ("queries", serde_json::json!({ "queries": queries }))
        }
        SessionEvent::WebCaptured {
            url,
            title,
            search_tool,
            search_engine,
            body_preview,
            language,
            oa_recovery,
            media_type,
        } => {
            let mut payload = serde_json::Map::new();
            payload.insert("url".into(), serde_json::json!(url));
            payload.insert("title".into(), serde_json::json!(title));
            payload.insert("search_tool".into(), serde_json::json!(search_tool));
            payload.insert("search_engine".into(), serde_json::json!(search_engine));
            payload.insert("body_preview".into(), serde_json::json!(body_preview));
            payload.insert("language".into(), serde_json::json!(language));
            payload.insert("media_type".into(), serde_json::json!(media_type));
            if let Some(r) = oa_recovery.as_ref() {
                payload.insert(
                    "oa_recovery".into(),
                    serde_json::json!({
                        "url": r.url,
                        "source": r.source.to_string(),
                        "version": r.version,
                        "license": r.license,
                    }),
                );
            }
            ("web", serde_json::Value::Object(payload))
        }
        SessionEvent::FromUrlBodyPreview { url, body_preview } => (
            "from_url_body_preview",
            serde_json::json!({ "url": url, "body_preview": body_preview }),
        ),
        SessionEvent::FromFileBodyPreview { path, body_preview } => (
            "from_file_body_preview",
            serde_json::json!({ "path": path, "body_preview": body_preview }),
        ),
        SessionEvent::LocalCaptured { path, score } => {
            ("local", serde_json::json!({ "path": path, "score": score }))
        }
        SessionEvent::SpecCaptured { spec_id } => {
            ("spec", serde_json::json!({ "spec_id": spec_id }))
        }
        SessionEvent::WebSearchFailed { error } => {
            ("web_error", serde_json::json!({ "error": error }))
        }
        SessionEvent::WebFetchFailed { url, error } => (
            "web_fetch_error",
            serde_json::json!({ "url": url, "error": error }),
        ),
        SessionEvent::WebSourceExcluded { url, reason } => (
            "web_source_excluded",
            serde_json::json!({ "url": url, "reason": reason }),
        ),
        SessionEvent::PlanUpdated { sub_questions } => (
            "plan_updated",
            serde_json::json!({ "sub_questions": sub_questions }),
        ),
        SessionEvent::SubQuestionStatusChanged { id, status } => (
            "sub_question_status_changed",
            serde_json::json!({ "id": id, "status": status }),
        ),
        SessionEvent::SourceFailed { source, error } => (
            "source_failed",
            serde_json::json!({
                "source": source,
                "error": error,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::CriticResult { score, gaps }) => (
            "critic",
            serde_json::json!({
                "score": score,
                "gaps": gaps,
            }),
        ),
        SessionEvent::VerificationResult { passed, issues } => (
            "verification",
            serde_json::json!({
                "passed": passed,
                "issues": issues,
            }),
        ),
        SessionEvent::IterationCompleted { iteration, score } => (
            "iteration_completed",
            serde_json::json!({
                "iteration": iteration,
                "score": score,
            }),
        ),
        SessionEvent::FollowUpQueries { queries } => (
            "follow_up_queries",
            serde_json::json!({ "queries": queries }),
        ),
        SessionEvent::Analysis(AnalysisEvent::ContradictionGraph {
            edges,
            sources_scanned,
        }) => (
            "contradiction_graph",
            serde_json::json!({
                "sources_scanned": sources_scanned,
                "edges": edges,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::LociAnalysis {
            loci,
            sources_scanned,
        }) => (
            "loci_analysis",
            serde_json::json!({
                "sources_scanned": sources_scanned,
                "loci": loci,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::DepthInvestigation { investigations }) => (
            "depth_investigation",
            serde_json::json!({
                "investigations": investigations,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::CrossLocusReconcile { reconcile }) => (
            "cross_locus_reconcile",
            serde_json::json!({
                "sources_scanned": reconcile.sources_scanned,
                "pairs": reconcile.pairs,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::SourceTensions { tensions }) => (
            "source_tensions",
            serde_json::json!({
                "sources_scanned": tensions.sources_scanned,
                "tensions": tensions.tensions,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::EvidenceDigest { digest }) => (
            "evidence_digest",
            serde_json::json!({
                "sources_scanned": digest.sources_scanned,
                "claims": digest.claims,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::TripleDraft { draft }) => (
            "triple_draft",
            serde_json::json!({
                "candidates": draft.candidates,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult { outcome, detail }) => (
            "synthesize",
            serde_json::json!({
                "outcome": outcome.as_str(),
                "detail": detail,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::SynthesisAudit { audit }) => (
            "synthesis_audit",
            serde_json::json!({
                "overall_score": audit.overall_score,
                "recommendation": audit.recommendation,
                "critic_reports": audit.critic_reports,
                "sources_used": audit.sources_used,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::CorpusCritic { report }) => (
            "corpus_critic",
            serde_json::json!({
                "score": report.score,
                "passed": report.passed,
                "coverage_score": report.coverage_score,
                "evidence_score": report.evidence_score,
                "balance_score": report.balance_score,
                "tension_score": report.tension_score,
                "issues": report.issues,
                "gaps": report.gaps,
            }),
        ),
        SessionEvent::Analysis(AnalysisEvent::GapFetch { result }) => (
            "gap_fetch",
            serde_json::json!({
                "attempted": result.attempted,
                "new_sources": result.new_sources,
                "failed_queries": result.failed_queries,
                "queries": result.queries,
                "note": result.note,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::SurgicalPatch { result }) => (
            "surgical_patch",
            serde_json::json!({
                "score_before": result.score_before,
                "score_after": result.score_after,
                "patches": result.patches,
                "note": result.note,
                "patched_finding_count": result.patched_finding_count,
                "patched_implication_count": result.patched_implication_count,
                "patched_open_question_count": result.patched_open_question_count,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::CiteCheck { result }) => (
            "cite_check",
            serde_json::json!({
                "passed": result.passed,
                "checked": result.checked,
                "gate_open": result.gate_open,
                "issues": result.issues,
                "failed_claims": result.failed_claims,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::Polish { result }) => (
            "polish",
            serde_json::json!({
                "control_chars_removed": result.control_chars_removed,
                "whitespace_normalized": result.whitespace_normalized,
                "empty_paragraphs_removed": result.empty_paragraphs_removed,
                "change_count": result.changes.len(),
                "note": result.note,
            }),
        ),
        SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit { result }) => (
            "readability_audit",
            serde_json::json!({
                "score": result.score,
                "passed": result.passed,
                "avg_finding_length": result.avg_finding_length,
                "missing_label_count": result.missing_label_count,
                "long_paragraph_count": result.long_paragraph_count,
                "issues": result.issues,
                "recommendations": result.recommendations,
            }),
        ),
        SessionEvent::Done {
            total_sources,
            pdf_count,
            youtube_count,
            excluded_count,
        } => (
            "done",
            serde_json::json!({
                "total_sources": total_sources,
                "pdf_count": pdf_count,
                "youtube_count": youtube_count,
                "excluded_count": excluded_count,
            }),
        ),
        SessionEvent::RunStep {
            step,
            status,
            detail,
        } => (
            "run_step",
            serde_json::json!({
                "step": step,
                "status": status,
                "detail": detail,
            }),
        ),
        SessionEvent::TierDone {
            completed,
            skipped,
            failed,
        } => (
            "tier_done",
            serde_json::json!({
                "completed": completed,
                "skipped": skipped,
                "failed": failed,
            }),
        ),
        SessionEvent::ConfigSnapshot {
            output_format,
            depth,
            iterations,
            tier,
            from_urls,
            from_files,
        } => (
            "config",
            serde_json::json!({
                "output_format": output_format,
                "depth": depth,
                "iterations": iterations,
                "tier": tier,
                "from_urls": from_urls,
                "from_files": from_files,
            }),
        ),
    };
    serde_json::to_string(&serde_json::json!({
        "kind": kind,
        "payload": payload,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Render a [`SessionEvent`] as a single machine-readable JSON line on stdout
/// (T-035). The prefix `ragent-research:` makes the line easy to grep in a
/// mixed transcript.
#[must_use]
pub fn render_session_event_json(event: &crate::session::SessionEvent) -> String {
    format!("ragent-research: {}", session_event_json(event))
}

/// Pretty-print a research item (T-034: `ragent research show`).
#[must_use]
pub fn render_show_output(
    name: &str,
    title: &str,
    topic: &str,
    status: &str,
    created: &str,
    modified: &str,
    sources: &[(String, String, String, String, Option<String>)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Research item: {name}\n"));
    out.push_str(&format!("Title:         {title}\n"));
    out.push_str(&format!("Topic:         {topic}\n"));
    out.push_str(&format!("Status:        {status}\n"));
    out.push_str(&format!("Created (UTC): {created}\n"));
    out.push_str(&format!("Modified (UTC):{modified}\n"));
    out.push_str(&format!("\nReferences ({}):\n", sources.len()));
    for (i, (kind, path, title, captured, oa_note)) in sources.iter().enumerate() {
        out.push_str(&format!(
            "  #{i:>2}  [{kind:<11}] {path:<32}  {title}  ({captured})\n",
        ));
        if let Some(note) = oa_note {
            out.push_str(&format!("        [OA recovery] {note}\n"));
        }
    }
    out
}

/// Pretty-print a research list (T-034: `ragent research list`).
#[must_use]
pub fn render_list_output(rows: &[(String, String, String, String, String)]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<22} {:<32} {:<11} {:<24} {:<24}\n",
        "NAME", "TITLE", "STATUS", "CREATED", "MODIFIED"
    ));
    out.push_str(&"-".repeat(120));
    out.push('\n');
    for (name, title, status, created, modified) in rows {
        out.push_str(&format!(
            "{:<22} {:<32} {:<11} {:<24} {:<24}\n",
            truncate(name, 22),
            truncate(title, 32),
            truncate(status, 11),
            truncate(created, 24),
            truncate(modified, 24),
        ));
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Pretty-print a search result list (T-030).
#[must_use]
pub fn render_search_output(hits: &[(String, String, String)]) -> String {
    let mut out = String::new();
    for (name, title, snippet) in hits {
        out.push_str(&format!("• {name} — {title}\n    {snippet}\n"));
    }
    if out.is_empty() {
        out.push_str("(no matches)\n");
    }
    out
}

/// Convenience: map a [`ResearchNameError`] to a user-facing message.
#[must_use]
pub fn explain_name_error(err: &ResearchNameError) -> String {
    err.to_string()
}

// ── Filesystem-backed LocalTool for the CLI ───────────────────────────────

use crate::local_gatherer::{GrepMatch, LocalTool};
use crate::research_name::ResearchName;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Filesystem-backed implementation of [`LocalTool`] for use by the
/// `ragent research` CLI when no agent tool registry is available (T-034).
///
/// Behaviour:
/// - [`FsLocalTool::glob`] runs a synchronous glob over `project_root` and
///   returns project-relative paths. The implementation is intentionally
///   simple — it does the standard `walkdir`-style recursion itself so the
///   CLI doesn't need to depend on the `glob` crate.
/// - [`FsLocalTool::grep`] is a line-by-line case-insensitive substring
///   match over any of `terms`.
/// - [`FsLocalTool::read`] reads the file as UTF-8.
/// - [`FsLocalTool::list_specs`] lists the directories directly under
///   `specs/` and returns their base names.
/// - [`FsLocalTool::spec_title`] reads the first `#` heading of
///   `specs/<id>/SPEC.md` if present.
pub struct FsLocalTool;

impl FsLocalTool {
    /// Build a new filesystem-backed local tool.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl Default for FsLocalTool {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl LocalTool for FsLocalTool {
    async fn glob(&self, project_root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
        // Translate a `**/*.ext` style pattern into a walkdir-style scan.
        // We only support two pattern shapes:
        //   1. `**/*.<ext>`     → match every file with that extension
        //   2. `*.<ext>`        → match every file with that extension (1-deep)
        // Anything else returns an empty vec — the gatherer treats that as
        // "no candidates" and moves on.
        let ext = pattern.rsplit('.').next().unwrap_or("");
        let ext = if ext == pattern { "" } else { ext };
        if ext.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<PathBuf> = Vec::new();
        walk(project_root, project_root, ext, &mut out).await?;
        out.sort();
        Ok(out)
    }

    async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
        let body = match tokio::fs::read_to_string(path).await {
            Ok(b) => b,
            Err(_) => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let lower = line.to_lowercase();
            if terms.iter().any(|t| lower.contains(&t.to_lowercase())) {
                out.push(GrepMatch {
                    line: i + 1,
                    text: line.to_string(),
                });
            }
        }
        Ok(out)
    }

    async fn read(&self, path: &Path) -> anyhow::Result<String> {
        Ok(tokio::fs::read_to_string(path).await?)
    }

    async fn list_specs(&self, project_root: &Path) -> anyhow::Result<Vec<String>> {
        let specs_dir = project_root.join("specs");
        let mut entries = match tokio::fs::read_dir(&specs_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }
            if ResearchName::try_new(&name).is_ok() {
                out.push(name);
            }
        }
        out.sort();
        Ok(out)
    }

    async fn spec_title(&self, project_root: &Path, spec_id: &str) -> anyhow::Result<String> {
        let path = project_root.join("specs").join(spec_id).join("SPEC.md");
        let body = match tokio::fs::read_to_string(&path).await {
            Ok(b) => b,
            Err(_) => return Ok(String::new()),
        };
        for raw in body.lines() {
            let line = raw.trim();
            if let Some(rest) = line.strip_prefix("# ") {
                return Ok(rest.trim().to_string());
            }
            if let Some(rest) = line.strip_prefix("## ") {
                return Ok(rest.trim().to_string());
            }
        }
        Ok(String::new())
    }
}

async fn walk(root: &Path, dir: &Path, ext: &str, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            // Skip the research/ output directory and obvious junk so the
            // gatherer doesn't index its own previous outputs.
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "research"
                || name == "target"
                || name == ".git"
                || name == "node_modules"
                || name.starts_with('.')
            {
                continue;
            }
            Box::pin(walk(root, &path, ext, out)).await?;
        } else if file_type.is_file() {
            let matches_ext = entry
                .file_name()
                .to_string_lossy()
                .rsplit('.')
                .next()
                .is_some_and(|e| e == ext);
            if matches_ext && let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{AnalysisEvent, SynthesisEvent};

    #[test]
    fn parse_help() {
        assert_eq!(ResearchCliCommand::parse("help"), ResearchCliCommand::Help);
        assert_eq!(ResearchCliCommand::parse(""), ResearchCliCommand::Help);
    }

    #[test]
    fn parse_create_with_topic() {
        let cmd = ResearchCliCommand::parse("create rust-async async/await idioms in stable Rust");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                sources_dir,
                template,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "rust-async");
                assert_eq!(topic, "async/await idioms in stable Rust");
                assert!(sources_dir.is_none());
                assert!(template.is_none());
                assert!(!use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_sources_dir_and_template() {
        let cmd = ResearchCliCommand::parse(
            "create foo topic words --sources-dir /tmp/notes --template deepdive",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                sources_dir,
                template,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(sources_dir.as_deref(), Some("/tmp/notes"));
                assert_eq!(template.as_deref(), Some("deepdive"));
                assert!(!use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_use_local_flag() {
        let cmd = ResearchCliCommand::parse("create foo a topic --use-local");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert!(use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_use_specs_flag() {
        let cmd = ResearchCliCommand::parse("create foo a topic --use-specs");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert!(!use_local);
                assert!(use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_use_low_relevance_flag() {
        let cmd = ResearchCliCommand::parse("create foo a topic --use-low-relevance");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_low_relevance,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert!(use_low_relevance);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_use_low_relevance_defaults_false() {
        let cmd = ResearchCliCommand::parse("create foo a topic");
        match cmd {
            ResearchCliCommand::Create {
                use_low_relevance, ..
            } => {
                assert!(!use_low_relevance);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_no_scholarly_flag() {
        let cmd = ResearchCliCommand::parse("create foo a topic --no-papers");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                no_papers,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert!(no_papers);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_no_scholarly_defaults_false() {
        let cmd = ResearchCliCommand::parse("create foo a topic");
        match cmd {
            ResearchCliCommand::Create { no_papers, .. } => {
                assert!(!no_papers);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_both_no_flags() {
        let cmd = ResearchCliCommand::parse(
            "create foo a topic --use-local --use-specs --sources-dir /tmp/x",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                sources_dir,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert_eq!(sources_dir.as_deref(), Some("/tmp/x"));
                assert!(use_local);
                assert!(use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_list_all() {
        let cmd = ResearchCliCommand::parse("list --all");
        assert!(matches!(cmd, ResearchCliCommand::List { all: true }));
    }

    #[test]
    fn parse_list_default() {
        let cmd = ResearchCliCommand::parse("list");
        assert!(matches!(cmd, ResearchCliCommand::List { all: false }));
    }

    #[test]
    fn parse_open() {
        let cmd = ResearchCliCommand::parse("open rust-async");
        assert!(matches!(cmd, ResearchCliCommand::Open { ref name } if name == "rust-async"));
    }

    #[test]
    fn parse_search() {
        let cmd = ResearchCliCommand::parse("search async patterns");
        assert!(
            matches!(cmd, ResearchCliCommand::Search { ref query } if query == "async patterns")
        );
    }

    #[test]
    fn parse_show() {
        let cmd = ResearchCliCommand::parse("show foo");
        assert!(matches!(cmd, ResearchCliCommand::Show { ref name } if name == "foo"));
    }

    #[test]
    fn parse_delete_with_yes() {
        let cmd = ResearchCliCommand::parse("delete foo --yes");
        assert!(matches!(cmd, ResearchCliCommand::Delete { ref name, yes: true } if name == "foo"));
    }

    #[test]
    fn parse_create_with_iterations_depth_format() {
        let cmd = ResearchCliCommand::parse(
            "create foo topic words --iterations 5 --depth deep --format executive-summary",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                iterations,
                depth,
                format,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(iterations, Some(5));
                assert_eq!(depth.as_deref(), Some("deep"));
                assert_eq!(format.as_deref(), Some("executive-summary"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_imrad_format() {
        let cmd = ResearchCliCommand::parse("create foo topic words --format imrad");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                format,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(format.as_deref(), Some("imrad"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_tier_defaults_to_none() {
        let cmd = ResearchCliCommand::parse("create foo a topic");
        match cmd {
            ResearchCliCommand::Create { tier, .. } => {
                assert!(tier.is_none());
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_with_tier() {
        let cmd = ResearchCliCommand::parse("create foo a topic --tier light");
        match cmd {
            ResearchCliCommand::Create {
                name, topic, tier, ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert_eq!(tier.as_deref(), Some("light"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_tier_does_not_swallow_topic_words() {
        let cmd = ResearchCliCommand::parse("create foo a topic --tier dissertation");
        match cmd {
            ResearchCliCommand::Create { topic, tier, .. } => {
                assert_eq!(topic, "a topic");
                assert_eq!(tier.as_deref(), Some("dissertation"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn help_message_lists_tier_option() {
        let h = ResearchCliCommand::build_help_message();
        assert!(h.contains("--tier"), "help missing `--tier`: {h}");
        assert!(
            h.contains("light|full|dissertation"),
            "help missing tier values: {h}"
        );
    }

    #[test]
    fn parse_create_with_fetch_concurrently() {
        let cmd = ResearchCliCommand::parse("create foo topic words --fetch-concurrently 20");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                fetch_concurrency,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(fetch_concurrency, Some(20));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_without_fetch_concurrently_defaults_to_none() {
        let cmd = ResearchCliCommand::parse("create foo topic words");
        match cmd {
            ResearchCliCommand::Create {
                fetch_concurrency, ..
            } => {
                assert_eq!(fetch_concurrency, None);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_ignores_non_numeric_fetch_concurrently() {
        // A non-numeric value leaves fetch_concurrency as None and is
        // consumed as the flag's argument (not pushed into the topic).
        let cmd = ResearchCliCommand::parse("create foo topic words --fetch-concurrently abc");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                fetch_concurrency,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "topic words");
                assert_eq!(fetch_concurrency, None);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_continue_with_message() {
        let cmd = ResearchCliCommand::parse("continue rust-async focus on async-std");
        match cmd {
            ResearchCliCommand::Continue { name, message } => {
                assert_eq!(name, "rust-async");
                assert_eq!(message.as_deref(), Some("focus on async-std"));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_continue_without_message() {
        let cmd = ResearchCliCommand::parse("continue foo");
        assert!(
            matches!(cmd, ResearchCliCommand::Continue { ref name, message: None } if name == "foo")
        );
    }

    #[test]
    fn parse_cluster() {
        let cmd = ResearchCliCommand::parse("cluster foo");
        assert!(
            matches!(cmd, ResearchCliCommand::Cluster { ref name, force: false } if name == "foo")
        );
    }

    #[test]
    fn parse_cluster_with_force() {
        let cmd = ResearchCliCommand::parse("cluster foo --force");
        assert!(
            matches!(cmd, ResearchCliCommand::Cluster { ref name, force: true } if name == "foo")
        );
    }

    #[test]
    fn parse_cluster_force_before_name() {
        let cmd = ResearchCliCommand::parse("cluster --force foo");
        assert!(
            matches!(cmd, ResearchCliCommand::Cluster { ref name, force: true } if name == "foo")
        );
    }

    #[test]
    fn parse_cluster_without_name_becomes_unknown() {
        let cmd = ResearchCliCommand::parse("cluster");
        assert!(matches!(cmd, ResearchCliCommand::Unknown(sub) if sub == "cluster"));
    }

    #[test]
    fn parse_archive() {
        let cmd = ResearchCliCommand::parse("archive foo");
        assert!(matches!(cmd, ResearchCliCommand::Archive { ref name } if name == "foo"));
    }

    #[test]
    fn parse_implicit_create_when_unknown_subcommand() {
        let cmd = ResearchCliCommand::parse("foo bar baz");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_local,
                use_specs,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "bar baz");
                assert!(!use_local);
                assert!(!use_specs);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_from_url_and_no_topic() {
        let cmd = ResearchCliCommand::parse("create myitem --from-url https://example.com/article");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_urls,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "");
                assert_eq!(from_urls, vec!["https://example.com/article".to_string()]);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_from_url_and_topic() {
        let cmd = ResearchCliCommand::parse(
            "create myitem rust async --from-url https://example.com/article",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_urls,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "rust async");
                assert_eq!(from_urls, vec!["https://example.com/article".to_string()]);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_from_url_before_other_flags() {
        let cmd = ResearchCliCommand::parse(
            "create myitem --from-url https://example.com --use-local --iterations 3",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_urls,
                use_local,
                iterations,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "");
                assert_eq!(from_urls, vec!["https://example.com".to_string()]);
                assert!(use_local);
                assert_eq!(iterations, Some(3));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_multiple_from_urls() {
        let cmd = ResearchCliCommand::parse(
            "create myitem --from-url https://example.com/a --from-url https://example.com/b",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_urls,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "");
                assert_eq!(
                    from_urls,
                    vec![
                        "https://example.com/a".to_string(),
                        "https://example.com/b".to_string()
                    ]
                );
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_from_file() {
        let cmd = ResearchCliCommand::parse("create myitem --from-file docs/notes.md");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_files,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "");
                assert_eq!(from_files, vec!["docs/notes.md".to_string()]);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn parse_create_with_multiple_from_files() {
        let cmd = ResearchCliCommand::parse(
            "create myitem --from-file docs/notes.md --from-file assets/paper.pdf",
        );
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                from_files,
                ..
            } => {
                assert_eq!(name, "myitem");
                assert_eq!(topic, "");
                assert_eq!(
                    from_files,
                    vec!["docs/notes.md".to_string(), "assets/paper.pdf".to_string()]
                );
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
    #[test]
    fn help_message_documents_from_file_flag() {
        let h = ResearchCliCommand::build_help_message();
        assert!(h.contains("--from-file"), "help missing `--from-file`: {h}");
    }
    #[test]
    fn help_message_documents_from_url_flag() {
        let h = ResearchCliCommand::build_help_message();
        assert!(h.contains("--from-url"), "help missing `--from-url`: {h}");
    }
    #[test]
    fn help_message_contains_documented_subcommands() {
        let h = ResearchCliCommand::build_help_message();
        for sub in [
            "create", "continue", "list", "open", "search", "show", "delete", "archive", "cluster",
        ] {
            assert!(h.contains(sub), "help missing `{sub}`");
        }
    }

    #[test]
    fn help_message_lists_imrad_format_option() {
        let h = ResearchCliCommand::build_help_message();
        assert!(
            h.contains("imrad"),
            "help missing `imrad` format option: {h}"
        );
    }

    #[test]
    fn render_session_event_json_for_phase() {
        let event = crate::session::SessionEvent::Phase {
            phase: crate::session::SessionPhase::Web,
        };
        let line = render_session_event_json(&event);
        assert!(line.starts_with("ragent-research:"));
        assert!(line.contains("\"web\""));
    }

    #[test]
    fn render_session_event_json_for_plan_updated() {
        let event = crate::session::SessionEvent::PlanUpdated {
            sub_questions: vec!["q1".into(), "q2".into()],
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"plan_updated\""));
        assert!(line.contains("\"q1\""));
    }

    #[test]
    fn render_session_event_json_for_source_failed() {
        let event = crate::session::SessionEvent::SourceFailed {
            source: Some("https://example.com".into()),
            error: "timeout".into(),
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"source_failed\""));
        assert!(line.contains("\"timeout\""));
    }

    #[test]
    fn render_session_event_json_for_critic_and_verification() {
        let event = crate::session::SessionEvent::Synthesis(SynthesisEvent::CriticResult {
            score: Some(72),
            gaps: vec!["missing citation".into()],
        });
        let line = render_session_event_json(&event);
        assert!(line.contains("\"critic\""));
        assert!(line.contains("72"));

        let event = crate::session::SessionEvent::VerificationResult {
            passed: false,
            issues: vec!["claim X unsupported".into()],
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"verification\""));
        assert!(line.contains("false"));
    }

    #[test]
    fn render_session_event_json_for_web_captured_includes_oa_recovery() {
        use crate::open_access::{RecoveredOpenAccess, RecoverySource};
        let event = crate::session::SessionEvent::WebCaptured {
            url: "https://doi.org/10.1234/example".into(),
            title: "Example paper".into(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            body_preview: String::new(),
            language: "ENGLISH".into(),
            media_type: "page".into(),
            oa_recovery: Some(Box::new(RecoveredOpenAccess {
                url: "https://pmc.ncbi.nlm.nih.gov/articles/PMC123456/".into(),
                source: RecoverySource::EuropePmc,
                license: Some("CC-BY-4.0".into()),
                version: Some("publishedVersion".into()),
            })),
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"web\""));
        assert!(line.contains("\"oa_recovery\""));
        assert!(line.contains("pmc.ncbi.nlm.nih.gov/articles/PMC123456/"));
        assert!(line.contains("europepmc"));
        assert!(line.contains("publishedVersion"));
    }

    #[test]
    fn render_session_event_json_for_web_captured_omits_oa_recovery_when_none() {
        let event = crate::session::SessionEvent::WebCaptured {
            url: "https://example.com".into(),
            title: "Example".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            body_preview: String::new(),
            language: "UNKNOWN".into(),
            media_type: "page".into(),
            oa_recovery: None,
        };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"web\""));
        assert!(!line.contains("\"oa_recovery\""));
    }

    #[test]
    fn render_session_event_json_for_contradiction_graph() {
        use crate::contradiction::{ContradictionClaim, ContradictionEdge};
        use crate::source::Source;
        use std::path::PathBuf;
        let src = Source::Web {
            url: "https://a.example".into(),
            title: "A".into(),
            captured_at: chrono::Utc::now(),
            published_at: None,
            body_path: PathBuf::new(),
            body: "improves performance".into(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
            author: None,
        };
        let edge = ContradictionEdge {
            claim_a: ContradictionClaim::from_source("claims better performance", 1, &src),
            claim_b: ContradictionClaim::from_source("claims worse performance", 2, &src),
            dimension: "performance".into(),
            note: "opposing performance claims".into(),
            strength: 50,
        };
        let event = crate::session::SessionEvent::Analysis(AnalysisEvent::ContradictionGraph {
            sources_scanned: 2,
            edges: vec![edge],
        });
        let line = render_session_event_json(&event);
        assert!(line.contains("contradiction_graph"));
        assert!(line.contains("\"sources_scanned\":2"));
        assert!(line.contains("performance"));
    }

    #[test]
    fn render_session_event_json_for_evidence_digest_and_triple_draft() {
        use crate::digest::{DigestClaim, DraftCandidate, EvidenceDigest, TripleDraft};
        let digest = EvidenceDigest {
            claims: vec![DigestClaim {
                text: "Evidence on Performance".into(),
                source_indices: vec![1, 2],
                support_count: 2,
                contested: true,
                note: "contested performance evidence".into(),
            }],
            sources_scanned: 2,
        };
        let event = crate::session::SessionEvent::Analysis(AnalysisEvent::EvidenceDigest {
            digest: digest.clone(),
        });
        let line = render_session_event_json(&event);
        assert!(line.contains("evidence_digest"));
        assert!(line.contains(r#""sources_scanned":2"#));
        assert!(line.contains("Performance"));

        let draft = TripleDraft {
            candidates: vec![DraftCandidate {
                label: "A".into(),
                body: "Consensus draft body.".into(),
                source_indices: vec![1],
                note: "consensus-leaning draft".into(),
            }],
        };
        let event = crate::session::SessionEvent::Analysis(AnalysisEvent::TripleDraft { draft });
        let line = render_session_event_json(&event);
        assert!(line.contains("triple_draft"));
        assert!(line.contains(r#""candidates""#));
        assert!(line.contains("Consensus draft body."));
    }

    #[test]
    fn render_session_event_json_for_corpus_critic_and_gap_fetch() {
        let report = crate::corpus_critic::CorpusCriticReport {
            score: 65,
            coverage_score: 70,
            evidence_score: 60,
            balance_score: 80,
            tension_score: 50,
            issues: vec!["shallow evidence on Cost".into()],
            gaps: vec!["Add cost evidence".into()],
            recommendations: Vec::new(),
            contested_ratio: 0,
            shallow_dimensions: vec!["Cost".into()],
            isolated_sources: Vec::new(),
            passed: true,
        };
        let event = crate::session::SessionEvent::Analysis(AnalysisEvent::CorpusCritic { report });
        let line = render_session_event_json(&event);
        assert!(line.contains("corpus_critic"));
        assert!(line.contains(r#""score":65"#));
        assert!(line.contains(r#""passed":true"#));

        let result = crate::corpus_critic::GapFetchResult {
            queries: vec!["AI coding agents cost evidence".into()],
            new_sources: 0,
            failed_queries: 0,
            attempted: true,
            note: "no web gatherer configured; gap-fill fetch skipped".into(),
        };
        let event = crate::session::SessionEvent::Analysis(AnalysisEvent::GapFetch { result });
        let line = render_session_event_json(&event);
        assert!(line.contains("gap_fetch"));
        assert!(line.contains(r#""attempted":true"#));
        assert!(line.contains("AI coding agents cost evidence"));
    }

    #[test]
    fn render_session_event_json_for_surgical_patch() {
        let result = crate::patcher::PatchResult {
            patches: vec![crate::patcher::SurgicalPatch {
                operation: "append_finding".to_string(),
                target: "Cost".to_string(),
                reason: "Coverage gap".to_string(),
                applied: true,
            }],
            patched_analysis: crate::analysis::AnalysisResult::default(),
            score_before: 55,
            score_after: 70,
            note: "Applied 1 patch".to_string(),
            patched_finding_count: 1,
            patched_implication_count: 0,
            patched_open_question_count: 1,
        };
        let event =
            crate::session::SessionEvent::Synthesis(SynthesisEvent::SurgicalPatch { result });
        let line = render_session_event_json(&event);
        assert!(line.contains("surgical_patch"));
        assert!(line.contains(r#""score_before":55"#));
        assert!(line.contains(r#""score_after":70"#));
        assert!(line.contains(r#""operation":"append_finding""#));
    }

    #[test]
    fn render_session_event_json_for_cite_check() {
        let result = crate::cite_checker::CitationCheckResult {
            passed: true,
            checked: 3,
            failed_claims: Vec::new(),
            issues: Vec::new(),
            gate_open: true,
        };
        let event = crate::session::SessionEvent::Synthesis(SynthesisEvent::CiteCheck {
            result: result.clone(),
        });
        let line = render_session_event_json(&event);
        assert!(line.contains("cite_check"));
        assert!(line.contains(r#""passed":true"#));
        assert!(line.contains(r#""checked":3"#));
        assert!(line.contains(r#""gate_open":true"#));

        let failed = crate::cite_checker::CitationCheckResult {
            passed: false,
            checked: 1,
            failed_claims: vec!["CITATION_VERIFICATION_FAILED: [#2] missing".into()],
            issues: vec!["[#2] unknown".into()],
            gate_open: false,
        };
        let event =
            crate::session::SessionEvent::Synthesis(SynthesisEvent::CiteCheck { result: failed });
        let line = render_session_event_json(&event);
        assert!(line.contains(r#""passed":false"#));
        assert!(line.contains("CITATION_VERIFICATION_FAILED"));
    }

    #[test]
    fn render_session_event_json_for_polish_and_readability_audit() {
        let polish = crate::readability::PolishResult {
            changes: vec![crate::readability::PolishChange {
                field: "summary".to_string(),
                description: "normalized whitespace".to_string(),
            }],
            control_chars_removed: 1,
            whitespace_normalized: 2,
            empty_paragraphs_removed: 3,
            note: "Polished".to_string(),
        };
        let event =
            crate::session::SessionEvent::Synthesis(SynthesisEvent::Polish { result: polish });
        let line = render_session_event_json(&event);
        assert!(line.contains("polish"));
        assert!(line.contains(r#""control_chars_removed":1"#));
        assert!(line.contains(r#""empty_paragraphs_removed":3"#));

        let audit = crate::readability::ReadabilityAudit {
            score: 85,
            passed: true,
            issues: vec!["issue".to_string()],
            recommendations: vec!["rec".to_string()],
            avg_finding_length: 400,
            missing_label_count: 0,
            long_paragraph_count: 0,
        };
        let event = crate::session::SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit {
            result: audit,
        });
        let line = render_session_event_json(&event);
        assert!(line.contains("readability_audit"));
        assert!(line.contains(r#""score":85"#));
        assert!(line.contains(r#""passed":true"#));
    }

    #[test]
    fn render_show_output_includes_oa_recovery_note() {
        let out = render_show_output(
            "oa-test",
            "OA Recovery Test",
            "topic",
            "complete",
            "2024-01-15T10:30:00Z",
            "2024-01-15T10:31:00Z",
            &[(
                "web".into(),
                "https://doi.org/10.1234/example".into(),
                "Example paper".into(),
                "2024-01-15T10:31:00Z".into(),
                Some("recovered from europepmc (https://pmc.ncbi.nlm.nih.gov/articles/PMC123456/); version=publishedVersion, license=CC-BY-4.0".into()),
            )],
        );
        assert!(out.contains("[OA recovery]"));
        assert!(out.contains("europepmc"));
        assert!(out.contains("publishedVersion"));
    }

    #[test]
    fn render_show_output_includes_metadata() {
        let out = render_show_output(
            "rust-async",
            "Rust Async",
            "async/await idioms",
            "complete",
            "2024-01-15T10:30:00Z",
            "2024-01-15T10:31:00Z",
            &[(
                "web".into(),
                "https://example.com".into(),
                "Example".into(),
                "2024-01-15T10:31:00Z".into(),
                None,
            )],
        );
        assert!(out.contains("rust-async"));
        assert!(out.contains("Rust Async"));
        assert!(out.contains("https://example.com"));
    }

    #[test]
    fn render_list_output_handles_empty() {
        let out = render_list_output(&[]);
        assert!(out.contains("NAME"));
    }

    #[test]
    fn truncate_short_string_passes_through() {
        assert_eq!(truncate("abc", 5), "abc");
    }

    #[test]
    fn truncate_long_string_ellipsises() {
        let s = "a".repeat(20);
        let t = truncate(&s, 5);
        assert_eq!(t.chars().count(), 5);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn parse_create_with_use_pdf_flag() {
        let cmd = ResearchCliCommand::parse("create foo a topic --use-pdf");
        match cmd {
            ResearchCliCommand::Create {
                name,
                topic,
                use_pdf,
                ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(topic, "a topic");
                assert!(use_pdf);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn parse_create_use_pdf_defaults_false() {
        let cmd = ResearchCliCommand::parse("create foo a topic");
        match cmd {
            ResearchCliCommand::Create { use_pdf, .. } => {
                assert!(!use_pdf);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
