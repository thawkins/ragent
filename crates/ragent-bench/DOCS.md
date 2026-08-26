# ragent-bench

Benchmark runner shared between the ragent TUI and CLI. Provides `/bench`
command parsing, benchmark data initialization/verification, mock and live
model runners, suite adapters, and workbook output.

## Workspace Dependencies

- ragent-agent
- ragent-llm
- ragent-tools-core
- ragent-types

## External Dependencies

- anyhow, serde, serde_json, chrono, uuid, sha2, tokio, tracing
- rust_xlsxwriter, futures, reqwest, flate2, async-trait, rustc-hash, calamine, tempfile

Dev-dependencies: criterion.

## Public API (crate root re-exports)

### Command parsing

- **BenchCommand** (enum) — Parsed `/bench` command (Help, List, Show, Init, Run, Status, OpenLast, Cancel).
- **BenchInitTarget** (enum) — Initialization target (Suite, All, Full).
- **BenchInitMode** (enum) — Initialization mode (Sample, Full).
- **BenchTarget** (enum) — Run target (Suite, Profile, All).
- **BenchRunOptions** (struct) — Options for `/bench run` (limit, samples, subset, release, scenario, language, temperature, top_p, max_tokens, deterministic, since, until, resume, no_exec, yes).
- **parse_bench_command** (fn) — Parse `/bench` arguments into a `BenchCommand`.

### Data layer

- **BenchCaseFixture** (struct) — One initialized benchmark case fixture.
- **BenchDataFile** (struct) — File metadata in the data manifest.
- **BenchDataManifest** (struct) — On-disk manifest for initialized benchmark data.
- **BenchDataSource** (struct) — Source metadata in the manifest.
- **BenchInitOutcome** (struct) — Outcome of initializing/verifying one suite/language.
- **BenchInitProgressEvent** (enum) — Progress event during `/bench init`.
- **bench_data_root** / **bench_data_root_for_language** (fns) — Build data root paths.
- **init_suite** / **init_suite_with_language** / **init_target** / **init_target_with_progress** (fns) — Initialize/verify benchmark data.
- **load_manifest** (fn) — Load a manifest from a data root.
- **verify_suite** / **verify_suite_with_language** (fns) — Verify benchmark data without mutating.

### Mock LLM

- **MockLlmClient** (struct) — Mock `LlmClient` that replays a `MockLlmScript` on every `chat()`.
- **MockLlmScript** (struct) — Canned `StreamEvent` sequence.
- **mock_llm_client** (fn) — Construct an `Arc<dyn LlmClient>` from a script.

### Model runner

- **BenchGeneratedSample** (struct) — One generated sample (text, tokens, finish_reason, duration).
- **BenchGenerationResult** (struct) — Model runner output for one case.
- **BenchModelRunner** (trait) — Synchronous benchmark model runner (selection, generate).
- **LiveBenchModelRunner** (struct) — Live runner backed by the provider registry.
- **MockBenchModelRunner** (struct) — Mock runner for tests.
- **ResolvedModelSelection** (struct) — Resolved provider/model selection.
- **resolve_model_context** / **resolve_selected_model** (fns) — Resolve model selections.
- **slugify_path_segment** (fn) — Filesystem-safe slug.

### Registry

- **BenchSuiteDef** (struct) — Static benchmark suite metadata.
- **BenchProfileDef** (struct) — Static benchmark profile metadata.
- **all_profiles** / **all_suites** (fns) — Return all registered profiles/suites.
- **expand_target** (fn) — Expand a `BenchTarget` into concrete `BenchSuiteDef`s.
- **find_profile** / **find_suite** (fns) — Look up by ID.
- **requires_confirmation** (fn) — Whether a target requires `--yes`.
- **resolve_suite_and_language** / **resolve_suite_language** (fns) — Resolve suite and language.

### Runner

- **BenchProgressHandle** (struct) — Shared progress+event handle for UI updates.
- **BenchRunEvent** (enum) — Incremental run event for UI progress.
- **BenchRunOutcome** (struct) — Outcome of a completed run.
- **BenchRunProgress** (struct) — Snapshot of active progress.
- **run_target** / **run_target_with_progress** (fns) — Execute a benchmark run.
- **validate_run_prerequisites** (fn) — Validate before a run.

### Suites (adapters)

- **BenchCaseEvaluation** (struct) — Evaluation result for one case.
- **BenchMetricEvaluation** (struct) — Normalized suite summary metric.
- **BenchSuiteAdapter** (trait) — Suite-specific adapter (suite_id, build_prompt, evaluate_case, summarize).
- **adapter_for_suite** (fn) — Resolve the adapter for one suite ID.
- Suite submodules: `apps`, `bigcodebench`, `crosscodeeval`, `ds1000`, `humaneval`, `livecodebench`, `mbpp`, `multipl_e`, `repobench`, `swebench`.

### Workbook

- **RUN_SHEET** / **METRICS_SHEET** / **CASES_SHEET** / **ARTIFACTS_SHEET** (consts) — Fixed sheet names.
- **RUN_COLUMNS** / **METRICS_COLUMNS** / **CASES_COLUMNS** / **ARTIFACTS_COLUMNS** (consts) — Fixed column headers.
- **BenchRunConfig** / **BenchResultSummary** / **BenchCaseResult** / **BenchArtifactRecord** (structs) — Workbook row types.
- **workbook_output_path** / **workbook_debug_sidecar_path** / **workbook_resume_state_path** (fns) — Path builders.
- **write_benchmark_workbook** (fn) — Write a benchmark workbook with fixed schema.

## Module: command

- **BenchInitTarget** / **BenchInitMode** / **BenchTarget** / **BenchRunOptions** / **BenchCommand** (types).
- **parse_bench_command** (fn).

## Module: data

- **BenchCaseFixture** / **BenchDataManifest** / **BenchDataSource** / **BenchDataFile** / **BenchInitOutcome** / **BenchInitProgressEvent** (types).
- **init_target** / **init_target_with_progress** / **bench_data_root** / **bench_data_root_for_language** / **load_manifest** / **verify_suite** / **verify_suite_with_language** / **init_suite** / **init_suite_with_language** (fns).

## Module: mock

- **MockLlmScript** / **MockLlmClient** (structs), **mock_llm_client** (fn).

## Module: model

- **ResolvedModelSelection** / **BenchGeneratedSample** / **BenchGenerationResult** / **BenchModelRunner** (trait) / **LiveBenchModelRunner** / **MockBenchModelRunner** (structs).
- **slugify_path_segment** / **resolve_selected_model** / **resolve_model_context** (fns).

## Module: registry

- **BenchSuiteDef** / **BenchProfileDef** (structs).
- **find_suite** / **find_profile** / **resolve_suite_language** / **resolve_suite_and_language** / **expand_target** / **requires_confirmation** / **all_profiles** / **all_suites** (fns).

## Module: runner

- **BenchRunOutcome** / **BenchRunProgress** / **BenchRunEvent** / **BenchProgressHandle** (structs/enums).
- **validate_run_prerequisites** / **run_target** / **run_target_with_progress** (fns).

## Module: suites

- **BenchCaseEvaluation** / **BenchMetricEvaluation** (structs).
- **BenchSuiteAdapter** (trait), **adapter_for_suite** (fn).
- Submodules: `apps`, `bigcodebench`, `crosscodeeval`, `ds1000`, `humaneval`, `livecodebench`, `mbpp`, `multipl_e`, `repobench`, `swebench`.

## Module: workbook

- Sheet/column constants, row structs, path builder functions, **write_benchmark_workbook** (fn).