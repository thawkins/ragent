# Benchmark Slash Command Plan

## Goal

Add a benchmark slash command that runs a curated set of major coding-AI benchmarks against the **currently selected provider/model**, stores results under:

`[PROJECT_ROOT]/benches/[benchname]/[YYYY-MM-DD UTC]/[provider]/[model].xlsx`

and writes the benchmark result as a native Excel workbook using the repo's internal XLSX writing code.

---

## Assumptions

1. **Slash command name:** `/bench`
2. **Primary output file:** `[model].xlsx`
3. **Date folder:** UTC `YYYY-MM-DD`
4. **Model file name:** sanitized slug of the selected model ID with `.xlsx`; raw provider/model are preserved in workbook metadata
5. **Execution model:** long-running benchmark runs execute as background tasks and surface progress in the TUI log/status/task list
6. **Benchmark scope:** use public benchmark definitions, datasets, and scoring rules, but execute generation, test orchestration, and scoring inside `ragent`
7. **Default behavior:** do **not** run all full suites by default; default to a curated “quick” profile and require explicit opt-in for full or expensive suites
8. **Workbook consistency:** every benchmark workbook uses the same sheet names and column schema so files can be compared directly without per-suite reshaping

---

## Top 10 Benchmark Set

This plan uses a practical top 10 that are widely cited, strongly coding-focused, and have public datasets or documented evaluation procedures that `ragent` can implement natively.

| # | Benchmark | What it measures | How it is run | Primary metrics | Official source |
|---|---|---|---|---|---|
| 1 | **HumanEval** | Function-level Python code generation | `ragent` loads tasks, generates completions, executes hidden tests in its own sandbox, and computes pass@k | `pass@1`, `pass@10`, `pass@100` | OpenAI HumanEval repo |
| 2 | **MBPP** | Basic Python problem solving | `ragent` prompts on short Python tasks, runs bundled assertions in its own sandbox, and aggregates pass rate | pass rate / accuracy | Google MBPP dataset card |
| 3 | **APPS** | Competitive-programming style coding problems | `ragent` loads APPS tasks, runs generated programs against public/private cases, and computes accuracy and optional BLEU locally | accuracy, BLEU | APPS repo + eval README |
| 4 | **DS-1000** | Realistic data-science code generation | `ragent` fills insertion prompts, runs testcase and constraint checks, and scores natively | accuracy | DS-1000 project page |
| 5 | **MultiPL-E** | Multi-language code generation | `ragent` loads translated tasks, invokes language-specific runtimes in its own sandbox, and computes functional correctness | pass@k / functional correctness | MultiPL-E repo |
| 6 | **SWE-bench Verified / Lite** | Real-world issue resolution with patches | `ragent` materializes task repos, applies generated patches, runs project tests in isolated workdirs, and computes resolution rate | resolution rate, instances resolved | SWE-bench docs |
| 7 | **LiveCodeBench** | Contamination-aware coding evaluation over time | `ragent` filters by release/scenario, runs native generation and evaluation flows, and computes scenario metrics | `pass@1`, `pass@5`, scenario scores | LiveCodeBench repo |
| 8 | **RepoBench** | Repository-level code completion | `ragent` reconstructs repository prompts, scores completions locally, and computes EM / Edit Similarity / CodeBLEU | EM, ES, CodeBLEU | RepoBench repo |
| 9 | **CrossCodeEval** | Cross-file code completion with retrieval/context | `ragent` builds prompt contexts, generates predictions, and computes official-style completion metrics itself | completion metrics from benchmark spec | CrossCodeEval repo |
| 10 | **BigCodeBench** | Practical, challenging code generation | `ragent` executes the benchmark spec internally, including prompt shaping, sandboxed execution, and pass@k scoring | calibrated `pass@k` / leaderboard-compatible results | BigCodeBench repo |

### Notes by Benchmark

#### 1. HumanEval
- Vendor or download the dataset definitions, but evaluate inside `ragent`.
- `ragent` should:
  1. build the benchmark prompt from `task_id`,
  2. generate one or more completions,
  3. execute hidden tests in an isolated sandbox,
  4. compute `pass@k` with the benchmark formula.
