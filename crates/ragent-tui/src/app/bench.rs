//! Benchmark command handling for the TUI.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};





// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{
    LogLevel, App
};

// Helpers

// Re-export status types from theme

impl App {
    /// Poll the active benchmark task, refresh the status line with progress,
    /// drain progress events, and surface the finished result when complete.
    pub fn poll_pending_bench(&mut self) {
        if self.active_bench_task_id.is_some()
            && let Some(progress) = self
                .active_bench_progress
                .as_ref()
                .and_then(|handle| handle.snapshot())
        {
            self.status = format!(
                "⏳ bench: {} {}/{}",
                progress.suite_id, progress.completed_cases, progress.total_cases
            );
        }
        self.drain_bench_progress_events();
        let outcome = {
            let mut guard = match self.bench_result.lock() {
                Ok(g) => g,
                Err(poisoned) => {
                    tracing::error!("bench_result mutex poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            guard.take()
        };
        let Some(outcome) = outcome else { return };
        self.drain_bench_progress_events();

        if let Some(task_id) = self.active_bench_task_id.take()
            && let Some(idx) = self.active_tasks.iter().position(|task| task.id == task_id)
        {
            self.active_tasks.remove(idx);
        }
        self.active_bench_summary = None;
        self.active_bench_started_at = None;
        self.active_bench_cancel = None;
        if let Some(progress) = &self.active_bench_progress {
            progress.clear();
        }
        self.active_bench_progress = None;

        match outcome {
            Ok(run) => {
                self.bench_last_summary = Some(run.message.clone());
                self.bench_last_workbooks = run.workbook_paths.clone();
                self.bench_last_finished_at = Some(chrono::Utc::now());
                self.force_new_message = true;
                self.append_assistant_text(&run.message);
                self.status = "bench: done".to_string();
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!(
                        "Finished /bench run — {} workbook(s)",
                        run.workbook_paths.len()
                    ),
                );
            }
            Err(msg) => {
                self.bench_last_summary = Some(format!("Benchmark run failed: {msg}"));
                self.bench_last_finished_at = Some(chrono::Utc::now());
                self.status = format!("⚠ bench failed: {msg}");
                self.force_new_message = true;
                self.append_assistant_text(&format!("From: /bench run\n❌ {msg}"));
                self.push_log_no_agent(LogLevel::Warn, format!("bench error: {msg}"));
            }
        }
    }

    pub(crate) fn drain_bench_progress_events(&mut self) {
        if let Some(handle) = &self.active_bench_progress {
            let events = handle.drain_events();
            for event in events {
                self.force_new_message = true;
                self.append_assistant_text(&self.render_bench_run_event(&event));
            }
        }
    }

    pub(crate) fn render_bench_run_event(&self, event: &ragent_bench::BenchRunEvent) -> String {
        match event {
            ragent_bench::BenchRunEvent::SuiteStarted {
                suite_id,
                language,
                total_cases,
            } => {
                format!(
                    "From: /bench run\n⏳ Running `{suite_id}` [{language}] — {total_cases} case(s)."
                )
            }
            ragent_bench::BenchRunEvent::CaseFinished {
                suite_id,
                language,
                case_id,
                status,
            } => {
                let icon = if status == "passed" { "✅" } else { "❌" };
                format!(
                    "From: /bench run\n{icon} `{suite_id}` [{language}] case `{case_id}` -> `{status}`."
                )
            }
        }
    }

    pub(crate) fn render_bench_init_event(&self, event: &ragent_bench::BenchInitProgressEvent) -> String {
        match event {
            ragent_bench::BenchInitProgressEvent::Starting {
                suite_id,
                language,
                mode,
                verify_only,
            } => {
                let action = if *verify_only {
                    "Verifying"
                } else if matches!(mode, ragent_bench::BenchInitMode::Full) {
                    "Loading full benchmark data for"
                } else {
                    "Loading benchmark data for"
                };
                format!("From: /bench init\n⏳ {action} `{suite_id}` [{language}]…")
            }
            ragent_bench::BenchInitProgressEvent::Finished {
                suite_id,
                language,
                verify_only,
                case_count,
                data_root,
                ..
            } => {
                let action = if *verify_only { "Verified" } else { "Loaded" };
                format!(
                    "From: /bench init\n✅ {action} `{suite_id}` [{language}] at `{}` ({} case(s)).",
                    data_root.display(),
                    case_count
                )
            }
        }
    }

    pub(crate) fn render_bench_list(&self) -> String {
        let mut output = String::from("From: /bench list\n## Benchmark Suites\n\n");
        output.push_str(
            "| suite | description | default | languages | language data | revision |\n| --- | --- | --- | --- | --- | --- |\n",
        );
        for suite in ragent_bench::all_suites() {
            let languages = suite.languages.join(", ");
            let local_partition = if suite.languages.len() > 1 {
                format!("local partitions: `benches/data/{}/<language>`", suite.id)
            } else {
                format!(
                    "local partition: `benches/data/{}/{}`",
                    suite.id, suite.default_language
                )
            };
            let language_data = if suite.language_source_note.is_empty() {
                local_partition
            } else {
                format!("{local_partition}; {}", suite.language_source_note)
            };
            output.push_str(&format!(
                "| `{}` | {} | `{}` | `{}` | {} | `{}` |\n",
                suite.id,
                suite.description,
                suite.default_language,
                languages,
                language_data,
                suite.revision
            ));
        }
        output.push_str("\n## Virtual Targets\n\n");
        output.push_str("| target | expands to | notes |\n| --- | --- | --- |\n");
        output.push_str(&format!(
            "| `all` | `{}` registered suites | Initializes or runs every known benchmark suite; `/bench init all --full` uses full ingestion where available and sample fixtures elsewhere. |\n",
            ragent_bench::all_suites().len()
        ));
        output.push_str(
            "| `full` | all suites, full upstream datasets | `/bench init full` is reserved for complete dataset ingestion and stays gated until every suite supports it. |\n",
        );
        output.push_str("\n## Profiles\n\n");
        output.push_str("| profile | suites | notes |\n| --- | --- | --- |\n");
        for profile in ragent_bench::all_profiles() {
            let suites = if profile.suites.is_empty() {
                "(none yet)".to_string()
            } else {
                profile.suites.join(", ")
            };
            let notes = if profile.expensive {
                format!("{} Requires `--yes`.", profile.description)
            } else {
                profile.description.to_string()
            };
            output.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                profile.id, suites, notes
            ));
        }
        output
    }

