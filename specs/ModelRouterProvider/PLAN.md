# ModelRouterProvider — Implementation Plan

## Architecture

The model router is a **virtual provider** that sits between the session
processor and the real provider backends. It implements the same `Provider` and
`LlmClient` traits as every other provider, so no changes to the orchestrator
or session processor are required.

```
ChatRequest
    │
    ▼
┌──────────────────┐
│  RouterProvider   │  ← implements Provider trait
│  (id: "router")  │
└─────��──┬─────────┘
         │
    ┌────▼─────┐
    │Classifier │  ← 14-dimension scorer + tier selector
    └────┬─────┘
         │  tier = COMPLEX
    ┌────▼──────────────┐
    │TierResolver       │  ← lookup tier → [model1, model2, ...]
    └────┬──────────────┘
         │
    ┌────▼─────────────┐
    │FallbackExecutor   │  ← try model1, on error → model2, ...
    └────┬─────────────┘
         │
    ┌────▼───────────────┐
    │Real Provider LLM   │  ← e.g. AnthropicClient.chat()
    └────────────────────┘
```

### Crate Location

New files within the existing `ragent-llm` crate:

```
crates/ragent-llm/src/
├── providers/
│   ├── router.rs              # RouterProvider + RouterClient
│   ├── router_classifier.rs   # 14-dimension classifier + tier selection
│   ├── router_config.rs       # RouterConfig, TierConfig, WeightConfig types
│   ├── router_modifiers.rs    # Prompt modifier detection and stripping
│   ├── router_stats.rs        # Routing statistics tracker
│   └── mod.rs                 # +1 line: pub mod router (and submodules)
```

Rationale for keeping it in `ragent-llm` rather than a new crate: the router
directly accesses `ProviderRegistry`, `LlmClient`, `ChatRequest`, and
`StreamEvent` — all defined in `ragent-llm`. A separate crate would create a
circular dependency. The router is ~1,200 lines total, which is proportional to
other provider modules (bedrock.rs is ~1,200 lines).

### Key Design Decisions

1. **In-process classifier** — No HTTP call, no external service. The
   classifier is pure Rust string analysis (word frequency, pattern matching,
   token counting). Runs in <5ms.

2. **FallbackExecutor** — Tries each model in the tier's list sequentially.
   On provider error (timeout, 5xx, auth), moves to next. Uses
   `tokio::time::timeout` per entry.

3. **Modifier detection** — Three regex patterns (slash prefix, bracket
   prefix, word prefix) checked before classification. If matched, classifier
   is skipped and the modifier's tier is used directly.

4. **Statistics** — `Arc<DashMap<Tier, AtomicU64>>` for lock-free concurrent
   counters. No blocking, no mutex contention.

5. **Config hot-reload** — `RouterConfig` is wrapped in `Arc<RwLock<>>`. The
   `/router reload` command acquires the write lock and parses the latest
   `ragent.json`. Ongoing requests hold a read lock on the config snapshot.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define RouterConfig, TierConfig, and WeightConfig types | FR-023, FR-024 | M | Critical | completed | — |
