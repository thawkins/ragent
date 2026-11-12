# MULTISESSION M1: Design Confirmation and Audit

Parent plan: [MULTISESSION.md](MULTISESSION.md)
Status: Complete (2026-11-12)
Scope: read-only audit; no code changed.

This is the M1 output: T1.1 to T1.4 below. Plan corrections that came out of
the audit have been applied to `MULTISESSION.md` (see "Changes to the plan"
at the end).

---

## T1.1 `App` field classification

Source: `pub struct App` in `crates/ragent-tui/src/app/state.rs`
(lines 3040-4292). **350 public fields**, every one classified below, with no
duplicates or omissions (checked by script against the struct).

| Class | Meaning | Count |
| --- | --- | --- |
| S | Per-session: moves into `SessionView` (M2) | 94 |
| GF | Global registry already keyed by session id; stays on `App`, filtered by session at render | 23 |
| P | Per-project: moves into a `ProjectContext` keyed by project root, shared by that project's sessions (M6) | 41 |
| G | Global UI / app state: stays on `App` | 192 |

### S: per-session (one M2 PR per group, in this order)

- **S1 Transcript and render cache (10):** `messages`, `scroll_offset`,
  `force_new_message`, `message_line_cache`, `message_cache_width`,
  `message_cache_dirty_from`, `message_stream_throttle_at`,
  `message_content_lines`, `message_content_lines_dirty`,
  `message_max_scroll`.
- **S2 Input and draft (11):** `input`, `input_cursor`, `kb_select_anchor`,
  `history_index`, `history_draft`, `pending_attachments`,
  `input_render_cache`, `last_prompt`, `input_queue`, `queue_next_pending`,
  `pending_send_after_compact`.
- **S3 Run lifecycle (38):** `session_id`, `agent_name`, `agent_info`,
  `current_agent_index`, `agent_stack`, `is_processing`, `cancel_flag`,
  `agent_halted`, `auto_compact_in_progress`, `compact_in_progress`,
  `compress_in_progress`, `auto_compact_failed`, `compact_result`,
  `prompt_start_time`, `tool_time_ms`, `llm_time_ms`,
  `last_task_completed_at`, `run_cost_banner`, `run_cost_banner_at`,
  `pending_stop_confirm`, `lead_session_created_at`, `role_mode`,
  `plan_approval_pending`, `pending_plan_task`, `pending_plan_restore`,
  `pending_rollback`, `rollback_result`, `pending_reverse_create`,
  `pending_reverse_create_folder`, `spec_impl_state`, `active_spec`,
  `autopilot_enabled`, `autopilot_token_budget`,
  `autopilot_time_limit_secs`, `autopilot_started_at`,
  `autopilot_pending_continue`, `autopilot_continued_this_wake`,
  `shell_cwd`.
- **S4 Attention (4):** `permission_queue`, `question_queue`,
  `pending_question_input`, `question_selected_index`.
- **S5 Usage and context (13):** `token_usage`, `llm_request_stats`,
  `last_input_tokens`, `stream_in_bytes`, `stream_out_bytes`,
  `security_verdicts`, `router_current_tier`, `router_current_model`,
  `context_snapshot_cache`, `context_snapshot_result`,
  `context_refresh_inflight`, `tasks_cache_rows`, `tasks_cache_dirty`
  (the `todos` table is keyed by `session_id`, `storage.rs:994`).
- **S6 Model selection (6):** `selected_model`,
  `cached_provider_model_label`, `selected_model_ctx_window`,
  `selected_thinking_level`, `provider_health`, `active_backend`.
- **S7 Team and swarm (12):** `active_team`, `team_members`,
  `team_message_counts`, `team_task_counts`, `team_task_counts_dirty`,
  `focused_teammate`, `swarm_state`, `swarm_result`,
  `swarm_unblock_last_poll`, `swarm_completion_last_poll`,
  `selected_agent_session_id`, `pending_forcecleanup`.

S3 is the largest group (38). Split it into two PRs: agent/run flags first,
then the plan/rollback/autopilot/spec continuations.

### GF: global, already keyed by session (23)

`active_tasks` (`TaskEntry.parent_session_id`, `ragent-agent/src/task/mod.rs:375`),
`agents_history`, `agents_history_cap`, `bg_tasks`, `subagent_launches`,
`subagent_launch_results`, `subagent_launch_cap`, `next_subagent_launch_id`,
`active_tasks_reconcile_result`, `active_tasks_reconcile_last`,
`active_tasks_reconcile_inflight`, `tool_step_map` (keyed by call id),
`pending_tool_args`, `last_step_per_session`, `substep_counter_per_session`,
`sid_to_display_name`, `next_agent_index`, `log_entries` (entries already
carry the short session id), `log_line_cache`, `log_cache_width`, `log_seq`,
`active_agents_teammate_ids`, `active_agents_teammate_ids_version`.

