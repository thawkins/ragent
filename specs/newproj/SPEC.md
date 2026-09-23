---
status: approved
audit:
  - { time: 1788939041, from: "none", to: "draft", actor: "system" }
  - { time: 1788939060, from: "draft", to: "approved", actor: "user" }
---
# Specification: `/new` — New Project Scaffolding Command

## Overview

This specification defines a `/new` slash command for ragent that scaffolds a brand-new
software project inside an **empty** current directory. The command creates the directory
layout that ragent expects (`.ragent/`, `specs/`, `log/`, `.gitignore`), generates a
minimal, runnable "Hello, World" application for the user-selected language and
application type, and optionally initialises a git repository with a remote on GitHub or
GitLab and pushes the initial commit.

The command takes the following flags:

| Flag | Purpose | Values |
| ---- | ------- | ------ |
| `--language` | Computer language to scaffold | e.g. `rust`, `python`, `go`, `ts` |
| `--type` | Type of application | `library`, `cmdline`, `tui`, `gui` |
| `--stack` | Optional framework(s) to include | e.g. `axum`, `warp`, `raylib`, `gtk4`, `ratatui`, … |
| `--github` / `--gitlab` | Initialise remote hosting and push | flag, mutually exclusive |

If neither `--github` nor `--gitlab` is supplied, no remote is configured and no push
occurs.

## Background

ragent already contains most of the machinery needed for this feature:

- **Slash-command registry** — commands are declared as static trigger entries in
  `crates/ragent-tui/src/app/state.rs` and dispatched in
  `crates/ragent-tui/src/app/slash.rs`. `/reverse` demonstrates flag-parsing precedent
  (`--tech`, `--create`).
- **VCS tooling** — `crates/ragent-tools-vcs` exposes GitHub and GitLab clients
  (auth, client, issues, PRs/MRs, pipelines) plus a full local `git` tool family
  (init is a trivial addition to the existing family).
- **Scaffolding precedent** — the research system writes self-contained artifact
  directories; the `/bench init` command already creates benchmark folders.
- **Project-guideline loader** — ragent auto-loads `AGENTS.md` from the project root,
  so the scaffolder should emit one so a freshly created project is immediately
  agent-friendly when opened with ragent.

The new work is a scaffolding engine plus command wiring; it reuses existing VCS
tooling for hosting setup.

## Requirements

### FR-001 — Command availability (ubiquitous)

The system shall provide a `/new` slash command available in the TUI (and CLI parity
where the command surface allows) that scaffolds a new project in the current working
directory.

### FR-002 — Empty-directory guard (event-driven)

**When** the user invokes `/new` in a directory that is not empty (ignoring ragent's own
artifacts: `.ragent/`, `log/`, `target/`), the system shall refuse to scaffold, report
the offending entries, and exit without modifying the filesystem.

### FR-003 — Required flag validation (event-driven)

**When** `/new` is invoked without a valid `--language` or `--type` value, the system
shall display a usage message listing valid values for both flags and abort without
creating any files.

### FR-004 — ragent workspace initialisation (ubiquitous)

The scaffolder shall create the ragent project layout in the target directory:

- `.ragent/` (agents, config placeholders)
- `specs/` (spec directory)
- `log/` (runtime logs, gitignored)
- `.gitignore` (pre-populated for the chosen language + ragent artifacts)
- `AGENTS.md` (minimal project-guidelines stub)

### FR-005 — Hello-world generation (ubiquitous)

The scaffolder shall generate a minimal, runnable "Hello, World" artifact set for the
selected `--language` and `--type` combination (e.g. a `main.rs` for a Rust cmdline app,
`main.py` for Python, `lib.rs` for a Rust library), buildable with the language's
default toolchain without any additional steps beyond installing the toolchain.

### FR-006 — App-type mapping (state-driven)

**While** the selected `--type` is `library`, the scaffolder shall generate a library
layout (no binary entrypoint; exported module with a public function);
**while** `--type` is `cmdline`, the scaffolder shall generate a console-entry
layout; **while** `--type` is `tui`, the scaffolder shall generate a terminal-UI
starter; **while** `--type` is `gui`, the scaffolder shall generate a
GUI starter appropriate to the language.

### FR-007 — Stack layering (optional feature)

