---
status: draft
---

# Manual Test Plan: `/new` — New Project Scaffolding Command

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session against a real GitHub/GitLab account where hosting tests are involved.

## Prerequisites

- ragent binary built from the feature branch (`cargo build`) and available on `PATH`.
- A scratch parent directory for disposable projects, e.g. `~/scratch/newproj-tests/`
  containing **only** empty subdirectories created for each test case.
- For GitHub tests: valid GitHub credentials configured in ragent (previously used
  `github` tools successfully in a session).
- For GitLab tests: valid GitLab credentials configured in ragent.
- A known non-empty directory for guard tests: create `~/scratch/newproj-tests/busy/`
  containing one stray file (`keepme.txt`).
- Language toolchains installed as required by each case: `rust` (cargo), `python` 3,
  `go`, and a TypeScript runtime (`node` + `npm`).

## Test Cases

### TC-001 — Help output with no arguments

**Requirement:** FR-012

**Preconditions:** ragent running in the TUI inside an empty directory
(`~/scratch/newproj-tests/tc001/`).

**Steps:**
1. Type `/new` into the message input and press `Enter`.
2. Read the displayed usage text.

**Test data:** `/new` (no arguments).

**Expected results:**
- Usage text is displayed in the message window showing the flag table
  (`--language`, `--type`, `--stack`, `--github`, `--gitlab`) and the supported
  values (`rust`, `python`, `go`, `typescript`; `library`, `cmdline`, `tui`, `gui`,
  `webapp`).
- No files or directories are created in the working directory.
- No git repository is initialised.

### TC-002 — Minimal Rust cmdline scaffold, no remote

**Requirement:** FR-001, FR-004, FR-005, FR-006, FR-008, FR-011, FR-019

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc002/`.

**Steps:**
1. Type `/new --language rust --type cmdline` and press `Enter`.
2. Watch the streamed progress lines in the message window.
3. After the summary report appears, quit ragent (`/quit`) and inspect the directory
   in a normal shell.

**Test data:** `/new --language rust --type cmdline`

**Expected results:**
- Progress lines stream step-by-step in the message window (no background-agent
  hand-off, no permission prompt beyond the usual tool approvals).
- Summary report lists: layout, generated files, language/type, and
  `remote: none`.
- Directory contains: `Cargo.toml`, `src/main.rs` printing `Hello, world!`,
  `.gitignore`, `AGENTS.md`, `specs/`, `log/`, `.ragent/`, plus the FR-019 docs set
  (`README.md`, `QUICKSTART.md`, `STATS.md`, `docs/`).
- `cargo run` in the directory builds and prints `Hello, world!`.
- `git status` reports "not a git repository" (no remote configured, no push).

### TC-003 — Non-empty directory guard

**Requirement:** FR-002

**Preconditions:** ragent running in the TUI inside `~/scratch/newproj-tests/busy/`
(containing `keepme.txt`). Note the exact byte contents of `keepme.txt`.

**Steps:**
1. Type `/new --language rust --type cmdline` and press `Enter`.
2. Read the refusal message.
3. In a shell, verify `keepme.txt` still exists and is unchanged.

**Test data:** `/new --language rust --type cmdline` in a directory containing
`keepme.txt`.

**Expected results:**
- Command aborts with a message naming the offending entries (`keepme.txt`).
- `keepme.txt` still exists with its original contents.
- No new files or directories were created; no git repo initialised.

### TC-004 — Missing required flags

**Requirement:** FR-003

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc004/`.

**Steps:**
1. Run `/new --language rust` (missing `--type`) and press `Enter`.
2. Run `/new --type tui` (missing `--language`) and press `Enter`.
3. Run `/new --language rust --type not-a-type` (invalid `--type` value) and press
   `Enter`.
4. After each, verify the directory in a shell.

**Test data:** the three invocations above.

**Expected results:**
- Each invocation displays a usage message listing valid values for the missing or
  invalid flag(s) (`library`, `cmdline`, `tui`, `gui`, `webapp`).
- No files are created after any of the three invocations.

### TC-005 — Library type produces no binary entrypoint

**Requirement:** FR-006

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc005/`.

**Steps:**
1. Type `/new --language rust --type library` and press `Enter`.
2. Quit ragent and inspect the directory.

**Test data:** `/new --language rust --type library`

**Expected results:**
- `src/lib.rs` exists with a public hello function; `src/main.rs` does **not** exist.
- `cargo build` succeeds; the included example test passes when run with the
  language's default test runner.

### TC-006 — Unknown stack warns and continues

**Requirement:** FR-007

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc006/`.

