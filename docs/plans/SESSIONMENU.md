# SESSIONMENU: Session Context Menu and Auto-fix

Feature ID: SESSIONMENU
Title: Per-session context menu in the sessions sidebar, plus PR/MR Auto-fix
Status: M1-M5 done; M6 planned.
Reference UX: Claude desktop app, Code mode (sidebar row context menu, Auto-fix)
Builds on: [MULTISESSION.md](MULTISESSION.md) (M1-M8 done, schema 5)
Roadmap: ROADMAP.md items 4 and 5

Overview
--------
The sessions sidebar (`crates/ragent-tui/src/sessions_sidebar.rs`) already
acts on the row under the cursor with single keys: `p` pin, `a` archive, `F2`
rename. Claude desktop exposes the same actions, and more, through a context
menu on each row. This plan adds that menu and implements every item in it,
then adds Auto-fix: a session bound to a pull request (GitHub) or merge
request (GitLab) is woken when CI fails, the branch conflicts, or a reviewer
comments, and fixes it.

Reference menu (Claude desktop, accelerator in brackets):

| # | Item | Key | ragent today | Gap |
| --- | --- | --- | --- | --- |
| 1 | Open PR | `G` | none | session-to-PR binding, browser open |
| 2 | Open in > | - | none | submenu: editor, file manager, terminal |
| 3 | Pin | `P` | `p` key, `/session pin` | menu entry only |
| 4 | Mark as unread / read | `U` | `Attention::Unread` (runtime only) | persisted, user-settable flag |
| 5 | Rename | `R` | `F2`, `/session rename` | menu entry only |
| 6 | Fork | `F` | none | copy transcript into a new child session |
| 7 | Move to section > | - | sections are projects only | user-defined sections |
| 8 | Archive | `A` | `a` key, `/session archive` | menu entry only |
| 9 | Delete | `D` | none | hard delete with confirmation |

Assumptions (correct these before M1 starts)
--------------------------------------------
- A1 The menu is the TUI sidebar only. The HTTP server and web UI get the
  storage APIs (Delete, Fork, sections, unread) as routes only where a route
  already exists for the neighbouring action (archive). No new web UI.
- A2 "Open in" offers: `$VISUAL`/`$EDITOR` in the session directory, the
  system file manager (`xdg-open` / `open` / `explorer`), and a new terminal
  in the session directory. Split view (two transcripts side by side) stays
  out of scope, as MULTISESSION.md already decided. Opening the same session
  in a second ragent process is not offered: two processes writing one
  session would corrupt its message order.
- A3 Fork copies the transcript up to now into a new session in the same
  directory; it does not create a git worktree. Worktree-per-session stays
  out of scope (MULTISESSION Q2). Auto-fix works on whatever branch the
  session directory has checked out.
- A4 Auto-fix vocabulary follows the provider: "pull request" / "PR" for
  GitHub, "merge request" / "MR" for GitLab, everywhere the user reads it
  (menu label, status line, wake-up prompt, slash output). Menu item 1 reads
  `Open PR` or `Open MR`.
- A5 On-prem: GitLab self-managed already works through `GITLAB_URL` /
  `instance_url`. GitHub Enterprise Server needs a configured API base URL
  (`GitHubClient::with_base_url` exists but is test-only today).

Functional requirements
-----------------------
Context menu

- FR-01 Opening: `m` or `Shift+F10` on a session row while the sidebar has
  focus, or right-click on a session row (which also moves the cursor there
  without switching sessions). Header rows and `+ New session` open no menu.
  Claude's hover `...` marker is not copied: it needs mouse-move tracking
  for one more way to do what right-click already does.
- FR-02 Rendering: a bordered popup anchored to the row (flipped upward when
  it would leave the frame), items with right-aligned accelerators,
  separators between the four groups shown in the reference, Delete drawn in
  the theme's error colour. Items that do not apply are hidden, not greyed
  (Open PR/MR with no bound request).
