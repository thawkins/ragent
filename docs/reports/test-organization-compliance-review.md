# Test Organization Compliance Review

**Project:** ragent (`/home/thawkins/Projects/ragent`)  
**Guidelines reviewed:** `AGENTS-RUST.md` § Test Organization (lines 32–45)  
**Scope:** all `crates/*` directories + root `src/`

---

## 1. Executive Summary

| Metric | Count |
|--------|------:|
| Total `#[test]` / `#[tokio::test]` functions across all crates | **6,468** |
| Total inline `#[cfg(test)]` modules in `src/**/*.rs` | **117** |
| Inline modules that are *correct* `#[path]` includes of external files | **15** |
| Inline modules that are *still inline* (non-compliant) | **102** |
| Total external test files in `crates/*/tests/` | **~450** |
| Root-level inline `#[cfg(test)]` in `src/panic_hook.rs` | **1** |
| Test functions with non-`test_` / `bench_` names | **852** |

**Verdict:** The project has a large, well-populated external `tests/` directory per crate, but it is **not fully compliant** with `AGENTS-RUST.md`. The guideline states *“Do not add new inline `#[cfg(test)]` modules to library source files. All new tests go in `tests/`.”* Over **100 source files still contain inline test modules**, many with dozens of tests. In addition, **one root-level `src/` file** (`src/panic_hook.rs`) contains an inline test module despite the workspace root exception that root tests should be in `tests/`.

---

## 2. Inline `#[cfg(test)]` Modules in `src/**/*.rs`

The following 117 source files still contain a `#[cfg(test)]` module. Files marked with ✅ already use the approved `#[path]` shim to include an external test file; the remainder are inline and should be migrated.

### `ragent-agent` (27 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-agent/src/compaction/convert.rs` | 183 | ✅ `#[path]` include |
| `crates/ragent-agent/src/compaction/estimator.rs` | 291 | ✅ `#[path]` include |
| `crates/ragent-agent/src/compaction/runner.rs` | 581 | ✅ `#[path]` include |
| `crates/ragent-agent/src/reference/parse.rs` | 141 | ✅ `#[path]` include |
| `crates/ragent-agent/src/skill/args.rs` | 188 | ✅ `#[path]` include |
| `crates/ragent-agent/src/skill/context.rs` | 336 | ✅ `#[path]` include |
| `crates/ragent-agent/src/skill/loader.rs` | 439 | ✅ `#[path]` include |
| `crates/ragent-agent/src/skill/mod.rs` | 504 | ✅ `#[path]` include |
| `crates/ragent-agent/src/goal/mod.rs` | 302 | ⚠️ inline |
| `crates/ragent-agent/src/loop_state.rs` | 428 | ⚠️ inline |
| `crates/ragent-agent/src/mcp/discovery.rs` | 567 | ⚠️ inline |
| `crates/ragent-agent/src/mcp/http.rs` | 330 | ⚠️ inline |
| `crates/ragent-agent/src/orchestrator/mod.rs` | 30 | ⚠️ inline |
| `crates/ragent-agent/src/perf/mod.rs` | 185 | ⚠️ inline |
| `crates/ragent-agent/src/reference/resolve.rs` | 415 | ⚠️ inline |
| `crates/ragent-agent/src/research_adapter.rs` | 291, 423, 573, 1078 | ⚠️ inline (multiple cfg blocks) |
| `crates/ragent-agent/src/session/archive.rs` | 690 | ⚠️ inline |
| `crates/ragent-agent/src/skill/bundled.rs` | 256 | ⚠️ inline |
| `crates/ragent-agent/src/skill/invoke.rs` | 321 | ⚠️ inline |
| `crates/ragent-agent/src/task/mod.rs` | 965 | ⚠️ inline |
| `crates/ragent-agent/src/template/mod.rs` | 427 | ⚠️ inline |
| `crates/ragent-agent/src/tool/new_agent.rs` | 209 | ⚠️ inline |
| `crates/ragent-agent/src/tool/team_memory_read.rs` | 176 | ⚠️ inline |
| `crates/ragent-agent/src/tool/team_memory_write.rs` | 222 | ⚠️ inline |
| `crates/ragent-agent/src/trigger/dynamic.rs` | 512 | ⚠️ inline |
| `crates/ragent-agent/src/trigger/mcp_notification.rs` | 486 | ⚠️ inline |
| `crates/ragent-agent/src/trigger/runtime.rs` | 306 | ⚠️ inline |

