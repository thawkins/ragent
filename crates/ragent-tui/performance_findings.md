ragent-tui Performance Review

Summary
-------
This review inspects crates/ragent-tui for runtime performance issues: expensive allocations, blocking IO on the UI thread, unnecessary redraws, inefficient data structures, and hot loops. Findings include concrete file:line references, severity, impact, and remediation steps with code suggestions. Suggested owners and estimated remediation effort are provided.

Findings
--------

1) Unnecessary/redraw-per-frame of entire TUI
   - Files: crates/ragent-tui/src/lib.rs
   - Location: run_tui main loop (approx lines 142-191, 180-204)
   - Symptoms / lines:
     - terminal.draw(|frame| layout::render(frame, &mut app))? called every loop unconditionally (lib.rs ~178-179)
     - The loop sleeps 50ms on the timer branch which forces ~20 FPS even with no changes (lib.rs ~180-186)
   - Severity: High
   - Impact: High CPU and battery usage, unnecessary render work when UI state unchanged, possible jank on slower machines.
   - Remediation:
     - Introduce a "dirty" flag on App (e.g. app.needs_redraw: bool) that is set when state-changing events occur (handle_event, handle_key_event, poll_pending_opt when it appends text, etc.). Only call terminal.draw when dirty or when a periodic minimum refresh is required (e.g. every 1s for clocks/elapsed timers).
     - Alternatively maintain a small timed redraw interval (e.g. 250ms) and otherwise only redraw on events.
     - Use event-driven redraw: when bus events or input events arrive, mark dirty and wake the UI to draw once.
   - Code suggestion (conceptual):
     - Add to App: pub needs_redraw: AtomicBool or plain bool (mutably accessed on UI thread).
     - Replace unconditional draw with:
       {
         if app.needs_redraw || last_draw.elapsed() >= std::time::Duration::from_millis(100) {
             terminal.draw(|frame| layout::render(frame, &mut app))?;
             app.needs_redraw = false;
             last_draw = Instant::now();
         }
       }
     - Set app.needs_redraw = true in all places that mutate state visible to the UI (app.handle_event, push_log, append_assistant_text, poll_pending_opt on success, input handlers, etc.).
   - Owner: performance-reviewer / lead
   - Estimate: medium

2) Quadratic traversal & repeated scanning in active-agents panel
   - Files: crates/ragent-tui/src/layout_active_agents.rs
   - Location: build_task_rows / render_active_agents_subpanel (approx lines 40-55, 116-129, render around 132-154)
   - Symptoms / lines:
     - build_task_rows collects children via tasks.iter().filter(...).collect() (layout_active_agents.rs ~50-55) and then recurses. For each node this scans the full tasks list -> O(n^2) in worst case.
     - render_active_agents_subpanel clones tasks from app.active_tasks.clone() on each render (layout_active_agents.rs ~135-138). Cloning and repeated scans on every frame.
   - Severity: High
   - Impact: When many active tasks/subagents exist (hundreds), rendering can become extremely expensive and cause frame stalls; CPU usage grows quadratically.
   - Remediation:
     - Precompute a parent -> children map once per render pass: iterate tasks once and push references into a HashMap<String, Vec<&TaskEntry>> keyed by parent_session_id.
     - Use that map in the recursive traversal so each TaskEntry is visited once.
     - Avoid cloning the entire tasks vector for rendering: either borrow (&app.active_tasks) or maintain a versioned cache (only rebuild when app.active_tasks changes).
   - Code suggestion (patch sketch):
     - In render_active_agents_subpanel(), replace:
         let tasks = app.active_tasks.clone();
       with:
         let tasks = &app.active_tasks;
         let mut by_parent: HashMap<&str, Vec<&TaskEntry>> = HashMap::new();
         for t in tasks.iter() {
             by_parent.entry(t.parent_session_id.as_str()).or_default().push(t);
         }
       Change build_task_rows to accept the map &HashMap<&str, Vec<&TaskEntry>> and walk children by lookup instead of scanning tasks.
   - Owner: performance-reviewer / layout maintainer
   - Estimate: medium

3) Per-frame allocations: building HashSets and formatting strings in active panel
   - Files: crates/ragent-tui/src/layout_active_agents.rs
   - Location: render_active_agents_subpanel (approx lines 140-152)
   - Symptoms:
     - custom_names and teammate_ids HashSets are rebuilt on every render by mapping and collecting (layout_active_agents.rs ~140-151).
     - Many format! calls and Span::styled allocations per-line inside render.
   - Severity: Medium
   - Impact: High allocation churn on every frame; GC (allocator) pressure and CPU overhead.
   - Remediation:
     - Compute custom_names/teammate_ids only when their source data changes (track a small "version" or timestamp on the underlying vectors, or compute lazily and cache until mutated).
     - When feasible, avoid allocating temporary Strings inside hot render paths: format only once or reuse buffers; use display adapters (Borrowed &str) where API allows.
   - Owner: performance-reviewer / layout maintainer
   - Estimate: low–medium

4) UI-blocking synchronous model discovery calls
   - Files: crates/ragent-tui/src/app.rs
   - Location: models_for_provider (approx lines 1558-1569 and 1571-1596)
   - Symptoms:
     - For 'ollama' and 'copilot' the code attempts to run async discovery synchronously by using tokio::runtime::Handle::try_current() and then tokio::task::block_in_place + handle.block_on(...). This blocks the UI thread while network/IO happens (app.rs ~1560-1563, ~1566-1569, ~1567-1569, ~1567-1589).
   - Severity: Medium
   - Impact: Potential UI freeze / jank during model discovery (which can be slow depending on remote), poor responsiveness when user opens provider selection.
   - Remediation:
     - Convert model discovery to be asynchronous: spawn a background task that queries models and sends them back to the UI via an event bus or channel; update app state and mark UI dirty when results arrive.
     - Avoid blocking the UI thread with block_on; if you must call from sync context, use tokio::task::spawn_blocking and return quickly.
   - Code suggestion (conceptual):
     - When opening the 'SelectModel' UI, if models not yet known, set a loading state and spawn a tokio::task::spawn(async move { let models = ...await...; send models back via event_bus.publish(ModelListReady{ provider, models }); });
     - App handles ModelListReady event and updates provider model list.
   - Owner: performance-reviewer / input/provider feature owner
   - Estimate: medium