**Steps:**
1. Type `/new --language rust --type cmdline --stack made_up_framework` and press
   `Enter`.
2. Quit ragent and inspect the directory.

**Test data:** `/new --language rust --type cmdline --stack made_up_framework`

**Expected results:**
- A visible warning names the unknown stack and states the base layout is used.
- Scaffold completes otherwise as in TC-002 (buildable, ragent layout present).

### TC-019 — Stack overlay on a binary app type emits a single entry point

**Requirement:** FR-007

**Preconditions:** ragent build with the T-019 fix; run from a shell.

**Steps:**
1. Run `ragent new --language rust --type tui --stack ratatui` in an empty
   directory, then repeat for `--type cmdline --stack axum`,
   `--type cmdline --stack warp`, `--type gui --stack raylib`, and
   `--type gui --stack gtk4`.
2. Inspect the entry-point source and run `cargo build` in one scaffold.

**Test data:** `ratatui`/`axum`/`warp`/`raylib`/`gtk4` overlays on their
matching app types.

**Expected results:**
- `src/main.rs` declares exactly **one** `fn main`; the base
  `Hello, world! (tui|gui starter)` entry point is gone.
- `cargo build` succeeds with no edits; running the ratatui scaffold draws the
  `Hello, world!` terminal UI (previously the generated file declared two
  `main`s and only compiled once one was deleted manually, after which the
  binary printed nothing).

### TC-007 — GitHub remote create and push

**Requirement:** FR-008, FR-011

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc007/`; valid GitHub credentials; the repo name derived from
the directory name (`tc007`) must not already exist on the GitHub account.

**Steps:**
1. Type `/new --language python --type cmdline --github` and press `Enter`.
2. Watch the remote-creation progress lines.
3. After the summary, open a browser to the GitHub account's repository list.

**Test data:** `/new --language python --type cmdline --github`

**Expected results:**
- Summary reports the new remote URL (e.g. `https://github.com/<user>/tc007`).
- `main.py` printing `Hello, world!` exists locally; `python3 main.py` runs.
- The GitHub account contains a new `tc007` repository whose initial commit contains
  the scaffold files; `git log` shows one commit.

### TC-008 — GitLab remote create and push

**Requirement:** FR-008, FR-011

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc008/`; valid GitLab credentials; `tc008` free on the account.

**Steps:**
1. Type `/new --language go --type cmdline --gitlab` and press `Enter`.
2. After the summary, open a browser to the GitLab account's projects list.

**Test data:** `/new --language go --type cmdline --gitlab`

**Expected results:**
- Summary reports the GitLab remote URL.
- `go.mod` and `main.go` printing `Hello, world!` exist; `go run .` runs.
- The GitLab account contains a new `tc008` project with the scaffold files pushed.

### TC-009 — Conflicting hosting flags

**Requirement:** FR-009

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc009/`.

**Steps:**
1. Type `/new --language rust --type cmdline --github --gitlab` and press `Enter`.
2. Verify the directory in a shell.

**Test data:** `/new --language rust --type cmdline --github --gitlab`

**Expected results:**
- Command aborts with a mutual-exclusion conflict message.
- No files created, no git repo, no remote on either host.

### TC-010 — Remote failure containment

**Requirement:** FR-010

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc010/`; GitHub credentials configured but **invalid**
(revoked token) so remote creation fails.

**Steps:**
1. Type `/new --language rust --type cmdline --github` and press `Enter`.
2. Read the failure report.
3. Verify local state in a shell.

**Test data:** `/new --language rust --type cmdline --github` with broken credentials.

**Expected results:**
- Report names the failed step (remote creation/auth) with an actionable message.
- Local scaffold is intact and buildable; `git log` shows the initial commit;
  `git remote -v` is empty (no half-configured remote).
- No partial repository was created on the hosting side.

### TC-011 — TypeScript TUI scaffold with known stack

**Requirement:** FR-005, FR-006, FR-007, FR-011

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc011/`; `node` + `npm` installed.

**Steps:**
1. Type `/new --language typescript --type tui --stack ink` and press `Enter`.
2. Quit ragent and inspect the directory.

