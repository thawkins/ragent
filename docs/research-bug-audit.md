# Research System Bug Audit — Test Coverage & Failure Modes

**Target crates:** `crates/ragent-research`, `crates/ragent-tui`  
**Focus:** user-reported `/research` issues: (1) command never shows 'ready' when finished, (2) research does not find sources, (3) TUI display is distorted/hard to read.  
**Scope:** identify root causes, suggest minimal fixes that do not change behavior elsewhere, and specify exact regression tests with clear names.

---

## 1. Root-cause summary

| # | Symptom | Root cause | Location |
|---|---|---|---|
| 1 | `/research create` never shows "ready" / completion | The TUI spawns `ResearchSession::run` in a detached `tokio::spawn` task and only `eprintln!`s the result. Nothing updates `self.status` or appends an assistant message when the async task finishes, so the status stays at `"research: writing research/{name}/RESEARCH.md…"`. | `crates/ragent-tui/src/app.rs` lines 13095–13111 |
| 2 | Research finds zero sources | `handle_research_command` builds `ResearchSession::new(manager.clone(), None, None)` — both the web gatherer and local gatherer are hard-coded to `None`. The session therefore skips both discovery phases and produces an empty source list. | `crates/ragent-tui/src/app.rs` line 13091 |
| 3 | TUI display is distorted/hard to read | Research CLI output (`render_list_output`, `render_search_output`, `render_show_output`) is wrapped in triple-backticks by the TUI, then passed through `render_markdown_to_ascii`. That function strips the leading `From: /` guard, converts markdown→HTML→text via `html2text` at width 120, and then runs `normalize_ascii_tables`. The pipeline re-wraps already-formatted plain-text tables and can treat box-drawing characters (`│`, `─`) as table separators even when they are not part of a table, collapsing rows and misaligning columns. | `crates/ragent-tui/src/app.rs` lines 274–318; `normalize_ascii_tables` lines 190–268 |

---

## 2. Detailed findings per crate

### 2.1 `crates/ragent-research`

#### 2.1.1 Session/manager lifecycle

- `ResearchSession::run` correctly emits `SessionEvent::Phase` and `SessionEvent::Done`, and transitions the item status `Draft → InProgress → Complete` via `start_gathering` / `complete_gathering`.
- However, there is **no test** that verifies the final on-disk status is `Complete` after `run()` succeeds; the existing integration test only checks that the file exists and contains the title.
- `ResearchManager::write_document` rewrites supporting files for the current sources, but it does **not delete stale supporting files** from a previous run (e.g. `web-03.md` after the source count drops from 3 to 2). No test covers this.
- `refresh_index()` is called from many mutating methods, but there is only one integration test that checks `INDEX.md` contains the item name; no test checks status columns or counts.

#### 2.1.2 Source gathering

- `WebGatherer` tests cover empty hits, search errors, fetch failures, and max-results limiting. Missing edge case: **all fetches fail after search returns hits** should still surface as an empty (not panicking) result.
- `LocalGatherer::gather_specs` currently ignores the keyword terms and adds every spec under `specs/` up to `max_local_sources` (the `let _ = terms;` line is a dead giveaway). There is no test asserting that specs are filtered by topic terms, so the reported "does not find sources" bug could also mask irrelevant specs being returned.
- `derive_terms` splits on whitespace only and does not strip punctuation, so a topic of `"async/await, tokio!"` yields terms `"async/await,"` and `"tokio!"`, which will rarely match file contents. No tests cover punctuation stripping.
- `LocalGatherer` trusts `glob` to return only files; it has no `is_file()` guard because of testability concerns. This is documented in a comment, but there is no contract test or `LocalTool` trait doc test asserting that production `glob` implementations really do filter directories.

#### 2.1.3 Document / I/O

- `ResearchIo::render_references_index` and `render_index` use `sanitize_inline` to escape `|`, newlines and backticks. No test verifies that a source title containing `|` or a newline does not break the markdown table.
- The assembled `RESEARCH.md` always contains placeholder text when findings/summary are empty. There is no snapshot/ golden-file test ensuring the exact output shape stays stable.

### 2.2 `crates/ragent-tui`

#### 2.2.1 Research command wiring

