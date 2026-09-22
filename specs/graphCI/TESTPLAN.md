---
status: draft
---

# Manual Test Plan — CodeIndex Graph Extension (graphCI)

This is a **manual** test plan. It contains human-readable test cases to be
executed by a tester in the ragent TUI. No automated test code is included.

## Prerequisites

1. **Rust toolchain** — Rust 1.85+ (edition 2024), `cargo build` succeeds for
   the ragent workspace.
2. **ragent binary** — built from the working tree that includes the graphCI
   extension (`cargo build` in the project root).
3. **A test project** — the ragent codebase itself is ideal. It has 1000+
   indexed files, 23000+ symbols, and a rich cross-file symbol graph.
4. **Code index enabled** — the tester has run `/codeindex on` and
   `/codeindex reindex` at least once so the SQLite index is populated.
5. **LLM provider configured** — at least one provider (Anthropic, OpenAI, or
   Ollama) is configured so the agent loop is available for tool-invocation
   tests. A local Ollama model is sufficient.
6. **Terminal** — a standard terminal with at least 120×40 character
   dimensions for the ratatui TUI.

## Test Cases

### TC-001 — Build the semantic graph

**Preconditions:**
- Code index is enabled and has been re-indexed at least once.
- The graph has NOT been built yet (fresh session or `graph_edges` table is
  empty).

**Steps:**

1. Launch ragent in the project root:
   ```
   ragent
   ```
2. At the TUI prompt, type:
   ```
   /codeindex graph build
   ```
   and press **Enter**.

3. Observe the assistant-text panel for the result message.

**Test data to enter:**
- Command: `/codeindex graph build`

**Expected results:**
- The output displays a success message containing:
  - The total number of edges created.
  - A breakdown of `EXTRACTED` vs `INFERRED` edge counts.
  - The elapsed time in milliseconds.
- The status bar shows a message like `codeindex: graph built (N edges)`.
- No error message appears.

---

### TC-002 — Graph build when index is disabled

**Preconditions:**
- Code index is disabled (`/codeindex off` has been run, or the index was
  never enabled).

**Steps:**

1. Launch ragent.
2. Type `/codeindex off` and press **Enter** (if not already off).
3. Type `/codeindex graph build` and press **Enter**.

**Test data to enter:**
- Command: `/codeindex graph build`

**Expected results:**
- The output displays the "not active" warning, e.g.:
  `⚠️ Code index is not active. Enable it first with /codeindex on.`
- No graph building occurs.
- The status bar shows `codeindex: not active`.

---

### TC-003 — Explain a well-known symbol

**Preconditions:**
- Code index is enabled.
- Graph has been built (TC-001 completed).

**Steps:**

1. Launch ragent.
2. Type `/codeindex explain CodeIndex` and press **Enter**.

**Test data to enter:**
- Command: `/codeindex explain CodeIndex`

**Expected results:**
- The output displays a structured explanation:
  - **Node:** `CodeIndex` with source file path, line number, community ID,
    and degree count.
  - **Connections:** a list of incoming and outgoing edges, each showing:
    - Direction: `-->` (outgoing) or `<--` (incoming).
    - Connected symbol name.
    - Edge kind in brackets: `[calls]`, `[imports]`, `[references]`, etc.
    - Confidence tag: `[EXTRACTED]` or `[INFERRED]`.
  - The list is limited to the top 50 connections.
- The status bar shows `codeindex: explain CodeIndex`.

---

### TC-004 — Explain a non-existent symbol

**Preconditions:**
- Code index is enabled.
- Graph has been built.

**Steps:**

1. Launch ragent.
2. Type `/codeindex explain NonExistentSymbol12345` and press **Enter**.

**Test data to enter:**
- Command: `/codeindex explain NonExistentSymbol12345`

**Expected results:**
- The output displays a message like:
  `No symbol named 'NonExistentSymbol12345' found in the index.`
- No crash or panic occurs.
- The status bar shows `codeindex: explain (not found)`.

---

### TC-005 — Find shortest path between two symbols

**Preconditions:**
- Code index is enabled.
- Graph has been built.

**Steps:**

1. Launch ragent.
2. Type `/codeindex path CodeIndex ParserRegistry` and press **Enter**.

**Test data to enter:**
- Command: `/codeindex path CodeIndex ParserRegistry`

**Expected results:**
- The output displays a shortest path, e.g.:
  ```
  Shortest path (N hops):
    CodeIndex --calls--> full_reindex() --calls--> ParserRegistry
  ```
  with each hop showing the edge kind and confidence tag.
- If no path exists, the output says:
  `No path found between 'CodeIndex' and 'ParserRegistry'.`
- The status bar shows `codeindex: path (N hops)` or `codeindex: path (none)`.

---

### TC-006 — Path with non-existent target

**Preconditions:**
- Code index is enabled.
- Graph has been built.

**Steps:**

1. Launch ragent.
2. Type `/codeindex path CodeIndex DoesNotExistXYZ` and press **Enter**.

**Test data to enter:**
- Command: `/codeindex path CodeIndex DoesNotExistXYZ`

