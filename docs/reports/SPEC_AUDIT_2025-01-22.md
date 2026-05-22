# SPEC.md Audit Report — 2025-01-22

## Executive Summary

This audit compares the current `SPEC.md` (checked into the repository) against the actual
implementation in the workspace crates. The specification is approximately **80 % accurate**
but has accumulated drift since the v0.1.0-alpha.86 release. The most impactful discrepancies
are incorrect tool counts, non-existent tools still listed, an out-of-date version string,
and two major sections that are stubs rather than documentation.

---

## 🔴 Critical Issues (Must Fix)

### 1. Version Number Out of Date

| Location | SPEC.md Claims | Actual (Cargo.toml / CHANGELOG) |
|----------|---------------|--------------------------------|
| §Executive Summary — Project Status | `v0.1.0-alpha.86` | `v0.1.0-alpha.88` |
| Appendix A — Version History | Last entry `alpha.86` | Missing `alpha.87` and `alpha.88` |

**Impact:** Users reading the spec will believe the project is two releases behind.

**Evidence:**
- `Cargo.toml`: `version = "0.1.0-alpha.88"`
- `CHANGELOG.md` last entry: "0.1.0-alpha.88 — Fixed context compaction bug in `compact_history_with_atomic_tool_calls`"

---

### 2. Tool Counts — Multiple Overcounts

The §3.2 and §3.2.1 summary tables claim significantly more tools than are actually
registered in the three tool crates (`ragent-tools-core`, `ragent-tools-extended`,
`ragent-tools-vcs`) and the agent's `create_default_registry()`.

| Category | SPEC Claims | **Actual Registered** | Error |
|----------|-----------|----------------------|-------|
| File Operations | 26 | **14** | +12 ❌ |
| Execution | 10 | **3** | +7 ❌ |
| Memory | 12 | **8** | +4 ❌ |
| Team | 21 | **20** | +1 ❌ |
| Code Index | 6 | **6** | ✅ exact |
| GitHub / GitLab | 29 (10 + 19) | **29** | ✅ exact |
| Office / PDF | 8 (6 + 2) | **8** | ✅ exact |
| Web | 3 | **3** | ✅ exact |
| Sub-agent | 5 | **5** | ✅ exact |

**File Operations — Actual 14 registered:**
- Core: `read`, `write`, `create`, `edit`, `multiedit`, `patch`, `copy_file`, `move_file`, `rm`, `mkdir`, `append_file`, `file_info`, `diff_files`
- Alias: `update_file` (delegates to `write`)

**Execution — Actual 3 registered:**
- Core: `bash`, `bash_reset`
- Alias: `run_code` (delegates to `bash`)

**Memory — Actual 8 registered:**
- `memory_read`, `memory_write`, `memory_replace`, `memory_store`, `memory_recall`, `memory_forget`, `memory_search`, `memory_migrate`

**Team — Actual 20 registered:**
- All 21 listed except `team_cleanup` is not exposed as a standalone tool (it is a team-lead-only operation handled through the team runtime, not the tool registry).

---

### 3. Non-Existent Tools Listed in SPEC.md

The following tools are named in §3.2 but **do not exist** anywhere in the codebase:

| Tool | SPEC Location | Evidence |
|------|--------------|----------|
| `execute_python` | §3.2 Execution Tools | Grepped across all 489 `.rs` files — zero matches. |
| `file_ops_tool` | §3.2 File Operations | Defined in `ragent-agent/src/tool/file_ops_tool.rs` but **never added** to any tool registry. Dead code. |
| `list_files`, `list_directory` | §3.2 File Operation Aliases | Not registered in `aliases.rs` or anywhere else. |
| `find_files` | §3.2 File Operation Aliases | Not registered. |
| `file_search` | §3.2 File Operation Aliases | Not registered. |
| `execute_code`, `execute_bash`, `run_shell_command`, `run_terminal_cmd` | §3.2 Execution Tools | Not registered as aliases. Only `run_code` exists. |

---

### 4. Structural Bug — Misplaced "Part V" Label

**Location:** Line 1198 of `SPEC.md`

**Problem:** The markdown heading `# Part V: External Integrations` appears **between
§5 (Configuration) and §6 (Terminal User Interface)**. According to the Table of Contents,
Part V should start at §18 (GitHub & GitLab Integration).

