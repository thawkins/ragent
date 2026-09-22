---
status: draft
audit:
  - { time: 1787368577, from: "none", to: "draft", actor: "system" }
---
# Specification: GitHub Repository Reverse-Engineering Prompt Generator

## Overview

The `/reverse` slash command takes a public GitHub repository URL (or an
`owner/repo` shorthand) and generates a synthetic prompt that someone might
have used to create that repository with an AI coding assistant. The command
fetches the repo's metadata, root file tree, and README content via the GitHub
API, then passes all that context to the currently selected LLM model to
generate the final prompt.

Optional flags let the user constrain the technology stack (`--tech`) and
automatically feed the generated prompt into `/spec create` (`--create <name>`).

## Background

### Existing infrastructure

- **GitHub API client** — `crates/ragent-tools-vcs/src/github/client.rs` provides
  `GitHubClient` with `get(path)` (returns `serde_json::Value`) and
  `get_bytes(path)` (returns raw bytes). Token resolution is handled by
  `github::auth::load_token()` (env var `GITHUB_TOKEN` or
  `~/.ragent/github_token`). The client is re-exported via
  `ragent_agent::github`.
- **Slash command dispatch** — `crates/ragent-tui/src/app/slash.rs` dispatches
  slash commands in `execute_slash_command_inner`. Each command is registered
  in the `SLASH_COMMANDS` constant in `state.rs`. Commands like `/spec`,
  `/research`, `/plan`, and `/swarm` demonstrate the pattern: parse args, show a
  status message, spawn a `tokio::spawn` task that calls
  `processor.process_message(&sid, &task, &agent, flag)`.
- **Spec creation** — `/spec create <specname> <feature>` generates a spec
  directory. The `/reverse --create <name>` flag will chain into this by
  invoking the slash command `/spec create <name> <generated-prompt>`.
- **Model resolution** — `App::apply_selected_model_and_thinking(&mut agent)`
  applies the user's currently selected model to an `AgentInfo` before
  dispatching to the session processor.

### GitHub API endpoints used

| Endpoint | Purpose |
| -------- | ------- |
| `GET /repos/{owner}/{repo}` | Repo metadata (description, language, stars, topics) |
| `GET /repos/{owner}/{repo}/contents` | Root file tree (names, types, sizes) |
| `GET /repos/{owner}/{repo}/readme` | README content (base64-encoded, decoded via `get_bytes` on the `download_url`) |

### URL parsing

The user may provide either:
1. A full GitHub URL: `https://github.com/owner/repo`
2. An SSH-style URL: `git@github.com:owner/repo.git`
3. A shorthand: `owner/repo`

The existing `GitHubClient::detect_repo` parses git-remote URLs; a new
`parse_repo_url` function will handle all three user-facing input forms.

## Requirements

### FR-001 — Slash command registered (ubiquitous)

The system **shall** register a `/reverse` slash command in the
`SLASH_COMMANDS` table in `crates/ragent-tui/src/app/state.rs` with a trigger of
`reverse` and a description documenting its usage.

> *Ubiquitous requirement — applies to all environments where the TUI is
> available.*

### FR-002 — Repo URL or shorthand required (ubiquitous)

The `/reverse` command **shall** require a GitHub repository identifier as
input, accepting either a full URL (`https://github.com/owner/repo`), an SSH
URL (`git@github.com:owner/repo.git`), or a shorthand (`owner/repo`).

> *Ubiquitous requirement — applies every time the command is invoked.*

### FR-003 — URL parsing (ubiquitous)

The system **shall** parse the user-provided repository identifier into an
`(owner, repo)` tuple, stripping trailing `.git`, trailing slashes, and
optional query-string fragments, and rejecting identifiers that do not
resolve to exactly two path segments.

> *Ubiquitous requirement — applies to every valid invocation.*

### FR-004 — GitHub authentication required (state-driven)

When no GitHub token is configured (neither `GITHUB_TOKEN` env var nor
`~/.ragent/github_token` file), the system **shall** display a clear error
message instructing the user to run `/github login` and **shall not** make any
API calls.

> *State-driven requirement — triggered by the absence of authentication
> state.*

### FR-005 — Fetch repo metadata (event-driven)

When a valid `(owner, repo)` is parsed and authentication is available, the
system **shall** fetch the repository metadata via
`GET /repos/{owner}/{repo}` and extract at minimum: description, primary
language, topics, star count, and default branch.

> *Event-driven requirement — triggered by successful URL parsing and
> authentication.*

### FR-006 — Fetch root file tree (event-driven)

When repo metadata is successfully fetched, the system **shall** fetch the
root-level file tree via `GET /repos/{owner}/{repo}/contents` and extract the
list of file and directory names at the repository root.

> *Event-driven requirement — triggered by successful metadata fetch.*

### FR-007 — Fetch README content (event-driven)