**Expected results:**
- The output displays:
  `Symbol 'DoesNotExistXYZ' not found in the index.`
- No crash occurs.

---

### TC-007 — List communities

**Preconditions:**
- Code index is enabled.
- Graph has been built.
- Community detection has been run (communities are computed during graph
  build or via a separate step).

**Steps:**

1. Launch ragent.
2. Type `/codeindex communities` and press **Enter**.

**Test data to enter:**
- Command: `/codeindex communities`

**Expected results:**
- The output displays a list of detected communities:
  - Each community shows: community ID, auto-generated label, member count.
  - Communities are sorted by member count (descending).
  - At least 1 community is shown (for the ragent codebase, expect ≥ 3).
- The status bar shows `codeindex: communities (N)`.

---

### TC-008 — List god nodes

**Preconditions:**
- Code index is enabled.
- Graph has been built.

**Steps:**

1. Launch ragent.
2. Type `/codeindex godnodes` and press **Enter**.

**Test data to enter:**
- Command: `/codeindex godnodes`

**Expected results:**
- The output displays the top 10 most-connected symbols:
  - Each row shows: rank, symbol name, source file, degree (edge count).
  - The list is sorted by degree (descending).
- The status bar shows `codeindex: godnodes`.

---

### TC-009 — Export graph to JSON and report

**Preconditions:**
- Code index is enabled.
- Graph has been built.

**Steps:**

1. Launch ragent.
2. Type `/codeindex graph export` and press **Enter**.
3. Exit ragent (press **q** or type `/quit` and press **Enter**).
4. In a terminal, list the `.ragent/codeindex/` directory:
   ```
   ls -la .ragent/codeindex/
   ```
5. Open `graph.json` in a text editor or `jq`:
   ```
   jq '.nodes | length' .ragent/codeindex/graph.json
   jq '.edges | length' .ragent/codeindex/graph.json
   ```
6. Open `GRAPH_REPORT.md` in a text editor or `cat` it:
   ```
   cat .ragent/codeindex/GRAPH_REPORT.md
   ```

**Test data to enter:**
- Command: `/codeindex graph export`

**Expected results:**
- The TUI output displays the paths to both files:
  - `.ragent/codeindex/graph.json`
  - `.ragent/codeindex/GRAPH_REPORT.md`
- `graph.json` exists and is valid JSON.
- `jq '.nodes | length'` returns a number > 0.
- `jq '.edges | length'` returns a number > 0.
- `GRAPH_REPORT.md` exists and contains Markdown with:
  - A title/heading.
  - A "God Nodes" or "Key Concepts" section.
  - A "Communities" section.
  - A "Suggested Questions" section (may be empty or have placeholder text).

---

### TC-010 — Graph query when graph is empty

**Preconditions:**
- Code index is enabled and has been re-indexed.
- Graph has NOT been built (fresh database, or edges table was cleared).

**Steps:**

1. Launch ragent.
2. Type `/codeindex explain CodeIndex` and press **Enter**.
3. Type `/codeindex path CodeIndex ParserRegistry` and press **Enter**.
4. Type `/codeindex communities` and press **Enter**.
5. Type `/codeindex godnodes` and press **Enter**.

**Test data to enter:**
- Commands: the four above.

**Expected results:**
- Each command outputs a message instructing the user to run
  `/codeindex graph build` first.
- No crash or panic occurs.
- The status bar shows `codeindex: graph empty` for each.

---

### TC-011 — Backward compatibility: existing `/codeindex` sub-commands

**Preconditions:**
- Code index is enabled.
- Graph extension is installed.

**Steps:**

1. Launch ragent.
2. Type `/codeindex show` and press **Enter**. Verify status and stats are
   displayed (same as before the extension).
3. Type `/codeindex lang` and press **Enter**. Verify the language list is
   displayed.
4. Type `/codeindex reindex` and press **Enter**. Verify re-indexing
   completes and reports file/symbol counts.
5. Type `/codeindex rebuild` and press **Enter**. Verify FTS rebuild
   completes.
6. Type `/codeindex help` and press **Enter**. Verify the help table includes
   all original sub-commands PLUS the new graph sub-commands.
7. Type `/codeindex off` and press **Enter**. Verify the index is disabled.
8. Type `/codeindex on` and press **Enter**. Verify the index is re-enabled.

**Test data to enter:**
- Commands: `/codeindex show`, `/codeindex lang`, `/codeindex reindex`,
  `/codeindex rebuild`, `/codeindex help`, `/codeindex off`, `/codeindex on`

**Expected results:**
- Each sub-command produces the same output format as before the extension.
- `/codeindex help` additionally lists the new sub-commands:
  `graph build`, `graph export`, `explain`, `path`, `communities`,
  `godnodes`.
- No existing sub-command is missing from the help table.
- No error occurs during any of the commands.

---

### TC-012 — Backward compatibility: existing `codeindex_*` LLM tools

**Preconditions:**
- Code index is enabled and has been re-indexed.
- An LLM provider is configured and the agent loop is available.

**Steps:**