### `ragent-bench` (3 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-bench/src/data.rs` | 803, 1326, 1443 | ⚠️ inline |
| `crates/ragent-bench/src/model.rs` | 573 | ⚠️ inline |
| `crates/ragent-bench/src/suites/mod.rs` | 189 | ⚠️ inline |

### `ragent-config` (1 module)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-config/src/telemetry.rs` | 274 | ⚠️ inline |

### `ragent-llm` (16 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-llm/src/providers/huggingface.rs` | 954 | ✅ `#[path]` include |
| `crates/ragent-llm/src/providers/router_classifier.rs` | 958, 962 | ✅ `#[path]` include |
| `crates/ragent-llm/src/providers/router_modifiers.rs` | 109, 110 | ✅ `#[path]` include |
| `crates/ragent-llm/src/providers/xai.rs` | 26 | ✅ `#[path]` include |
| `crates/ragent-llm/src/providers/anthropic.rs` | 613 | ⚠️ inline |
| `crates/ragent-llm/src/providers/bedrock.rs` | 1252 | ⚠️ inline |
| `crates/ragent-llm/src/providers/bedrock_credentials.rs` | 296 | ⚠️ inline |
| `crates/ragent-llm/src/providers/bedrock_sigv4.rs` | 257 | ⚠️ inline |
| `crates/ragent-llm/src/providers/copilot.rs` | 1535 | ⚠️ inline |
| `crates/ragent-llm/src/providers/gemini.rs` | 689 | ⚠️ inline |
| `crates/ragent-llm/src/providers/ollama.rs` | 751 | ⚠️ inline |
| `crates/ragent-llm/src/providers/ollama_cloud.rs` | 872 | ⚠️ inline |
| `crates/ragent-llm/src/providers/openai.rs` | 27 | ⚠️ inline |
| `crates/ragent-llm/src/providers/openai_responses.rs` | 617 | ⚠️ inline |
| `crates/ragent-llm/src/providers/thinking.rs` | 332 | ⚠️ inline |
| `crates/ragent-llm/src/providers/tool_cache.rs` | 328 | ⚠️ inline |

### `ragent-research` (36 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-research/src/adaptive.rs` | 127 | ⚠️ inline |
| `crates/ragent-research/src/analysis.rs` | 785 | ⚠️ inline |
| `crates/ragent-research/src/chapter.rs` | 166 | ⚠️ inline |
| `crates/ragent-research/src/cite_checker.rs` | 142 | ⚠️ inline |
| `crates/ragent-research/src/cli.rs` | 1096 | ⚠️ inline |
| `crates/ragent-research/src/contradiction.rs` | 376 | ⚠�� inline |
| `crates/ragent-research/src/corpus_critic.rs` | 423 | ⚠️ inline |
| `crates/ragent-research/src/diagram.rs` | 377 | ⚠️ inline |
| `crates/ragent-research/src/digest.rs` | 435 | ⚠️ inline |
| `crates/ragent-research/src/document.rs` | 2305 | ⚠️ inline (71 tests) |
| `crates/ragent-research/src/engine.rs` | 499 | ⚠️ inline |
| `crates/ragent-research/src/gather_log.rs` | 157 | ⚠️ inline |
| `crates/ragent-research/src/io.rs` | 393 | ⚠️ inline |
| `crates/ragent-research/src/item.rs` | 597 | ⚠️ inline (43 tests) |
| `crates/ragent-research/src/local_gatherer.rs` | 638 | ⚠️ inline |
| `crates/ragent-research/src/locus.rs` | 266 | ⚠️ inline |
| `crates/ragent-research/src/manager.rs` | 967, 1549 | ⚠️ inline (two modules) |
| `crates/ragent-research/src/open_access.rs` | 371 | ⚠️ inline |
| `crates/ragent-research/src/patcher.rs` | 511 | ⚠️ inline |
| `crates/ragent-research/src/plan_dep.rs` | 216 | ⚠️ inline |
| `crates/ragent-research/src/planner.rs` | 302 | ⚠️ inline |
| `crates/ragent-research/src/readability.rs` | 340 | ⚠️ inline |
| `crates/ragent-research/src/reconcile.rs` | 321 | ⚠️ inline |
| `crates/ragent-research/src/research_name.rs` | 276 | ⚠️ inline |
| `crates/ragent-research/src/run_config.rs` | 210 | ⚠️ inline |
| `crates/ragent-research/src/run_manifest.rs` | 433 | ⚠️ inline |
| `crates/ragent-research/src/session.rs` | 2218 | ⚠️ inline (33 tests) |
| `crates/ragent-research/src/source.rs` | 405 | ⚠️ inline |
| `crates/ragent-research/src/source_registry.rs` | 62 | ⚠️ inline |
| `crates/ragent-research/src/state.rs` | 351 | ⚠️ inline |
| `crates/ragent-research/src/status.rs` | 103 | ⚠️ inline |
| `crates/ragent-research/src/synthesis.rs` | 474 | ⚠️ inline |
| `crates/ragent-research/src/tier_router.rs` | 38, 43, 56, 310 | ⚠️ inline (4 cfg blocks) |
| `crates/ragent-research/src/verify.rs` | 155 | ⚠️ inline |
| `crates/ragent-research/src/web_date.rs` | 251 | ⚠️ inline |
| `crates/ragent-research/src/web_gatherer.rs` | 1924 | ⚠️ inline (57 tests) |

