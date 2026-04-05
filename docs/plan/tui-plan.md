# TUI Performance & Consistency Improvement Plan

**Target crate:** `crates/ragent-tui`

## Overview
The TUI module is feature‑rich but exhibits several performance hotspots and consistency issues identified in the existing `performance_findings.md`. This plan outlines concrete, prioritized steps to address the most impactful problems, improve UI responsiveness, and make the codebase more maintainable.

---

## High‑Impact Items (Priority: 0‑1)

### 1. Reduce Unnecessary Redraws (Dirty‑Flag)
- **Problem:** The main loop in `src/lib.rs` calls `terminal.draw(...)` on every iteration, even when nothing changed. A fixed 20 fps timer forces continuous CPU work.
- **Goal:** Render only when the UI state actually changes, or at a capped minimum interval (e.g., 250 ms).
- **Action Steps:**
  1. Add a `needs_redraw: bool` field to `App` (or an `AtomicBool` if accessed from other threads).
  2. Set `needs_redraw = true` in every place that mutates UI‑visible state: input handling, event handling, log appends, agent step updates, markdown cache changes, provider discovery, model list arrival, etc.
  3. In the main loop, replace the unconditional `draw` with:
     ```rust
     if app.needs_redraw || last_draw.elapsed() >= MIN_REDRAW {
         terminal.draw(|f| layout::render(f, &mut app))?;
         app.needs_redraw = false;
         last_draw = Instant::now();
     }
     ```
  4. Provide a fallback timer (e.g., `MIN_REDRAW = Duration::from_millis(250)`) for things like clock display.
  5. Add unit tests exercising the flag via a mock event bus.
- **Owner:** performance reviewer / UI lead.
- **Effort:** Medium.

### 2. Fix Quadratic Traversal in Active‑Agents Panel
- **Problem:** `layout_active_agents.rs` builds the tree of tasks by scanning the full task list for each node (`children: Vec<&TaskEntry> = tasks.iter().filter(...).collect()`). This is O(n²) with many sub‑agents.
- **Goal:** Linear‑time construction per render pass.
- **Action Steps:**
  1. In `render_active_agents_subpanel`, compute a `HashMap<&str, Vec<&TaskEntry>>` that groups tasks by `parent_session_id` in a single pass.
  2. Pass this map to a revised `build_task_rows` that looks up children via `map.get(parent_sid)` instead of filtering the whole list.
  3. Change the signature to accept `&HashMap<...>` and avoid cloning `app.active_tasks` (use a shared reference).
  4. Update the unit tests (e.g., `test_active_agents_render`) to reflect the new function signature.
- **Owner:** layout maintainer.
- **Effort:** Medium.

---

## Medium‑Impact Items (Priority: 2)

### 3. Cache & Allocation Optimisations in Active‑Agents Rendering
- **Problem:** Per‑frame recreation of `HashSet`s (`custom_names`, `teammate_ids`) and many `format!`/`String` allocations.
- **Goal:** Reuse cached collections and avoid allocation churn.
- **Action Steps:**
  1. Store `custom_names` and `teammate_ids` as fields on `App` that are updated only when the underlying vectors change (e.g., after a team member joins or custom agent list updates).
  2. Use `Cow<'a, str>` or `&str` where possible in `build_task_rows` to avoid copying strings.
  3. Replace `format!` for static column widths with `write!` into a pre‑allocated `String` buffer reused across rows.
- **Owner:** performance reviewer.
- **Effort:** Low‑Medium.

### 4. Asynchronous Model Discovery
- **Problem:** `models_for_provider` blocks the UI thread with `block_in_place` while performing network‑bound discovery for Ollama/Copilot.
- **Goal:** Perform discovery in the background and update UI when ready.
- **Action Steps:**
  1. Refactor the discovery code into an async function returning `Result<Vec<Model>, Error>`.
  2. When the provider selection UI opens, set a “loading” state and spawn a tokio task that sends a `ModelListReady` event via the bus.
  3. On receipt, update `app.provider_models` and set `needs_redraw = true`.
  4. Remove any `block_in_place` calls.
- **Owner:** input/provider feature owner.
- **Effort:** Medium.

### 5. Markdown Rendering Cache – LRU Instead of Full Clear
- **Problem:** `md_render_cache` (a `HashMap<u64, String>`) is cleared wholesale when it reaches 256 entries, causing bursty re‑renders.
- **Goal:** Use an LRU cache to evict only the least‑recently‑used entry.
- **Action Steps:**
  1. Add `lru = "0.12"` (or similar) to `ragent-tui` Cargo.toml.
  2. Replace `HashMap<u64, String>` with `lru::LruCache<u64, Arc<String>>` capped at 256.
  3. Store `Arc<String>` to avoid cloning on retrieval.
  4. Adjust `render_markdown_to_ascii` to use `Arc::clone()` when returning cached content.
- **Owner:** performance reviewer / markdown module maintainer.
- **Effort:** Low‑Medium.

---

## Low‑Impact / Polish Items (Priority: 3‑4)

| # | Item | Description | Owner | Effort |
|---|------|-------------|-------|--------|
| 6 | Reduce lower‑casing allocations in directory listing (`populate_directory_menu`). | Compute `name_lower` once per entry. | UI/FS maintainer | Low |
| 7 | Clipboard image copy – avoid `to_vec()` duplication. | Use `ImageData::into_owned()` or `ImageBuffer::from_raw` directly. | State maintainer | Low |
| 8 | Git branch detection at startup – make async. | Spawn a background task that runs `git rev-parse --abbrev-ref HEAD` and updates `app.git_branch`. | Startup maintainer | Low |
| 9 | Hot‑path formatting across layout files – combine with redraw optimisation to minimise impact. | Audit `format!` usage, replace with reusable buffers where profiling shows hot spots. | Layout maintainer | Medium (profiling required) |

---

## Implementation Roadmap

| Milestone | Tasks | Estimated Duration |
|-----------|-------|-------------------|
| **M1 – Redraw & Async Discovery** | 1, 4 | 2 weeks |
| **M2 – Active‑Agents Optimisation** | 2, 3 | 1.5 weeks |
| **M3 – Markdown LRU Cache** | 5 | 1 week |
| **M4 – Polish & Misc** | 6‑9 | 1 week |

All milestones include unit‑test coverage and CI validation. After M1‑M2, run a performance benchmark suite (`cargo bench -p ragent-tui`) to verify CPU usage drops and frame latency improves.

---

## Acceptance Criteria
- UI CPU usage on idle screen drops below 5 % on a mid‑range laptop.
- Rendering remains smooth (≥30 fps) when 200+ active tasks are displayed.
- Model discovery no longer blocks the UI; a loading spinner appears briefly.
- Markdown cache hit‑rate > 90 % after warm‑up.
- No new regression tests fail; existing tests pass.

---

*Prepared by the planning agent after reviewing the codebase and performance findings.*