| T-002 | Implement 14-dimension classifier | FR-004, FR-005 | L | Critical | completed | — |
| T-003 | Implement tier boundary selection logic | FR-006, FR-007, FR-008, FR-009 | S | Critical | completed | T-002 |
| T-004 | Implement configurable classifier weights | FR-010, FR-024 | M | High | completed | T-001, T-002 |
| T-005 | Implement prompt modifier detection and stripping | FR-016, FR-017, FR-018, FR-019, FR-020 | M | Critical | completed | — |
| T-006 | Implement RouterProvider struct | FR-001, FR-003 | S | Critical | completed | T-001 |
| T-007 | Implement RouterClient with FallbackExecutor | FR-002, FR-011, FR-012, FR-013, FR-014, FR-015 | L | Critical | pending | T-006, T-002, T-005 |
| T-008 | Register RouterProvider in default registry | FR-003 | S | Critical | completed | T-006 |
| T-009 | Implement disabled-mode passthrough | FR-026 | S | High | pending | T-006 |
| T-010 | Implement context-aware classification | FR-021, FR-022 | M | Medium | pending | T-002, T-007 |
| T-011 | Implement streaming and tool-use passthrough | FR-034, FR-035, FR-036 | M | Critical | pending | T-007 |
| T-012 | Implement thinking/reasoning default for REASONING tier | FR-037, FR-038 | S | Medium | pending | T-007 |
| T-013 | Implement routing statistics tracker | FR-031, FR-032 | S | High | pending | T-007 |
| T-014 | Implement `/router status` and `/router tiers` | FR-027 | M | High | completed | T-006, T-007 |
| T-015 | Implement `/router on` and `/router off` commands | FR-028, FR-029 | M | High | completed | T-006 |
| T-016 | Implement `/router tier` subcommands | FR-027 | M | High | pending | T-006 |
| T-017 | Implement `/router weights` and `/router boundaries` | FR-027 | M | Medium | completed | T-001, T-004 |
| T-018 | Implement `/router test` command | FR-030 | M | High | completed | T-002, T-003 |
| T-019 | Implement `/router stats` and `/router stats reset` | FR-027, FR-033 | S | Medium | pending | T-013 |
| T-020 | Implement `/router reload` command | FR-025 | M | High | completed | T-001 |
| T-021 | Implement `/router help` command | FR-027 | S | Low | completed | — |
| T-022 | Add `router` to SLASH_COMMANDS autocomplete | FR-042 | S | High | completed | T-014 |
| T-023 | Implement TUI status bar tier display | FR-043 | S | Low | pending | T-007 |
| T-024 | Implement classifier error fallback to MEDIUM | FR-039 | S | High | completed | T-002 |
| T-025 | Implement per-entry timeout in fallback chain | FR-014 | S | Medium | pending | T-007 |
| T-026 | Persist router config changes to ragent.json | FR-028, FR-029, FR-005 | M | High | pending | T-001, T-015 |
| T-027 | Write unit tests for classifier | NFR-001, NFR-004, FR-004 | L | Critical | completed | T-002 |
| T-028 | Write unit tests for modifier detection | FR-016, FR-017, FR-018, FR-019 | M | Critical | completed | T-005 |
| T-029 | Write unit tests for fallback executor | FR-012, FR-013, FR-015 | M | Critical | pending | T-007 |
| T-030 | Write integration test (router → provider contract) | NFR-002, NFR-003, FR-002 | M | High | pending | T-007, T-008 |
| T-031 | Update SPEC.md and PROVIDERS.md documentation | — | S | Low | pending | T-008 |
| T-032 | Add router state fields to App struct | FR-044, FR-045, FR-046, FR-047 | S | High | completed | — |
| T-033 | Render Router status indicator in status bar Line 2 right | FR-044, FR-045, FR-047, FR-048, FR-049 | M | High | completed | T-032 |
| T-034 | Update Router indicator on tier change events | FR-046, NFR-006 | S | High | pending | T-032, T-007 |
| T-035 | Write unit tests for Router status indicator rendering | FR-044, FR-045, FR-047, FR-048, FR-049, NFR-006 | M | High | pending | T-033, T-034 |
## Task Details

### T-001 — RouterConfig Types (M, Critical)

Define configuration types in `router_config.rs`:

- `RouterConfig` — top-level struct with `enabled`, `tiers`, `weights`,
  `boundaries`, `context_messages`, `default_timeout_ms`
- `TierConfig` — tier name + ordered `Vec<TierEntry>` + optional `timeout_ms`
- `TierEntry` — `provider: String`, `model: String`
- `WeightConfig` — 14 named `f64` fields, one per dimension
- `BoundaryConfig` — three `f64` thresholds: `simple_medium`,
  `medium_complex`, `complex_reasoning`
- Implement `Default` for all types providing the built-in defaults (FR-024)
- Implement `Deserialize` / `Serialize` for `ragent.json` round-tripping

### T-002 — 14-Dimension Classifier (L, Critical)