- FR-03 Keys inside the menu: `Up/Down` move, `Enter` activates, `Right`
  opens a submenu, `Left`/`Esc` closes one level, accelerators (`G P U R F A
  D`, case-insensitive) activate directly. The menu consumes every key while
  open and is dismissed by a click outside it. Modal dialogs (permission,
  question) still take precedence, as the sidebar already does.
- FR-04 Existing single keys stay: `p`, `a`, `F2`. New direct keys while the
  sidebar has focus (no menu needed): `u` unread toggle, `f` fork, `D`
  (shift) delete. `r` and `g` are left to the menu, since `r` is close to
  `Alt+R`.

Menu items

- FR-05 Open PR/MR: opens the bound request's web URL with the platform
  opener (the same helper as "Open in > file manager"). Shown only when the
  session has a binding (FR-14).
- FR-06 Open in: submenu with Editor, File manager, Terminal. Each spawns a
  detached process with the session directory as cwd and never blocks the
  TUI. Editor is hidden when neither `VISUAL` nor `EDITOR` is set; Terminal
  uses a configurable command (`tui.open_terminal`, default
  `$TERMINAL` or `x-terminal-emulator` on Linux, `open -a Terminal` on
  macOS). A spawn failure is reported on the status line.
- FR-07 Pin: same as `p` (`App::toggle_session_pin`). Label reads `Unpin`
  when pinned.
- FR-08 Mark as unread / Mark as read: toggles a persisted `unread` flag.
  Focusing a session clears it (as `Attention::Unread` does today). A run
  that ends while the session is in the background sets it, so the existing
  `o` marker now survives a restart.
- FR-09 Rename: same as `F2` (switch, prefill `/session rename <title>`).
- FR-10 Fork: creates a new top-level interactive session in the same
  directory, titled `<source title> (fork)`, not linked to its source, and
  copies every message of the source. A running source is refused. The fork
  opens focused. `/session fork [id]` does the same.
- FR-11 Move to section: submenu lists `Default (project)`, every
  user-defined section, and `New section...` (prompts for a name). A session
  in a user section is shown under that section instead of its project.
  Sections are global (not per project), ordered by name, collapsible like
  project headings, and appear after `Pinned` and before the projects. Pin
  still wins: a pinned session shows only under `Pinned`.
  `/session section <name>|--none`, `/session section new|rename|delete`.
  Deleting a section returns its sessions to their projects.
- FR-12 Archive: same as `a`. Unchanged rule: refuses a running session.
- FR-13 Delete: confirmation dialog naming the session title, its child
  session count (sub-agents, teammates; forks are unlinked and survive) and
  "cannot be undone". A running session is refused before asking. On yes:
  close its view, delete the session row, its descendants, messages and
  their search entries (M5 adds the PR/MR binding). Also `/session delete
  <id|prefix>`, which asks the same confirmation and also finds archived
  sessions.

Auto-fix

- FR-14 Binding: a session is bound to at most one PR/MR. Binding happens
  (a) automatically when a VCS tool in that session creates one
  (`github_create_pr`, `gitlab_create_mr`), (b) with `/autofix bind <number|url>`,
  (c) automatically at session focus when the session directory's current
  branch has exactly one open PR/MR and the session has no binding (status
  line offers it; it is not bound silently). `/autofix unbind` removes it.
- FR-15 Provider detection: from the session directory's `origin` remote via
  the existing `VcsProvider` parsing; GitHub Enterprise hosts are recognised
  from the configured host list (FR-21). A remote that matches nothing makes
  Auto-fix unavailable for that session with a status-line reason.
- FR-16 Toggle: `/autofix on|off` per session, plus a menu entry
  `Auto-fix: on/off` in the first group when the session has a binding.
  Default off; `vcs.autofix.default_on` turns it on for new bindings.
  Persisted per session.
- FR-17 What wakes a session (each one is a separate trigger):
  - CI: a check run / commit status (GitHub) or the head pipeline (GitLab)
    for the bound head SHA finishes failed.
  - Conflicts: GitHub `mergeable == false` / `mergeable_state == dirty`;
    GitLab `detailed_merge_status == conflict` (or `has_conflicts`).
  - Review comments: a new unresolved review thread or comment on the PR/MR
    by someone other than the token's user.
  Each trigger fires once per (head SHA, trigger, item id), so a failure is
  not re-reported until a new commit is pushed.