1. Launch ragent.
2. Type the following prompt and press **Enter**:
   ```
   Use the codeindex_search tool to search for "ParserRegistry"
   ```
3. Wait for the agent to invoke `codeindex_search` and display results.
4. Type:
   ```
   Use the codeindex_symbols tool to list all structs in the codeindex crate
   ```
   and press **Enter**.
5. Wait for the agent to invoke `codeindex_symbols`.
6. Type:
   ```
   Use the codeindex_references tool to find all references to "CodeIndex"
   ```
   and press **Enter**.
7. Wait for the agent to invoke `codeindex_references`.
8. Type:
   ```
   Use the codeindex_dependencies tool to show imports for "src/lib.rs"
   ```
   and press **Enter**.
9. Wait for the agent to invoke `codeindex_dependencies`.
10. Type:
    ```
    Use the codeindex_status tool to show the current index status
    ```
    and press **Enter**.
11. Wait for the agent to invoke `codeindex_status`.

**Test data to enter:**
- Prompts: as listed above (each asks the agent to call a specific tool).

**Expected results:**
- The agent successfully invokes each of the six existing `codeindex_*`
  tools.
- Each tool returns results in the same format as before the extension.
- No tool is missing from the registry.
- No permission error occurs (codeindex tools are hardwired always-allowed).

---

### TC-013 — New `codeindex_explain` LLM tool

**Preconditions:**
- Code index is enabled.
- Graph has been built.
- An LLM provider is configured.

**Steps:**

1. Launch ragent.
2. Type:
   ```
   Use the codeindex_explain tool to explain the "CodeIndex" symbol
   ```
   and press **Enter**.
3. Wait for the agent to invoke `codeindex_explain`.

**Test data to enter:**
- Prompt: `Use the codeindex_explain tool to explain the "CodeIndex" symbol`

**Expected results:**
- The agent invokes the `codeindex_explain` tool with `symbol: "CodeIndex"`.
- The tool returns a structured output with node metadata and connections.
- The output is consistent with TC-003 (same underlying API).

---

### TC-014 — New `codeindex_path` LLM tool

**Preconditions:**
- Code index is enabled.
- Graph has been built.
- An LLM provider is configured.

**Steps:**

1. Launch ragent.
2. Type:
   ```
   Use the codeindex_path tool to find the path from "CodeIndex" to "ParserRegistry"
   ```
   and press **Enter**.
3. Wait for the agent to invoke `codeindex_path`.

**Test data to enter:**
- Prompt: as above.

**Expected results:**
- The agent invokes `codeindex_path` with `from: "CodeIndex"` and
  `to: "ParserRegistry"`.
- The tool returns the shortest path or a "no path" message.

---

### TC-015 — Graph tools during background reindex (busy state)

**Preconditions:**
- Code index is enabled.
- Graph has been built.
- A large codebase is indexed so that a reindex takes several seconds.

**Steps:**

1. Launch ragent.
2. Type `/codeindex reindex` and press **Enter**.
3. While the reindex is in progress (status bar shows "reindexing"),
   immediately type:
   ```
   Use the codeindex_explain tool to explain "CodeIndex"
   ```
   and press **Enter**.
4. Wait for the agent to respond.

**Test data to enter:**
- `/codeindex reindex` (to trigger background reindex).
- Prompt for `codeindex_explain` (while reindex runs).

**Expected results:**
- The `codeindex_explain` tool returns a `codeindex_busy` response
  (non-blocking), consistent with the behaviour of existing codeindex tools
  when the store lock is held.
- No deadlock or hang occurs.
- Once the reindex completes, a subsequent `codeindex_explain` call succeeds
  normally.

---

### TC-016 — No new top-level slash command

**Preconditions:**
- graphCI extension is installed.

**Steps:**

1. Launch ragent.
2. Type `/graph` and press **Enter**.
3. Type `/explain` and press **Enter**.
4. Type `/path` and press **Enter**.
5. Type `/communities` and press **Enter**.
6. Type `/godnodes` and press **Enter**.

**Test data to enter:**
- Commands: `/graph`, `/explain`, `/path`, `/communities`, `/godnodes`

**Expected results:**
- Each command is NOT recognised as a valid top-level slash command (the TUI
  shows "unknown command" or passes it as a prompt to the LLM).
- The only valid entry point for graph features is `/codeindex <subcommand>`.

---

## Cleanup

After all manual tests are complete:

1. Delete the generated graph artifacts:
   ```
   rm -f .ragent/codeindex/graph.json
   rm -f .ragent/codeindex/GRAPH_REPORT.md
   ```
2. Optionally clear the graph edges from the database:
   - Run `/codeindex reindex` to rebuild the index from scratch (this will
     also rebuild the graph if edge derivation is wired into reindex).
3. If the test project was a clone of the ragent repo, discard any
   `.ragent/codeindex/` changes with:
   ```
   git checkout .ragent/codeindex/
   ```
   or simply delete the directory:
   ```
   rm -rf .ragent/codeindex/
   ```
4. Disable the code index:
   ```
   /codeindex off
   ```