Implement `PromptClassifier` in `router_classifier.rs`:

- `classify(prompt: &str, history: &[ChatMessage], config: &RouterConfig) -> ClassificationResult`
- 14 scoring functions, one per dimension, each returning 0.0–1.0:
  - `score_token_count` — character count buckets (short/medium/long/very-long)
  - `score_vocabulary_complexity` — ratio of long words (8+ chars) and unique
    words to total words; detect technical terms via heuristics
  - `score_syntax_complexity` — count nested clauses (commas, semicolons,
    parenthetical groups, conditional keywords like if/unless/either)
  - `score_domain_specificity` — keyword lists for medical, legal, financial,
    engineering, scientific domains
  - `score_ambiguity` — detect open-ended question markers ("what if",
    "could you", "might", "perhaps", "explore")
  - `score_context_dependency` — pronoun density, references to "it", "that",
    "the above", "previous"
  - `score_reasoning_depth` — count inference markers ("therefore", "because",
    "implies", "prove", "deduce", "hypothesis")
  - `score_creativity_level` — detect creative request markers ("imagine",
    "design", "create", "invent", "compose", "write a story")
  - `score_emotional_complexity` — sentiment-bearing words, nuance markers
    ("subtle", "delicate", "nuanced", "sensitive")
  - `score_multimodality` — references to images, diagrams, files, URLs,
    base64 data
  - `score_instruction_complexity` — count numbered steps, bullet lists,
    constraint words ("must", "should", "never", "always", "ensure")
  - `score_knowledge_recency` — temporal markers ("latest", "current",
    "2024/2025", "recently", "updated", "news")
  - `score_code_complexity` — code fence detection, programming keywords,
    architecture patterns (MVC, microservice, dependency injection)
  - `score_mathematical_complexity` — LaTeX markers, equation symbols,
    Greek letters, proof keywords ("theorem", "lemma", "corollary")
- `ClassificationResult` — 14 scores, composite score, selected tier
- All scoring functions are pure and deterministic (NFR-004)

### T-003 — Tier Boundary Selection (S, Critical)

Add `select_tier(composite: f64, boundaries: &BoundaryConfig) -> Tier` to the
classifier. Simple threshold comparison against the three configurable
boundaries. Unit testable in isolation.

### T-004 — Configurable Weights (M, High)

Extend the classifier to accept a `WeightConfig` and compute the weighted sum
instead of using hardcoded weights. Validate that weights sum to ~1.0 (±0.05)
and normalise if not.

### T-005 — Prompt Modifier Detection (M, Critical)

Implement `detect_modifier(prompt: &str) -> Option<(Tier, &str)>` in
`router_modifiers.rs`:

- Slash prefix regex: `^/(simple|medium|complex|max|reasoning|basic|cheap|balanced|advanced|think|deep)\s+`
- Bracket prefix regex: `^\[(simple|medium|complex|max|reasoning|basic|cheap|balanced|advanced|think|deep)\]\s*`
- Word prefix regex: `^(simple|basic|complex|reasoning|medium|deep)\s+mode[\s:,]+`
- Alias resolution via the table in FR-019
- Return the tier and the remaining prompt (with modifier stripped)
- Return `None` when no modifier is found (fall through to classifier)

### T-006 — RouterProvider Struct (S, Critical)

Implement `RouterProvider` in `router.rs`:

- `id()` → `"router"`, `name()` → `"Model Router"` (FR-001)
- `default_models()` → single virtual model `"auto"` representing the router
- `create_client()` → construct a `RouterClient` with a reference to the
  `ProviderRegistry` and `RouterConfig`

### T-007 — RouterClient + FallbackExecutor (L, Critical)

Implement `RouterClient` in `router.rs`:

- Hold `Arc<ProviderRegistry>`, `Arc<RwLock<RouterConfig>>`, and
  `RouterStats` (Arc-wrapped for concurrent access)
