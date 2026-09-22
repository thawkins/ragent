---
status: implemented
audit:
  - { time: 1787493532, from: "none", to: "draft", actor: "system" }
---
# Optimise `ragent-agent` Crate Performance

## Context

`ragent-agent` is the core agent/runtime crate in the ragent workspace. It owns
the session processor, agent loop, tool dispatch, memory and compaction
pipelines, background tasks, team coordination, MCP bridging, skill handling,
and reference resolution. Recent exploratory code review identified many small
but cumulative inefficiencies: repeated allocations, redundant storage round
trips, lock contention, per-call construction of expensive objects such as
`reqwest::Client`, and frequent cloning of large agent prompts and sub-agent
results.

The goal is to apply a focused, low-risk performance pass that removes the
hottest overheads without redesigning the agent architecture. Optimisations
must preserve existing behaviour, test coverage, and public API stability
unless a deliberate breaking change is explicitly documented.

## Requirements

### Ubiquitous requirements

FR-001: The optimisation work **shall** target only `crates/ragent-agent` and
its direct dependencies, leaving other workspace crates untouched unless a
change there is strictly necessary for the optimisation.

FR-002: Every optimisation **shall** be accompanied by existing or newly-added
tests that continue to pass; regressions in behaviour, public API surface, or
performance characteristics are not acceptable.

FR-003: The optimisation work **shall** follow the EARS-driven
specification/plan discipline: this `AGENTSPEC.md` and the companion
`AGENTPLAN.md` define scope, and individual tasks are tracked to completion.

### Event-driven requirements

FR-004: When the session processor dispatches a tool, it **shall** avoid
rebuilding shared context objects (e.g. `ToolContext`, `PermissionChecker`
state) for every tool call; context that is constant for the session **shall**
be reused or shared.

FR-005: When the agent loop builds the system prompt for a sub-agent or
background agent, it **shall** avoid deep-cloning the entire `AgentInfo` prompt
string on every resolution.

FR-006: When `wait_agents` drains completed sub-agents, it **shall** avoid
cloning full `TaskEntry` results into intermediate collections when only the
final aggregated payload is needed by the caller.

FR-007: When a URL reference (`@https://...`) or HTTP MCP request is resolved,
the implementation **shall** reuse a single `reqwest::Client` rather than
constructing a new client per request.

FR-008: When `BackgroundTaskService` flushes task output to storage, it **shall**
write only when stdout/stderr/progress actually changed, and **shall** batch
the output and status updates into a single storage transaction where possible.

FR-009: When fuzzy file references are resolved repeatedly, the
implementation **shall** cache the project file list per working directory with
an mtime/TTL invalidation strategy, or reuse an existing index/glob cache.

FR-010: When memory visualisation is generated, the implementation **shall**
fetch tags for all memory rows in a single query rather than issuing one SQLite
query per row.

FR-011: When skill substitution is applied, the implementation **shall** avoid
multiple full-string replacements by scanning the template once and building
the result in a single pass.

FR-012: When the goal evaluator summarises conversation history, it **shall**
build the context string in a single pass with pre-sized buffers instead of
allocating a formatted string per message and an intermediate `Vec`.

### State-driven requirements

FR-013: While the system prompt is being composed, `AgentInfo` and large prompt
strings **shall** be reference-counted (`Arc<str>` / `Arc<AgentInfo>`) so that
read-only resolution paths do not copy prompt text.

FR-014: While tool definitions are serialised to the LLM, the tool registry
**shall** be able to return borrowed or shared definitions rather than cloning
an entire `Vec<ToolDef>` for every request.

FR-015: While `BackgroundTaskService` state is accessed concurrently, related
hash maps **shall** be consolidated under a single lock or replaced with a
concurrent map (`DashMap`) to reduce multi-lock acquisition convoys.

FR-016: While `AgentManager` tracks sub-agent tasks, `TaskEntry` **shall** be
stored in a way that avoids cloning large result strings on read-heavy paths
such as `list_agents`, `running_background_count`, and `drain_completed`.

FR-017: While permission checks are performed for file tools, the
implementation **shall** avoid a blocking `canonicalize` syscall on every call
by caching canonical path results per step.

FR-018: While the bundled system prompt templates are constructed, they
**shall** be built once and reused rather than allocated on every composition.

### Optional requirements

FR-019: The reference resolver **may** replace manual char-boundary truncation
with `floor_char_boundary` where the toolchain permits.

FR-020: The team mailbox poll loop **may** increase its idle interval or skip
mailbox reads when no notification has been received, to reduce idle disk I/O.

### Unwanted requirements

FR-021: The optimisation **shall not** remove or weaken existing security
behaviour in the permission system, bash safety layers, or file-path guards.

FR-022: The optimisation **shall not** introduce `unsafe` blocks, hidden
clones inside hot iterators, or unwrap/expect on production paths.

FR-023: The optimisation **shall not** redesign the agent loop, MCP protocol,
or storage schema; changes are limited to low-overhead replacements of existing
implementation patterns.

FR-024: The optimisation **shall not** change user-visible output strings or
CLI/TUI commands beyond what is required for the performance improvements.

## Glossary

- **Hot path** — A code path that executes frequently during normal agent
  operation, such as tool dispatch, prompt composition, and message processing.
- **`AgentInfo`** — The struct that holds a built-in or custom agent's name,
  description, and full system prompt text.
- **`TaskEntry`** — The in-memory record of a sub-agent or background task,
  including its result string.
- **`reqwest::Client`** — The HTTP client type used for URL reference fetching,
  update checks, and HTTP MCP transport.
- **`DashMap`** — A concurrent hash map implementation used to reduce lock
  contention compared with `RwLock<HashMap>`.
- **`Arc<str>`** — A reference-counted, immutable string slice used to share
  large prompt text without cloning.
- **EARS** — Easy Approach to Requirements Syntax, the notation used for
  requirements in this specification.
