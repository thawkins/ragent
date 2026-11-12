# MULTISESSION: Multiple Concurrent Sessions with a Project-Sectioned Sidebar

Feature ID: MULTISESSION
Title: Multiple sessions, session switching, and a left-hand session sidebar grouped by project
Status: Implemented (M1-M8).
Reference UX: Claude desktop app, Code mode (session sidebar + one focused transcript)

Overview
--------
Today the TUI owns exactly one primary session. `App` (in
`crates/ragent-tui/src/app/state.rs`, ~300 public fields) holds one
`session_id: Option<String>`, one `messages: Vec<Message>`, one
`is_processing` flag and one set of per-session caches (tool step map, log,
input queue, context snapshot, teams/agents panels). Events from the
`EventBus` are filtered by `App::is_current_session`
(`app/event_handler.rs`), so anything that belongs to another session is
dropped. `App::load_session` (`app/session_ops.rs`) "switches" by
overwriting the single slot, and the only entry point is the `--session`
resume path in `crates/ragent-tui/src/lib.rs`.

This plan turns the TUI into a multi-session host:

- Many sessions can be open at once; each keeps running in the background
  while another one is focused.
- A left-hand sidebar lists every session, sectioned by project, with
  status indicators, and can be collapsed.
- The user switches the focused session from the sidebar, a keyboard
  shortcut, or a slash command, with no loss of in-flight work.

Claude desktop Code-mode conventions to follow
----------------------------------------------
These are the behaviours the sidebar and switching model copy. Each one is
mapped to a concrete requirement below.

| Convention (Claude desktop, Code mode) | ragent requirement |
| --- | --- |
| "New session" action at the top of the sidebar | FR-01 |
| Sessions grouped under their project (folder) heading; headings collapse | FR-02 |
| Rows ordered by most recent activity; each row shows title + relative time | FR-03 |
| Row status indicator: running, waiting on you (permission/question), unread result, idle | FR-04 |
| Sessions keep working when not focused; a background session that needs you raises a notification | FR-05, FR-06 |
| Title auto-generated from the first prompt; user can rename | FR-07 |
| Pin keeps a session at the top; Archive hides it (not delete) and can be undone | FR-08 |
| Search/filter box over the session list | FR-09 |
| Sidebar can be collapsed to give the transcript the full width | FR-10 |
| Each session carries its own working directory, model and permission mode | FR-11 |
| One focused session fills the main pane; switching is instant and keeps scroll position and draft input | FR-12 |

Functional requirements
-----------------------
- FR-01 New session: `Ctrl+N` and a `+ New session` row at the top of the
  sidebar create a session in the focused session's project directory.
  `/session new [path]` creates one in another directory; the path must pass
  the same containment rule as the HTTP server (cwd plus
  `dirs.allowed_roots`, SEC-062 in `ragent-server/src/routes/mod.rs`).
  (`/new` is already the project-scaffold command.)
- FR-02 Project sections: sessions are grouped by project. A project is the
  canonical git work-tree root of `sessions.directory`, falling back to the
  directory itself. Heading text is the root's basename; on a basename
  collision, append the shortest disambiguating parent path. Headings toggle
  collapsed with `Enter`/`Space`; collapsed state persists. The current
  launch project is listed first, the rest by most recent activity.
- FR-03 Rows: title (truncated with `...`), relative age ("2m", "3h",
  "4d") from `updated_at`, ordered `updated_at DESC` within a section.
- FR-04 Status glyphs (ASCII per AGENTS-LANGUAGE.md): `*` animated/bright
  for running, `!` for waiting on the user (pending permission or question),
  `o` for unread completed result, blank for idle, `x` for last turn errored.
- FR-05 Background execution: a non-focused session continues its agent
  loop, tool calls, subagents and teams. Switching never cancels work.
  `Esc`/`Ctrl+X` cancel only the focused session.
- FR-06 Attention routing: a permission request or question from a
  background session sets `!` on its row, increments a status-bar badge
  ("2 sessions need you"), and fires the existing desktop notification
  path if enabled. The permission dialog is shown only when that session is
  focused; it is never shown over a different session's transcript.