- Safety requirement: execution must occur in a locked-down sandbox/container managed by `ragent`.

#### 2. MBPP
- Dataset provides prompts and tests.
- `ragent` should:
  1. load tasks,
  2. prompt the selected model,
  3. execute generated code in its sandbox,
  4. run bundled assertions,
  5. aggregate pass rate.
- Keep the execution model compatible with public MBPP semantics, but do not depend on an external evaluator.

#### 3. APPS
- Reimplement the APPS generation and scoring loop directly in `ragent`.
- `ragent` should:
  1. load problem statements and tests,
  2. generate candidate programs,
  3. execute them against APPS test cases,
  4. compute accuracy and optional text/code similarity metrics.
- APPS is heavy and should be exposed with both **sampled subset** and **full** modes.

#### 4. DS-1000
- Uses realistic StackOverflow-derived data-science tasks.
- Requires insertion/completion prompts and multi-criteria evaluation.
- First implementation should likely target the Python insertion-style prompt only.
- All checking logic should live in `ragent-bench`, with benchmark-specific constraint evaluators implemented in Rust.

#### 5. MultiPL-E
- `ragent` should ingest the translated task corpus and run evaluation itself.
- The benchmark adapter should:
  1. build language-specific prompts,
  2. select the requested target language,
  3. execute generated code in a language-aware sandbox,
  4. compute functional correctness without calling the upstream harness.

#### 6. SWE-bench Verified / Lite
- Patch-generation benchmark.
- Requires:
  - codebase snapshot,
  - issue text,
  - generated patch,
  - isolated repo test execution.
- MVP should target **Lite** first, then **Verified**.
- `ragent` should own patch application, test selection/execution, and resolution scoring rather than exporting predictions to an external evaluator.

#### 7. LiveCodeBench
- Supports multiple scenarios and release windows.
- MVP should support `codegeneration` first; other scenarios can be added behind flags.
- Release windows make this benchmark a good candidate for date/version flags in `/bench`.
- `ragent` should implement the scenario runners and scoring rules internally instead of delegating to `lcb_runner`.

#### 8. RepoBench
- Repository-level masked-line completion benchmark.
- Has explicit generation + evaluation semantics.
- Best stored as a completion benchmark with per-language/per-setting metadata.
- Metrics should be computed by `ragent` directly from stored predictions and gold targets.

#### 9. CrossCodeEval
- Cross-file completion benchmark with retrieval-context variants.
- Public benchmark materials describe prompt and scoring structure.
- Good candidate for provider-backed inference via a generated prompt file adapter.
- Retrieval, context construction, and scoring should all be native to `ragent-bench`.

#### 10. BigCodeBench
- Has a public benchmark specification and reproducible scoring rules.
- Strong fit for a native “suite runner + sandbox executor” abstraction in `ragent`.
- Should be treated as an advanced/expensive benchmark in the command UX.

---

## Slash Command Design

## Primary command

`/bench`

### Proposed subcommands

| Command | Purpose |
|---|---|
| `/bench list` | Show supported benchmark suites and profiles |
| `/bench init <benchmarkname>` | Create `benches/data/<benchmarkname>` and download/configure the dataset and metadata required for that benchmark |
| `/bench show` | Show current model/provider and benchmark defaults |
| `/bench run <suite-or-profile>` | Run one suite, a named profile, or `all` |
| `/bench status` | Show active/last benchmark run status |
| `/bench open last` | Show the latest workbook path |
| `/bench cancel` | Cancel the active benchmark task |

### Proposed syntax

```text
/bench init humaneval
/bench init swebench-lite
/bench run quick
/bench run humaneval
/bench run swebench-lite --limit 25
/bench run livecodebench --release release_v6 --scenario codegeneration
/bench run all --yes
```

### Supported arguments