### `ragent-specs` (5 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-specs/src/constitution.rs` | 506 | ⚠️ inline |
| `crates/ragent-specs/src/impl_runner.rs` | 838, 1298 | ⚠️ inline (two modules) |
| `crates/ragent-specs/src/manager.rs` | 559 | ⚠️ inline |
| `crates/ragent-specs/src/plan_parser.rs` | 724 | ⚠️ inline |
| `crates/ragent-specs/src/validate.rs` | 1674 | ✅ `#[path]` include |

### `ragent-telemetry` (1 module)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-telemetry/src/prometheus.rs` | 422 | ⚠️ inline |

### `ragent-tools-core` (4 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-tools-core/src/askpass.rs` | 398 | ✅ `#[path]` include |
| `crates/ragent-tools-core/src/bash.rs` | 1422 | ✅ `#[path]` include |
| `crates/ragent-tools-core/src/cron_log.rs` | 270 | ⚠️ inline |
| `crates/ragent-tools-core/src/edit_log.rs` | 711 | ⚠️ inline |

### `ragent-tools-extended` (14 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-tools-extended/src/browser/actions.rs` | 860 | ⚠️ inline |
| `crates/ragent-tools-extended/src/browser/cdp.rs` | 418 | ⚠️ inline |
| `crates/ragent-tools-extended/src/browser/launch.rs` | 292 | ⚠️ inline |
| `crates/ragent-tools-extended/src/browser/mod.rs` | 549 | ⚠️ inline |
| `crates/ragent-tools-extended/src/document_extract.rs` | 149 | ⚠️ inline |
| `crates/ragent-tools-extended/src/finance/cache.rs` | 101 | ⚠️ inline |
| `crates/ragent-tools-extended/src/finance/error.rs` | 52 | ⚠️ inline |
| `crates/ragent-tools-extended/src/finance/providers/twelvedata.rs` | 559 | ⚠️ inline |
| `crates/ragent-tools-extended/src/finance/rate_limit.rs` | 106, 114 | ⚠️ inline |
| `crates/ragent-tools-extended/src/finance/throttle.rs` | 58, 66 | ⚠️ inline |
| `crates/ragent-tools-extended/src/libreoffice_common.rs` | 223 | ⚠️ inline |
| `crates/ragent-tools-extended/src/masterfetch/search/engine.rs` | 686 | ⚠️ inline |
| `crates/ragent-tools-extended/src/masterfetch/search/mod.rs` | 457 | ⚠️ inline |
| `crates/ragent-tools-extended/src/office_common.rs` | 109 | ⚠️ inline |

### `ragent-tui` (7 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-tui/src/app/cron.rs` | 666 | ⚠️ inline |
| `crates/ragent-tui/src/app/tests.rs` | 3 | ⚠️ inline |
| `crates/ragent-tui/src/app.rs` | 28 | ⚠️ inline |
| `crates/ragent-tui/src/layout.rs` | 5010 | ⚠️ inline |
| `crates/ragent-tui/src/layout_statusbar.rs` | 706 | ⚠️ inline |
| `crates/ragent-tui/src/research_adapter.rs` | 103 | ⚠️ inline |
| `crates/ragent-tui/src/research_progress.rs` | 669 | ⚠️ inline |

