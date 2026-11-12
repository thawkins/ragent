# Implementation Plan: Project Object and Project-Grouped Sessions Browser

**Spec ID:** `projects` - **Status:** draft, awaiting answers to "Open Questions"

## Approach

Introduce a first-class `Project` that owns a local worktree, a base provider/model, and
an optional description of the remote the worktree was checked out from (GitHub or GitLab
initially). Every session is created inside a project and carries its `project_id`. A
session may additionally own a git worktree of the project's repository at a specific
branch. A new sessions browser lists sessions grouped under collapsible project headers,
modelled on the reference screenshot (project name header, session rows, a branch marker
with a change count, and a state dot).

Findings in the current code that shape the plan:

- `sessions.project_id` already exists (`crates/ragent-storage/src/storage.rs:904`,
  `Session::project_id` at `crates/ragent-agent/src/session/mod.rs:101`) but is always the
  empty string. The plan gives it a meaning; no column has to be added for the link itself.
- There is **no sessions browser in the TUI today**. Sessions are listed only by
  `ragent session list` (`src/main.rs:1420`) and `GET /sessions`
  (`crates/ragent-server/src/routes/mod.rs:403`). The browser is new work, not a regroup.
- `SessionManager::create_session(directory)` has 12 production call sites (TUI x5, task
  x2, team, automation, skill, ACP server, OpenAI route) plus direct
  `Storage::create_session` calls in the server route and archive import. Keeping that
  signature and resolving the project from the directory avoids touching all of them.
- Remote parsing exists: `ragent_tools_vcs::vcs_provider::{VcsProvider, parse_reverse_repo}`
  already distinguishes GitHub from GitLab (including self-hosted hosts and nested groups),
  and `ragent_tools_vcs::git::run_git_output` is the timeout-bounded git runner.
- The active model is a single global setting (`selected_model`, read in
  `crates/ragent-tui/src/app/init.rs:187`). The project base model becomes the seed for a
  new session in that project; the global setting remains the fallback.
- Schema changes go through `Storage::migrate` with `SCHEMA_VERSION` (currently 3) and the
  column-add loop at `storage.rs:1180`.

## Data Model

### `Project` (new, `crates/ragent-agent/src/project/mod.rs`)

| Field | Type | Notes |
|-------|------|-------|
| `id` | `String` | UUID v4 |
| `name` | `String` | Defaults to the worktree directory name; unique, user-editable |
| `worktree` | `PathBuf` | Canonical path of the local checkout (git toplevel when in a repo); unique |
| `base_model` | `Option<ModelRef>` | `provider_id` + `model_id`; `None` falls back to the global `selected_model` |
| `remote` | `Option<ProjectRemote>` | `None` for a purely local directory |
| `created_at` / `updated_at` / `archived_at` | `DateTime<Utc>` | Same conventions as `Session` |

### `ProjectRemote`

| Field | Type | Notes |
|-------|------|-------|
| `kind` | `RemoteKind` | Enum: `GitHub`, `GitLab` (exhaustive match; new hosts add a variant) |
| `host` | `String` | `github.com`, `gitlab.com`, or a self-hosted GitLab host |
| `path` | `String` | `owner/repo` or `group/subgroup/project` |
| `url` | `String` | Remote URL as configured, credentials stripped before storing |
| `remote_name` | `String` | Git remote name, default `origin` |
| `default_branch` | `Option<String>` | From `git symbolic-ref refs/remotes/<remote>/HEAD` when available |

A remote on an unrecognised host is stored as `remote = None` (the project still works as
a local project); this is logged at `debug!`, not treated as an error.

### `Session` additions

| Field | Type | Notes |
|-------|------|-------|
| `project_id` | `String` | Existing field; now always a valid `projects.id` |
| `branch` | `Option<String>` | Branch the session's worktree is checked out at |
| `worktree` | `Option<PathBuf>` | Session-owned git worktree; `None` means the session runs in the project worktree |

`Session::directory` stays the effective working directory: the session worktree when one
exists, otherwise the project worktree (or a subdirectory of it).

### Storage (`SCHEMA_VERSION` 3 -> 4)