- `chat()` implementation:
  1. Check `config.enabled`; if false, route to MEDIUM tier default (FR-026)
  2. Call `detect_modifier()` on the prompt text (FR-016–FR-020)
  3. If no modifier, call `classifier.classify()` with last N messages (FR-021)
  4. Resolve tier → `Vec<TierEntry>` from config
  5. For each entry: resolve provider from registry, create client, call
     `chat()`, stream result on success
  6. On error: log warning, try next entry (FR-012, FR-040)
  7. If all fail: return error (FR-013)
  8. Increment stats counters (FR-031, FR-032)

### T-008 — Registry Registration (S, Critical)

Add `registry.register(Box::new(router::RouterProvider::new()))` to
`create_default_registry()` in `mod.rs`. One-line change.

### T-009 — Disabled-Mode Passthrough (S, High)

When `RouterConfig.enabled == false`, `RouterClient.chat()` resolves the
MEDIUM tier's primary model and delegates directly. No classification, no
modifier detection. Acts as a transparent pass-through.

### T-010 — Context-Aware Classification (M, Medium)

Extend `classify()` to accept an optional `history: &[ChatMessage]` parameter.
When present and `config.context_messages > 0`, include the last N messages'
text content in the classification input. Modifier-detected prompts skip this
step (FR-022).

### T-011 — Streaming and Tool-Use Passthrough (M, Critical)

`RouterClient.chat()` returns a `Stream<Item = StreamEvent>` from the resolved
provider. No transformation — the stream is passed through as-is. Tool-use
fields in `ChatRequest` (tools, tool_choice, tool_result) are forwarded
unchanged.

### T-012 — REASONING Tier Thinking Default (S, Medium)

When tier is REASONING and `ChatRequest.thinking` is `None`, check if the
resolved model's `ModelInfo.thinking_config` is `Some`. If so, apply
`ThinkingConfig { enabled: true, level: ThinkingLevel::High }` before forwarding.

### T-013 — Routing Statistics (S, High)

Implement `RouterStats` in `router_stats.rs`:

- `AtomicU64` counters for: total_requests, simple_count, medium_count,
  complex_count, reasoning_count, simple_fallbacks, medium_fallbacks,
  complex_fallbacks, reasoning_fallbacks
- `Instant`-based classification_time accumulator (average)
- `increment_tier(tier)`, `increment_fallback(tier)`, `record_classify_time(dur)`
- `snapshot() -> StatsSnapshot` for display
- `reset()` to zero all counters

### T-014 — /router status and /router tiers (M, High)

Add slash-command handler in `app.rs` for `/router status` (shows enabled/disabled,
current config summary, stats) and `/router tiers` (shows all tier mappings
with model lists and fallback order).

### T-015 — /router on and /router off (M, High)

Toggle `RouterConfig.enabled`, persist the change to `ragent.json` under
`provider.router.enabled`, and display confirmation. Must acquire the config
write lock.

### T-016 — /router tier subcommands (M, High)

Parse `/router tier <name> set|add|remove <provider>/<model>`. Modify the
in-memory `RouterConfig` and persist to `ragent.json`. Validate that the
provider exists in the registry and the model is known.

### T-017 — /router weights and /router boundaries (M, Medium)

Parse `/router weights set <dim> <value>` and `/router weights reset`.
Parse `/router boundaries set <boundary> <value>` with validation (0.0–1.0,
ascending order). Display current values on `/router weights` and
`/router boundaries`.

### T-018 — /router test (M, High)

Run the full classifier pipeline on the provided prompt text, then display:
- All 14 dimension scores (name: value)
- Composite weighted score
- Boundary thresholds
- Selected tier
- Primary model for that tier

No request is forwarded to any provider.

### T-019 — /router stats (S, Medium)

Display the `StatsSnapshot` from `RouterStats` in a formatted table.
`/router stats reset` calls `RouterStats::reset()`.

### T-020 — /router reload (M, High)

Re-read `provider.router` from `ragent.json`, construct a new `RouterConfig`,
swap it into the `Arc<RwLock<RouterConfig>>`. Display confirmation or error
if parsing fails.

### T-021 — /router help (S, Low)

Print the subcommand table from FR-027.