- FR-18 Wake-up: the watcher injects a user-role message into the bound
  session ("CI failed on PR #67: job `clippy` ... Fix it, commit and push.")
  with the failing job names and a log tail (GitLab job log, GitHub Actions
  job log), the conflicting files, or the comment text and location. If the
  session is busy, the message is queued on that session's input queue. A
  background session gets the existing `!`/unread marker and an
  `on_notification` hook with `RAGENT_NOTIFICATION_KIND=autofix`.
- FR-19 Guard rails: at most `vcs.autofix.max_attempts` (default 3) wake-ups
  per trigger kind per head SHA; after that the session is marked needing the
  user and Auto-fix pauses for that PR/MR. Auto-fix never merges, never
  force-pushes, and never resolves review threads on its own; the wake-up
  prompt says so. It runs under the session's normal permission mode, so a
  push still prompts unless the session is in a mode that allows it.
- FR-20 Lifecycle: when the PR/MR is merged or closed, the watcher stops,
  the binding stays (read-only) so `Open PR/MR` still works, and the status
  line says "PR #67 merged". Archiving or deleting the session stops the
  watcher.
- FR-21 Config (`vcs` block in `ragent.json`, global config only, the same
  SEC-001 rule that keeps project config from adding MCP servers):
  `vcs.github.hosts: [{ host, api_url }]` for GitHub Enterprise,
  `vcs.autofix.poll_secs` (default 60, min 30), `max_attempts`,
  `default_on`. GitLab keeps `GITLAB_URL` / its existing config.
- FR-22 Status: the sidebar row shows a PR/MR glyph (`#`) when bound and
  `#!` when Auto-fix is waiting on the user; the status bar shows
  `PR #67 auto-fix: CI failing (attempt 2/3)` for the focused session.
  `/autofix status` prints binding, provider, last poll, last trigger and
  attempt counts.

Non-functional requirements
---------------------------
- NFR-01 Polling only; no webhooks (ragent has no public endpoint). One
  watcher task per process polls all bound, enabled sessions, batched per
  repository, with ETag/`If-None-Match` on GitHub to stay inside rate limits.
  Backoff on 403/429 using the existing `http_client` retry rules.
- NFR-02 The watcher runs only while ragent runs. Bindings and trigger
  dedup state are persisted so a restart neither loses nor repeats a
  trigger.
- NFR-03 Tokens come only from the existing auth paths
  (`github/auth.rs`, `gitlab/auth.rs`); nothing new stores credentials.
- NFR-04 No behaviour change for a user who never opens the menu or binds a
  PR/MR.
- NFR-05 Schema changes are additive (schema 6), as in MULTISESSION D4.

Design
------

### D1. Context menu widget (`crates/ragent-tui/src/session_menu.rs`)

- `SessionMenu { session_id, anchor: Rect, items: Vec<MenuItem>, cursor,
  submenu: Option<Box<SessionMenu>> }` lives in `SidebarState::menu`.
- `MenuItem { label, accel: Option<char>, action: MenuAction, danger: bool }`
  and `MenuAction` enum (`OpenRequest`, `OpenIn(OpenTarget)`, `TogglePin`,
  `ToggleUnread`, `Rename`, `Fork`, `MoveTo(SectionTarget)`, `NewSection`,
  `ToggleAutofix`, `Archive`, `Delete`, `Separator`). Exhaustive match in
  one `dispatch` function; each arm calls the existing `App` method.
- `build_menu(app, session_id)` decides visibility (binding present, pinned,
  unread, editor set). Rendering is painted after the chat and sidebar and
  before modal dialogs, so a permission prompt still covers it.
- Key and mouse routing: `sessions_sidebar::handle_key` / `handle_mouse`
  check `state.menu` first. Right-click is `MouseEventKind::Down(Right)`,
  already delivered by crossterm with mouse capture on.
- Delete confirmation reuses the existing confirm-dialog path used by
  other destructive slash commands (check in M1 which one; do not add a
  second dialog type).