```sql
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    worktree TEXT NOT NULL UNIQUE,
    provider_id TEXT,
    model_id TEXT,
    remote_kind TEXT,          -- 'github' | 'gitlab' | NULL
    remote_host TEXT,
    remote_path TEXT,
    remote_url TEXT,
    remote_name TEXT,
    default_branch TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived_at TEXT
);
ALTER TABLE sessions ADD COLUMN branch TEXT;
ALTER TABLE sessions ADD COLUMN worktree TEXT;
CREATE INDEX IF NOT EXISTS idx_sessions_project_updated
    ON sessions(project_id, updated_at DESC) WHERE archived_at IS NULL;
```

Backfill inside the same migration transaction: for each distinct `sessions.directory`
with an empty `project_id`, insert one project (`name` = directory basename, `worktree` =
directory, no remote, no base model) and set `project_id` on its sessions. The migration
runs no git commands; remote detection and git-toplevel merging happen lazily the first
time the project is opened (T-004).

## Behaviour

1. **Project resolution.** `ProjectManager::resolve_for_dir(dir)` finds the git toplevel
   (falling back to `dir` itself outside a repo), returns the project whose `worktree`
   matches, or creates one and detects its remote. `SessionManager::create_session(dir)`
   calls it, so every existing call site produces an attached session with no signature
   change. Sub-agent, team, task, automation and skill sessions inherit the project of
   their working directory, which is their parent's.
2. **Remote detection.** `git remote get-url origin` through `run_git_output`, parsed by
   `parse_reverse_repo`, mapped to `ProjectRemote`. Refreshed on `/project refresh`.
3. **Explicit creation.** `create_session_in_project(project_id, branch: Option<&str>)`
   creates a session in a named project; with a branch it first creates the session
   worktree (item 4).
4. **Session worktree.** `git fetch <remote> <branch>` then
   `git worktree add <root>/<project-name>/<branch-slug> <branch>` (creating the branch
   from the remote default branch when it does not exist). `<root>` is the ragent data
   directory `worktrees/` folder. Archiving a session leaves the worktree on disk;
   `/project prune` removes worktrees of archived sessions that have no uncommitted
   changes (`git worktree remove`, never `--force`).
5. **Base model.** A new or resumed session in a project applies the project's
   `base_model` when set, else the global `selected_model`. `/model` keeps changing the
   current session only; `/project model` writes the project base.

## Surfaces

### TUI

- **Sessions browser overlay** (`/sessions`, plus a key binding chosen in T-010): a
  scrollable list grouped by project, most recently updated project first.
  - Project header row: name, collapse marker (`v` / `>`), remote kind tag
    (`[github]` / `[gitlab]`).
  - Session row: title (fallback: first 8 id chars), branch name when set, files-changed
    count from `SessionSummary`, and a state marker (`*` running, `!` awaiting
    permission, blank idle). ASCII only, per the project style rule.
  - Keys: `Up`/`Down` move, `Left`/`Right` collapse/expand, `Enter` resumes the session
    (or toggles a header), `n` new session in the highlighted project, `Del` archives
    with a confirmation, `/` filters by title, `Esc` closes.
- **`/project` command:** `list`, `show`, `add <path|url>`, `use <name>`, `rename`,
  `model <provider>/<model>`, `refresh`, `prune`, `remove`, `help`. `add <url>` clones via
  the existing clone path and registers the result.
- **Status bar:** show the active project name and session branch.

### CLI (`src/main.rs`)

- `ragent session list` prints sessions grouped under project headings; `--project <name>`
  filters; `--json` emits the grouped structure.
- `ragent session new --project <name> [--branch <branch>]`.
- `ragent project list | show <name> | add <path|url> | remove <name>`.

### Server (`crates/ragent-server`)

- `GET /projects`, `POST /projects`, `GET /projects/{id}`, `DELETE /projects/{id}`,
  `GET /projects/{id}/sessions`.
- `POST /sessions` accepts `project_id` and optional `branch` as an alternative to
  `directory`; the SEC-062 permitted-roots check still applies to the resolved directory.
- `SessionResponse` gains `project_id`, `branch`, `worktree`. OpenAPI table
  (`routes/openapi.rs`) and `docs/openapi` updated.

### Session archive (`session/archive.rs`)

Export records the project name and remote description; import resolves or creates the
project for the import directory.

## Tasks