- FR-07 Titles: after the first assistant reply, generate a short title
  (existing summary/title helpers or a cheap model call; reuse what
  `sessions.title` writers already do). `F2` or `/session rename <title>` renames.
- FR-08 Pin/archive: `p` pins/unpins (a "Pinned" pseudo-section sits above
  all projects); `a` archives via the existing
  `SessionManager::archive_session`; `/session list --archived` lists
  archived sessions and `/session unarchive <id>` restores one. Archiving a running session asks for
  confirmation and stops it first.
- FR-09 Filter: `/` while the sidebar has focus opens a filter line that
  matches title and project name (case-insensitive substring).
- FR-10 Sidebar visibility: `Alt+S` toggles the sidebar and focuses it
  when shown (`Ctrl+B` is taken by cursor-left); it auto-hides
  below a minimum terminal width (proposed 100 columns) and returns when
  the terminal is wide enough. Width is resizable by mouse drag on the
  border and persisted.
- FR-11 Per-session context: each session owns its working directory,
  model/provider selection, permission mode and agent. Changing the model
  in one session does not change it in another.
- FR-12 Switching: `Ctrl+Up`/`Ctrl+Down` move focus to the previous/next
  session in sidebar order, `Alt+R` toggles between the two most
  recently focused sessions (`Ctrl+Tab` is not delivered by most
  terminals), `Enter` on a sidebar row focuses it, `/session switch
  <id|title-prefix>` switches by name. Each session keeps its own scroll
  offset, draft input, input queue and panel selections.
- FR-13 Startup: with no `--session`, restore the set of sessions that were
  open at exit and focus the last one; `--session <id>` focuses that one.
  `--new` forces a fresh session.

Keybindings above were checked for conflicts in M1 (T1.3 in
MULTISESSION-M1.md).

Non-functional requirements
---------------------------
- NFR-01 Switching renders the target session within one frame (no
  storage read on the hot path; messages for open sessions stay in memory).
- NFR-02 Memory: only "open" sessions hold messages in memory. Sessions
  that are listed but not open hold only their `sessions` row. An LRU cap
  (proposed 8 open idle sessions) evicts the transcript of idle,
  non-focused sessions; running sessions are never evicted.
- NFR-03 The sidebar list is refreshed from storage on session events, not
  per frame (same pattern as `status_bar_cache.rs`).
- NFR-04 No behaviour change for a user who never opens a second session,
  apart from the sidebar being visible (and collapsible).
- NFR-05 Existing databases open without manual migration (additive schema
  change plus backfill).

Current architecture (what has to change)
-----------------------------------------
| Area | Today | Problem for multi-session |
| --- | --- | --- |
| `App` state | One flat struct, per-session and global fields mixed | Cannot hold two transcripts |
| Event routing | `is_current_session` drops foreign events | Background sessions go dark |
| `SessionProcessor` | One `Arc`, shared; `SessionManager::session_state_cache` already keyed by id | Mostly fine; config and cwd are process-wide |
| Working directory | `App.cwd` / `cwd_path` from the process cwd | Sessions in other projects need their own root |
| Config | `session_processor.load_config_cached()` reads the launch project | Project-local config must follow the session |
| Storage | `sessions` has `project_id` (always written as `""`), `directory`, `updated_at`, `archived_at`, `title` | Needs `project_id` populated, plus `pinned` |
| Layout | `layout::render` -> `render_chat` fills the frame | Needs a left column |
| Screen | `ScreenMode` has only `Chat` | Fine; sidebar is a panel, not a screen |

Design
------

### D1. Split `App` into global state and `SessionView`

Introduce `crates/ragent-tui/src/app/session_view.rs`:

```rust
/// Everything that belongs to one open session in the TUI.
pub struct SessionView {
    pub session_id: String,
    pub directory: PathBuf,
    pub project: ProjectKey,
    pub messages: Vec<Message>,
    pub is_processing: bool,
    pub attention: Attention,        // None | Permission | Question | Unread | Error
    pub input: InputDraft,           // text, cursor, queue
    pub scroll: ScrollState,
    pub tool_step_map: HashMap<String, (String, u32, u32)>,
    pub log: LogBuffer,
    pub pending_permission: Option<PermissionRequest>,
    pub model: ModelSelection,
    pub permission_mode: PermissionMode,
    // ... remaining per-session fields moved out of App
}
```