### D2. Storage (`crates/ragent-storage`, schema 6)

```sql
ALTER TABLE sessions ADD COLUMN unread INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN section_id TEXT;
CREATE TABLE IF NOT EXISTS sidebar_sections (
    id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS session_requests (       -- Auto-fix binding
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,            -- 'github' | 'gitlab'
    host TEXT NOT NULL, repo TEXT NOT NULL, number INTEGER NOT NULL,
    web_url TEXT NOT NULL, head_branch TEXT NOT NULL, head_sha TEXT,
    state TEXT NOT NULL,               -- open | merged | closed
    autofix INTEGER NOT NULL DEFAULT 0,
    last_polled_at TEXT, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS autofix_events (         -- dedup + attempt count
    session_id TEXT NOT NULL, head_sha TEXT NOT NULL,
    kind TEXT NOT NULL,                -- ci | conflict | review
    item_id TEXT NOT NULL, fired_at TEXT NOT NULL,
    PRIMARY KEY (session_id, head_sha, kind, item_id));
```

- Like `pinned`, the new session columns stay out of `SessionRow`; they get
  their own getters/setters (`set_session_unread`, `unread_session_ids`,
  `set_session_section`, `session_sections`).
- `delete_session(id)`: one transaction, recursive CTE over `parent_id`
  restricted to non-`interactive`/non-`fork` kinds (children such as
  sub-agents go; forks stay and have their `parent_id` set to NULL), then
  parts, messages, bindings, sessions. The existing archive CTE is the model.
- `fork_session(src, upto_message_id) -> SessionRow`: one transaction that
  creates the row (`kind = 'fork'`, `parent_id = src`) and copies messages
  and parts with new ids, preserving order and timestamps.
- `list_sessions_grouped` returns `fork` sessions alongside `interactive`
  ones (they are user sessions); sub-agent kinds stay hidden.
- Agent layer: `SessionManager::delete_session` / `fork_session` wrap the
  storage calls and drop `session_state_cache` entries
  (`remove_session_state`).

### D3. Sidebar data (`sessions_sidebar::build_rows`)

Row order becomes: `+ New session`, `Pinned`, user sections (by name), then
projects as today. The cached storage snapshot gains the section list,
section membership and the unread and binding sets; it is still reloaded
only when `SidebarState::dirty` is set. `Attention::Unread` becomes
"runtime unread OR persisted unread"; focusing clears both.

### D4. VCS additions (`crates/ragent-tools-vcs`)

New read-only client functions (not new agent tools; the watcher calls them
directly, and the agent keeps its existing tools for fixing):

| Need | GitHub | GitLab |
| --- | --- | --- |
| Find request for a branch | `GET /repos/{o}/{r}/pulls?head={o}:{branch}&state=open` | `GET /projects/{id}/merge_requests?source_branch=&state=opened` |
| Request state, head SHA, conflicts | `GET /pulls/{n}` (`mergeable`, `mergeable_state`, `head.sha`, `merged`) | `GET /merge_requests/{iid}` (`detailed_merge_status`, `has_conflicts`, `sha`, `state`) |
| CI result for head SHA | `GET /commits/{sha}/check-runs` + `/status` | `head_pipeline` on the MR, then `/pipelines/{id}/jobs` |
| Failure log tail | Actions job log (`/actions/jobs/{id}/logs`) | existing `gitlab_get_job_log` path |
| Review comments | GraphQL `pullRequest.reviewThreads` (has `isResolved`) | `GET /merge_requests/{iid}/discussions` (`resolvable`, `resolved`) |

- `vocab.rs` gains `RequestNoun { short: "PR"|"MR", long: "pull request"|"merge request" }`
  from `VcsProvider`, used by every user-facing string in this plan (A4).
- GitHub Enterprise: `GitHubClient::for_host(host)` resolves `api_url` from
  `vcs.github.hosts` (FR-21), and the remote parser accepts those hosts.
  GraphQL URL for GHES is `<api_url>/graphql` with `/api/v3` stripped.

### D5. Auto-fix watcher (`crates/ragent-agent/src/autofix/`)