| Flag | Meaning |
|---|---|
| `--limit <N>` | Limit number of benchmark items for this run |
| `--samples <K>` | Number of generations per case when benchmark supports pass@k |
| `--subset <name>` | Benchmark-specific subset (`lite`, `verified`, `hard`, etc.) |
| `--release <version>` | Dataset release/version (LiveCodeBench, etc.) |
| `--scenario <name>` | Scenario selector (`codegeneration`, `repair`, `execution`, etc.) |
| `--language <lang>` | Language selection for multilingual suites |
| `--force-download` | Re-download and rebuild initialized benchmark data |
| `--verify-only` | Check initialized benchmark data without downloading |
| `--since <YYYY-MM-DD>` / `--until <YYYY-MM-DD>` | Time slicing for date-based suites |
| `--resume` | Resume a partial run from prior artifacts |
| `--no-exec` | Generate outputs only, skip official execution/evaluation |
| `--yes` | Required for full or expensive benchmark profiles |

### Benchmark initialization

`/bench init <benchmarkname>` prepares the local benchmark data root:

```text
[PROJECT_ROOT]/benches/data/[benchmarkname]/
```

It should:

1. create the directory if missing,
2. download or materialize the benchmark dataset/spec files,
3. normalize them into the on-disk format expected by `ragent-bench`,
4. write benchmark-local metadata such as dataset revision and initialization time,
5. verify the initialized contents before reporting success.

`/bench run ...` should use these initialized data folders as the only supported dataset source for execution. If the required folder is missing or invalid, the command should fail fast with a message directing the user to run `/bench init <benchmarkname>`.

### Named profiles

| Profile | Suites | Purpose |
|---|---|---|
| `quick` | HumanEval, MBPP | Fast sanity check for selected model |
| `standard` | HumanEval, MBPP, DS-1000, RepoBench, CrossCodeEval | Balanced recurring benchmark profile |
| `agentic` | SWE-bench Lite, LiveCodeBench codegeneration | Real-world / workflow-oriented evaluation |
| `all` | All ten suites | Explicit opt-in only |

### Current model resolution

The command should resolve the same active model the TUI currently uses:

1. selected provider/model from TUI storage
2. effective thinking config
3. active agent name
4. current auth/provider base URL settings

The benchmark metadata should record:

- provider ID
- provider display name
- model ID
- model display name if known
- active thinking level/config
- benchmark command and flags
- timestamp(s)
- git commit SHA of the repo under test

---

## Result Storage Design

## Output path

Each benchmark run writes one workbook to:

```text
[PROJECT_ROOT]/benches/[benchname]/[YYYY-MM-DD UTC]/[provider]/[model].xlsx
```

Example:

```text
[PROJECT_ROOT]/benches/
  humaneval/
    2026-04-28/
      anthropic/
        claude-sonnet-4-20250514.xlsx
```

This path intentionally makes each workbook a single comparable artifact per benchmark, provider, model, and date.

### Benchmark data roots

Each benchmark also has a persistent initialized data directory:

```text
[PROJECT_ROOT]/benches/data/[benchmarkname]/
```

Example:

```text
[PROJECT_ROOT]/benches/
  data/
    humaneval/
      manifest.json
      dataset/
    swebench-lite/
      manifest.json
      dataset/
      repos/
```

`/bench init <benchmarkname>` owns creation and population of these folders. `/bench run ...` reads from them and should not perform implicit dataset downloads.

### Workbook writer

Use the existing internal Office XLSX writer rather than introducing a new spreadsheet stack:

- `crates/ragent-agent/src/tool/office_write.rs`
- `crates/ragent-tools-extended/src/office_write.rs`

The current `write_xlsx()` implementation already writes workbook/sheet/row data through `rust_xlsxwriter`. The benchmark plan should reuse or extract that internal writer for `ragent-bench` so benchmark exports follow the same internal Office path.

### Workbook schema

Every benchmark workbook should use the same sheet set and the same column order. Benchmark-specific details should be encoded as rows, key/value pairs, or normalized metric names rather than adding ad hoc columns.

#### Sheet 1: `run`

One row describing the benchmark run:

| Column | Meaning |
|---|---|
| `run_id` | Stable run identifier |
| `bench_name` | Canonical benchmark ID |
| `date_utc` | UTC date folder value |
| `started_at_utc` | Run start timestamp |
| `finished_at_utc` | Run end timestamp |
| `provider_id` | Canonical provider ID |
| `provider_name` | Provider display name |
| `model_id` | Raw model ID |
| `model_slug` | Sanitized model file stem |
| `thinking_enabled` | Effective thinking toggle |
| `thinking_level` | Effective thinking level |
| `thinking_budget_tokens` | Effective thinking budget if any |
| `command` | Slash command invocation |
| `status` | completed / failed / skipped / cancelled |
| `dataset_revision` | Dataset/spec revision used |
| `evaluator_version` | `ragent-bench` evaluator version |
| `git_commit_sha` | Repo commit SHA |
| `project_root` | Absolute project root |
| `data_root` | Initialized benchmark data directory |
| `data_revision` | Initialized data revision identifier |
| `notes` | Freeform warnings/notes summary |

#### Sheet 2: `metrics`

Normalized metric table:

| Column | Meaning |
|---|---|
| `run_id` | Links back to `run` sheet |
| `bench_name` | Benchmark ID |
| `metric_name` | Normalized metric name (`pass_at_1`, `accuracy`, `codebleu`, etc.) |
| `metric_value` | Numeric value |
| `metric_unit` | ratio / count / seconds / score |
| `split_name` | Optional dataset split |
| `subset_name` | Optional subset (`lite`, `verified`, `hard`) |
| `language` | Optional language |
| `sample_count` | Number of attempted cases/samples contributing |
| `passed_count` | Optional count |
| `failed_count` | Optional count |
| `skipped_count` | Optional count |
| `notes` | Metric notes |

#### Sheet 3: `cases`

One row per benchmark item or evaluation case:

| Column | Meaning |
|---|---|
| `run_id` | Links back to `run` sheet |
| `bench_name` | Benchmark ID |
| `case_id` | Benchmark task/case identifier |
| `case_index` | Numeric ordering index |
| `split_name` | Dataset split |
| `subset_name` | Dataset subset |
| `language` | Target language |
| `prompt_hash` | Hash of normalized prompt/context |
| `response_hash` | Hash of model output |
| `status` | passed / failed / skipped / error |
| `score` | Per-case numeric score if applicable |
| `duration_ms` | End-to-end case runtime |
| `tokens_input` | Prompt tokens |
| `tokens_output` | Completion tokens |
| `samples_generated` | Number of candidates generated |
| `sandbox_backend` | sandbox/runtime used |
| `error_code` | Normalized error code |
| `error_message` | Short error summary |
| `notes` | Additional case notes |

#### Sheet 4: `artifacts`

References to secondary artifacts without changing workbook schema:

| Column | Meaning |
|---|---|
| `run_id` | Links back to `run` sheet |
| `bench_name` | Benchmark ID |
| `artifact_kind` | log / patch / prompt / transcript / trace |
| `artifact_label` | Human-readable label |
| `relative_path` | Relative artifact path if sidecars are kept |
| `content_hash` | Artifact hash if available |
| `notes` | Artifact notes |

### Optional sidecar artifacts

The workbook is the primary result artifact. Optional sidecars may still be written beside it for debugging or replay:

- `[model].log`
- `artifacts/`

Sidecars are supplemental only; comparisons should rely on the workbook schema.

### Naming and sanitization rules

- `benchname`: keep canonical benchmark ID
- `provider`: keep canonical provider ID
- `model`: sanitize `/`, `:`, whitespace, and non-filesystem-safe characters to `_`
- preserve raw IDs in workbook cells instead of only in filenames

---

## Architecture Proposal

## New crate

Add a new workspace crate:

`crates/ragent-bench`

### Responsibilities

1. benchmark registry and suite metadata
2. benchmark run planning and option parsing
3. provider/model invocation abstraction
4. prompt/result adapters per suite
5. XLSX workbook writing and result aggregation
6. native execution and evaluation engines for benchmark suites

### Core modules

```text
crates/ragent-bench/src/
  lib.rs
  command.rs           # /bench argument model
  registry.rs          # benchmark definitions + profiles
  model_runner.rs      # invoke current provider/model
  results.rs           # normalized workbook row models
  workbook.rs          # XLSX schema + writer integration
  data.rs              # benchmark data roots, manifests, verification
  workspace.rs         # path creation, temp dirs, slugs
  suites/
    humaneval.rs
    mbpp.rs
    apps.rs
    ds1000.rs
    multipl_e.rs
    swebench.rs
    livecodebench.rs
    repobench.rs
    crosscodeeval.rs
    bigcodebench.rs
  execution/
    sandbox.rs         # isolated process/container execution helpers
    python.rs          # Python code execution + assertion runner
    repos.rs           # repo materialization, patch apply, targeted test runs
    metrics.rs         # pass@k, EM, Edit Similarity, CodeBLEU, resolution rate
    languages.rs       # language/runtime registry for multi-language suites
```