5) Markdown rendering cache eviction strategy
   - Files: crates/ragent-tui/src/app.rs and state.rs
   - Location: render_markdown_to_ascii (app.rs ~221-266) and md_render_cache definition (state.rs ~908)
   - Symptoms:
     - The cache is a HashMap<u64, String> and is fully cleared when size >= 256 (app.rs ~260-264). Clearing the whole cache causes a large number of re-renders on next use and makes cache behaviour bursty.
     - Cache entries are cloned when returned which duplicates Strings (app.rs ~236-238).
   - Severity: Low–Medium
   - Impact: Inefficient cache eviction and allocation spikes; missed reuse opportunities.
   - Remediation:
     - Replace HashMap with an LRU cache (e.g., lru crate) bounded to an appropriate capacity (e.g. 256). On access, the entry is promoted, and eviction is incremental.
     - Return an Arc<String> or keep entries in-place and return references where lifetimes permit to avoid clone. If App needs to hand out owned Strings, consider storing Arc<String> to cheaply clone.
   - Owner: performance-reviewer
   - Estimate: low

6) Blocking git detection at startup
   - Files: crates/ragent-tui/src/app.rs
   - Location: detect_git_branch called during App::new (app.rs ~312 and ~1523-1536)
   - Symptoms:
     - detect_git_branch spawns a synchronous git process via Command::output during initialization. For very slow workdirs, this might delay startup.
   - Severity: Low
   - Impact: Slight startup delay; not a runtime hotspot.
   - Remediation:
     - Run lightweight or make this asynchronous: spawn a background task to detect git branch and update app when ready.
   - Owner: low priority (lead)
   - Estimate: low

7) Large clipboard image handling copies buffer
   - Files: crates/ragent-tui/src/app/state.rs
   - Location: save_clipboard_image_to_temp (state.rs ~181-193)
   - Symptoms:
     - img_data.bytes.as_ref().to_vec() clones the full pixel buffer before constructing ImageBuffer (state.rs ~181). For very large images (up to the 50 MB cap), this double-copies memory temporarily.
   - Severity: Low–Medium
   - Impact: Higher memory usage and allocation overhead when pasting large images.
   - Remediation:
     - Avoid the extra copy by consuming the incoming buffer when ownership permits, or use ImageBuffer::from_raw with a Vec<u8> without an intermediate clone. If arboard::ImageData exposes owned storage, reuse it; otherwise document the copy and keep the size cap.
     - If ImageBuffer must own a Vec<u8>, prefer constructing the Vec directly instead of to_vec() from a slice if you can obtain an owned buffer.
   - Owner: performance-reviewer / state owner
   - Estimate: low

8) Frequent string lowercasing in directory listing
   - Files: crates/ragent-tui/src/app.rs
   - Location: populate_directory_menu (app.rs ~1960-1972)
   - Symptoms:
     - name.to_lowercase() is computed repeatedly for filter comparisons; currently name.to_lowercase() is called per entry inside the loop (app.rs ~1969-1971).
   - Severity: Low
   - Impact: Minor allocation churn on directory browsing with many entries.
   - Remediation:
     - Compute name_lower = name.to_lowercase() once per entry and reuse it for checks.
   - Owner: performance-reviewer
   - Estimate: low

9) Hot-path formatting and repeated allocations across layout code
   - Files: crates/ragent-tui/src/layout.rs, app.rs, widgets/*
   - Symptoms:
     - Layout and widget rendering perform numerous format!/String allocations per-line on every draw (many locations, see grep results). While acceptable at modest scales, it contributes to allocation pressure on frequent redraws.
   - Severity: Low
   - Impact: Per-frame allocation overhead and CPU usage.
   - Remediation:
     - Combine with the redraw optimization (only render when needed) to remove most per-frame allocations.
     - Where hotspots are identified (via flamegraph/profiling), consider formatting into a reusable buffer or caching rendered lines until inputs change.
   - Owner: performance-reviewer / UI maintainers
   - Estimate: medium (profiling required)

Notes and next steps
--------------------
- The highest-impact items are reducing redraw frequency and fixing the quadratic active-tasks traversal. Implementing those two will likely produce the largest runtime improvements.
- After implementing the above, run a runtime profiler (e.g. perf/flamegraph, or instrumented logging with timings) while exercising the UI with many active tasks and large message history to validate improvements.

References (selected lines)
---------------------------
- Redraw & main loop: crates/ragent-tui/src/lib.rs lines ~172-190
- Active agents: crates/ragent-tui/src/layout_active_agents.rs lines ~40-55, ~116-129, ~132-154
- Markdown cache and render: crates/ragent-tui/src/app.rs lines ~221-266; state.rs md_render_cache ~908
- models_for_provider blocking: crates/ragent-tui/src/app.rs lines ~1558-1569, ~1566-1569, ~1566-1596
- Clipboard image copy: crates/ragent-tui/src/app/state.rs lines ~181-183

Acknowledgements
----------------
Thanks — let me know if you want I can implement the high-impact fixes (dirty redraw + active tasks traversal) as a PR with tests and microbenchmarks.