### T-022 — SLASH_COMMANDS Autocomplete (S, High)

Add `SlashCommandDef { trigger: "router", description: "..." }` to the
`SLASH_COMMANDS` constant in `state.rs`.

### T-023 — TUI Status Bar Tier Display (S, Low)

When the active provider is `router`, append the last-used tier to the
provider display in the status bar (e.g. `router:MEDIUM`). Requires passing
the last tier through the event bus or app state.

### T-024 — Classifier Error Fallback (S, High)

Wrap the classifier call in a catch-all. On panic or unexpected error, log a
warning and return tier MEDIUM. Prevents classifier bugs from crashing the
session.

### T-025 — Per-Entry Timeout (S, Medium)

When `TierEntry.timeout_ms` or `TierConfig.timeout_ms` is set, wrap the
provider `chat()` call in `tokio::time::timeout`. On timeout, log and try next
fallback entry.

### T-026 — Persist Config Changes (M, High)

Implement a `persist_router_config()` function that writes the current
`RouterConfig` (serialised as JSON) back into the `provider.router` section of
`ragent.json`. Used by `/router on|off`, `/router tier`, `/router weights`,
and `/router boundaries`.

### T-027 — Classifier Unit Tests (L, Critical)

Test every dimension scorer independently with crafted inputs:
- Simple prompt ("What is 2+2?") → SIMPLE
- Code prompt with function + tests → COMPLEX
- Mathematical proof prompt → REASONING
- Creative writing prompt → MEDIUM
- Modifier-prefixed prompts → override tier regardless of content
- Boundary edge cases (score exactly on boundary)
- Weight normalisation when sum ≠ 1.0
- Performance: classify 1,000 prompts in <5ms average

### T-028 — Modifier Detection Unit Tests (M, Critical)

Test all three modifier formats:
- Slash: `/simple hello`, `/max analyze this`
- Bracket: `[complex] refactor`, `[cheap] list files`
- Word: `deep mode: prove this`, `basic mode, what time`
- Mixed/ambiguous: `[simple /complex` → bracket wins
- No modifier: plain prompts fall through
- Stripping: verify modifier is removed from forwarded prompt

### T-029 — Fallback Executor Unit Tests (M, Critical)

Test the fallback chain:
- Primary succeeds → no fallback attempted
- Primary fails → fallback succeeds → result returned
- Primary fails, fallback fails → second fallback succeeds
- All fail → error returned
- Timeout triggers fallback
- Stats counters incremented correctly

### T-030 — Integration Test (M, High)

Test the full `RouterProvider → RouterClient → ProviderRegistry` path:
- Register mock providers that return known responses
- Send a `ChatRequest` via `RouterClient.chat()`
- Verify the correct provider was selected
- Verify streaming events pass through unchanged
- Verify tool-use requests route correctly

### T-031 — Documentation (S, Low)

Update `SPEC.md` with router provider section, `PROVIDERS.md` with router
configuration reference, and `README.md` with router feature mention.

### T-032 — Add Router state fields to App struct (S, High)

Add two fields to the `App` struct in `state.rs`:

- `router_enabled: bool` — whether the router provider is active and routing
  is enabled (default `false`).
- `router_current_tier: Option<String>` — the last tier selected by the
  router for the most recent request (e.g. `Some("SIMPLE")`, `Some("MEDIUM")`,
  `Some("COMPLEX")`, `Some("REASONING")`). `None` when no request has been
  routed yet or the router is not active.

These fields are set from session processor events and read by the status bar
renderer. When the active provider changes away from `router`, both fields
should be reset to `false` / `None`.

### T-033 — Render Router status indicator in status bar Line 2 right (M, High)

Extend `build_line2_right()` in `layout_statusbar.rs` to render a Router
indicator alongside the existing CodeIdx, InternalLLM, and YOLO indicators.
The indicator is only shown when `app.router_enabled` is true **or**
`app.router_current_tier` is `Some` (covers the case where router is the active
provider).

Rendering rules (matching the existing indicator pattern):

