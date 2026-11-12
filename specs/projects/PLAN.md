# Implementation Plan: Project Object and Sessions Sidebar Upgrade

**Spec ID:** `projects` - **Status:** draft, awaiting answers to "Open Questions"

**Builds on:** branch `feat/multisession-m2-session-view` (MULTISESSION M1-M8 plus the
schema-5 session-kind follow-up; plan in `docs/plans/MULTISESSION.md` on that branch).
That branch is not merged into `main`. This work must start from it, or after it merges.

## Approach

MULTISESSION already delivers the sessions sidebar: sessions grouped by project,
collapsible headings, pin, archive, filter, attention markers, per-session directory and
model. In it a "project" is only a string: `sessions.project_id` holds the git work-tree
root path computed by `ragent_storage::project_root`. There is no project record, so
nothing can hold a base model or a remote description, and worktree-per-session was left
out of scope (MULTISESSION Q2).

This plan promotes the project to a stored object and closes the gap between the existing
sidebar and the reference screenshot:

1. A `projects` table and a `Project` type: local worktree, base provider/model, optional
   remote description (GitHub or GitLab initially).
2. Every session is created in, and attached to, a project record.
3. A session may own a git worktree of the project repository at a chosen branch.
4. The sidebar renders what the screenshot shows: chevron project headings, a branch
   marker with a count on each row, and a right-aligned state marker.

## What Exists (on the base branch) and What Changes

| Area | Exists today | Change |
|------|--------------|--------|
| Project identity | `sessions.project_id` = root path string; `project_root()` in `ragent-storage/src/project.rs` | `project_id` becomes a `projects.id`; the path moves to `projects.worktree` |
| Grouping API | `Storage::list_sessions_grouped() -> Vec<ProjectSessions { project_id, sessions }>` | `ProjectSessions` carries the `ProjectRow`; projects with no sessions are listed too |
| Session creation | `create_session`, `create_session_with`, `create_child_session`, `create_session_of_kind` | Each resolves (or creates) the project record; child sessions copy the parent's `project_id` |
| Linked worktrees | `project_root()` treats a linked worktree as its own project | A session worktree is attached to the owning project explicitly, so it groups under it |
| Per-session model | `SessionView::selected_model` (FR-11) | Seeded from the project's base model when a session is created |
| Sidebar rows | `+ New session`, `Pinned`, project sections; row = title, relative age, attention marker (`*` `!` `o` `x`) | Add chevron, branch marker + count, right-aligned state marker; see "Sidebar" |
| Sidebar labels | `project_labels()` derives the heading from the path basename | Heading is `Project::name` (user-editable) |
| Persisted sidebar state | `tui.sessions.sidebar` stores collapsed sections keyed by path | Re-keyed by project id in the migration |
| Remote parsing | `ragent_tools_vcs::vcs_provider::{VcsProvider, parse_reverse_repo}`; `git::run_git_output` | Reused for remote detection; no new parser |
| Schema | `SCHEMA_VERSION = 5` (`pinned`, `kind`) | Version 6 |

## Data Model

### `Project` (new: `ragent-storage/src/project.rs` row type, `ragent-agent/src/project/mod.rs` domain type)

| Field | Type | Notes |
|-------|------|-------|
| `id` | `String` | UUID v4 |
| `name` | `String` | Defaults to the worktree basename, with the existing shortest-parent disambiguation on collision; user-editable |
| `worktree` | `PathBuf` | Canonical local checkout root (what `project_root()` returns today); unique |
| `base_model` | `Option<ModelRef>` | `provider_id` + `model_id`; `None` falls back to the global `selected_model` setting |
| `remote` | `Option<ProjectRemote>` | `None` for a local-only directory |
| `created_at` / `updated_at` / `archived_at` | `DateTime<Utc>` | Same conventions as `Session` |

### `ProjectRemote`

| Field | Type | Notes |
|-------|------|-------|
| `kind` | `RemoteKind` | Enum `GitHub`, `GitLab`; matched exhaustively, a new host adds a variant |
| `host` | `String` | `github.com`, `gitlab.com`, or a self-hosted GitLab host |
| `path` | `String` | `owner/repo` or `group/subgroup/project` |
| `url` | `String` | Remote URL with any embedded credentials stripped before storing |
| `remote_name` | `String` | Git remote name, default `origin` |
| `default_branch` | `Option<String>` | From `git symbolic-ref refs/remotes/<remote>/HEAD` |

A remote on an unrecognised host is stored as `remote = None`; the project still works as
a local project.

### `Session` additions

| Field | Type | Notes |
|-------|------|-------|
| `project_id` | `String` | Existing column; now always a `projects.id` |
| `branch` | `Option<String>` | Branch of the session-owned worktree |
| `worktree` | `Option<PathBuf>` | Session-owned git worktree; `None` means the session runs in the project worktree |