**If/when** a `--stack` value is supplied, the scaffolder shall add the framework
dependency/import and a minimal framework-specific starter snippet (e.g. a route
handler for `axum`) on top of the FR-005 base, only for stacks the scaffolder knows;
**if/when** the stack is unknown, the scaffolder shall warn and continue with the
base layout.

The starter snippet is a complete binary entry point (it declares `main`), so on a
binary app type (`cmdline`, `tui`, `gui`) it shall **replace** the FR-005 base entry
point rather than be appended after it: the generated source declares exactly one
`main`, builds untouched with the language's default toolchain, and runs the
framework starter as generated. Any non-entry-point base content (a library body) is
retained alongside the starter. Appending the starter after the base entry point is
explicitly rejected because it yields two `main` functions in one file - a source
that only compiles once the base function is deleted, and whose surviving binary is
the base `Hello, world!` print rather than the framework starter.

### FR-008 — Remote-initialisation flags (optional feature)

**If/when** `--github` or `--gitlab` is supplied, the scaffolder shall initialise a git
repository, create the hosting repository, set it as `origin`, and push the initial
commit; **if/when** neither flag is supplied, no remote is configured, no hosting
repository is created, and no push occurs.

For GitHub the credential is resolved through the shared chain: the
`GITHUB_TOKEN` environment variable, then the stored `/github login` token
file, then the authenticated `gh` CLI. Because `/github login` reuses the
Copilot provider's OAuth application, the token it stores is a GitHub App token
(`ghu_`) that cannot create repositories (`POST /user/repos` answers
`403 Resource not accessible by integration`); such a stored token defers to
the `gh` CLI credential when one is available, and is otherwise used unchanged
so read-only GitHub tooling keeps working. `RAGENT_GITHUB_NO_GH_CLI=1`
disables the `gh` fallback.

The remote half must run off the async runtime's own thread: the GitHub and
GitLab API calls go through a `reqwest::blocking` client, which spawns a
private runtime and panics if that runtime is dropped from within an async
context. In the CLI parity path (`ragent new --github`) the call is therefore
executed inside a blocking worker; the TUI already runs it on a worker
`std::thread` with no ambient runtime.

### FR-009 — Mutual exclusion of hosting flags (unwanted behaviour)

The system shall **not** accept both `--github` and `--gitlab` in one invocation; it
shall report the conflict and abort without scaffolding or any remote setup.

### FR-010 — Remote-init failure containment (unwanted behaviour)

The system shall **not** leave a partially-initialised remote state when remote
initialisation fails (auth failure, network error, existing repo): the scaffolder shall
keep the local scaffold and report the failed step with an actionable message, leaving
the user to retry `git remote add` + push manually or re-run `/new --github` style
flow later.

### FR-011 — Summary report (event-driven)

**When** scaffolding completes (fully or partially), the system shall print a summary
listing: created layout, generated files, chosen language/type/stack, and remote
status (none / remote URL / failed).

### FR-012 — Help surface (optional feature)

**If/when** the user types `/new help` (or `/new` with no arguments), the system shall
display usage, flag table, and supported language/type/stack combinations.

### FR-013 — CLI parity (optional feature)

**If/when** ragent is run in non-TUI mode (`--no-tui`), the same `/new` behaviour shall
be reachable (e.g. `ragent run "/new ..."` or a dedicated CLI surface) with identical
flags and identical behaviour.

### FR-014 — Foreground execution (state-driven)

**While** the command runs in the TUI, the scaffolder shall run as a foreground
operation with streamed progress lines in the message window rather than a background
sub-agent, so the user sees each step as it happens.

### FR-015 — Idempotent remote retry (state-driven)

**While** a previous scaffold left a git repo without a remote (the FR-010 partial
state), a subsequent invocation of the remote-init flow shall add the remote and push
without failing on "already initialised" local git state.

### FR-016 — No silent overwrite (unwanted behaviour)

The system shall **not** overwrite an existing `.gitignore`, `AGENTS.md`, or other
scaffold files if they already exist in the target directory; existing files are
reported in the summary and left untouched.

### FR-017 — Supported language registry (ubiquitous)