`App` keeps global state (providers, theme, config, plugin/connector
stores, global panels) and gains:

```rust
pub session: SessionView,        // the focused session (plain field)
pub background: SessionRegistry, // IndexMap<String, SessionView> + MRU (M3)
pub sidebar: SidebarState,       // visible, width, focus, filter, collapsed, cursor
```

The focused view is a plain field, not an `active()` accessor: a method
call borrows all of `App`, which breaks the many sites that read
`self.session.messages` while touching other `App` fields. Switching (M3)
swaps the target view out of `background` into `session` with
`std::mem::swap`, so the swap is O(1) and rendering code only ever sees
`self.session`.

Migration strategy (keeps the diff reviewable and tests green at every
step):

1. Add `SessionView` and the `App::session` field (done in M2 group S1).
2. Move fields one cohesive group at a time (messages + render cache;
   input + queue; tool step / log maps; permission state; context snapshot;
   teams/agents panel selection). Each group is one PR: move field, replace
   `self.x` with `self.session.x`, run the crate tests. The replacement is
   compiler-driven: every rustc E0609 "no field on type App" span for a
   moved field is rewritten, so other types' same-named fields
   (`request.messages`, panel `scroll_offset`s) are never touched.
3. The field classification is fixed by M1 T1.1: 94 per-session fields in
   groups S1-S7 (S3 split into two PRs), 23 session-keyed global
   registries that stay on `App`, 41 per-project fields deferred to M6, and
   192 global fields. Nothing outside S1-S7 moves in M2.

### D2. Event routing

Implemented in M3 (`crates/ragent-tui/src/app/sessions.rs`).
`App::handle_event` looks up the event's owner with
`App::background_owner`: the background session with that id, the parent
of a sub-agent (`TaskEntry::parent_session_id`), or the lead of a teammate
session. The existing handler (now `handle_focused_event`, with its 57
`is_current_session` arms unchanged) then runs against the owner. For a
background owner, the view is swapped into `App::session`, handled, and
swapped back. The global status line is restored after the swap.

- Events for the focused session: handled as today.
- Events for an open background session: applied to that `SessionView`
  (messages, `is_processing`, attention). Dialogs read only
  `App::session`, so nothing opens over the focused session. A run that ends
  in the background sets `SessionView::unread`.
- Session-scoped housekeeping polls (compaction, rollback, swarm, question
  timeout, autopilot) also run for each background session
  (`App::poll_background_sessions`).
- Events for a session that is listed but not open: update the sidebar row
  status only.
- Session-keyed global maps (`tool_step_map`, `last_step_per_session`,
  `substep_counter_per_session`, `sid_to_display_name`) are no longer
  cleared by `load_session`; entries are removed when a session closes.
- New sessions are created eagerly (`/session new` calls
  `ensure_session`) before any prompt is sent, so a session's id is known
  before its first event. The `SessionCreated` arm's `session_id.is_none()`
  guard is left as is; it only fires for the lazily created first
  session.

### D3. Per-session working directory and config

- Session-scoped tools must resolve paths against `SessionView.directory`,
  not the process cwd. Audit every `std::env::current_dir()` and `App.cwd`
  use in `ragent-agent` and the tools crates; pass the directory through
  the existing per-call context instead. Never call `set_current_dir` on
  switch: background sessions run concurrently.
- Config: `Config::load` (`ragent-config/src/config.rs:1827`) resolves
  project config from the process cwd. Add `Config::load_for(dir)` and pass
  the session directory through `SessionProcessor::load_config_cached` so
  each session reads its own project config. Global (user) config remains
  shared.
- Permissions: the `file:read` auto-grant inside cwd
  (`ragent-agent/src/session/permissions.rs:809`) must use the session
  working directory.
- The agent loop and tools already use `sessions.directory` /
  `ToolContext.working_dir` (M1 T1.2).
- Phase gate: until D3 landed, sessions from other directories could not
  be focused. M6 removed the gate.