**Current layout:**
```
§5 Configuration
# Part V: External Integrations   ← WRONG PLACE
§6 Terminal User Interface
§7 HTTP Server & API
...
§17 Orchestrator
§18 GitHub & GitLab Integration   ← Should be Part V start
```

---

## 🟠 High Issues (Should Fix Soon)

### 5. MCP Integration — Stub Section

**Location:** §19

**SPEC content:** `*(Section pending — see Table of Contents for planned content)*`

**Actual implementation:** MCP has substantial real code:
- `crates/ragent-agent/src/mcp/` — discovery, client wrapper, tool bridging
- `rmcp` workspace dependency in `Cargo.toml`
- 10 known MCP servers with auto-discovery (`mcp-server-filesystem`, `mcp-server-github`, `mcp-server-git`, `mcp-server-postgres`, `mcp-server-sqlite`, `mcp-server-memory`, `mcp-server-brave-search`, `mcp-server-fetch`, `mcp-server-puppeteer`, etc.)
- TUI slash commands: `/mcp discover`, `/mcp list`, `/mcp call`
- `McpToolWrapper` is registered in the tool registry

**Recommendation:** Document the current MCP implementation or at minimum remove the
"Section pending" placeholder and replace with a brief description of what's implemented.

---

### 6. Auto-Update Mechanism — Aspirational Section

**Location:** §20

**SPEC content:** `*(To be documented)*`

**Actual implementation:** **Zero code** for auto-update exists in the entire codebase.
Grepped for `auto_update`, `update_check`, `version_check` across all 489 Rust files —
zero matches.

**TUI slash commands exist:** `/update`, `/update install` — but they are stubs that
do nothing (or print "not implemented").

**Recommendation:** Delete §20 entirely or mark it as "Planned — not yet implemented".
Do not present it as a current subsystem.

---

### 7. Missing Configuration Fields in §5.2 Schema

The §5.2 JSON schema example omits many real config keys that exist in
`crates/ragent-config/src/config.rs`:

| Missing Field | Type | Purpose |
|---------------|------|---------|
| `tool_visibility` | `ToolVisibilityConfig` | Toggle visibility of 7 tool families (office, github, gitlab, teams, agents, plan, codeindex) |
| `dirs` | `DirsConfig` | Directory / file path allowlist and denylist |
| `bash` | `BashConfig` | User-defined bash command allowlist / denylist |
| `gitlab` | `GitLabIntegrationConfig` | GitLab instance URL + PAT |
| `internal_llm` | `InternalLlmConfig` | Embedded local LLM for helper tasks |
| `stream` | `StreamConfig` | LLM streaming timeouts and retries |
| `memory` sub-fields | `MemoryConfig` | `decay`, `auto_extract`, `compaction`, `eviction`, `semantic` (only partially shown) |

---

### 8. Missing Environment Variables in §5.3

The environment variable table lists only 7 entries. The actual codebase references
many more provider-specific and feature-specific variables:

- `AZURE_AI_FOUNDRY_API_KEY`
- `AZURE_AI_FOUNDRY_BASE`
- `OLLAMA_API_KEY`
- `HF_TOKEN`
- `HUGGING_FACE_HUB_TOKEN`
- `GEMINI_API_KEY`
- `RAGENT_API_KEY_HUGGINGFACE`
- `RAGENT_TOKEN`
- `RAGENT_LOG_LEVEL`
- `RAGENT_YES`

---

## 🟡 Medium Issues (Nice to Fix)

### 9. Missing Slash Commands in §6.2 Reference Table

The following slash commands are implemented in the TUI (`crates/ragent-tui/src/app/state.rs`,
`app.rs`) but are **not listed** in the §6.2 slash command table:

| Command | Purpose | Code Location |
|---------|---------|---------------|
| `/config show` | Display all application paths and resolved configuration | `app.rs:5130` |
| `/dirs` | Manage directory / file permission lists | `state.rs:614` |
| `/profile on\|off` | Toggle the agent-loop profiler panel | `state.rs:530` |
| `/theme default\|high-contrast` | Switch UI theme | `state.rs:674` |
| `/status [clear]` | Show status message history | `state.rs:679` |
| `/mouse on\|off` | Toggle mouse support | `state.rs:683` |

