# Thinking/Reasoning Level Selection — Implementation Plan

> **Date:** 2025-01-17  
> **Status:** Draft Plan  
> **Priority:** P2 (Medium — major feature)

---

## 1. Motivation & Context

Users should be able to select the depth of thinking/reasoning for their chosen model,
just as they select the model itself. Different providers expose different knobs:

| Provider | Parameter | Values | Model support |
|----------|-----------|--------|---------------|
| **Anthropic (Claude)** | `thinking.type` + `effort` | `enabled`/`adaptive`/`disabled` + `budget_tokens` or `effort` | Opus 4.7+, Sonnet 4.6, Mythos |
| **OpenAI** | `reasoning_effort` | `low`, `medium`, `high`, `none` | o1, o3, GPT-5+ |
| **Google Gemini** | `thinkingLevel` | `minimal`, `low`, `medium`, `high`, `auto` | Gemini 3+ (varies by sub-model) |
| **GitHub Copilot** | `reasoning_effort` | `low`, `medium`, `high`, `none` | Reasoning-capable models |
| **Ollama (Local)** | `think` | `true`, `false` | deepseek-r1, qwen3, etc. |
| **Ollama Cloud** | `think` | `true`, `false` | Reasoning-capable models |
| **Hugging Face** | N/A | N/A | Model-specific (no standard) |
| **Generic OpenAI** | `reasoning_effort` (inherited) | `low`, `medium`, `high`, `none` | OpenAI-compatible endpoints |

### Current Gaps

1. **`Capabilities` struct** has only a boolean `reasoning` field — no concept of _available thinking levels_ per model.
2. **`ChatRequest.options`** is an untyped `HashMap<String, Value>` — each provider interprets keys differently with no standardisation.
3. **No UI** in the model picker for selecting thinking levels — the picker shows only a boolean `reasoning` indicator.
4. **No per-agent or per-session** thinking level configuration — only the `"disabled"` case is hardcoded in some agents.
5. **No discovery API** — thinking-level support is hardcoded per model, never fetched from provider APIs.

---

## 2. Milestones & Tasks

### Milestone 1: Data Model — `ThinkingConfig` enum & `Capabilities` extension

**Objective:** Define a standard, provider-agnostic thinking configuration type and extend `Capabilities` to describe available levels per model.

| ID | Task | Details | Priority |
|----|------|---------|----------|
| 1.1 | Define `ThinkingLevel` enum | `enum ThinkingLevel { Auto, Off, Low, Medium, High }` — provider-agnostic values that each provider maps to its own API parameters. Placed in `ragent-types` as a shared type. | P1 |
| 1.2 | Define `ThinkingConfig` struct | `struct ThinkingConfig { enabled: bool, level: ThinkingLevel, budget_tokens: Option<u32>, display: Option<ThinkingDisplay> }` with `ThinkingDisplay { Full, Summarized, Omitted }` for Anthropic. | P1 |
| 1.3 | Extend `Capabilities` with `thinking_levels` | Add `thinking_levels: Vec<ThinkingLevel>` to `Capabilities` to declare which levels a model supports. Default: empty vec (no thinking support). Backward-compatible with serde default. | P1 |
| 1.4 | Add `thinking` field to `ChatRequest` | Replace ad-hoc `options["thinking"]` parsing with a proper `thinking: Option<ThinkingConfig>` field on `ChatRequest`. Keeps `options` for truly provider-specific extras. | P2 |
| 1.5 | Add `thinking` field to `ModelConfig` | `pub thinking: Option<ThinkingConfig>` — allows users to set a default thinking level per model in `ragent.json`. | P2 |
| 1.6 | Update `ModelInfo` | Add `thinking_config: Option<ThinkingConfig>` that carries the default or user-configured thinking for each model. | P2 |

**Files:**
- `crates/ragent-types/src/thinking.rs` (new) — `ThinkingLevel`, `ThinkingConfig`, `ThinkingDisplay`
- `crates/ragent-types/src/lib.rs` — add `pub mod thinking;` and re-exports
- `crates/ragent-config/src/config.rs` — extend `Capabilities` with `thinking_levels`, extend `ModelConfig` with `thinking`
- `crates/ragent-llm/src/llm.rs` — add `thinking: Option<ThinkingConfig>` to `ChatRequest`

---

### Milestone 2: Provider Adapters — Map `ThinkingConfig` to provider API parameters

**Objective:** Each provider implementation maps the standard `ThinkingConfig` to its native API parameters.