When repo metadata is successfully fetched, the system **shall** fetch the
README content. The README endpoint returns a JSON object with a `download_url`
field; the system **shall** use `GitHubClient::get_bytes` on that URL to
retrieve the raw README text. If no README exists, the system **shall** proceed
with an empty README string rather than failing.

> *Event-driven requirement — triggered by successful metadata fetch.*

### FR-008 — Context assembly (ubiquitous)

The system **shall** assemble a context block containing the repo metadata,
the root file tree listing, and the README content (truncated to a maximum of
8000 characters) and **shall** pass this context to the currently selected LLM
model via `SessionProcessor::process_message` with a prompt instructing the
model to generate a synthetic creation prompt.

> *Ubiquitous requirement �� applies to every successful data-gathering
> phase.*

### FR-009 — Use currently selected model (ubiquitous)

The system **shall** use the user's currently selected model (as held in
`App::selected_model`) for the LLM generation step. If no model is selected, the
system **shall** fall back to `resolve_default_model` as other slash commands
do.

> *Ubiquitous requirement — applies on every invocation.*

### FR-010 — Display generated prompt (ubiquitous)

The system **shall** display the LLM's generated prompt in the TUI chat panel
as an assistant message so the user can read, copy, or reuse it.

> *Ubiquitous requirement — applies on every successful generation.*

### FR-011 — `--tech` flag (optional)

The system **may** accept an optional `--tech <stack>` flag that constrains the
generated prompt to the specified technology stack. When provided, the tech
stack **shall** be included in the context block sent to the LLM.

> *Optional requirement — the flag is not required for the command to
> function.*

### FR-012 — `--create <name>` flag (optional)

The system **may** accept an optional `--create <name>` flag. When provided,
after the LLM generates the synthetic prompt, the system **shall** automatically
invoke `/spec create <name> <generated-prompt>` by calling
`execute_slash_command` with the assembled command string.

> *Optional requirement — the flag is not required for the command to
> function.*

### FR-013 — `--help` subcommand (optional)

The system **may** accept a `help` subcommand (`/reverse help`) that displays a
usage message documenting the command, its required argument, and all optional
flags.

> *Optional requirement — the help subcommand is not required for normal
> operation.*

### FR-014 — GitHub API error handling (unwanted)

If any GitHub API call returns a non-success status (404, 403, 429, 500, etc.),
the system **shall** display a human-readable error message that includes the
HTTP status code and the repository identifier, and **shall not** crash or
proceed to the LLM step.

> *Unwanted requirement — guards against the undesirable condition of an API
> failure.*

### FR-015 — Rate limit handling (unwanted)

If the GitHub API returns a 403 or 429 rate-limit response, the system
**shall** surface the rate-limit reset time (if available in the response
headers) in the error message and **shall not** retry automatically.

> *Unwanted requirement — guards against the undesirable condition of hitting
> the rate limit.*

### FR-016 — Invalid input rejection (unwanted)

When the user provides an identifier that does not parse into a valid
`(owner, repo)` pair (e.g. empty string, single word, three-segment path), the
system **shall** display a usage message and **shall not** make any API calls.

> *Unwanted requirement — guards against the undesirable condition of
> malformed input.*

### FR-017 — Non-existent repository (unwanted)

When the GitHub API returns a 404 for the repository metadata endpoint, the
system **shall** display a message stating that the repository was not found
(or is private and the token lacks access) and **shall not** attempt subsequent
file-tree or README fetches.

> *Unwanted requirement — guards against the undesirable condition of
> targeting a missing repository.*

### FR-018 — Progress status (ubiquitous)

While the GitHub API calls and LLM generation are in progress, the system
**shall** display a status indicator prefixed with `⏳` so the TUI does not
auto-expire the status to "ready" prematurely, matching the pattern used by
`/research` and `/spec` commands.

> *Ubiquitous requirement — applies during all async operations.*

### FR-019 — Autocomplete entry (ubiquitous)

The `/reverse` command **shall** appear in the slash-command autocomplete menu
when the user types `/r` or `/rev`, using the `SLASH_COMMANDS` registration.

> *Ubiquitous requirement — applies to autocomplete rendering.*

## Non-Functional Requirements

### NFR-001 — No new dependencies

The implementation **shall not** add any new crate dependencies beyond those
already present in the workspace (reqwest, serde_json, anyhow, tokio, ragent_agent,
ragent_tools_vcs).

### NFR-002 — No panics on user-facing paths

The implementation **shall not** use `.unwrap()` or `.expect()` on any path that
processes user input or API responses. All `Result` and `Option` values
**shall** be propagated or handled with explicit error messages.

### NFR-003 — README truncation

The README content **shall** be truncated to 8000 characters before being
included in the LLM context to avoid exceeding model context windows for
repositories with very large READMEs.

### NFR-004 — Clippy and fmt clean

The implementation **shall** pass `cargo clippy` and `cargo fmt --check` without
warnings on the modified files.