Note: `/config show` *is* mentioned in the Executive Summary release highlights but
omitted from the formal command reference table.

---

### 10. Release Highlights Block Out of Date

The "Current Release Highlights" bullet list in the Executive Summary is at the alpha.86
level. It should include the alpha.87 and alpha.88 release notes:

- **alpha.87** — Fixed `read` tool instructions (clarified `end_line` is absolute line number);
  strengthened remote push prohibitions in `AGENTS.md`; reorganized SPEC.md sections and numbering.
- **alpha.88** — Fixed context compaction bug in `compact_history_with_atomic_tool_calls`;
  resolved premature loop break when trimming tool call pairs.

---

### 11. Search Category Miscount

§3.2.1 claims "Search: 4 | grep and aliases". In reality `grep` is the **only**
registered search tool. `list` and `glob` are categorized under file operations.
The "4" count appears to include aliases that do not exist.

---

### 12. Provider Count in Executive Summary

The Key Capabilities table says "8 providers" but the provider list (§3.1) documents 10:
Anthropic, OpenAI, GitHub Copilot, Ollama (local), Ollama Cloud, HuggingFace,
Generic OpenAI, Google Gemini, Azure Resource (File), Azure AI Foundry.

---

## ✅ Accurate Sections (No Issues Found)

| Section | What Was Checked | Result |
|---------|-----------------|--------|
| §3.1 LLM Providers | All 10 provider implementation files in `crates/ragent-llm/src/providers/` | ✅ Exact match |
| §2 Architecture — Crate list | All 15 crate directories in `crates/` | ✅ Exact match |
| §2.2 Crate dependency graph | Verified against `Cargo.toml` workspace members | ✅ Exact match |
| §4 Security & Permissions | Permission system, 7-layer bash security, rules evaluation | ✅ Matches code and tests |
| §8 Code Index | 6 tools registered | ✅ Exact match |
| §18 GitHub & GitLab | 29 tools (10 + 19) | ✅ Exact match |
| §3.2 Office / PDF | 8 tools (6 + 2) | ✅ Exact match |
| §3.2 Web | 3 tools | ✅ Exact match |
| §3.2 Sub-agent | 5 tools | ✅ Exact match |
| §14 Teams | Team lifecycle, tasks, messaging | ✅ Matches `ragent-team` crate |
| §4.2 Bash Security | All 7 layers with test counts | ✅ Matches test files |

---

## Recommendations (Prioritized)

### Immediate (Next Commit)
1. Update version string to `v0.1.0-alpha.88` and add alpha.87 / alpha.88 to Appendix A.
2. Correct tool counts in §3.2.1 summary table.
3. Remove non-existent tools (`execute_python`, `file_ops_tool`, dead aliases) from §3.2.
4. Move or delete the misplaced `# Part V: External Integrations` label at line 1198.

### Short Term (This Week)
5. Write a brief §19 MCP Integration section documenting the current discovery + tool-wrapper implementation.
6. Delete §20 Auto-Update or mark it as "Planned — not yet implemented".
7. Expand §5.2 configuration schema to include `tool_visibility`, `dirs`, `bash`, `gitlab`, `internal_llm`, `stream`, and full `memory` sub-fields.
8. Expand §5.3 environment variable table to include all provider-specific keys.

### Medium Term (Next Sprint)
9. Add missing slash commands (`/config show`, `/dirs`, `/profile`, `/theme`, `/status`, `/mouse`) to §6.2.
10. Audit the Swarm (§15), Autopilot (§16), and Orchestrator (§17) sections against actual TUI state fields to confirm they are descriptive rather than aspirational.

---

## Audit Methodology

- **5 parallel `explore` sub-agents** were used to audit:
  1. Version + changelog accuracy
  2. Tool registry counts vs. SPEC claims
  3. Provider implementation files vs. SPEC provider list
  4. Crate directory structure vs. SPEC architecture
  5. New / missing features in code vs. SPEC documentation
- **Targeted `grep` searches** confirmed absence of purported tools (`execute_python`, aliases).
- **File reading** of `SPEC.md` line ranges verified structural issues (Part V label placement).

---

*Report generated: 2025-01-22*
*Auditor: ragent codebase index + manual verification*