| ID | Task | Details | Priority |
|----|------|---------|----------|
| 2.1 | Anthropic adapter | Map `ThinkingConfig` → `thinking.type`, `thinking.budget_tokens` (manual), `thinking.type = "adaptive"` + `effort` (adaptive). Support `display`. | P1 |
| 2.2 | OpenAI adapter | Map `ThinkingConfig` → `reasoning_effort` parameter. `Off` → `"none"`, `Low` → `"low"`, `Medium` → `"medium"`, `High` → `"high"`, `Auto` → omit parameter. | P1 |
| 2.3 | Gemini adapter | Map `ThinkingConfig` → `thinkingConfig.thinkingLevel`. `Off` → `"minimal"`, `Low` → `"low"`, `Medium` → `"medium"`, `High` → `"high"`, `Auto` → omit or `"auto"`. Support `include_thoughts`. | P1 |
| 2.4 | Copilot adapter | Map `ThinkingConfig` → `reasoning_effort`. Uses same mapping as OpenAI. Refactor existing `reasoning_effort_from_options`. | P1 |
| 2.5 | Ollama adapter | Map `ThinkingConfig` → `think` boolean. `Off` → `false`, any other → `true`. | P2 |
| 2.6 | Ollama Cloud adapter | Same as Ollama. | P2 |
| 2.7 | HuggingFace adapter | No standard thinking parameter — log a warning if `ThinkingConfig` is set and `enabled` is true, then ignore. | P3 |
| 2.8 | Generic OpenAI adapter | Inherits from OpenAI adapter automatically (uses `OpenAiClient`). | P1 |
| 2.9 | Update model defaults | Each provider's `default_models()` populates `thinking_levels` based on which levels the model actually supports (see research table above). | P1 |

**Files (per provider):**
- `crates/ragent-llm/src/providers/anthropic.rs`
- `crates/ragent-llm/src/providers/openai.rs`
- `crates/ragent-llm/src/providers/gemini.rs`
- `crates/ragent-llm/src/providers/copilot.rs`
- `crates/ragent-llm/src/providers/ollama.rs`
- `crates/ragent-llm/src/providers/ollama_cloud.rs`
- `crates/ragent-llm/src/providers/huggingface.rs`
- `crates/ragent-llm/src/providers/generic_openai.rs`

---

### Milestone 3: Model Picker UI — Thinking level column & selector

**Objective:** The model picker dialog shows available thinking levels and lets the user choose one.

| ID | Task | Details | Priority |
|----|------|---------|----------|
| 3.1 | Extend `ModelPickerEntry` | Add `thinking_levels: Vec<ThinkingLevel>` field. Populated from `ModelInfo.capabilities.thinking_levels`. | P1 |
| 3.2 | Add thinking-level column to table | In the model picker table, add a column "Thinking" that shows e.g. `"Off/Low/Med/High"` or `"Auto"` based on `thinking_levels`. For models with no thinking, show `"—"`. | P1 |
| 3.3 | Add thinking-level selector state | Add `SelectThinkingLevel` variant to `ProviderSetupStep` — shown after model selection when the model supports variable thinking levels. Presents the available levels as a list to choose from. | P1 |
| 3.4 | Persist selected thinking level | Store selected thinking level alongside `selected_model` in storage. Add `thinking_level` to session state. Load on startup. | P2 |
| 3.5 | Show current thinking level in status bar | Display the current thinking level alongside the model name (e.g. `"Claude Opus 4.7 [thinking: high]"`). | P2 |
| 3.6 | Add `/thinking` slash command | Allow runtime switching: `/thinking off`, `/thinking low`, `/thinking medium`, `/thinking high`, `/thinking auto`. | P2 |

**Files:**
- `crates/ragent-tui/src/app/state.rs` — extend `ModelPickerEntry`, add `SelectThinkingLevel` variant, add `/thinking` command handler
- `crates/ragent-tui/src/app.rs` — update `model_to_picker_entry`, render selector, persist choice
- `crates/ragent-tui/src/input.rs` — render thinking-level column in table
- `crates/ragent-tui/src/layout.rs` — status bar display

---

### Milestone 4: Session Integration — Pass `ThinkingConfig` through the pipeline

**Objective:** The selected thinking level flows from session state → tool processor → `ChatRequest` → provider adapter.

| ID | Task | Details | Priority |
|----|------|---------|----------|
| 4.1 | Add `thinking` field to session state | `session.thinking: ThinkingConfig` stored in `SessionState`. Default: `Auto` for reasoning-capable models, `Off` for others. | P1 |
| 4.2 | Set thinking from model selection | When a model is selected, set the default thinking to `Auto` (if model supports reasoning) or `Off` (if not). | P1 |
| 4.3 | Pass thinking to `ChatRequest` | In `processor.rs` where `ChatRequest` is constructed, set `request.thinking = Some(self.session.thinking.clone())`. | P1 |
| 4.4 | Deprecate `options["thinking"]` path | Remove or mark as deprecated the old `options.get("thinking")` path in providers. New path uses `request.thinking`. Fall back to `options` for backward compatibility. | P2 |
| 4.5 | Agent-level default thinking | Allow agents to specify a default `thinking` in their definition (OASF schema). Merge: agent default → config → user selection. | P3 |
| 4.6 | Update `/model` to reset thinking | When switching models via `/model`, reset thinking to the new model's default (Auto if reasoning-capable). | P2 |