- `handle_research_command` constructs `ResearchSession` with `None` for both gatherers. This means:
  - Web search is never used, even if the provider supports `websearch`.
  - Local cross-referencing is never used.
  - The only possible result is the skeleton document with zero sources.
- The create path spawns the session into a detached `tokio::spawn` and does not send any completion event back to the TUI. The `TuiResearchObserver` forwards events to the event bus, but the `ToolCallStart` and `ToolCallEnd` events use **different `call_id`s** (`"research-{nanos}"` vs hard-coded `"research-end"`), so the log panel may display the start/end as unrelated entries.

#### 2.2.2 Display pipeline

- `render_markdown_to_ascii` only runs when the text starts with `"From: /"`. Research CLI responses are formatted as `"From: /research list\n\n```\n…\n```"`, so they enter the pipeline.
- `html2text::from_read(..., 120)` re-wraps the preformatted block at 120 columns. The subsequent `normalize_ascii_tables` then sees the box-drawing characters produced by the CLI renderers and may treat them as table separators.
- `normalize_ascii_tables` identifies a "table line" simply by presence of `│` **or** a line made only of `─`, `┬`, `┼`, `┴`, ` `. Any unrelated text containing those characters (e.g. a tree diagram, a separator banner, or a code block) can be mis-detected and mangled.
- There are no tests feeding real `ragent_research::render_*_output` strings through `render_markdown_to_ascii`; the existing markdown tests use hand-written markdown and only check loose `contains`.

---

## 3. Suggested minimal fixes

Keep changes scoped to `/research` paths; do not alter general TUI markdown rendering for other slash commands unless it is a pure bug fix.

### 3.1 `ragent-research`

1. **`derive_terms`: strip punctuation.** Add a small helper that removes leading/trailing punctuation from each token before length filtering. This improves keyword matching without changing the public signature.
2. **`LocalGatherer::gather_specs`: actually use `terms`.** Filter `spec_ids` to those whose title contains any term (case-insensitive), and fall back to all specs only when the filter yields fewer than, say, 3 results. This satisfies FR-009’s "cross-reference relevant specs" intent.
3. **`ResearchManager::write_document`: remove stale supporting files.** Before writing, list `sources/web-*.md`, `sources/local-*.md`, `sources/other-*.md` and delete files whose index exceeds the current count for that prefix.
4. **`ResearchIo::render_references_index`: truncate very long cells.** Cap the displayed `path`/`title`/`relevance` lengths to avoid one source with a huge URL distorting the whole table.

### 3.2 `ragent-tui`

