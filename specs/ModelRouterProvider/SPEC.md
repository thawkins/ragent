---
status: draft
audit:
  - { time: 1780840385, from: "none", to: "draft", actor: "system" }
---
# ModelRouterProvider — Intelligent Model Routing Virtual Provider

## Overview

This specification defines a **virtual provider** for ragent that acts as an
intelligent model router. Inspired by [FreeRouter](https://github.com/openfreerouter/freerouter),
the router analyses every prompt using a **14-dimension weighted classifier** and
automatically selects the cheapest model that can satisfy the request. It presents
itself as a standard `Provider` + `LlmClient` implementation so it plugs into the
existing provider registry with zero changes to the session processor.

Key capabilities:

- **14-dimension prompt classification** — token count, vocabulary complexity,
  syntax complexity, domain specificity, ambiguity, context dependency,
  reasoning depth, creativity level, emotional complexity, multimodality,
  instruction complexity, knowledge recency, code complexity, and
  mathematical complexity.
- **Tier-based routing** — prompts are classified into one of four tiers
  (SIMPLE, MEDIUM, COMPLEX, REASONING), each mapped to an ordered list of
  provider/model pairs with automatic fallback.
- **Prompt modifiers** — explicit tier overrides via `/simple`, `[complex]`,
  `deep mode:` and similar prefixes, stripped before forwarding to the LLM.
- **Automatic fallback** — when the primary model for a tier is unavailable
  (timeout, rate-limit, auth error), the next model in the tier's fallback
  chain is tried transparently.
- **Slash commands** — `/router` family for setup, configuration, monitoring,
  and testing.
- **Config-driven** — all tier mappings, classifier weights, and fallback
  chains live in `ragent.json` and can be reloaded without restart.

## Requirements

### Virtual Provider

**FR-001** (Ubiquitous) The system shall implement the `Provider` trait for a
virtual provider with `id()` returning `"router"` and `name()` returning
`"Model Router"`.

**FR-002** (Ubiquitous) The system shall implement the `LlmClient` trait for a
`RouterClient` that intercepts `chat()` calls, classifies the prompt, selects a
target provider/model according to the active tier mapping, and delegates the
request to the resolved provider's `LlmClient`.

**FR-003** (Ubiquitous) The system shall register the router provider in
`create_default_registry()` so it appears alongside existing providers without
requiring explicit user configuration.

### 14-Dimension Classifier

**FR-004** (Ubiquitous) The system shall classify each prompt by scoring it
across the following 14 dimensions:

| # | Dimension | What It Measures |
|---|-----------|-----------------|
| 1 | Token count | Length of the message |
| 2 | Vocabulary complexity | Frequency of rare or technical words |
| 3 | Syntax complexity | Depth of nested clauses and conditionals |
| 4 | Domain specificity | Presence of specialised terminology |
| 5 | Ambiguity | Open-endedness of the request |
| 6 | Context dependency | Reliance on prior conversation |
| 7 | Reasoning depth | Number of logical inference steps required |
| 8 | Creativity level | Degree of original generation needed |
| 9 | Emotional complexity | Nuance in tone or sentiment |
| 10 | Multimodality | References to images, files, or non-text content |
| 11 | Instruction complexity | Count of distinct steps or constraints |
| 12 | Knowledge recency | Need for up-to-date information |
| 13 | Code complexity | Programming difficulty (language, paradigm, scope) |
| 14 | Mathematical complexity | Formal mathematics, proofs, calculations |

**FR-005** (Ubiquitous) The system shall combine the 14 dimension scores using
configurable weights into a composite complexity score.

**FR-006** (State-driven) While the composite complexity score falls below the
SIMPLE→MEDIUM boundary threshold, the system shall classify the prompt as tier
SIMPLE.

**FR-007** (State-driven) While the composite complexity score is at or above
the SIMPLE→MEDIUM boundary and below the MEDIUM→COMPLEX boundary, the system
shall classify the prompt as tier MEDIUM.

**FR-008** (State-driven) While the composite complexity score is at or above
the MEDIUM→COMPLEX boundary and below the COMPLEX→REASONING boundary, the
system shall classify the prompt as tier COMPLEX.

**FR-009** (State-driven) While the composite complexity score is at or above
the COMPLEX→REASONING boundary, the system shall classify the prompt as tier
REASONING.

**FR-010** (Optional) Where the user has overridden classifier dimension weights
in `ragent.json` under `provider.router.weights`, the system shall apply those
weights in place of the built-in defaults.

### Tier Routing and Fallback

**FR-011** (Ubiquitous) The system shall maintain a tier configuration mapping
each tier (SIMPLE, MEDIUM, COMPLEX, REASONING) to an ordered list of
provider/model pairs. The first entry is the primary target; subsequent
entries are fallback candidates.

**FR-012** (Event-driven) When the router selects a target provider/model for a
tier and the provider returns an error (timeout, rate-limit, 5xx, auth failure),
the system shall retry the request with the next provider/model in the tier's
fallback list.

**FR-013** (State-driven) While every provider/model in the tier's fallback
list has been exhausted without a successful response, the system shall return
an error to the caller indicating that all models for the tier are unavailable.

**FR-014** (Optional) Where a tier entry specifies a `timeout_ms` field, the
system shall apply that timeout to requests routed to that entry, overriding the
default router timeout.

**FR-015** (Event-driven) When a provider/model in a tier's fallback list
succeeds after the primary failed, the system shall log a warning with the
original error and the fallback model that succeeded.

### Prompt Modifiers

**FR-016** (Event-driven) When a prompt begins with a slash prefix
(`/simple`, `/medium`, `/complex`, `/max`, `/reasoning`, `/basic`, `/cheap`,
`/balanced`, `/advanced`, `/think`, `/deep`), the system shall override the
classifier's tier decision and route to the corresponding tier, stripping the
prefix from the prompt before forwarding.

**FR-017** (Event-driven) When a prompt begins with a bracket prefix
(`[simple]`, `[medium]`, `[complex]`, `[max]`, `[reasoning]`, `[basic]`,
`[cheap]`, `[balanced]`, `[advanced]`, `[think]`, `[deep]`), the system shall
override the classifier's tier decision and route to the corresponding tier,
stripping the bracket prefix from the prompt before forwarding.

**FR-018** (Event-driven) When a prompt begins with a word prefix
(`deep mode:`, `basic mode,`, `simple mode:`, `complex mode:`, `reasoning
mode:`, `medium mode:`), the system shall override the classifier's tier
decision and route to the corresponding tier, stripping the word prefix from
the prompt before forwarding.

**FR-019** (Ubiquitous) The system shall map modifier aliases to tiers
according to the following table:

| Aliases | Target Tier |
|---------|-------------|
| `simple`, `basic`, `cheap` | SIMPLE |
| `medium`, `balanced` | MEDIUM |
| `complex`, `advanced` | COMPLEX |
| `max`, `reasoning`, `think`, `deep` | REASONING |

**FR-020** (Unwanted) The system shall not forward prompt modifiers (slash,
bracket, or word prefixes) to the underlying LLM — they must be stripped
before the request is sent.

### Context-Aware Classification

**FR-021** (Optional) Where the chat request contains conversation history,
the system shall include the last N messages (configurable, default 3) in the
classification context to improve accuracy for follow-up questions.

**FR-022** (Event-driven) When a prompt modifier is detected, the system shall
skip context-aware classification and use the modifier's tier directly.

### Configuration

**FR-023** (Ubiquitous) The system shall accept router configuration via
`ragent.json` under the `provider.router` key with the following structure:

```jsonc
{
  "provider": {
    "router": {
      "enabled": true,
      "tiers": {
        "SIMPLE": {
          "models": [
            { "provider": "ollama", "model": "qwen3:0.6b" },
            { "provider": "anthropic", "model": "claude-haiku-4-5-20250315" }
          ],
          "timeout_ms": 15000
        },
        "MEDIUM": {
          "models": [
            { "provider": "anthropic", "model": "claude-sonnet-4-20250514" }
          ]
        },
        "COMPLEX": {
          "models": [
            { "provider": "anthropic", "model": "claude-opus-4-20250115" }
          ]
        },
        "REASONING": {
          "models": [
            { "provider": "anthropic", "model": "claude-opus-4-20250115" }
          ],
          "timeout_ms": 120000
        }
      },
      "weights": {
        "token_count": 0.07,
        "vocabulary_complexity": 0.08,
        "syntax_complexity": 0.07,
        "domain_specificity": 0.08,
        "ambiguity": 0.07,
        "context_dependency": 0.07,
        "reasoning_depth": 0.10,
        "creativity_level": 0.07,
        "emotional_complexity": 0.05,
        "multimodality": 0.07,
        "instruction_complexity": 0.08,
        "knowledge_recency": 0.05,
        "code_complexity": 0.08,
        "mathematical_complexity": 0.06
      },
      "boundaries": {
        "simple_medium": 0.25,
        "medium_complex": 0.50,
        "complex_reasoning": 0.75
      },
      "context_messages": 3,
      "default_timeout_ms": 30000
    }
  }
}
```

**FR-024** (Ubiquitous) The system shall provide built-in default tier
mappings, classifier weights, and boundary thresholds so that the router
functions without a `provider.router` configuration block.

**FR-025** (Event-driven) When the `/router reload` slash command is issued,
the system shall reload the router configuration from `ragent.json` without
restarting the application.

**FR-026** (State-driven) While the `provider.router.enabled` field is `false`
or absent, the router provider shall still be registered but shall pass all
requests through to the MEDIUM tier default model without classification.

### Slash Commands

**FR-027** (Ubiquitous) The system shall provide a `/router` slash command
prefix with the following subcommands:

| Subcommand | Description |
|---|---|
| `/router on` | Enable the router (set `enabled: true`) |
| `/router off` | Disable the router (set `enabled: false`) |
| `/router status` | Show router state, current tier, and enabled/disabled status |
| `/router tiers` | Display all tier mappings and their model lists |
| `/router tier <name> set <provider>/<model>` | Set the primary model for a tier |
| `/router tier <name> add <provider>/<model>` | Append a fallback model to a tier |
| `/router tier <name> remove <provider>/<model>` | Remove a model from a tier's list |
| `/router weights` | Display the 14 dimension weights |
| `/router weights set <dimension> <value>` | Override a single dimension weight |
| `/router weights reset` | Restore built-in default weights |
| `/router boundaries` | Display the three tier boundary thresholds |
| `/router boundaries set <boundary> <value>` | Set a boundary threshold (0.0–1.0) |
| `/router test <prompt>` | Classify a prompt and show the dimension scores, composite score, and selected tier |
| `/router stats` | Display cumulative routing statistics (requests per tier, fallback count, total cost estimate) |
| `/router stats reset` | Zero out cumulative routing statistics |
| `/router reload` | Reload router config from `ragent.json` |
| `/router help` | Show available `/router` subcommands |

**FR-028** (Event-driven) When the `/router on` command is issued, the system
shall persist `enabled: true` to the `provider.router` configuration in
`ragent.json` and activate routing for subsequent requests.

**FR-029** (Event-driven) When the `/router off` command is issued, the system
shall persist `enabled: false` to the `provider.router` configuration in
`ragent.json` and deactivate routing for subsequent requests.

**FR-030** (Event-driven) When the `/router test <prompt>` command is issued,
the system shall classify the prompt, display all 14 dimension scores, the
composite score, the tier boundary thresholds, and the resulting tier
selection — without forwarding the request to any provider.

### Routing Statistics

**FR-031** (Ubiquitous) The system shall track cumulative routing statistics
in memory, including: total requests, requests per tier, fallback activations
per tier, and average classification time.

**FR-032** (Event-driven) When a request is routed via the router, the system
shall increment the appropriate tier counter and, if a fallback was used, the
fallback counter for that tier.

**FR-033** (Event-driven) When the `/router stats` command is issued, the
system shall display the cumulative statistics in a human-readable table.

### Tool-Use and Streaming Compatibility

**FR-034** (Ubiquitous) The router shall transparently pass through all
streaming events (text deltas, tool-use blocks, tool-result blocks) from the
resolved provider without modification.

**FR-035** (Ubiquitous) The router shall support tool-use (function calling)
requests by forwarding the full `ChatRequest` including tools and tool_choice
to the resolved provider.

**FR-036** (Unwanted) The system shall not modify, reorder, or filter
streaming events or tool-use content from the resolved provider.

### Thinking / Reasoning

**FR-037** (Optional) Where the resolved provider/model supports the
`ChatRequest.thinking` field, the router shall forward the thinking
configuration unchanged.

**FR-038** (State-driven) While the tier is REASONING and the `ChatRequest`
has no `thinking` configuration, the system shall apply a default
`ThinkingConfig` with `enabled: true` and `level: "high"` if the resolved
model supports thinking.

### Error Handling

**FR-039** (Event-driven) When the classifier encounters an error (e.g.
invalid weights, boundary values out of range), the system shall fall back
to the MEDIUM tier and log a warning.

**FR-040** (Event-driven) When a fallback model request also fails, the
system shall continue to the next fallback entry in the tier until the list
is exhausted.

**FR-041** (Unwanted) The system shall not retry the same provider/model
pair within a single request — each entry in the fallback list is attempted at
most once per request.

### TUI Integration

**FR-042** (Ubiquitous) The system shall register a `router` entry in the
`SLASH_COMMANDS` constant for TUI slash-command autocomplete.

**FR-043** (Event-driven) When the active provider is `router`, the TUI
status bar shall display the current tier (e.g. `router:MEDIUM`) alongside
the provider name.

**FR-044** (State-driven) While the router provider is the active provider and
routing is enabled, the TUI status bar shall display a dedicated "Router"
indicator in the Line 2 right (service status) section showing that the router
is active and the currently selected tier (e.g. `Router:●SIMPLE`,
`Router:●MEDIUM`, `Router:●COMPLEX`, `Router:●REASONING`).

**FR-045** (State-driven) While the router provider is the active provider but
routing is disabled, the TUI status bar shall display a "Router" indicator
with a disabled visual state (e.g. `Router:✗OFF`), using the error color
palette.

**FR-046** (Event-driven) When a prompt is routed and the tier selection
changes, the system shall update the Router status indicator to reflect the
new tier on the next status bar render cycle, without requiring user action.

**FR-047** (State-driven) While the active provider is not `router`, the TUI
status bar shall not display the Router status indicator.

**FR-048** (Ubiquitous) The Router status indicator shall use semantic color
coding: green (`HEALTHY`) for enabled and actively routing, and red (`ERROR`)
for disabled. The tier label shall be displayed in the `TEXT` color with bold
modifier.

**FR-049** (State-driven) While the terminal width is in Minimal responsive
mode (<80 chars), the Router status indicator shall abbreviate to a compact
form showing only the tier initial and enabled icon (e.g. `R:●S`,
`R:●M`, `R:●C`, `R:●R`, `R:✗`).

## Non-Functional Requirements

**NFR-001** (Ubiquitous) The classifier shall complete classification in under
5 milliseconds for prompts up to 10,000 tokens on commodity hardware.

**NFR-002** (Ubiquitous) The router shall add no more than 50 milliseconds of
latency to any request (excluding the time taken by the resolved provider).

**NFR-003** (Ubiquitous) The router module shall introduce zero new crate
dependencies beyond what is already in the workspace.

**NFR-004** (Ubiquitous) The classifier shall be fully deterministic — the
same prompt and configuration shall always produce the same tier.

**NFR-005** (Ubiquitous) Router configuration changes persisted to
`ragent.json` shall survive application restart.

**NFR-006** (Ubiquitous) The Router status indicator shall update within a
single render cycle (≤100 ms) of a tier change event, producing no perceptible
lag in the status bar display.

## Out of Scope

- Cost estimation per request (future: track token usage from provider
  responses and compute cost from model pricing tables).
- Multi-region routing (routing to the same model in different AWS/GCP
  regions for latency optimisation).
- A/B testing or canary routing between models.
- Classifier fine-tuning from observed user corrections.
- External classifier service (the classifier runs in-process).