`sessions.directory` stays the effective working directory (the session worktree when one
exists). Like `pinned` and `kind`, the two new columns stay out of the legacy two-schema
`SessionRow` mapping and get their own accessors.

### Migration (schema 5 -> 6, one transaction, no git commands)

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
```

Backfill: one project per distinct `sessions.project_id` path (archived sessions
included), then rewrite `sessions.project_id` from the path to the new id, and re-key the
collapsed-section list in the `tui.sessions.sidebar` setting. The existing
`idx_sessions_project_updated` index keeps working unchanged. Remote detection runs
lazily the first time a backfilled project is focused, not in the migration.

## Behaviour

1. **Project resolution.** `ProjectManager::resolve_for_dir(dir)` returns the project
   whose `worktree` equals `project_root(dir)`, creating it (and detecting its remote) on
   first use. All four session-creation functions call it; child sessions take the
   parent's project without resolving.
2. **Remote detection.** `git remote get-url origin` through `run_git_output`, parsed by
   `parse_reverse_repo`, mapped to `ProjectRemote`. `/project refresh` re-runs it.
3. **Session worktree.** `/session new --branch <branch>` (and the sidebar's new-session
   action) runs `git fetch <remote> <branch>` then
   `git worktree add <root>/<project>/<branch-slug> <branch>`, creating the branch from
   the remote default branch when it does not exist. The session stores `branch`,
   `worktree`, the owning `project_id`, and `directory` = the worktree. The SEC-062
   containment rule treats the worktree root as a permitted root.
4. **Worktree cleanup.** Archiving leaves the worktree on disk. `/project prune` removes
   worktrees of archived sessions that are clean (`git worktree remove`, never `--force`)
   and reports the ones it skipped.
5. **Base model.** A new session's `SessionView::selected_model` is seeded from the
   project's `base_model`, else the global setting. `/model` keeps changing only the
   focused session. `/project model <provider>/<model>` writes the project base.
6. **Per-project services.** The M8 code index, MCP client, specs and agents are keyed by
   project root. A session worktree is a different directory of the same project: key
   those services by `project_id` so the worktree session shares them. Verify this in
   `project_services.rs` before T-006 lands.

## Sidebar: Screenshot Capabilities Mapped to Work

| Screenshot element | Existing | Work |
|--------------------|----------|------|
| `+ New` at the top | `+ New session` row, `Ctrl+N` | Offer project and optional branch when the cursor is on a heading |
| `Search` at the top | `/` filter while focused | Add a visible `Search` row that starts the same filter |
| Project heading with chevron | Collapsible heading, no chevron | Render `v` / `>` after the name; show `[github]` / `[gitlab]` when a remote is known |
| Projects with no sessions | Not listed (grouping comes from sessions) | List every non-archived project |
| Branch icon on each row | None | Branch marker; colour distinguishes a session-owned worktree from the shared project worktree |
| Number next to the branch icon | None | Count from `SessionSummary::files_changed` (see Open Question 3) |
| Right-aligned dot (unread, needs attention) | Marker at the row start | Move to the right edge; keep `*` `!` `o` `x` meanings and colours |
| Cloud icon (remote session) | None | Out of scope: ragent has no cloud sessions |
| Highlighted focused row | Exists | No change |
| Relative age | Exists, not in the screenshot | Keep when the row is wide enough; drop first when space is short |

All glyphs stay ASCII, per the project style rule.

## Other Surfaces

- **`/project` command:** `list`, `show`, `add <path|url>`, `rename <name>`,
  `model <provider>/<model>`, `refresh`, `prune`, `remove`, `help`. `add <url>` clones
  through the existing clone path and registers the result. Entry added to the command
  catalogue.
- **CLI (`src/main.rs`):** `ragent session list` groups by project (`--project <name>`,
  `--json`); `ragent project list | show | add | remove`.
- **Server (`ragent-server`):** `GET/POST /projects`, `GET/DELETE /projects/{id}`,
  `GET /projects/{id}/sessions`; `POST /sessions` accepts `project_id` and optional
  `branch`; `SessionResponse` gains `project_id`, `branch`, `worktree`; OpenAPI updated.
- **Session archive (`session/archive.rs`):** export records the project name and remote;
  import resolves or creates the project for the import directory.

## Tasks

| ID | Title | Crate | Effort | Depends |
|----|-------|-------|--------|---------|
| T-001 | `projects` table, session columns, backfill, setting re-key, `SCHEMA_VERSION` 6 | ragent-storage | M | - |
| T-002 | `ProjectRow` CRUD; `ProjectSessions` carries the project; empty projects listed; `branch`/`worktree` accessors | ragent-storage | M | T-001 |
| T-003 | `Project`, `ProjectRemote`, `RemoteKind`, `ProjectManager::resolve_for_dir`, project events | ragent-agent | M | T-002 |
| T-004 | Remote detection (GitHub, GitLab incl. self-hosted), credential stripping, lazy refresh | ragent-agent, ragent-tools-vcs | S | T-003 |
| T-005 | All four session-creation paths attach a project record; child sessions inherit | ragent-agent | S | T-003 |
| T-006 | Session worktree create/prune; per-project services keyed by `project_id` | ragent-agent, ragent-tui | L | T-005 |
| T-007 | Base model seeding and `/project model` | ragent-tui | S | T-005 |
| T-008 | `/project` command and `/session new --branch` | ragent-tui | M | T-006 |
| T-009 | Sidebar data: headings from `Project::name`, empty projects, branch + count per row | ragent-tui | M | T-005 |
| T-010 | Sidebar rendering: chevron, remote tag, branch marker, right-aligned state marker, `Search` row | ragent-tui | M | T-009 |
| T-011 | CLI grouped `session list` and `project` subcommands | ragent (root) | S | T-005 |
| T-012 | Server routes, `SessionResponse` fields, OpenAPI | ragent-server | M | T-005 |
| T-013 | Session archive carries the project | ragent-agent | S | T-005 |
| T-014 | SPEC.md, QUICKSTART.md, TUI-QUICKSTART.md, CHANGELOG.md, `docs/plans/MULTISESSION.md` Q2 note | docs | S | all |

Effort key: S = under half a day, M = about one day, L = two to three days. Total: about
9 to 11 working days. T-001 to T-005 (about 3.5 days) deliver the stored project with
attached sessions; T-009/T-010 (about 2 days) deliver the screenshot-level sidebar;
T-006 (2 to 3 days) delivers session worktrees.

## Verification

Each task lands with tests in the crate's `tests/` directory, registered in `tests/all.rs`.

| Task | Test file | Checks |
|------|-----------|--------|
| T-001 | `ragent-storage/tests/test_projects_migration.rs` | A v5 database under `target/temp/` upgrades; one project per distinct root; every session's `project_id` resolves to a project; collapsed sections survive the re-key; second open is a no-op |
| T-002 | `ragent-storage/tests/test_projects_crud.rs` | CRUD round trip; unique `worktree`; grouped list includes an empty project and still uses `idx_sessions_project_updated` |
| T-003/4 | `ragent-agent/tests/test_project_manager.rs`, `test_project_remote.rs` | `resolve_for_dir` is stable from a subdirectory; https and ssh GitHub URLs, gitlab.com, self-hosted GitLab with nested groups; unknown host yields `None`; credentials stripped |
| T-005 | `ragent-agent/tests/test_session_project_attach.rs` | Interactive, sub-agent, teammate, skill and automation sessions all carry a valid project; children match the parent |
| T-006 | `ragent-agent/tests/test_session_worktree.rs` | Worktree created at the requested branch; the session groups under the owning project, not a new one; prune skips a dirty worktree |
| T-007 | `ragent-tui/tests/test_project_base_model.rs` | New session takes the project base model; `/model` in one session leaves the project base and other sessions unchanged |
| T-009/10 | extend `ragent-tui/tests/test_sessions_sidebar.rs` | Painted rows show the chevron, remote tag, branch + count and right-aligned marker; an empty project is listed; a renamed project shows its new name; narrow widths drop the age before the title |
| T-012 | `ragent-server/tests/test_projects_routes.rs` | Route round trips; `POST /sessions` with `project_id`; containment check still rejects outside paths |

Git-using tests build their repositories under `target/temp/<unique-name>` and pass the
worktree root explicitly; no test writes into the repository.

## Open Questions

1. **Session worktree default.** Plan assumes opt-in: a session uses the project worktree
   unless a branch is requested. Alternative: every new session gets its own worktree and
   branch, which matches the screenshot's one-branch-per-row look but costs a checkout
   and disk per session.
2. **Worktree location.** Plan assumes the ragent data directory
   (`worktrees/<project>/<branch>`). Alternative: a sibling directory of the project
   worktree.
3. **Meaning of the number beside the branch icon.** Plan uses files changed, which is
   already tracked. Alternatives: commits ahead of the default branch, or an open
   pull/merge request number (needs a GitHub/GitLab API call per row).
4. **Icon colour in the screenshot.** One row has a green icon. Plan assumes colour means
   "session owns a worktree". If it should mean "pull/merge request open or merged", that
   adds a remote status poll and belongs in a follow-up.