Implemented in M6:

- `Config::load_for(dir)` reads the global config plus
  `<dir>/.ragent/ragent.json` under the same project-local guard. The M-025
  cache became one entry per directory, so interleaved projects do not
  evict each other. Unlike `load()`, it never writes the first-run default
  `ragent.json`: opening a session must not create files in a project.
  `Config::load()` is unchanged; it still resolves the relative
  `.ragent/ragent.json`, so its `config_paths` are byte-identical.
- `SessionProcessor::load_config_for(dir)`: a session whose directory has
  its own `.ragent/ragent.json` (and is not the process cwd) gets that
  project's config. Every other session gets the launch config, exactly as
  before. Consequence: a project with no config file of its own inherits
  the launch project's overlay (pre-M6 behaviour). The agent loop now
  resolves the session directory before loading config.
- `check_permission_with_prompt` gained a trailing `working_dir` argument
  (callers pass `ToolContext.working_dir`). The `file:read` auto-grant is
  judged against it, and relative resources (`"."`, `src/x.rs`) are
  resolved against it, as the tools do.
- TUI: `SessionView::directory` holds each session's directory. Focusing a
  session moves `App::cwd`, `cwd_path` and `git_branch` there and drops the
  memory and skill caches. The M1 audit counted about 25 TUI sites; the
  real number was about 95 process-cwd reads, plus 48 `Config::load()`
  calls, 9 `current_config()` calls and 5 `load_config_cached()` calls.
  All of them now read the focused session's `cwd_path` (a compiler-guided
  rewrite; closures capture a clone). They stay on the process cwd only
  where that is right: startup, the log root, `/bench` (tied to the ragent
  checkout), the `target/temp` clipboard helper and the `/websearch`
  diagnostic.
- `/session new <path>` checks the path the way the HTTP server does
  (SEC-062): it must be an existing directory inside the launch directory
  or a `dirs.allowed_roots` entry.

Still launch-project-bound (open question R4): the code index, MCP server
connections, spec manager, custom agent definitions and plugin commands
are created once at startup for the launch project and shared by every
session. A session in another project gets correct config, permissions,
file tools and slash-command paths, but codeindex search and MCP tools
still answer for the launch project.

### D4. Storage changes (`crates/ragent-storage`)

Additive migration in the `sessions` table:

```sql
ALTER TABLE sessions ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_sessions_project_updated
    ON sessions(project_id, updated_at DESC) WHERE archived_at IS NULL;
```

Implemented in M4 as schema version 4:

- `ragent_storage::project_root(dir)` (`src/project.rs`) returns the nearest
  ancestor holding a `.git` directory or file (linked worktrees count), or
  the directory itself. A missing directory is returned unchanged, so its
  sessions still group under their last known path; M5 marks those
  sections "missing" at render time.
- `Storage::create_session` stores that root as `project_id`, and
  `SessionManager::create_session` returns the same value.
- The migration's slow path backfills every row whose `project_id` is `''`,
  once, inside the migration transaction.
- `pinned` is kept out of `SessionRow` so the legacy two-schema row
  mapping stays untouched; it has its own `set_session_pinned` /
  `pinned_session_ids`.
- Other new APIs: `unarchive_session`, `list_archived_sessions`, and
  `list_sessions_grouped` (top-level sessions only, as `ProjectSessions`
  groups ordered by newest session). Rename reuses the existing
  `update_session`. No `touch_session` was needed: `create_message`
  already bumps the session's `updated_at` on every message.
- The open session ids persist under the `tui.sessions.open` setting
  (focused first, then most recently focused). The sidebar's own state
  (width, visibility, collapsed projects) joins it under `tui.sessions.*`
  in M5.

### D5. Sidebar rendering (`crates/ragent-tui/src/sessions_sidebar.rs`)

Implemented in M5:

- `layout::render` asks `sessions_sidebar::split_area` for
  `(Option<sidebar>, chat)`. The sidebar is a bordered block whose right
  border is the drag handle; `render_chat` takes the chat `Rect` instead
  of `frame.area()`. Modal overlays still centre on the whole frame.