### Why a separate crate

- keeps benchmark orchestration out of TUI/UI code
- enables future CLI/HTTP reuse
- fits the repo’s extracted-crate architecture
- isolates heavyweight benchmark dependencies

---

## Integration Points

## TUI

### Files likely to change

- `crates/ragent-tui/src/app/state.rs`
  - add `/bench` slash command definition
- `crates/ragent-tui/src/app.rs`
  - parse and dispatch `/bench`
  - show current benchmark status and latest workbook path
- `crates/ragent-tui/tests/`
  - add slash-command coverage for `/bench init`, `/bench list`, `/bench run`, error handling, and status rendering

### UX behavior

- `/bench list` prints available suites and profiles
- `/bench init <benchmarkname>` creates/verifies `benches/data/<benchmarkname>` and reports dataset revision/status
- `/bench run ...` immediately creates a background task
- status bar/log show benchmark progress
- `/bench cancel` maps to task cancellation
- expensive/full benchmark runs require explicit confirmation or `--yes`

## Core runtime

Potential touch points:

- `src/main.rs`
  - wire `ragent-bench` into the binary
- `crates/ragent-agent`
  - expose a stable model invocation helper reusable by the benchmark runner
  - optionally reuse session/message plumbing or add a lightweight prompt-completion path
  - expose or share the existing internal XLSX writer used by the Office tools

### Data initialization

`ragent-bench` should own dataset initialization rather than scattering download logic across suites.

Recommended flow:

1. `registry.rs` defines per-benchmark data requirements.
2. `data.rs` resolves `[PROJECT_ROOT]/benches/data/[benchmarkname]`.
3. Each suite implements `init()` to download/normalize its dataset into that directory.
4. A per-benchmark manifest records dataset revision, source URL(s), checksum(s), and initialization status.
5. Run-time suite adapters consume only the initialized local directory.

## HTTP server (optional later)

Not required for MVP, but the core runner should be reusable from:

- future `POST /bench/run`
- future benchmark history endpoints

---

## Model Invocation Strategy

The benchmark runner should **not** pretend every benchmark is a chat session.

### Proposed abstraction

Add a model-facing helper that can:

1. resolve the current provider/model/auth/base URL
2. run deterministic or sampled completions
3. expose benchmark-friendly knobs:
   - temperature
   - top_p
   - `n`/samples
   - max tokens
   - thinking config

### Recommendation

Use a shared benchmark-facing `ModelRunner` abstraction rather than driving benchmarks through `SessionProcessor::process_message()`. That keeps benchmark prompts/results clean and avoids session-side tool/permission behavior unless a benchmark explicitly needs agentic execution.

For agentic benchmarks like SWE-bench, add a separate adapter that can optionally use the full agent loop later. MVP should still keep patch application, test execution, and scoring inside `ragent`.

---

## Execution and Safety Model

Many benchmark suites execute model-generated code. The implementation should separate:

1. **Prompt generation**
2. **Model inference**
3. **Evaluation/execution**

### Safety rules

- Run executable-code benchmarks inside controlled sandbox/container wrappers owned by `ragent`
- Allow `ragent` to use containers or restricted subprocesses as an implementation detail, but not upstream benchmark harnesses
- Write temp/intermediate files under:
  - `target/temp/bench/`
- Never execute untrusted code directly in the main app process

### Dependency management

The plan should treat datasets and language runtimes as explicit prerequisites for `ragent`'s native evaluator:

- initialized benchmark data under `[PROJECT_ROOT]/benches/data/[benchmarkname]`
- download permissions for `/bench init`
- required language runtimes/interpreters/compilers
- sandbox backend availability
- per-suite environment metadata

The slash command should surface **skipped** vs **failed** clearly when prerequisites are missing. Missing benchmark data should direct the user to `/bench init <benchmarkname>`.