**Files:**
- `crates/ragent-agent/src/session/processor.rs` — construct `ChatRequest` with `thinking`
- `crates/ragent-agent/src/agent/mod.rs` — agent-level thinking defaults
- `crates/ragent-storage/src/storage.rs` — persist thinking level

---

### Milestone 5: Discovery — Fetch available thinking levels from provider APIs

**Objective:** Where provider APIs expose model metadata (thinking support, available levels), fetch and cache it instead of using hardcoded defaults.

| ID | Task | Details | Priority |
|----|------|---------|----------|
| 5.1 | Anthropic model list API | Check if Anthropic API `/models` endpoint returns thinking capability flags per model. If so, use it. | P3 |
| 5.2 | OpenAI model list API | OpenAI `/models` endpoint does not return reasoning-effort support. Hardcoded fallback is acceptable. | P3 |
| 5.3 | Gemini model list API | Gemini API model listing may include thinking-level support. Investigate and integrate. | P3 |
| 5.4 | Copilot model list API | Copilot model discovery already fetches live model lists. Extend to include thinking-level metadata if available in the API response. | P2 |
| 5.5 | Cache discovered metadata | Store discovered model capabilities (including thinking levels) in the SQLite storage for fast reload. | P3 |

**Files:**
- `crates/ragent-llm/src/providers/anthropic.rs` — model discovery
- `crates/ragent-llm/src/providers/gemini.rs` — model discovery
- `crates/ragent-llm/src/providers/copilot.rs` — extend existing discovery
- `crates/ragent-storage/src/storage.rs` — cache storage

---

### Milestone 6: Config File Support — `ragent.json` thinking configuration

**Objective:** Users can set default thinking levels per model and per provider in their config file.

| ID | Task | Details | Priority |
|----|------|---------|----------|
| 6.1 | Config parsing for `thinking` | Parse `thinking` block in `ModelConfig`: `{ "level": "medium", "budget_tokens": 16000 }`. | P2 |
| 6.2 | Config parsing for provider-level thinking | Support a `thinking` block at `ProviderConfig` level as a fallback default for all models under that provider. | P3 |
| 6.3 | Merge precedence | User selection > agent default > config per-model > config per-provider > built-in default. | P2 |
| 6.4 | Update config example/docs | Update SPEC.md and examples with new `thinking` config block syntax. | P2 |

**Files:**
- `crates/ragent-config/src/config.rs` — parse `thinking` in `ModelConfig` and `ProviderConfig`
- `docs/userdocs/` — update configuration documentation
- `SPEC.md` — document new config fields

---

### Milestone 7: Tests & Documentation

**Objective:** Thoroughly test the new feature and update all relevant documentation.

| ID | Task | Details | Priority |
|----|------|---------|----------|
| 7.1 | Unit tests for `ThinkingLevel` | Test serialization/deserialization, default values, partial equality. | P1 |
| 7.2 | Unit tests for provider adapters | For each provider, test that `ThinkingConfig` maps to the correct native API parameters. Test all level variants. | P1 |
| 7.3 | Unit tests for config parsing | Test that `thinking` blocks parse correctly from JSON config. | P1 |
| 7.4 | Integration test: full pipeline | Mock provider, set thinking level, create session, verify `ChatRequest` carries the correct `ThinkingConfig`. | P2 |
| 7.5 | TUI tests for thinking selector | Test that the thinking-level selector dialog appears for reasoning-capable models. | P2 |
| 7.6 | Update SPEC.md | Document `ThinkingLevel`, `ThinkingConfig`, new config options, and `/thinking` command. | P1 |
| 7.7 | Update AGENTS.md | Remove outdated `options["thinking"]` references, add new thinking-level guidelines. | P2 |
| 7.8 | Update QUICKSTART.md | Add example of thinking-level configuration. | P3 |

**Files:**
- `crates/ragent-types/tests/test_thinking.rs` (new)
- `crates/ragent-llm/tests/test_thinking_adapters.rs` (new)
- `crates/ragent-config/tests/test_thinking_config.rs` (new)
- `crates/ragent-agent/tests/test_thinking_pipeline.rs` (new)
- `crates/ragent-tui/tests/test_thinking_selector.rs` (new)
- `SPEC.md`, `AGENTS.md`, `QUICKSTART.md`

---