- Rows: `+ New session`, a `Pinned` section, then the launch project
  first, then the other projects by recency. A pinned session appears only
  under `Pinned`. The rows come from `build_rows`; `widgets/selectable_list.rs`
  was not reused, because a plain cursor/scroll over the row vector was
  smaller.
- Focus: `Alt+S` shows and focuses, focuses, or hides the sidebar (`Tab` is
  already `SwitchAgent`). While it has focus: `Up/Down/PageUp/PageDown/Home/End`
  move; `Enter`/`Space` act on the row; `Left/Right` collapse/expand a
  heading; `p`, `a` and `F2` (switch, then prefill `/session rename`)
  act on the session under the cursor; `/` starts a filter; `Esc` clears
  the filter, then returns focus. Every key is consumed while it has focus.
  The keys are handled after all modal dialogs, so a permission prompt
  still wins.
- Global keys: `Ctrl+N` new session, `Ctrl+Up`/`Ctrl+Down` previous/next
  session in sidebar order (skipping sessions that cannot be opened here),
  `Alt+R` the most recently focused session.
- Mouse: a click on a row acts on it; a click on the right border starts a
  drag that resizes; the wheel moves the cursor; a click outside drops
  focus and falls through to the chat.
- Status bar: a `N need you` badge at the right end of line 1 while
  background sessions wait on the user (FR-06). It takes width from line 1
  instead of adding a span, so the existing status-bar layout is unchanged
  when the badge is absent.
- Storage data (groups, pins, missing directories) is cached and reloaded
  only when `SidebarState::dirty` is set: by session-created, -updated and
  message-end events, focus changes and `/session` commands. Attention
  markers are read live from the open views each frame.
- `Attention::Error` (marker `x`) is set when a background session's run
  fails and is cleared, like `Unread`, when the session is focused.
- Persisted under the `tui.sessions.sidebar` setting: visibility, width and
  collapsed sections.
- The sidebar is visible by default (FR-10). The shared test helper
  `support::make_app` turns it off, so existing layout tests keep the
  full-width chat they were written against; sidebar tests turn it on.

### D6. Slash commands

Add one `session` trigger to the slash table in `app/state.rs` with
subcommands handled in `app/slash.rs`: `/session new [path]`,
`/session switch <id|prefix>`, `/session list [--archived]`,
`/session rename <title>`, `/session archive`, `/session unarchive <id>`,
`/session pin`, `/session close` (unload the transcript, session stays
listed), `/session sidebar`. `/new` (project scaffold), `/resume` (resume a
halted agent) and `/clear` keep their current meaning.

### D7. Server and other surfaces

Confirmed in M1: `crates/ragent-server` routes are session-addressed and
the ACP server is a separate stdio process. No change in scope.

Milestones
----------
Estimates assume one developer familiar with the codebase. They include
tests, docs and review rounds.

M1 - Design confirmation and audit (2-3 days) - DONE, see MULTISESSION-M1.md
- T1.1 Classify every `App` field as global or per-session; attach the list
  to the tracking issue.
- T1.2 Audit `current_dir`/`App.cwd`/`set_current_dir` uses in agent and
  tools crates (input to D3).
- T1.3 Check proposed keybindings for conflicts; finalise FR-12 bindings.
- T1.4 Confirm server/ACP need no change (D7).
- Verify: written checklist reviewed; no code change.

M2 - `SessionView` extraction, single session only (1-1.5 weeks) - DONE
- T2.1 Add `SessionView` and `App::session` (done). `SessionRegistry` is
  deferred to M3, where it first holds more than one view.
- T2.2 Move per-session fields group by group (D1); one PR per group.
  Done: all 94 S1-S7 fields live in `SessionView`
  (`crates/ragent-tui/src/app/session_view.rs`), built by
  `SessionView::new(agent_name, agent_info, current_agent_index)`; the
  caller sets the startup model and backend.
- Verify: full `ragent-tui` test suite passes after each PR; no
  user-visible change (NFR-04).

M3 - Multi-session core and routing (1 week) - DONE
- T3.1 `SessionRegistry` holds background views in most-recently-focused
  order (index 0 = last focused); the focused view stays in `App::session`.