**Test data:** `/new --language typescript --type tui --stack ink`

**Expected results:**
- Scaffold contains `package.json` with `ink` in dependencies, a `src/` entrypoint
  rendering a hello TUI, and the ragent layout (`.gitignore`, `AGENTS.md`, `specs/`,
  `log/`, `.ragent/`).
- Summary lists the stack as `ink`.
- `npm install && npm run build` (or documented equivalent) succeeds.

### TC-012 — Re-run over existing files does not overwrite

**Requirement:** FR-016

**Preconditions:** TC-002's directory `~/scratch/newproj-tests/tc002/` still exists
with its scaffold. Edit `AGENTS.md` to contain a unique marker line
`MARKER-TC-012` and save.

**Steps:**
1. Relaunch ragent in `~/scratch/newproj-tests/tc002/`.
2. Type `/new --language rust --type cmdline` and press `Enter`.
3. Inspect `AGENTS.md` in a shell.

**Test data:** repeated `/new` in a previously scaffolded directory; edited
`AGENTS.md` containing `MARKER-TC-012`.

**Expected results:**
- Summary reports pre-existing files were left untouched (lists `AGENTS.md`,
  `.gitignore`, etc.).
- `AGENTS.md` still contains `MARKER-TC-012`.
- No scaffold file has regressed to template contents.

### TC-013 — CLI parity in non-TUI mode

**Requirement:** FR-013

**Preconditions:** an empty directory `~/scratch/newproj-tests/tc013/`; shell outside
the TUI.

**Steps:**
1. Run `ragent run "/new --language go --type library" --no-tui` (or the documented
   CLI equivalent) from the directory's parent after `cd` into it, following the
   documented usage.
2. Inspect the directory.

**Test data:** `ragent run "/new --language go --type library" --no-tui`

**Expected results:**
- Same scaffold as the TUI path: `go.mod` with library module (no `main.go` binary
  entry), ragent layout, summary output on stdout, `remote: none`.

### TC-014 — Idempotent remote retry after partial failure

**Requirement:** FR-010, FR-015

**Preconditions:** After TC-010, keep the directory `~/scratch/newproj-tests/tc010/`
(scaffold present, no remote). Restore valid GitHub credentials in ragent; ensure
`tcb010` naming does not conflict — the repo name is derived from the directory name,
so rename the directory to `~/scratch/newproj-tests/tc014/` first and `cd` into it.

**Steps:**
1. Relaunch ragent in `tc014/`.
2. Type `/new --language rust --type cmdline --github` and press `Enter`.
3. Inspect git state and the GitHub account.

**Test data:** second `/new --github` run in a directory with an existing local repo
and no remote.

**Expected results:**
- No "already initialised" error from local git; the remote is added and the initial
  commit pushed.
- Summary reports the remote URL; the GitHub repo contains the scaffold files.

### TC-015 — `/new help` detailed help page

**Requirement:** FR-018, NFR-001

**Preconditions:** ragent running in the TUI inside an empty directory
(`~/scratch/newproj-tests/tc015/`).

**Steps:**
1. Type `/new help` and press `Enter`.
2. Read the displayed help page.
3. (Optional cross-check) In throwaway empty directories, scaffold once with each
   listed `--language` and `--type` value to confirm the help page's value lists
   match what the command actually accepts.

**Test data:** `/new help`

**Expected results:**
- Help page states the command's purpose (scaffold a new project in an empty
  directory).
- Every argument (`--language`, `--type`, `--stack`, `--github`, `--gitlab`) is
  documented with: a description, required/optional status, accepted values, and the
  default behaviour when omitted.
- `--github` and `--gitlab` are marked mutually exclusive.
- At least two worked example invocations are shown: one minimal (no hosting) and
  one with a hosting flag.
- The listed `--language` values match the FR-017 registry (`rust`, `python`, `go`,
  `typescript`) and the `--type` values match FR-006 (`library`, `cmdline`, `tui`,
  `gui`).
- No files are created and no scaffold is run by the help invocation itself.

### TC-016 — Documentation files and docs folder generated