| ID | Title | Crate | Effort | Depends |
|----|-------|-------|--------|---------|
| T-001 | `projects` table, `sessions.branch`/`worktree` columns, index, backfill, `SCHEMA_VERSION` 4 | ragent-storage | M | - |
| T-002 | `ProjectRow` CRUD, `list_sessions_by_project`, `SessionRow` new fields, update hoisted SQL constants | ragent-storage | M | T-001 |
| T-003 | `Project`, `ProjectRemote`, `RemoteKind`, `ProjectManager` (CRUD, `resolve_for_dir`), events `ProjectCreated`/`ProjectUpdated` | ragent-agent | M | T-002 |
| T-004 | Remote detection from the git remote (GitHub, GitLab incl. self-hosted), credential stripping, lazy refresh for backfilled projects | ragent-agent, ragent-tools-vcs | S | T-003 |
| T-005 | `create_session` resolves the project; `create_session_in_project`; `Session` gains `branch`/`worktree` | ragent-agent | M | T-003 |
| T-006 | Session worktree create/prune (`git worktree add/remove` through `run_git_output`) | ragent-agent, ragent-tools-vcs | M | T-005 |
| T-007 | Project base model applied on session create/resume; `/project model` | ragent-tui, ragent-agent | S | T-005 |
| T-008 | `/project` slash command, help, command catalogue entry | ragent-tui | M | T-005 |
| T-009 | Sessions browser state + grouping model (pure, testable without a terminal) | ragent-tui | M | T-005 |
| T-010 | Sessions browser overlay rendering, key handling, key binding, session switch | ragent-tui | L | T-009 |
| T-011 | CLI: grouped `session list`, `session new`, `project` subcommands | ragent (root) | S | T-005 |
| T-012 | Server routes, `SessionResponse` fields, OpenAPI | ragent-server | M | T-005 |
| T-013 | Session archive export/import carries project | ragent-agent | S | T-005 |
| T-014 | SPEC.md, QUICKSTART.md, TUI-QUICKSTART.md, CHANGELOG.md | docs | S | all |

Effort key: S = under half a day, M = about one day, L = two to three days. Total: about
10 to 12 working days. T-001 to T-005 (about 4 days) deliver attached sessions end to end;
T-009/T-010 (about 4 days) deliver the browser.

## Verification

Each task lands with tests in the crate's `tests/` directory, registered in `tests/all.rs`.

| Task | Test file | Checks |
|------|-----------|--------|
| T-001 | `ragent-storage/tests/test_projects_migration.rs` | v3 database upgrades; each distinct directory gets one project; every session has a non-empty `project_id`; migration is idempotent |
| T-002 | `ragent-storage/tests/test_projects_crud.rs` | CRUD round trip; unique `worktree`; grouped list uses the new index (query-plan assertion, as in `test_sessions_list_plan.rs`) |
| T-003/4 | `ragent-agent/tests/test_project_manager.rs`, `test_project_remote.rs` | `resolve_for_dir` is stable for a repo subdirectory; https and ssh GitHub URLs, gitlab.com, self-hosted GitLab with nested groups; unknown host yields `None`; embedded credentials are stripped |
| T-005 | `ragent-agent/tests/test_session_project_attach.rs` | Every creation path yields a session with a valid project; child sessions share the parent's project |
| T-006 | `ragent-agent/tests/test_session_worktree.rs` | Worktree is created at the requested branch under a `target/temp` root; prune skips dirty worktrees |
| T-009 | `ragent-tui/tests/test_sessions_browser_model.rs` | Grouping, ordering, collapse state, filter |
| T-010 | `ragent-tui/tests/test_sessions_browser_render.rs` | Rendered rows for a fixed fixture; key handling; `Enter` switches session |
| T-012 | `ragent-server/tests/test_projects_routes.rs` | Route round trips; `POST /sessions` with `project_id`; permitted-roots check still rejects outside paths |

All git-using tests build their repositories under `target/temp/<unique-name>` and pass
the worktree root explicitly; no test writes into the repository.

## Open Questions

1. **Session worktree default.** Plan assumes it is opt-in: a session uses the project
   worktree unless a branch is requested. Alternative: every new session gets its own
   worktree and branch (matches the reference screenshot's one-branch-per-session look,
   costs disk and a checkout per session).
2. **Worktree location.** Plan assumes the ragent data directory (`worktrees/<project>/<branch>`).
   Alternative: a sibling directory next to the project worktree.
3. **Row count in the browser.** Plan shows files changed (already tracked in
   `SessionSummary`). The reference screenshot's number next to the branch marker may
   mean something else (commits or pull-request number); confirm the intended meaning.
4. **Sub-agent sessions in the browser.** Plan hides sessions with a `parent_id` and shows
   only top-level sessions.