    pub(crate) fn render_bench_show(&self) -> String {
        let selected_model = self
            .selected_model
            .clone()
            .unwrap_or_else(|| "(not selected)".to_string());
        let last = if self.bench_last_workbooks.is_empty() {
            "(none)".to_string()
        } else {
            self.bench_last_workbooks
                .iter()
                .map(|path| format!("`{}`", path.display()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "From: /bench show\n## Benchmark Defaults\n\n\
             - **Selected model:** `{selected_model}`\n\
             - **Virtual all target:** every registered benchmark suite\n\
             - **Virtual full target:** full upstream dataset ingestion for every suite (gated until all suites support it)\n\
             - **Quick profile:** `humaneval`, `mbpp`\n\
             - **Standard profile:** `humaneval`, `mbpp`, `ds1000`, `repobench`, `crosscodeeval`\n\
             - **Agentic profile:** `swebench-lite`, `livecodebench`\n\
             - **Last workbook(s):** {last}\n"
        )
    }

    pub(crate) fn render_bench_status(&self) -> String {
        if let Some(task_id) = &self.active_bench_task_id {
            let summary = self
                .active_bench_summary
                .as_deref()
                .unwrap_or("benchmark task running");
            let started = self
                .active_bench_started_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| "(unknown)".to_string());
            let cancellation = if self
                .active_bench_cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                "\n- **Cancellation:** requested"
            } else {
                ""
            };
            let progress = self
                .active_bench_progress
                .as_ref()
                .and_then(ragent_bench::BenchProgressHandle::snapshot)
                .map(|progress| {
                    format!(
                        "\n- **Progress:** suite `{}` ({}/{}) — case `{}/{}`",
                        progress.suite_id,
                        progress.suite_index,
                        progress.total_suites,
                        progress.completed_cases,
                        progress.total_cases
                    )
                })
                .unwrap_or_default();
            return format!(
                "From: /bench status\n## Active Benchmark Run\n\n- **Task ID:** `{}`\n- **Status:** `running`\n- **Summary:** {}\n- **Started:** `{}`{}{}\n",
                task_id, summary, started, progress, cancellation
            );
        }
        if let Some(summary) = &self.bench_last_summary {
            let finished = self
                .bench_last_finished_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| "(unknown)".to_string());
            return format!(
                "From: /bench status\n## Last Benchmark Run\n\n- **Finished:** `{finished}`\n- **Workbook count:** `{}`\n\n{}",
                self.bench_last_workbooks.len(),
                summary
            );
        }
        "From: /bench status\nNo benchmark runs yet.".to_string()
    }

    pub(crate) fn render_bench_open_last(&self) -> String {
        if self.bench_last_workbooks.is_empty() {
            return "From: /bench open last\nNo benchmark workbooks available yet.".to_string();
        }
        let mut output = String::from("From: /bench open last\n## Latest Benchmark Results\n\n");
        for path in &self.bench_last_workbooks {
            output.push_str(&format!("- `{}`\n", path.display()));
        }
        if let Some(summary) = &self.bench_last_summary {
            output.push_str("\n");
            output.push_str(summary);
        }
        output
    }

    pub(crate) fn start_bench_run(
        &mut self,
        raw_command: &str,
        target: ragent_bench::BenchTarget,
        options: ragent_bench::BenchRunOptions,
    ) {
        if self.active_bench_task_id.is_some() {
            self.status = "⚠ A benchmark run is already active.".to_string();
            return;
        }

        let selected_model = match self.selected_model.as_deref() {
            Some(model) => model,
            None => {
                self.status = "⚠ /bench run requires a configured model — use /model".to_string();
                return;
            }
        };
        let config = ragent_agent::Config::load().unwrap_or_default();
        let selection = match ragent_bench::resolve_model_context(
            selected_model,
            self.provider_registry.as_ref(),
            self.storage.as_ref(),
            &config,
            self.effective_thinking_config_for_agent(&self.agent_info),
        ) {
            Ok(selection) => selection,
            Err(e) => {
                self.status = format!("⚠ Invalid model selection: {e}");
                return;
            }
        };

        let project_root = match std::env::current_dir() {
            Ok(path) => path,
            Err(e) => {
                self.status = format!("⚠ Could not resolve current directory: {e}");
                return;
            }
        };

        if let Err(e) = ragent_bench::validate_run_prerequisites(&project_root, &target, &options) {
            self.status = format!("⚠ {e}");
            self.append_assistant_text(&format!("From: /bench run\n❌ {e}"));
            return;
        }

        let task_id = format!("bench-{}", chrono::Utc::now().timestamp_millis());
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = ragent_bench::BenchProgressHandle::default();
        let target_label = match &target {
            ragent_bench::BenchTarget::Suite(id) => id.as_str(),
            ragent_bench::BenchTarget::Profile(id) => id.as_str(),
            ragent_bench::BenchTarget::All => "all",
        };
        self.active_bench_task_id = Some(task_id.clone());
        self.active_bench_summary = Some(format!(
            "`{target_label}` on `{}/{}`",
            selection.provider_id, selection.model_id
        ));
        self.active_bench_started_at = Some(chrono::Utc::now());
        self.active_bench_cancel = Some(cancel.clone());
        self.active_bench_progress = Some(progress.clone());
        self.status = "⏳ bench: running…".to_string();
        self.push_log_no_agent(LogLevel::Info, format!("benchmark task started: {task_id}"));
        self.append_assistant_text(&format!(
            "From: /bench run\n⏳ Started benchmark run for `{}` on `{}/{}.`\n\n- **Task ID:** `{}`\n- **Use:** `/bench status` for progress, `/bench cancel` to stop, `/bench open last` after completion.",
            target_label,
            selection.provider_id,
            selection.model_id,
            task_id
        ));

        let entry = ragent_agent::task::TaskEntry {
            id: task_id,
            parent_session_id: self.session_id.clone().unwrap_or_default(),
            child_session_id: "bench".to_string(),
            agent_name: "bench".to_string(),
            task_prompt: raw_command.to_string(),
            background: true,
            status: ragent_agent::task::TaskStatus::Running,
            result: Some("benchmark run in progress".to_string()),
            error: None,
            created_at: chrono::Utc::now(),
            completed_at: None,
            reported: false,
            waiter_count: 0,
        };
        self.active_tasks.push(entry);

        let bench_result = Arc::clone(&self.bench_result);
        let raw_command = raw_command.to_string();
        let provider_registry = Arc::clone(&self.provider_registry);
        let storage = Arc::clone(&self.storage);
        let mock_outputs = self.bench_mock_outputs.clone();
        let progress_for_thread = progress.clone();
        std::thread::spawn(move || {
            let model_runner: Result<Box<dyn ragent_bench::BenchModelRunner>, String> =
                if let Some(outputs) = mock_outputs {
                    Ok(Box::new(ragent_bench::MockBenchModelRunner::new(
                        selection.clone(),
                        outputs,
                    )))
                } else {
                    ragent_bench::LiveBenchModelRunner::new(
                        selection.clone(),
                        provider_registry,
                        storage,
                    )
                    .map(|runner| Box::new(runner) as Box<dyn ragent_bench::BenchModelRunner>)
                    .map_err(|e| e.to_string())
                };
            let outcome = model_runner.and_then(|runner| {
                ragent_bench::run_target_with_progress(
                    &project_root,
                    runner.as_ref(),
                    &raw_command,
                    &target,
                    &options,
                    &cancel,
                    Some(&progress_for_thread),
                )
                .map_err(|e| e.to_string())
            });
            match bench_result.lock() {
                Ok(mut guard) => {
                    *guard = Some(outcome);
                }
                Err(poisoned) => {
                    let mut guard = poisoned.into_inner();
                    *guard = Some(Err("benchmark result lock poisoned".to_string()));
                }
            }
        });
    }

}