**Required change:** `App::load_session` (`app/session_ops.rs:4483`) clears
`tool_step_map`, `last_step_per_session`, `substep_counter_per_session` and
`sid_to_display_name`. With several open sessions that wipes the other
sessions' state. M3 must replace the clear with per-session removal on
session close.

### P: per-project (41)

`cwd`, `cwd_path`, `git_branch`, `project_files_cache`,
`project_files_cache_cwd`, `project_files_cache_refreshed_at`,
`project_files_cache_count`, `code_index`, `code_index_enabled`,
`code_index_stats_cache`, `code_index_stats_last_refresh`,
`code_index_busy`, `code_index_graph_busy`, `code_index_graph_spawned`,
`code_index_reindex_spawned`, `code_index_bg_result`,
`code_index_watch_session`, `memory_cache_entries`, `memory_cache_count`,
`memory_cache_dirty`, `memory_entry_count`, `memory_entry_count_pending`,
`memory_last_updated`, `memory_stats_last_refresh`, `spec_manager`,
`custom_agent_defs`, `custom_agent_diagnostics`, `cycleable_agents`,
`active_agents_custom_names`, `active_agents_custom_names_version`,
`skill_registry_cache`, `skill_dirs_cache`, `skill_registry_last_refresh`,
`cached_skill_registry`, `plugin_commands`, `mcp_servers`,
`mcp_status_map`, `mcp_client_adopted`, `mcp_startup_reported`,
`config_paths`, `trigger_runtime`.

Until M6 these stay on `App` and describe the launch project. That holds
because, before M6, sessions from other projects open read-only (plan D3).
`trigger_runtime` and the MCP fields are the uncertain ones: MCP servers can
come from global or project config. Confirm in M6 whether they split into a
global part and a project part.

### G: global (192)

Everything else stays on `App`:
- dialog, overlay and store state (plugin store, connector catalogue, backend
  panel, provider setup, router drafts, confirmations);
- side-panel visibility, scroll offsets, cached `*_area` and `*_max_scroll`
  rectangles, `*_content_lines` for side panels;
- background jobs (`bench_*`, `active_bench_*`, `newproj_*`, `govcreate_*`,
  `research_*`, `websearch_test_result`);
- process services (`event_bus`, `storage`, `session_processor`,
  `provider_registry`, `md_worker`, `webapi_*`);
- global UI (`status*`, `theme_mode`, `mouse_enabled`, `input_history` and its
  file, `quota_percent`).

Fields to reset on every session switch: `text_selection`, `scrollbar_drag`,
`context_menu`, `status_bar_cache` (invalidate), `slash_menu`, `file_menu`.

---

## T1.2 Working-directory audit

No production `set_current_dir` call exists anywhere (only in tests).
There are 50 production `current_dir()` call sites.

### Already session-scoped (no change)

- The agent loop resolves `working_dir` from `sessions.directory`, and uses
  the process cwd only when the session row is missing:
  `ragent-agent/src/session/loop_steps.rs:314`,
  `session/processor.rs:4250` (`announce_instruction_files`).
- Tools receive `ToolContext.working_dir` (`ragent-tools-core/src/lib.rs:155`).
  Path containment (`check_path_within_allowed_roots_cached`, `lib.rs:425`)
  is checked against that working dir plus `allowed_roots`. File tools
  therefore already follow the session's directory.
- `SESSION_STATE_CACHE` (`session/mod.rs:74`) and the team mailbox map
  (`team/mailbox.rs:236`) are keyed by id.

### Must change for cross-project sessions (M6)

| # | Site | Problem | Fix |
| --- | --- | --- | --- |
| 1 | `ragent-config/src/config.rs:1827` (`Config::load` cache) | Project config is resolved from the **process** cwd, so every session uses the launch project's config | Add `Config::load_for(dir)`; give `SessionProcessor::load_config_cached` a directory argument. Callers: `loop_steps.rs:170`, `task/mod.rs:939`, `task/mod.rs:1636`, `acp_dispatch.rs:107`, `agent/mod.rs:2782`, `plugin.rs:117`, `skill/mod.rs:540` |
| 2 | `ragent-agent/src/session/permissions.rs:809` | The `file:read` auto-grant inside the cwd uses the process cwd, so reads in another project's session would prompt every time | Pass the session working dir into the permission check |
| 3 | TUI, about 25 sites: `app/helpers.rs:503` (`current_working_dir`), `app/event_handler.rs:1355,1672,2875`, `app/input_handler.rs:1340,1394`, `app/models.rs:1093,1718,2168`, `app/slash.rs:5074`, `app/session_ops.rs:4287`, `app/backend_panel.rs:313`, `app/bench.rs:479`, `app/init.rs:75`, `layout_teams.rs:371,430`, `layout.rs`, `clipboard.rs:219` | Read the launch directory instead of the focused session's | Replace with the focused session's project root |