The scaffolder shall maintain a registry of supported languages with per-language
scaffold recipes, covering every id in the codeindex scanner's
`SUPPORTED_LANGUAGES` list. Application languages (46 canonical values including
`shell`, `zsh`, `fish`) scaffold runnable four-app-type hello-world projects;
data/markup/build formats (`toml`, `yaml`, `json`, `xml`, `html`, `css`, `scss`,
`sql`, `markdown`, `protobuf`, `verilog`, `vhdl`, `terraform`, `openscad`,
`cmake`, `gradle`, `gradle_kts`, `maven`, `nix`, `hcl`) scaffold manifest plus
cmdline/library sample documents, with `tui`/`gui` degrading to manifest-only
layouts. Scanner dialect ids (`tsx`, `jsx`, `c_header`, `cpp_header`) map to
their parent language as accepted aliases.

### FR-018 — `/new help` detailed help function (event-driven)

**When** the user types `/new help`, the system shall display a dedicated help page
that explains the purpose of the `/new` command and documents every argument the
command accepts. For each argument (`--language`, `--type`, `--stack`, `--github`,
`--gitlab`) the help page shall show: the argument name, a one-line description of
what it does, whether it is required or optional, its accepted values (for
`--language` and `--type` the full value list; for `--stack` representative known
examples), and the default behaviour when the argument is omitted (e.g. no
`--github`/`--gitlab` means no remote is created and nothing is pushed). The help
page shall include at least two worked example invocations: one minimal (no hosting)
and one with a hosting flag.

### NFR-001 — Help content consistency (state-driven)

**While** the `/new help` output is rendered, the system shall derive the accepted
`--language`, `--type`, and `--stack` values from the same internal registries the
scaffolder uses (the FR-017 language registry and the FR-006/FR-007 type and stack
tables), so that the help text cannot drift out of sync with what the command
actually supports.

### FR-019 — Project documentation scaffold (ubiquitous)

The scaffolder shall generate, in addition to the FR-004 layout and FR-005
application artifacts, the following documentation files in the target directory:

- `README.md` — project title (directory name), one-paragraph description, and
  language/app-type-specific build-and-run instructions (e.g. `cargo run`,
  `python3 main.py`, `go run .`, `npm run build`).
- `QUICKSTART.md` — a minimal get-running guide: prerequisites (toolchain
  install pointer), the exact command to build and run the hello-world artifact,
  and the expected first output (`Hello, world!`).
- `STATS.md` — an initial project-statistics document containing: generated
  timestamp (UTC), selected language, app type, stack (or none), count of
  generated files, and the ragent version that produced the scaffold.

The scaffolder shall also create a `docs/` folder in the target directory.

### NFR-002 — Documentation template consistency (state-driven)

**While** the FR-019 documentation files are generated, the system shall render them
from the same per-language recipe data that produces the code artifacts (build
commands, entrypoint paths), so the README/QUICKSTART instructions always match
the generated project (e.g. QUICKSTART's run command for a Rust cmdline scaffold
is `cargo run`).

## Non-Requirements

- Interactive TUI wizard dialogs — flags only for the first release.
- Scaffolding languages beyond the FR-017 registry.
- Importing an existing project ("new" means greenfield).
- Non-git VCS (mercurial, svn).
- Pushing branches other than the initial commit's default branch.

## Acceptance Checks

- `/new --language rust --type cmdline` in an empty dir produces a buildable cargo
  project + ragent layout, no remote.
- `/new --language rust --type cmdline --github` in an empty dir additionally creates
  a private GitHub repo and pushes.
- `/new` in a non-empty dir aborts with a clear message and zero file mutations.
- `/new --language rust --type cmdline --github --gitlab` aborts with the conflict
  message.
- `/new help` lists flags and supported values.
- `/new help` explains the command purpose and documents every argument with its
  values, optionality, defaults, and worked examples (FR-018).
- Scaffold generates `README.md`, `QUICKSTART.md`, `STATS.md`, and a `docs/` folder
  whose build/run instructions match the chosen language/type/stack (FR-019,
  NFR-002).

## EARS Template Coverage

| Template | Requirements |
| -------- | ------------ |
| Ubiquitous | FR-001, FR-004, FR-005, FR-016, FR-019 |
| Event-driven | FR-002, FR-003, FR-011, FR-018 |
| State-driven | FR-006, FR-014, FR-015, NFR-001, NFR-002 |
| Optional | FR-007, FR-008, FR-012, FR-013 |
| Unwanted | FR-009, FR-010, FR-016 |