| Condition | Display (Full/Compact) | Display (Minimal) | Icon | Color |
|---|---|---|---|---|
| Router enabled, tier = SIMPLE | `Router:●SIMPLE` | `R:●S` | `●` (HEALTHY) | Green |
| Router enabled, tier = MEDIUM | `Router:●MEDIUM` | `R:●M` | `●` (HEALTHY) | Green |
| Router enabled, tier = COMPLEX | `Router:●COMPLEX` | `R:●C` | `●` (HEALTHY) | Green |
| Router enabled, tier = REASONING | `Router:●REASONING` | `R:●R` | `●` (HEALTHY) | Green |
| Router enabled, no tier yet | `Router:●…` | `R:●…` | `●` (HEALTHY) | Green |
| Router disabled (active provider) | `Router:✗OFF` | `R:✗` | `✗` (ERROR) | Red |
| Not router provider | (hidden) | (hidden) | — | — |

The tier label uses `colors::TEXT` with `Modifier::BOLD`. The icon uses
`colors::HEALTHY` (green) when enabled, `colors::ERROR` (red) when disabled.

### T-034 — Update Router indicator on tier change events (S, High)

When the session processor completes a routing decision (in `RouterClient.chat()`),
publish an event via the event bus (or update app state directly) containing
the selected tier. The TUI app state update handler sets
`app.router_current_tier = Some(tier.to_string())` and
`app.router_enabled = true`. When the router is disabled or the active
provider changes away from `router`, both fields are reset. This ensures the
indicator updates within a single render cycle (NFR-006).

### T-035 — Write unit tests for Router status indicator rendering (M, High)

Test cases covering FR-044 through FR-049 and NFR-006:

1. **Router enabled, each tier** — verify correct display string and colors
   for SIMPLE, MEDIUM, COMPLEX, REASONING.
2. **Router enabled, no tier yet** — verify `●…` placeholder display.
3. **Router disabled** — verify `✗OFF` display with error color.
4. **Not router provider** — verify indicator is hidden.
5. **Responsive modes** — verify Full, Compact, and Minimal abbreviations.
6. **Tier change updates** — verify that changing `router_current_tier`
   between requests produces the correct updated display.
7. **Render cycle timing** — verify the indicator reads directly from app
   state fields (no async delay), confirming NFR-006 compliance.

## Estimated Effort

| Priority | Tasks | Estimated Total |
|---|---|---|
| Critical | T-001, T-002, T-003, T-005, T-006, T-007, T-008, T-011, T-027, T-028, T-029 | 6L + 2M + 2S ≈ 20 days |
| High | T-004, T-009, T-013, T-014, T-015, T-016, T-018, T-020, T-022, T-024, T-026, T-030, T-032, T-033, T-034, T-035 | 3M + 7S + 2M + 2S + 1M + 2S ≈ 12 days |
| Medium | T-010, T-012, T-017, T-019, T-025 | 1M + 4S ≈ 3 days |
| Low | T-021, T-023, T-031 | 3S ≈ 1.5 days |

**Total estimate: ~37 developer-days**

Critical-path: T-001 → T-006 → T-007 → T-011 → T-008 (provider is usable)
Then: T-002 → T-003 → T-005 → T-007 (classifier is wired in)
Then: T-027 → T-028 → T-029 (tests confirm correctness)

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Classifier accuracy is poor for edge cases | Medium | Medium | Configurable weights + boundaries let users tune; `/router test` provides visibility; modifier overrides let users force tier when classifier is wrong |
| Fallback chain adds latency on failures | Low | Medium | Per-entry timeouts prevent long waits; most failures are fast (auth, 4xx) |
| Circular dependency with ProviderRegistry | Low | High | Router is in `ragent-llm` alongside registry; `RouterClient` takes `Arc<ProviderRegistry>` at construction, no circular crate dep |
| Config persistence races | Low | Medium | Write lock on `RwLock<RouterConfig>` serialises updates; file write is last step |
| Weight drift after many user edits | Low | Low | `/router weights reset` restores defaults; normalisation on load |