- `AutofixWatcher` is a tokio task owned by `SessionProcessor`, started at
  TUI startup, with a `watch` channel for "bindings changed".
- Each tick: load enabled, open bindings; group by (provider, host, repo);
  fetch state per request; compute triggers; drop ones already in
  `autofix_events`; enforce `max_attempts`; record; then publish
  `Event::AutofixTrigger { session_id, kind, summary, prompt }` on the
  `EventBus`.
- The TUI handles that event through the existing MULTISESSION D2 routing:
  focused or background, the prompt is sent (or queued if busy) through the
  same path as a typed user message, and the attention marker and
  `on_notification` hook fire for background sessions.
- Binding on create (FR-14a): the PR/MR create tools return number and URL
  already; the tool-result handler for those two tool names writes the
  binding for the calling session (`ToolContext.session_id`).
- A head SHA change (the agent pushed) resets attempt counts for the new
  SHA, by construction of the dedup key.

### D6. Slash commands

- `/session fork [id]`, `/session delete <id|prefix>`,
  `/session unread [id]`, `/session section ...` (FR-11).
- New `/autofix` trigger: `bind <number|url>`, `unbind`, `on`, `off`,
  `status`, `help`. Output uses the provider noun (A4).

Milestones
----------
Estimates: one developer familiar with the codebase, tests and docs
included.

M1 - Confirm and wire the menu shell (2 days) - DONE
- T1.1 A1-A5 confirmed. Delete (M2) will use the existing destructive
  row-action dialog (`App::row_action_confirm`, `RowAction`) with a new
  `RowAction::DeleteSession` variant: default `No`, so a stray `Enter`
  cannot delete.
- T1.2 `crates/ragent-tui/src/session_menu.rs`: `SessionMenu` in
  `SidebarState::menu`, painted by `layout::render` after the chat and
  before every modal overlay; `sessions_sidebar::handle_key`/`handle_mouse`
  give it first refusal. Items: Pin/Unpin (`P`), Rename (`R`), Archive
  (`A`), dispatched to the same `App` methods as `p`, `F2` (now
  `sessions_sidebar::begin_rename`) and `a`. Submenus arrive with their
  first user in M3. The menu closes if the sidebar is hidden.
- Verify: `ragent-tui/tests/test_session_menu.rs` (9 tests) renders the
  menu into a `TestBackend` and checks: `m` opens it below its row with
  accelerators and a rule between groups; `Shift+F10` opens and `Esc`
  closes; header rows open nothing; right-click opens for the clicked row
  without switching; `P` pins and the label becomes `Unpin`; `End`+`Enter`
  archives; a click on Rename prefills `/session rename`; a click outside
  closes without acting; the menu flips above a row near the bottom.

M2 - Mark as unread, Delete, Fork (3-4 days) - DONE
- T2.1 Schema 6: `sessions.unread` (additive, default 0) with
  `Storage::set_session_unread` / `unread_session_ids`. A run that ends in
  the background stores the mark; focusing a session clears it. The
  sidebar caches the set (`SidebarState::unread`) and shows `o` for an
  idle session marked in storage, so the mark survives a restart and shows
  for sessions that are not open. Menu `Mark as unread`/`Mark as read`
  (`U`), sidebar key `u`, `/session unread [id-or-title]`.
- T2.2 `Storage::delete_session` (one transaction: the session and all
  descendants, their messages and search index entries, run cost
  summaries, background task rows and todos; memories and initiatives are
  project knowledge and stay) and `session_tree_ids`;
  `SessionManager::delete_session` also drops cached session state. The TUI
  asks through the existing row-action dialog
  (`RowAction::DeleteSession`, default `No`), naming the title and the
  sub-session count. Menu `Delete` (`D`, red), sidebar key `D`,
  `/session delete <id-or-title>` (archived sessions too). An open session
  is closed first; the focused one hands focus to the most recent other
  session.
- T2.3 `Storage::fork_session` / `SessionManager::fork_session`: a new
  top-level interactive session in the same directory titled
  `<title> (fork)`, every message copied with a new id, original
  timestamps and its own search entries. Menu `Fork` (`F`), sidebar key
  `f`, `/session fork [id-or-title]`; the fork opens focused.