### Low priority / leave as is

- `team/manager.rs:1978,1983`: these use the process cwd only when the lead
  session lookup fails.
- `ragent-tools-core/src/bash.rs:191` (`bash_scratch_dir`): scratch location
  only.
- `session/archive.rs:469`: import containment. It should use the focused
  project root once imports run from the TUI against a non-launch project.
- `lib.rs:455,585,683,694` (TUI startup) and the CLI binaries (`src/*.rs`):
  launch-time only, correct as is.

### Other process-global state

`AGENT_LOOP_PROFILER` (`session/profiler.rs:422`) is a single global, so the
profile panel will mix samples from concurrent sessions. That is acceptable
for v1; note it in the panel title.

---

## T1.3 Keybinding and command conflicts

| Proposed | Finding | Decision |
| --- | --- | --- |
| `Ctrl+N` new session | Free in the chat handler (`input.rs` `handle_key`); the only `'n'` match is an unmodified key inside a dialog (`input.rs:664`) | **Keep** |
| `Ctrl+B` toggle sidebar | **Conflict:** `MoveCursorLeft` (`input.rs:931`, `input.rs:1113`) | **Change to `Alt+S`** (free; used Alt letters are a c e g i l m o p q t v x y) |
| `Ctrl+Up` / `Ctrl+Down` prev/next session | Used only inside the modal router-setup dialog (`input.rs:3065,3075`), not in chat | **Keep**. Some terminals and multiplexers intercept these, so `/session switch` is the fallback |
| `Ctrl+Tab` last two sessions | Terminals do not deliver `Ctrl+Tab` distinctly without the kitty keyboard protocol; `Tab`/`BackTab` is already `SwitchAgent` (`input.rs:1304`) | **Change to `Alt+R`** ("recent"; free) |
| `F2` rename | No `F2` handler exists | **Keep** (sidebar focus only) |
| `p`, `a`, `/`, arrows, `Enter` in sidebar | Active only while the sidebar has focus | **Keep** |
| Sidebar focus via `Tab` | `Tab` is `SwitchAgent` | `Alt+S` shows and focuses the sidebar, `Esc` returns focus to input; mouse click also focuses |
| `/new` | **Conflict:** existing project scaffold command (`state.rs:1025`, `slash.rs:1085`) | Use a **`/session` command family** (the `session` trigger is unused) |
| `/resume`, `/clear` | Existing, with other meanings | Leave untouched |

New command set: `/session new [path]`, `/session switch <id|prefix>`,
`/session list [--archived]`, `/session rename <title>`, `/session archive`,
`/session unarchive <id>`, `/session pin`, `/session close`,
`/session sidebar` (toggle).

---

## T1.4 Server and ACP

- **HTTP server** (`crates/ragent-server/src/routes/mod.rs:167-175`): already
  addressed by session (`/sessions`, `/sessions/{id}`,
  `/sessions/{id}/abort`, `/sessions/{id}/permission/{req_id}`,
  `/sessions/{id}/tasks`). There is no global "current session". **No change.**
- **Reusable rule:** session creation over HTTP restricts the directory to
  server cwd plus `dirs.allowed_roots` (SEC-062, `routes/mod.rs:455`).
  `/session new <path>` in the TUI must apply the same containment rule.
  Added to the plan as FR-01.
- **Embedded web API** (`App.webapi_server`): shares the processor, so it is
  unaffected.
- **ACP server** (`src/acp_server.rs`): a separate stdio process with no TUI
  session state. **No change.**

R5 (team state) answered: team state lives in TUI fields (S7) and the team
manager is keyed by `lead_session_id`; no process-global team state was
found.

---

## Changes to the plan

Applied to `MULTISESSION.md`:

1. FR-01 / D6: `/new` replaced by the `/session` family. `/session new` paths
   are checked against the SEC-062 containment rule.
2. FR-10: sidebar toggle `Ctrl+B` changed to `Alt+S`.
3. FR-12: `Ctrl+Tab` changed to `Alt+R`.
4. D1: field counts and group order taken from T1.1, with S3 split in two.
5. D2: GF maps must no longer be cleared on switch.
6. D3: the config-cache and permission-check sites from T1.2 are now named.
7. M1 marked complete.

## Open items for the user

- Q1 and R3/R4 in the plan are still undecided (background sessions on exit,
  a concurrency cap, sharing MCP/LSP per project).
- No tracking issue has been created yet. The T1.1 checklist above is ready
  to paste into one.