### `ragent-types` (3 modules)

| File | Line | Status |
|------|------:|--------|
| `crates/ragent-types/src/cron.rs` | 865 | ⚠️ inline |
| `crates/ragent-types/src/strutil.rs` | 87 | ⚠️ inline |
| `crates/ragent-types/src/trigger.rs` | 260 | ⚠️ inline |

### Root `src/`

| File | Line | Status |
|------|------:|--------|
| `src/panic_hook.rs` | 150 | ⚠️ inline (2 tests) |

---

## 3. `tests/` Directories per Crate

All 17 workspace crates have a `tests/` directory. `ragent-codeindex`, `ragent-server`, `ragent-storage`, `ragent-team`, and `ragent-tools-vcs` have *no* remaining inline modules. Below is a high-level inventory.

| Crate | External `.rs` test files | Inline modules remaining | Notes |
|-------|---------------------------:|-----------------------:|-------|
| `ragent-agent` | 61 | 27 | Many tests relocated to `tests/`; several large inline modules remain |
| `ragent-bench` | 2 | 3 | Inline tests still in `src/data.rs`, `src/model.rs`, `src/suites/mod.rs` |
| `ragent-codeindex` | 25 | 0 | ✅ Fully compliant |
| `ragent-config` | 20 | 1 | Single inline module in `src/telemetry.rs` |
| `ragent-llm` | 18 | 16 | A few modules already use `#[path]`; most still inline |
| `ragent-prompt_opt` | 2 | 0 | ✅ Fully compliant |
| `ragent-research` | 18 | 36 | Highest inline count, including `document.rs` (71 tests) and `web_gatherer.rs` (57) |
| `ragent-server` | 4 | 0 | ✅ Fully compliant |
| `ragent-specs` | 13 | 5 | `validate.rs` uses `#[path]`; others inline |
| `ragent-storage` | 13 | 0 | ✅ Fully compliant |
| `ragent-team` | 14 | 0 | ✅ Fully compliant |
| `ragent-telemetry` | 16 | 1 | Single inline module in `src/prometheus.rs` |
| `ragent-tools-core` | 16 | 4 | `askpass.rs`/`bash.rs` use `#[path]`; `cron_log.rs`/`edit_log.rs` inline |
| `ragent-tools-extended` | 47 | 14 | Largest external test surface, but many inline modules remain |
| `ragent-tools-vcs` | 2 | 0 | ✅ Fully compliant |
| `ragent-tui` | 51 | 7 | Several UI layout/app inline tests remain |
| `ragent-types` | 11 | 3 | `cron.rs` has 56 inline tests |

*Root-level tests directory does not exist; the root `tests/` folder mentioned by the guideline is absent.*

---

## 4. Private-Item Test Access (`pub(crate)` vs `#[path]` shims)

`AGENTS-RUST.md` recommends:

> *Public-API tests*: use `use ragent_<crate>::module::Item;` — no source changes needed.  
> *Private-item tests*: widen the tested items to `pub(crate)` where necessary and re-import the source module via `#[path = "../src/<module>.rs"] mod <module>;`. Provide shims for `super::` and `crate::` references at the test file root.  
> *Complex cases*: use `#[cfg(test)] #[path = "../../tests/test_<module>.rs"] mod test_<module>;` in the source file.

### Correct `#[path]` shim usage (15 modules)

These source files correctly include an external test file from `tests/inline/`, using `#[path]` so the external module is compiled inside the crate's module tree and `super::*` resolves to the source module.