- T3.2 Event dispatch (D2), including subagent/team child sessions.
- T3.3 Per-session cancel; background sessions keep running (FR-05).
  `cancel_flag` is per session since M2, so Esc cancels only the focused
  session.
- T3.4 Attention state (`SessionView::attention()`, `Attention` enum,
  `SessionRegistry::waiting_count`) and permission dialog gating (FR-06).
  The status-bar badge, the error glyph and desktop notifications stay in M5
  and M7.
- T3.5 Slash commands `/session [list]|new|switch|close|help` (D6).
  `switch` matches an id, id prefix or suffix, or title prefix. Sessions
  and paths outside the launch directory are refused until M6. Keybindings
  (FR-10, FR-12) arrive with the sidebar in M5.
- Verify: `crates/ragent-tui/tests/test_multisession.rs` (10 tests) drives
  two sessions with synthetic bus events instead of a mock provider (the
  provider only produces those same events). It asserts A's text lands in
  A's transcript and not B's, a permission request in A queues on A
  without a dialog over B, a background run end marks A unread, the
  focused status survives background events, and switch, close and stored
  load leave other sessions' state intact. With routing disabled, the 3
  routing tests fail.

M4 - Storage and project grouping (3-4 days) - DONE
- T4.1 `pinned` column, index, project_id population and backfill (D4).
- T4.2 Grouped list, pin, archive listing and unarchive APIs; rename and
  activity ordering reuse existing behaviour (D4). Exposed as
  `/session rename|pin|archive|unarchive` and `/session list --all|--archived`.
  `/session archive` refuses a running session rather than asking to stop
  it.
- T4.3 Persist and restore open sessions (FR-13): saved on every focus
  change and at startup. `ragent` reopens up to 8 of them (skipping
  archived, deleted and other-directory sessions) and falls back to a fresh
  session. `ragent --new` forces a fresh one, and `ragent session resume
  <id>` is unchanged. `run_tui` takes a `StartupSession` enum in place of
  `Option<String>`. Sidebar UI state is deferred to M5 with the sidebar.
- Verify: `crates/ragent-storage/tests/test_session_grouping.rs` (6 tests:
  a v3 database rewound from a real file under `target/temp/` migrates and
  backfills; grouping across two git roots, a linked worktree and a missing
  directory; pin and archive round trips). `test_multisession.rs` gains 6
  tests for restart restore, archived skip, rename, pin, archive/unarchive
  and the grouped list.

M5 - Sidebar UI (1 week) - DONE
- T5.1 Layout split and `sessions_sidebar.rs` rendering (D5).
- T5.2 Keyboard and mouse interaction, filter, collapse, resize.
- T5.3 Status markers (`!` waiting, `*` running, `x` failed, `o` unread),
  relative time (`now`, `5m`, `3h`, `4d`, `2w`), status-bar badge.
- T5.4 Auto-hide below 100 columns; `Alt+S` toggle (FR-10).
- Verify: `crates/ragent-tui/tests/test_sessions_sidebar.rs` (15 tests)
  renders the full layout into a `TestBackend` and asserts on the painted
  rows: project section and order, focused highlight, hidden when narrow
  or off, pinned section first, collapse, filter, waiting and failed
  markers, badge, keyboard and mouse switching, drag resize, and persisted
  width and collapse. With the sidebar forced off, the 10 rendering tests
  fail. The plan's "empty list" case is covered by the hidden and filter
  cases; two-project grouping is covered in storage
  (`test_session_grouping.rs`).

M6 - Per-session directory and config (1-1.5 weeks) - DONE (R4 open)
- T6.1 Thread session directory through config, permissions and the TUI
  (D3). Tools already took `ToolContext.working_dir`.
- T6.2 Project-keyed config cache (`Config::load_for`, one cache entry per
  directory).
- T6.3 Lift the other-directory gate; `/session new <path>` with the
  SEC-062 containment rule.