**Requirement:** FR-019, NFR-002

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc016/`.

**Steps:**
1. Type `/new --language rust --type cmdline` and press `Enter`.
2. After the summary, quit ragent and inspect the directory in a shell.
3. Open `README.md`, `QUICKSTART.md`, and `STATS.md`; read the build/run instructions.
4. Run the run-command given in `QUICKSTART.md` and compare its output with the
   documented first output.
5. Repeat steps 1–4 in a second scratch directory with
   `/new --language python --type cmdline` to confirm the instructions change with
   the language.

**Test data:** `/new --language rust --type cmdline` and
`/new --language python --type cmdline` in separate empty directories.

**Expected results:**
- `README.md` exists with the project title (directory name), a description, and
  Rust build/run instructions (`cargo run`).
- `QUICKSTART.md` exists with prerequisites, the exact build/run command, and the
  expected first output (`Hello, world!`).
- `STATS.md` exists and records: a UTC timestamp, language `rust`, app type
  `cmdline`, stack `none`, a count of generated files, and the ragent version.
- `docs/` folder exists (may contain an empty index or placeholder).
- For the Python run: `README.md`/`QUICKSTART.md` show `python3 main.py` instead of
  `cargo run`, and `STATS.md` records language `python`.
- The run command in `QUICKSTART.md` actually prints `Hello, world!` (instructions
  match the generated project — NFR-002).
- All four artifacts appear in the FR-011 summary and in the initial git commit when
  a hosting flag is used.

### TC-017 — GitHub app-token downgrade to the `gh` CLI

**Requirement:** FR-008

**Preconditions:** A stored token at `~/.config/ragent/github_token` whose value is a
GitHub App token (`ghu_`), and an authenticated `gh` CLI (`gh auth status` shows a
`repo`-scoped token).

**Steps:**
1. Run `ragent new --language rust --type cmdline --github` in an empty directory.
2. Inspect the `Remote:` line and `git remote -v`.
3. Repeat with `RAGENT_GITHUB_NO_GH_CLI=1` in a fresh empty directory.

**Test data:** stored `ghu_` token + `gh auth token` credential.

**Expected results:**
- Step 1 reports `Remote: https://github.com/<user>/<dir>`; `origin` is set and the
  initial commit is pushed. The app token is bypassed in favour of the `gh`
  credential (verified: `POST /user/repos` returns 422 "name blank" with the `gh`
  token versus 403 "Resource not accessible by integration" with the `ghu_` token).
- Step 3 fails contained at `Remote: failed at github repo create: GitHub API error
  403: Resource not accessible by integration` with the local scaffold intact — and
  the process must not panic (previously the CLI path panicked with "Cannot drop a
  runtime in a context where blocking is not allowed" while dropping the
  `reqwest::blocking` runtime inside the main async runtime).

### TC-018 — CLI `--github` runs the remote half off the async runtime

**Requirement:** FR-008, FR-010

**Preconditions:** ragent build with the T-018 fix; run from a shell (not the TUI).

**Steps:**
1. Run `ragent new --language rust --type cmdline --github` and then
   `ragent run "/new --language go --type cmdline --github"` in empty directories.
2. Confirm neither invocation panics and each prints a `Remote:` outcome.

**Expected results:**
- No panic report is written to `log/panics/`.
- The remote half runs inside a blocking worker, so the `reqwest::blocking` client
  can create and destroy its private runtime without an ambient async context.

### TC-020 — Webapp starter for a web language; manifest-only for the rest

**Requirement:** FR-006

**Preconditions:** ragent running in the TUI inside empty directory
`~/scratch/newproj-tests/tc020/`.

**Steps:**
1. Type `/new --language rust --type webapp` and press `Enter`.
2. Quit ragent and inspect the directory.
3. Repeat with `/new --language go --type webapp` and
   `/new --language java --type webapp` in their own empty directories.

**Test data:** `/new --language rust --type webapp`; `--language go`; a language
with no web idiom such as `java`.

**Expected results:**
- rust: `src/main.rs` exists, contains a `Hello, world!` greeting, and
  `cargo build` succeeds.
- go: `main.go` exists, contains a `Hello, world!` greeting, and `go build .`
  succeeds.
- java: the manifest (and no app source) is created — the layout degrades to
  manifest-only, matching how data/DSL formats degrade for `tui`/`gui`.

## Cleanup

- Delete all scratch project directories: `rm -rf ~/scratch/newproj-tests/`.
- Delete the repositories created by TC-007 (`tc007`), TC-008 (`tc008`), and
  TC-014 (`tc014`) from the GitHub/GitLab accounts.
- Quit any ragent sessions left open (`/quit`).