- Changed from the design above, for simplicity: a fork is not linked to
  its source (no `fork` kind, no `parent_id`). Linking it would have made
  archive and delete cascade into forks and hidden them from the sidebar's
  top-level listing, and nothing reads the link. Delete and Fork refuse a
  running session ("stop it first"), as Archive already does, instead of
  stopping it or cutting the in-flight turn.
- Verify: `ragent-storage/tests/test_session_delete_fork.rs` (6: unread
  round trip skipping archived; delete removes the tree, messages and index
  entries and nothing else; delete of a missing id; fork copies messages
  with new ids, order, timestamps and search entries and survives deleting
  the source; fork of a missing id creates nothing; a v5 database migrates
  to v6 intact). `test_session_menu.rs` grows to 17: all six items and
  their accelerators, unread marker until focused, unread shown for a
  closed session, fork focuses the copy, fork and delete refuse a running
  session, delete asks with `No` as default, `Yes` deletes a background
  session, deleting the focused session focuses another.

M3 - Sections and Open in (3 days) - DONE
- T3.1 Schema 7: `sidebar_sections` (name unique ignoring case) and
  `sessions.section_id`. Storage: `create_section`, `rename_section`,
  `delete_section` (its sessions go back to their projects),
  `list_sections` (by name), `set_session_section`, `session_sections`
  (non-archived members of existing sections). The sidebar builds one
  ordered section list (`sessions_sidebar::sections`) used by both the rows
  and `Ctrl+Up`/`Ctrl+Down`: `Pinned`, user sections, then projects. A
  section collapses like a project; an empty one shows no header.
  Menu `Move to section >` lists `Project (default)`, the sections (the
  current place marked `*`) and `New section...`, which focuses the session
  and prefills `/session section new `. `/session section` lists;
  `<name>` / `--none` move the focused session; `new <name>` creates and
  moves; `rename <name>` and `delete [name]` act on a section (default: the
  focused session's).
- T3.2 `crates/ragent-tui/src/open_in.rs`: `launch_spec(target, dir, env)`
  resolves the program from a `LaunchEnv` (OS, variables, `PATH`) and
  `spawn` starts it detached. Editor: `$VISUAL`, else `$EDITOR`, on `.`; a
  terminal editor (vim, nano, helix, `emacs -nw`, ...) runs in a new
  terminal, since it would fight the TUI for its own. File manager:
  `xdg-open` / `open` / `explorer`. Terminal: `$TERMINAL`, else the first
  common Linux terminal on `PATH`; Terminal.app; `cmd`. Menu `Open in >`
  lists only the targets that resolve. The `tui.open_terminal` setting in
  FR-06 was not added: `$TERMINAL` covers it. Test hook:
  `SidebarState::launch_log` records instead of starting.
- The menu gained submenus (`SessionMenu::submenu`): `Right`/`Enter`/click
  opens, `Left`/`Esc` closes one level, a click outside closes both. It
  opens to the right of the menu, or to the left when there is no room.
- Verify: `ragent-storage/tests/test_sidebar_sections.rs` (5: create and
  order, duplicate and empty names, rename conflicts, move/delete returning
  sessions with archived left out, back to project, v6 to v7 migration),
  `ragent-tui/tests/test_open_in.rs` (7: file manager per OS, `VISUAL`
  before `EDITOR`, no editor variable, terminal editors inside a terminal,
  terminal found on a fake `PATH`, none found, macOS), and
  `test_session_menu.rs` grows to 23 (move via submenu with the row placed
  under the section header, New section prefill and create, `Esc` closes
  only the submenu, pin beats section, Open in records the file-manager
  launch, `/session section` commands).

M4 - Binding on top of projects (about 3 days) - DONE (revised after PR #68)
PR #68 (spec `projects`) landed while M4 was starting. It already has:
`ragent_agent::project::remote` (`RemoteKind`, `ProjectRemote { kind,
host, path }`, `parse_remote_url`, `detect_remote`; github.com, gitlab.com,
`gitlab.*` and the configured GitLab instance), a project record per
checkout that stores its remote, `SessionCheckout { branch, worktree,
pr_count }` per session, and `project::requests` (`RequestSource` clients
that never send the GitLab token to another host, one list call per
project, rate-limit back-off, the per-branch count beside each sidebar
row). M4 builds on these instead of adding a second remote parser.
- Already done before the merge: `vcs.github.hosts` config (global only;
  a project-local file is rejected) and schema 9 `session_requests`
  (`set_session_request`, `session_request`, `clear_session_request`,
  `root_session_id`; deleted with its session).
- T4.1 GitHub Enterprise: `classify_host` and `RequestSource::source_for`
  accept the `vcs.github.hosts` hosts and use their `api_url`
  (`GitHubClient::with_base_url`). The token for those hosts is
  `GH_ENTERPRISE_TOKEN` / `GITHUB_ENTERPRISE_TOKEN`, never the github.com
  token. `RemoteKind::noun()` gives `PR`/`pull request` or `MR`/`merge
  request` (A4).
- T4.2 Find and read one request: `open_requests_for_branch(source,
  remote, branch)` and `get_request(source, remote, number)` next to
  `fetch_request_counts`, returning number, title, web URL, source branch
  and state (open, merged, closed).
- T4.3 Binding: `github_create_pr` / `gitlab_create_mr` bind the new
  request to the calling session's top-level session through
  `ToolContext.storage` (a binding failure is logged, never fails the
  tool). `/autofix bind [number|url]` binds the given request, or with no
  argument the session branch's single open one; `/autofix unbind`;
  `/autofix status` shows the binding and, when there is none, offers the
  branch's open request. The plan's focus-time offer is dropped: it would
  make a network call on every session switch.
- T4.4 Menu: `Open PR` / `Open MR` (`G`) first, shown when bound; opens the
  web URL with the platform opener.
- Verify: request lookups against a `wiremock` server for GitHub, GitHub
  Enterprise (`/api/v3`) and GitLab self-managed; binding storage
  round trip; `/autofix` commands and the menu item in `ragent-tui`.
- Done as planned, with these details:
  - The remote parser keeps its old signatures; `parse_remote_url_with_hosts`
    and `detect_remote_with_hosts` take the `vcs.github.hosts` list.
    `ProjectManager` and `PullRequestCounts` get the hosts through
    `with_github_hosts` (TUI, server and `ragent project` CLI pass them).
  - A GitHub remote on a host that is neither github.com nor configured gets
    no API client, so no token is sent to it.
  - `bind_session(storage, session, source, remote, BindTarget)` returns
    `Bound`, `NoneOpen` or `Several`; only `Bound` writes. Storage gained
    `bind_session_request` (binds the top-level session); the VCS
    `StorageBackend` trait gained the same method with a refusing default.
  - `RequestNoun` also carries the number sigil (`#` / `!`).
  - The menu item is shown only when bound; it opens the stored URL with
    `open_in::url_spec`, which refuses anything but `http(s)://`.
  - Tests: `ragent-agent` `test_request_binding` (7), `ragent-tools-vcs`
    `test_request_binding` (3), `ragent-tui` `test_autofix` (4) and two
    more in `test_session_menu`.

M5 - Auto-fix watcher (1 week) - DONE
- T5.1 `AutofixWatcher`, trigger computation, dedup, attempt cap, merge/close
  lifecycle (FR-17, FR-19, FR-20).
- T5.2 `AutofixTrigger` event routing into focused and background sessions,
  queueing when busy, notification hook (FR-18).
- T5.3 Toggle, status bar text, sidebar glyphs (FR-16, FR-22).
- Verify: `ragent-agent/tests/test_autofix_watcher.rs` drives the watcher
  with a scripted mock server through: CI fail -> one trigger; same SHA next
  tick -> none; new SHA -> counts reset; three failures -> paused and marked
  needing the user; conflict and review triggers; merged -> stops. A TUI
  test asserts a background session gets the prompt queued and the `!`
  marker without a dialog over the focused session.
- Done as planned, with these details:
  - Storage (schema 10): `session_requests` gains `autofix`,
    `autofix_paused`, `head_sha`, `last_polled_at`, `last_trigger`; new
    `autofix_events` table. Rebinding keeps the on/off setting; paused state
    and head commit are kept only when the same request is bound again.
    Deleting a session deletes its `autofix_events` rows.
  - `vcs.autofix { poll_secs, max_attempts, default_on }` (global config
    only, like `vcs.github`). `default_on` applies when a session is bound
    for the first time, by a create tool or by `/autofix bind`.
  - `ragent_agent::autofix::AutofixWatcher` takes a source resolver (the
    TUI passes `PullRequestCounts::source_for`, so the GHES and GitLab host
    rules of M4 apply) and is started once at TUI startup; `/autofix on`
    wakes it at once. One poll per bound request per tick (not batched per
    repository; NFR-01 batching and ETags are not done).
  - Triggers: GitHub failed check runs (`failure`, `timed_out`) and failed
    commit statuses, Actions job log tail; `mergeable == false` or
    `mergeable_state == dirty`; inline review comments and review bodies by
    anyone but the token's user (REST, so dedup on id per R1). GitLab head
    pipeline failed jobs (`allow_failure` jobs left out) with the job log
    tail; `detailed_merge_status == conflict` or `has_conflicts`;
    unresolved resolvable notes by anyone but the token's user.
  - Attempts are wake-ups per kind per head commit; the next one pauses
    Auto-fix (`paused` event, `#!` glyph) until `/autofix on`. R3's
    `max_daily` is not done.
  - `Event::AutofixTrigger { session_id, kind, summary, prompt }`. The TUI
    sends the prompt as a user message, queues it when the session is busy,
    and opens a session that is not open in the background first. A
    background session is marked unread and fires `on_notification` with
    kind `autofix`. `paused` and `closed` events only add a notice.
  - Menu `Auto-fix: on/off` (`X`) under `Open PR/MR`; sidebar `#` / `#!`;
    the status bar shows the focused session's latest summary.
  - Tests: `ragent-agent` `test_autofix_watcher` (7), `ragent-tui`
    `test_autofix` (4 more) and `test_session_menu` (1 more).

M6 - Docs and polish (1-2 days)
- SPEC.md (sidebar menu, `/autofix`, `vcs` config), QUICKSTART.md and
  TUI-QUICKSTART.md (menu keys, Auto-fix walkthrough for GitHub and GitLab),
  CHANGELOG.md.

Total: about 4-5 weeks. M5 is the schedule risk (two providers, three
trigger kinds, rate limits).

Testing
-------
- All tests in `crates/<crate>/tests/test_*.rs`, registered in that crate's
  `tests/all.rs`; run with `cargo nextest run -p <crate> -E 'test(test_x::)'`.
- Tests write only under `target/temp/`; no real network: every VCS test
  uses a local mock server.

Risks and open questions
------------------------
- R1 GitHub review-thread resolution is GraphQL-only; a token without
  GraphQL access (some fine-grained PATs) falls back to REST review comments
  without resolved state, so a resolved thread could re-trigger. Mitigation:
  dedup on comment id, so each comment triggers at most once.
- R2 Two ragent processes with the same session open would both poll and
  both wake it. Mitigation: the `autofix_events` primary key makes the
  second insert fail, and only the inserter publishes.
- R3 An agent that pushes a "fix" that does not fix CI burns attempts per
  SHA forever (each push is a new SHA). Proposal: also cap total wake-ups per
  binding per day (`max_daily`, default 10).
- Q1 Should Delete be allowed on a session that has an open PR/MR bound?
  Proposal: yes, with the bound request named in the confirmation; the PR/MR
  itself is never touched.
- Q2 Should Fork also fork the Auto-fix binding? Proposal: no; the fork is
  unbound, since two sessions fixing one branch would race.

Out of scope
------------
- Split view and opening a session in a second ragent window.
- Worktree-per-session (and so "fork into a new branch").
- Webhook delivery, auto-merge, and resolving review threads automatically.
- Other forges (Bitbucket, Gitea).