- Verify: `ragent-config/tests/test_config_load_for.rs` (3: reads the given
  project, sees an edit, keeps two projects apart),
  `ragent-agent/tests/test_permission_working_dir.rs` (3: auto-grant
  inside the session directory, prompt outside it, relative paths resolved
  against it; the inside case fails under the old process-cwd rule), and `test_multisession.rs` (switching to
  a session in another directory moves `cwd_path` and back; `/session new
  <path>` inside, outside and missing). The plan's projA/projB file-tool
  test was not needed: file tools have taken `ToolContext.working_dir`
  since before M1 (T1.2).

M7 - Titles, polish, docs (2-3 days) - DONE
- T7.1 Auto-title (FR-07): when a session's first real reply ends
  (`MessageEnd`, not the `init` exchange) and its stored title is empty,
  it is titled from its first prompt by `title_from_prompt`: the first
  line, whitespace collapsed, cut at a word boundary to 60 characters. No
  model call, so no cost and no failure path; a user title
  (`/session rename`, F2) is never replaced.
- T7.2 Notifications (FR-06): ragent had no built-in desktop notification
  path, so the existing hooks system carries it. A new `on_notification`
  trigger (Claude: `Notification`) is fired by the TUI when a background
  session starts waiting, finishes or fails
  (`hooks::fire_notification_hooks`, the same configured-plus-plugin hook
  list a turn uses), with `RAGENT_NOTIFICATION_KIND`,
  `RAGENT_NOTIFICATION_MESSAGE` and `RAGENT_SESSION_ID`. The Claude stdin
  payload carries the event name but not a `message` field.
- T7.3 SPEC.md (6.1 sidebar row, new 6.1a, `/session` row), QUICKSTART.md
  (multiple sessions, the `on_notification` example, key bindings),
  TUI-QUICKSTART.md (`--new`, sidebar section, key bindings), and
  CHANGELOG.md (Added entry plus the permission fix).
- Verify: `test_multisession.rs` gains 4 (title helper, focused and
  background titling, user title kept, a real `on_notification` hook
  writing its variables to a file); `test_plugin_hooks.rs` gains the
  trigger parse/display/serde test.

M8 - Per-project services (R4 decided: per project) (about 1.5-2 weeks)
- Phase A (DONE): code index, spec manager, custom agents and plugin commands
  follow the session's project. The processor keeps a code index per open
  project (`code_index_for(working_dir)`; the launch project keeps the
  existing slot). The TUI opens a project's index and watcher when a
  session in that project is first focused (if that project's config
  enables indexing), and drops it when no open session uses the project.
  Specs, custom agents and plugin commands are reloaded on project change.
  Implemented in `crates/ragent-tui/src/app/project_services.rs` and
  `SessionProcessor::code_index_for` / `register_project_code_index` /
  `release_project_code_index`. A project's index is opened only when its
  config enables indexing and it already has a `.ragent` directory, so
  focusing a session never creates files in an unrelated project. A
  directory outside every open project gets no index (codeindex tools
  report it unavailable) rather than the launch project's. Verified by
  `crates/ragent-tui/tests/test_project_services.rs` (4 tests).