---

## Benchmark Support Tiers

## Phase A — Local MVP

Implement first:

1. HumanEval
2. MBPP
3. DS-1000
4. RepoBench
5. CrossCodeEval

Reason:
- clear prompt/result shapes
- less repo-patching complexity
- good mix of function-level, data-science, and cross-file completion

## Phase B — Native Suite Expansion

Add:

6. APPS
7. MultiPL-E
8. LiveCodeBench

## Phase C — Agentic / patch-based

Add:

9. SWE-bench Lite
10. SWE-bench Verified
11. BigCodeBench full integration

Note: BigCodeBench can also be Phase B if its native runner lands before the patch-based suites.

---

## Proposed Implementation Phases

## Phase 1: Command, Data, and Workbook Schema

- create `ragent-bench` crate
- define benchmark registry
- define profiles
- define `BenchRunConfig`, `BenchSuite`, `BenchResultSummary`, `BenchCaseResult`
- define benchmark data manifests and initialization state
- add `/bench init <benchmarkname>` command parsing and validation
- define path builder for `[PROJECT_ROOT]/benches/[benchname]/[date]/[provider]/[model].xlsx`
- define path builder for `[PROJECT_ROOT]/benches/data/[benchmarkname]/`
- define the fixed workbook sheets and column schema
- add `/bench list`, `/bench init`, `/bench show`, `/bench run`, `/bench status`, `/bench cancel` command parsing only

## Phase 2: Model Runner

- add provider/model resolution helper reused by benchmarks
- make sampled completions configurable
- capture model metadata and effective thinking config
- add deterministic run options for reproducibility

## Phase 3: Dataset Initialization

- implement benchmark data roots under `[PROJECT_ROOT]/benches/data/[benchmarkname]/`
- implement per-suite `init()` flows
- write per-benchmark data manifests with revision/checksum/source metadata
- add verification and reinitialization support

## Phase 4: XLSX Result Writing

- create output directories
- reuse or extract the internal `write_xlsx()` path from the Office tooling
- map normalized run/metric/case/artifact rows into workbook sheets
- write `[model].xlsx` at the end of each benchmark run
- optionally write debug sidecars beside the workbook
- add resume support keyed by benchmark + model + date + config hash

## Phase 5: MVP Benchmark Adapters

- HumanEval adapter
- MBPP adapter
- DS-1000 adapter
- RepoBench adapter
- CrossCodeEval adapter

## Phase 6: Native Suite Runners

- APPS adapter
- MultiPL-E adapter
- LiveCodeBench adapter
- BigCodeBench adapter
- shared native metrics implementations for pass@k, CodeBLEU, Edit Similarity, and resolution scoring

## Phase 7: Agentic / patch-based adapters

- SWE-bench Lite adapter
- SWE-bench Verified adapter
- optional later: LiveCodeBench repair/execution/test-output scenarios

## Phase 8: TUI UX

- background-task integration
- progress messages
- cancel support
- latest-results display
- benchmark help text in `/help` and slash autocomplete

## Phase 9: Validation and Docs

- unit tests for benchmark data path building and manifest validation
- unit tests for path building, slugging, config parsing, manifest writing
- integration tests with fake model runners
- TUI slash-command tests for `/bench init`, `/bench run`, and missing-data failures
- docs in `SPEC.md`, `QUICKSTART.md`, and command help text

---

## Data Model Sketch

```json
{
  "run_id": "bench-2026-04-28T08:23:01Z-anthropic-claude-sonnet-4-20250514",
  "project_root": "/path/to/project",
  "date_utc": "2026-04-28",
  "provider_id": "anthropic",
  "model_id": "claude-sonnet-4-20250514",
  "model_slug": "claude-sonnet-4-20250514",
  "thinking": {
    "enabled": true,
    "level": "high",
    "budget_tokens": 16000
  },
  "command": "/bench run standard --samples 1",
  "data_root": "/path/to/project/benches/data/humaneval",
  "data_revision": "HumanEval-1.2",
  "suites": [
    {
      "id": "humaneval",
      "status": "completed",
      "metric": {
        "pass_at_1": 0.84
      },
      "result_file": "benches/humaneval/2026-04-28/anthropic/claude-sonnet-4-20250514.xlsx"
    }
  ],
  "overall": {
    "completed": 5,
    "failed": 0,
    "skipped": 2
  }
}
```