1. **Wire real gatherers in `handle_research_command`.** Use the existing `WebGatherer`/`LocalGatherer` factory or expose a helper from `ragent-research` that builds a session from the current tool registry / provider, rather than passing `None, None`.
2. **Report completion back to the TUI.** Send a completion event (e.g. `Event::TextDelta` or `Event::AgentComplete`) from the spawned task and update `self.status` / append a ready message. Alternatively, block on the task in a background future and update state when it resolves.
3. **Match `call_id` in `TuiResearchObserver`.** Store the `call_id` used in `ToolCallStart` and reuse it in `ToolCallEnd` so the log panel correctly pairs the start and end.
4. **Bypass markdown conversion for research preformatted output.** Detect the `\n```\n` fence in research responses and return the inner text unchanged, or use a dedicated renderer for `/research` outputs so already-formatted tables are not re-wrapped.

---

## 4. Exact tests to add

Add the tests below to the named files. Each name follows the project convention `test_<component>_<scenario>`.

### 4.1 `crates/ragent-research`

In `crates/ragent-research/tests/test_research_integration.rs`:

- `test_session_run_leaves_item_in_complete_status`
  - Runs a session with fake web/local tools and asserts `mgr.show(name).status == ResearchStatus::Complete`.
- `test_session_run_web_fetch_all_fail_returns_empty_sources`
  - Search returns 3 hits, all fetch calls fail, outcome has 0 sources.
- `test_write_document_removes_stale_supporting_files`
  - Write doc with 3 web sources, then rewrite with 1 web source; assert `sources/web-02.md` and `web-03.md` are gone.
- `test_refresh_index_reflects_complete_status`
  - After session, read `research/INDEX.md` and assert the row contains `"complete"`.

In `crates/ragent-research/src/local_gatherer.rs` tests:

- `test_gather_specs_filters_by_terms`
  - Specs `auth-refactor` and `model-router`; terms `["auth"]`; only `auth-refactor` is captured.
- `test_gather_specs_falls_back_to_all_when_filter_too_narrow`
  - Terms `["zzzz"]`; because fewer than 3 match, all specs are returned.
- `test_derive_terms_strips_punctuation`
  - Input `"async/await, tokio!"` yields `["async", "await", "tokio"]`.
- `test_derive_terms_keeps_apostrophes_in_contractions`
  - Optional: ensure `"don't"` is not split into `"don"` and `"t"`.

In `crates/ragent-research/src/io.rs` tests:

- `test_render_references_index_sanitizes_pipes_and_newlines`
  - Source with title `"a|b`c\nline"` renders a single unbroken table row.
- `test_render_index_includes_status_column`
  - Build two `IndexEntry` rows and assert the markdown table contains `"complete"` and `"draft"`.

### 4.2 `crates/ragent-tui`

Create `crates/ragent-tui/tests/test_research_tui.rs`:

- `test_research_session_create_uses_configured_gatherers`
  - Construct a `ResearchSession` with fake web/local tools via whatever helper the fix exposes; assert it returns sources (regression for the `None, None` bug).
- `test_tui_research_observer_pairs_tool_call_start_and_end_ids`
  - Publish events through a test `EventBus` subscriber and assert that a `ToolCallStart` with a given `call_id` is followed by a `ToolCallEnd` with the **same** `call_id`.
- `test_tui_research_create_status_updated_on_completion`
  - Spawn the task and drive it to completion inside the test runtime; assert the app status changes from `"research: writing…"` to a ready message.
- `test_render_markdown_to_ascii_preserves_research_list_output`
  - Pass the real output of `ragent_research::render_list_output(&sample_rows)` wrapped in triple backticks through `render_markdown_to_ascii`; assert column headers remain aligned and no rows are collapsed.
- `test_render_markdown_to_ascii_preserves_research_show_output`
  - Same for `render_show_output`.
- `test_normalize_ascii_tables_does_not_mangle_non_table_box_drawing`
  - Input contains `│` inside a plain paragraph (not a table); assert it passes through unchanged.
- `test_render_markdown_to_ascii_code_block_not_rewrapped_as_table`
  - Markdown with a fenced code block containing box-drawing characters is rendered with the code block intact, not normalized as a table.

In `crates/ragent-tui/tests/test_markdown_table.rs`:

- `test_render_markdown_table_exact_width_stability`
  - Use a known input and assert the output line lengths are deterministic for a given input, rather than only loose `contains`.
- `test_normalize_ascii_tables_handles_empty_cells`
  - Table row with an empty middle cell does not panic or shift columns.

---

## 5. Flaky / environment-dependent patterns observed

- `cargo test -p ragent-tui` currently fails 9 tests in `tests/test_slash_commands.rs` with `cwd: NotFound` and `file_menu` assertions. These failures are unrelated to `/research` but will mask any new research-TUI tests that rely on a valid working directory. They should be stabilized before adding new directory-sensitive tests.
- `App::new` calls `Self::detect_git_branch()` which spawns `git rev-parse`. This is not currently mocked and may fail or return different values in CI; any new TUI tests that instantiate `App` inherit this flakiness.
- `make_app()` in the markdown tests constructs a full `App` with an in-memory storage and default provider registry, which is heavy. Consider a lighter test helper that exposes only `render_markdown_to_ascii`/`normalize_ascii_tables` without spinning up the full app.

---

## 6. Current test counts

- `ragent-research`: 174 unit tests + 6 integration tests + 1 doc test; all passing.
- `ragent-tui`: 29 test files, but 9 unrelated failures in `test_slash_commands.rs` at the time of this audit.

Recommended next step: implement the minimal fixes in sections 3.1 and 3.2, then add the regression tests from section 4 in order of priority, starting with the two highest-impact tests:

1. `test_research_session_create_uses_configured_gatherers`
2. `test_tui_research_create_status_updated_on_completion`
3. `test_render_markdown_to_ascii_preserves_research_list_output`