- Phase B implementation: `SessionProcessor::open_project_mcp` /
  `release_project_mcp` / `tool_registry_for` / `project_mcp_client`. A
  project's registry is the base tools without any MCP wrapper
  (`Tool::mcp_wrapper_info`), plus its own servers' wrappers. It is rebuilt
  when the base registry's version moves, so tools registered at runtime
  still appear. `TurnClient::tool_registry` is resolved once per turn and
  every list, lookup and dispatch in the turn reads it. Instead of
  re-keying each cache, every `ToolRegistry` now counts versions in its own
  range (`registry number << 32`), so two registries never share a version
  and the existing version-keyed caches tell them apart. Opening registers
  a placeholder first, so a release that arrives during the connects stops
  that client rather than leaking it. `/mcp` lists the focused project's
  own client when it has one; the launch project's tracked list is
  unchanged. Verified by `ragent-agent/tests/test_project_mcp.rs` (5:
  isolation both ways, base-change rebuild, release fallback, distinct
  versions, and stdio servers starting in the project root) and an
  end-to-end TUI test in `test_project_services.rs` (open, `/mcp` lists the
  project's server, release on close).
- Not covered: connector-bridged servers (`/connectors`) stay on the launch
  client. The launch project's MCP status events still drive the launch
  list only.
- Phase B (DONE): MCP per project. Before it, MCP tools were global registry
  entries, so a session in project B saw and called project A's MCP tools.
  Each non-launch project now gets its own client when its first session is
  focused. Its servers are the user-global ones plus its enabled plugins'.
  A project's own `.ragent/ragent.json` cannot add servers: the
  project-local config guard drops them (SEC-001), and that stays. Stdio
  servers start in the project root (`McpClient::with_working_dir`). The
  client is shut down when the project's last session closes, and on exit.
- Verify: per-project routing tests for the code index and MCP overlay
  (two projects, each sees only its own), plus release on last close.

Follow-up - Session kinds and parent links (schema 5, after PR review)
- Problem found in real use: every sub-agent, teammate, forked-skill and
  automation session was created with `SessionManager::create_session`,
  which always stored `parent_id = NULL`. The sidebar therefore listed
  about 4000 historical agent runs as top-level sessions.
- `sessions.kind` (default `interactive`) records what created a session.
  `SessionManager::create_child_session(dir, parent, kind)` is used by
  `task` (sub-agents), `team::manager` (teammates, parented to the lead)
  and `skill::invoke` (forked skills); automation runs use
  `create_session_of_kind`. A missing parent row is logged, not fatal, so a
  team rebuilt from disk can still spawn. `Storage::archive_session` and
  `unarchive_session` apply to the session and all its descendants through
  a recursive CTE. `list_sessions_grouped` returns interactive top-level
  sessions only. `list_child_sessions` and `session_kind` expose the tree.
- Existing data: the old rows carry no reliable marker separating
  interactive sessions from agent runs. On the user's instruction, every
  live session except the 19 in `tui.sessions.open` was archived once with
  a single shared `archived_at` stamp, so the operation can be undone
  exactly.

Total: roughly 5-7 weeks. M2 is the largest risk to schedule because of
the size of `App`.

Testing
-------
- All new tests live in `crates/<crate>/tests/test_*.rs` and are registered
  in that crate's `tests/all.rs` (AGENTS-LANGUAGE.md "Test Organization").
- Tests write only under `target/temp/` (AGENTS.md "Tests must not write
  into the repository").
- Proposed files: `test_session_registry.rs`, `test_session_routing.rs`,
  `test_sessions_sidebar.rs` (ragent-tui); `test_session_grouping.rs`
  (ragent-storage); `test_session_directory_isolation.rs` (ragent-agent).

Risks and open questions
------------------------
- R1 `App` size: ~300 fields and a 10k-line `layout.rs`. Mitigation: the
  group-by-group move in M2 with tests after each step.
- R2 Process-global state: anything that relies on process cwd or a
  `static` registry becomes wrong once two projects run at once. M1 T1.2
  sizes this; D3's read-only gate lets earlier milestones ship.
- R3 Resource use: N concurrent sessions mean N concurrent provider streams
  and tool processes. Open question: cap concurrent running sessions
  (proposal: no cap, but show count in status bar)?
- R4 Shared singletons: MCP clients, LSP, code index, spec manager, custom
  agents and plugin commands are per processor today, built for the launch
  project. After M6 this is the remaining gap for sessions in other
  projects (see D3). Open question: share per project or per session?
  Proposal: per project root, reference-counted by open sessions.
- R5 Team/swarm panels are lead-session centric
  (`lead_session_created_at`, `selected_agent_session_id`). They become
  per-`SessionView`; confirm in M1 that no team state is process-global.
- Q1 Should closing the TUI stop background sessions, or leave them
  running headless (as Claude desktop does when the window closes)?
  Proposal for v1: stop them, persist state, restore on next launch.
- Q2 Worktree-per-session (Claude desktop offers isolated worktrees) is
  out of scope for v1; listed as a possible follow-up only.
  Delivered since by spec `projects` (`specs/projects/PLAN.md`): a project
  is a stored record, and `/session new --branch <branch>` gives a session
  its own git worktree.

Out of scope
------------
- Split view (two transcripts side by side).
- Worktree-per-session isolation.
- Remote/cloud sessions.
- Changes to the web/server UI.