| Source file | External test file |
|-------------|--------------------|
| `crates/ragent-agent/src/compaction/convert.rs` | `tests/inline/test_compaction_convert.rs` |
| `crates/ragent-agent/src/compaction/estimator.rs` | `tests/inline/test_compaction_estimator.rs` |
| `crates/ragent-agent/src/compaction/runner.rs` | `tests/inline/test_compaction_runner.rs` |
| `crates/ragent-agent/src/reference/parse.rs` | `tests/inline/reference_parse.rs` |
| `crates/ragent-agent/src/skill/args.rs` | `tests/inline/skill_args.rs` |
| `crates/ragent-agent/src/skill/context.rs` | `tests/inline/skill_context.rs` |
| `crates/ragent-agent/src/skill/loader.rs` | `tests/inline/skill_loader.rs` |
| `crates/ragent-agent/src/skill/mod.rs` | `tests/inline/skill_mod.rs` |
| `crates/ragent-llm/src/providers/huggingface.rs` | `tests/inline/huggingface.rs` |
| `crates/ragent-llm/src/providers/router_classifier.rs` | `tests/inline/router_classifier.rs`, `tests/inline/router_classifier_extended.rs` |
| `crates/ragent-llm/src/providers/router_modifiers.rs` | `tests/inline/router_modifiers.rs` |
| `crates/ragent-llm/src/providers/xai.rs` | `tests/inline/xai.rs` |
| `crates/ragent-specs/src/validate.rs` | `tests/inline/validate.rs` |
| `crates/ragent-tools-core/src/askpass.rs` | `tests/inline/askpass.rs` |
| `crates/ragent-tools-core/src/bash.rs` | `tests/inline/bash.rs` |

All of these external files use `use super::*;` and are therefore testing crate-private items through the `#[path]` mechanism as intended.

### External test files using `crate::` without a `#[path]` shim

The following standalone integration tests reference `crate::` items. The ones in `tests/inline/*.rs` resolve correctly because they are compiled via `#[path]`; the three non-`inline` files should be verified.

| File | `crate::` usage | Likely status |
|------|---------------|---------------|
| `crates/ragent-codeindex/tests/test_codeindex.rs` | `use crate::Config;` | Likely *string literal* inside test data, not an actual Rust import. ✅ OK |
| `crates/ragent-codeindex/tests/test_rust_parser.rs` | `use crate::config::Config as AppConfig;` | Same — string literal inside parser test input. ✅ OK |
| `crates/ragent-telemetry/tests/test_subsystem.rs` | `use crate::TelemetryError;` | No `#[path]` in `subsystem.rs`. **Suspicious** unless `TelemetryError` is publicly re-exported. |
| `crates/ragent-agent/tests/inline/test_compaction_*.rs` | `use crate::compaction::...;` | ✅ OK — compiled via `#[path]` |
| `crates/ragent-llm/tests/inline/router_*.rs` | `use crate::...;` | ✅ OK — compiled via `#[path]` |
| `crates/ragent-specs/tests/inline/validate.rs` | `use crate::spec::{Spec, SpecId};` | ✅ OK — compiled via `#[path]` |

**Observation:** `crates/ragent-telemetry/tests/test_subsystem.rs` imports `crate::TelemetryError` but the source file `crates/ragent-telemetry/src/subsystem.rs` does **not** contain a `#[path = "../tests/test_subsystem.rs"]` include. The test file appears to be a normal integration test. It will compile only if `TelemetryError` is part of the crate's public API. A quick read of `src/lib.rs` shows `pub use subsystem::{TelemetryState, TelemetrySubsystem};` but does not re-export `TelemetryError`. This may be a latent compile error or the item may be public through another path. This should be flagged for follow-up.

### Inline modules that test private items (`use super::*`)

Almost every inline module uses `use super::*;`, which is expected because the tests live in the same file/module. When these are migrated, the recommended pattern is to make the tested items `pub(crate)` and use a `#[path]` include so the external test file still has `super::*` resolving correctly. No correctness issue for now, just organization.

---

## 5. Test Naming Conventions

`AGENTS-RUST.md` requires: `test_<component>_<scenario>` (or `bench_` for benchmarks).

### External `tests/` files

| Crate | `#[test]` functions | Non-conforming names |
|-------|--------------------:|---------------------:|
| `ragent-agent` | 432 | 34 |
| `ragent-bench` | 46 | 0 |
| `ragent-codeindex` | 236 | 2 |
| `ragent-config` | 147 | 53 |
| `ragent-llm` | 214 | 7 |
| `ragent-prompt_opt` | 4 | 0 |
| `ragent-research` | 104 | 104 |
| `ragent-server` | 46 | 0 |
| `ragent-specs` | 426 | 66 |
| `ragent-storage` | 123 | 5 |
| `ragent-team` | 52 | 4 |
| `ragent-telemetry` | 185 | 0 |
| `ragent-tools-core` | 102 | 31 |
| `ragent-tools-extended` | 1242 | 20 |
| `ragent-tools-vcs` | 6 | 0 |
| `ragent-tui` | 794 | 2 |
| `ragent-types` | 147 | 4 |
| **Total** | **4306** | **332** |