---

## Repo Files Likely to Change

### New

- `BENCHPLAN.md`
- `crates/ragent-bench/**`
- `crates/ragent-tui/tests/test_bench_command.rs`
- `crates/ragent-bench/tests/**`

### Existing

- `Cargo.toml`
- `src/main.rs`
- `crates/ragent-agent/src/tool/office_write.rs`
- `crates/ragent-tools-extended/src/office_write.rs`
- `crates/ragent-tui/src/app/state.rs`
- `crates/ragent-tui/src/app.rs`
- `crates/ragent-tui/tests/test_slash_commands.rs`
- `SPEC.md`
- `QUICKSTART.md`
- possibly `README.md`

---

## Test Plan

### Unit tests

- benchmark registry lookup
- benchmark data root path building
- benchmark init manifest parsing/verification
- profile expansion
- model slug sanitization
- UTC date folder generation
- workbook row serialization
- workbook sheet schema stability
- workbook path generation
- benchmark command argument parsing

### Integration tests

- `/bench init humaneval` creates `benches/data/humaneval`
- `/bench init humaneval --verify-only` reports initialized state without mutation
- `/bench list` output
- `/bench run humaneval` creates the benchmark workbook path
- fake model runner produces a valid `.xlsx` workbook
- resume/restart behavior
- missing runtime / missing dataset produces `skipped` with clear reason
- `/bench run humaneval` fails fast with a clear `/bench init humaneval` hint if data is absent
- workbook schema remains identical across HumanEval, MBPP, and SWE-bench sample outputs

### End-to-end smoke tests

- quick profile against a fake provider
- one real lightweight benchmark adapter behind feature flag or ignored test

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Benchmark runtime is too long for interactive use | High | Use background tasks and profiles (`quick`, `standard`, `all`) |
| Executing untrusted code is unsafe | High | Container/sandbox execution only |
| Public benchmark specs drift or evolve | Medium | Store dataset/spec revision in the workbook and version native evaluators explicitly |
| Dataset downloads are huge | Medium | Add prerequisite checks and explicit opt-in downloads |
| Initialization partially succeeds and leaves corrupt data | High | Use per-benchmark manifests, checksums, temp download dirs, and atomic finalize/rename |
| Spreadsheet schemas diverge between benchmarks | High | Lock sheet names and column order in one shared schema module and test against golden workbooks |
| Results are not reproducible | Medium | Record thinking config, temperature, samples, evaluator version, dataset revision, and commit SHA in every workbook |
| Selected model path differs between TUI and headless contexts | Medium | Centralize provider/model resolution in core runner |

---

## Recommended MVP Cut

If implementation needs to be staged, start with:

1. `/bench list`
2. `/bench init humaneval`
3. `/bench run quick`
4. HumanEval
5. MBPP
6. workbook creation at `[PROJECT_ROOT]/benches/[benchname]/[date]/[provider]/[model].xlsx`
7. background-task integration

Then expand to DS-1000 / RepoBench / CrossCodeEval before moving into APPS, LiveCodeBench, SWE-bench, and BigCodeBench.

This gives immediate value while preserving the larger top-10 design.

---

## Research References

- HumanEval: https://github.com/openai/human-eval
- MBPP dataset card: https://huggingface.co/datasets/google-research-datasets/mbpp
- APPS: https://github.com/hendrycks/apps
- DS-1000: https://ds1000-code-gen.github.io/
- MultiPL-E: https://github.com/nuprl/MultiPL-E
- SWE-bench: https://www.swebench.com/SWE-bench/
- SWE-bench evaluation guide: https://github.com/SWE-bench/SWE-bench/blob/main/docs/guides/evaluation.md
- LiveCodeBench: https://github.com/LiveCodeBench/LiveCodeBench
- RepoBench: https://github.com/Leolty/repobench
- CrossCodeEval: https://github.com/amazon-science/cceval
- BigCodeBench: https://github.com/bigcode-project/bigcodebench