## 3. Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                       Data Model (ragent-types)                      │
│  ThinkingLevel { Auto, Off, Low, Medium, High }                      │
│  ThinkingConfig { enabled, level, budget_tokens, display }           │
│  Capabilities { reasoning: bool, thinking_levels: Vec<ThinkingLevel>,│
│                 streaming, vision, tool_use }                        │
└──────────────┬──────────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Config (ragent-config)                           │
│  ModelConfig { name, cost, capabilities, thinking: Option<TC> }      │
│  ProviderConfig { env, api, models, options, thinking?: Option<TC> } │
└──────────────┬──────────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                  Session Pipeline (ragent-agent)                      │
│  SessionState.thinking: ThinkingConfig                                │
│       ↓                                                               │
│  processor.rs builds ChatRequest { thinking: Some(TC), ... }          │
└──────────────┬──────────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                  LLM Layer (ragent-llm)                               │
│  Each provider maps TC → native params:                               │
│  Anthropic: thinking.type + effort/budget_tokens + display            │
│  OpenAI:    reasoning_effort = "low"|"medium"|"high"|"none"           │
│  Gemini:    thinkingConfig.thinkingLevel = "minimal"|...|"auto"       │
│  Copilot:   reasoning_effort (same as OpenAI)                         │
│  Ollama:    think = true/false                                        │
└──────────────┬──────────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       TUI (ragent-tui)                                │
│  ModelPicker: shows "Thinking" column (levels supported)              │
│  SelectThinkingLevel: dialog after model selection                    │
│  Status bar: "Claude Opus 4.7 [thinking: high]"                      │
│  /thinking command: runtime switching                                 │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. Key Design Decisions

### 4.1 Why a typed `ThinkingConfig` instead of `options` dict?
The current `options: HashMap<String, Value>` is fragile — each provider reads different keys, there's no type safety, and there's no way to validate values at config-parse time. A typed struct gives us compile-time safety, auto-complete in the TUI, and clear serialization.

### 4.2 Provider-agnostic levels vs. provider-specific levels
We use a small set of standard levels (`Auto`, `Off`, `Low`, `Medium`, `High`) that cover all providers' parameter ranges. Each provider adapter maps these to its native API values. This keeps the UI and config format provider-agnostic.

### 4.3 Backward compatibility
- Old `options["thinking"]` path still works as a fallback; providers check `ChatRequest.thinking` first, then fall back to `options`.
- `Capabilities` gains `thinking_levels` with `#[serde(default)]` so old configs still parse.
- `ModelConfig.thinking` is optional; absent = default behavior (Auto for reasoning-capable).

### 4.4 Default thinking level
- For models with `reasoning = true` → default is `Auto` (let the model decide).
- For models with `reasoning = false` → default is `Off`.
- Exception: Anthropic Claude Opus 4.7+ always uses adaptive thinking — `Off` maps to `minimal` in Gemini terms.

---

## 5. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Provider API changes break adapters | Medium | High | Isolate mapping in provider adapter files; add integration tests with recorded responses |
| Thinking levels differ per model version | Medium | Medium | Model-specific `thinking_levels` in `Capabilities`; update with each model release |
| User confusion with too many options | Low | Low | Default to `Auto` for reasoning models; `Off` for others; `/thinking` command for power users |
| Backward compat break for custom agents | Low | Medium | Old `options["thinking"]` path still works; agents that set it continue to function |
| Performance overhead of thinking on cheap models | Low | Low | `Off` is always available; user can choose per-model |

---

## 6. Effort Estimate

| Milestone | Files Changed | New Files | Estimated Time |
|-----------|--------------|-----------|----------------|
| M1: Data Model | 3 | 1 | 2–3 hours |
| M2: Provider Adapters | 8 | 0 | 4–6 hours |
| M3: Model Picker UI | 3 | 0 | 3–4 hours |
| M4: Session Integration | 3 | 0 | 2–3 hours |
| M5: Discovery | 3 | 0 | 4–6 hours (investigation-heavy) |
| M6: Config File | 2 | 0 | 1–2 hours |
| M7: Tests & Docs | 1 | 5 | 3–4 hours |
| **Total** | **~23** | **6** | **19–28 hours** |

---

## 7. Future Considerations (Post-MVP)

- **Per-request thinking override** — allow `/think low` on a single request without changing the session default.
- **Thinking token budget slider** — for Anthropic's `budget_tokens`, a numeric input in the TUI.
- **Thinking display toggle** — control whether thinking content is shown/omitted/summarised in the UI (Anthropic/Mythos).
- **Provider model endpoint discovery** — fetch `thinking_levels` from provider metadata APIs instead of hardcoding.
- **Agent-level thinking presets** — e.g. a `debug` agent defaults to `High` thinking, a `chat` agent defaults to `Off`.