The 332 non-conforming names are mostly descriptive sentences such as `env_var_name_is_stable`, `default_state_is_profiling_disabled`, `finance_config_defaults_to_yahoo`, `system_prompt_cache_is_lazy`. These do **not** match the mandated `test_<component>_<scenario>` pattern. While readable, they are technically violations.

### Inline `src/` modules

| Crate | Inline `#[test]` functions | Non-conforming names |
|-------|--------------------------:|---------------------:|
| `ragent-agent` | 140 | 10 |
| `ragent-bench` | 13 | 0 |
| `ragent-codeindex` | 0 | 0 |
| `ragent-config` | 16 | 0 |
| `ragent-llm` | 88 | 10 |
| `ragent-prompt_opt` | 0 | 0 |
| `ragent-research` | 486 | 466 |
| `ragent-server` | 0 | 0 |
| `ragent-specs` | 117 | 0 |
| `ragent-storage` | 0 | 0 |
| `ragent-team` | 0 | 0 |
| `ragent-telemetry` | 9 | 0 |
| `ragent-tools-core` | 16 | 5 |
| `ragent-tools-extended` | 119 | 26 |
| `ragent-tools-vcs` | 0 | 0 |
| `ragent-tui` | 29 | 0 |
| `ragent-types` | 66 | 3 |
| **Total** | **1099** | **520** |

The inline modules contain an even higher proportion of non-conforming names. `ragent-research` is the worst offender: 466 of 486 inline test functions do not start with `test_`.

### Examples of non-conforming names

- `parse_tool_list_extracts_tools`
- `empty_openai_tools_array_is_valid`
- `stopper_continues_within_budget`
- `log_dir_resolves_under_working_dir`
- `detect_pdf`
- `truncate_bytes_no_ellipsis_keeps_short_strings`

---

## 6. Priority Recommendations

1. **Migrate remaining inline modules.** `ragent-research`, `ragent-llm`, `ragent-tools-extended`, `ragent-agent`, and `ragent-specs` are the biggest contributors. Start with the largest inline modules (`document.rs`, `web_gatherer.rs`, `cron.rs`, `analysis.rs`, `item.rs`) because they will yield the biggest compliance improvement.
2. **Verify `ragent-telemetry/tests/test_subsystem.rs`.** It references `crate::TelemetryError` without a `#[path]` include. Confirm it compiles as a standalone integration test or migrate/inline it.
3. **Move root `src/panic_hook.rs` tests.** The root `src/panic_hook.rs` contains an inline `#[cfg(test)]` module. These should move to `tests/test_panic_hook.rs` at the workspace root (or to an appropriate crate test directory if the module is moved).
4. **Naming convention cleanup.** Decide whether to enforce `test_<component>_<scenario>` strictly. If strict enforcement is desired, rename the 852 non-conforming functions. If the convention is advisory, update the guideline to say so.
5. **Use `#[path]` for all private-item tests.** For modules that are migrated but test private items, ensure the target test file is included via `#[path]` or the items are promoted to `pub(crate)`. Avoid standalone integration tests that rely on private items.

---

## 7. Compliance Score

| Criterion | Status |
|-----------|--------|
| All crates have a `tests/` directory | ✅ Yes (17/17) |
| No new inline `#[cfg(test)]` modules added | ⚠️ Cannot verify age; 117 exist today |
| All tests located in `tests/` (no inline) | ❌ No — 102 non-`#[path]` inline modules remain |
| Root tests in `tests/` (not inline in `src/`) | ❌ No — `src/panic_hook.rs` has inline tests |
| `#[path]` shims used correctly for private tests | ✅ Yes for 15 modules; pattern is understood |
| Naming convention `test_<component>_<scenario>` | ❌ No — 852/6468 functions non-conforming |

**Overall compliance:** Partial — external test structure is strong, but a large backlog of inline modules and non-conforming test names remains.
