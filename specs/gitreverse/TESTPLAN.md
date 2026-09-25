---
status: draft
---

# Manual Test Plan: GitHub Repository Reverse-Engineering Prompt Generator

This is a manual test plan for the `/reverse` slash command. All test cases
are executed by a human in the ragent TUI.

## Prerequisites

1. **Ragent built from source** — run `cargo build` in the project root and
   launch `./target/debug/ragent` (or use `cargo run`).
2. **GitHub authentication** — run `/github login` in the TUI and complete the
   OAuth device flow so that `/github status` shows a configured token.
   Alternatively, set `GITHUB_TOKEN` in the environment before launching.
3. **LLM provider configured** — use `/provider` to configure at least one
   provider (e.g. Anthropic, OpenAI, or a local Ollama instance) and use
   `/model` to select a model. Verify `/model` shows a non-empty selection.
4. **Working directory** — launch ragent from a directory that is a git repo
   (the ragent repo itself works) so that session creation succeeds.
5. **Test repositories** — the following public GitHub repos are used as test
   data:
   - `thawkins/ragent` — a real repo with a README, multiple languages, and a
     clear structure.
   - `octocat/Hello-World` — a minimal repo with a very short README.
   - `torvalds/linux` — a large repo with a long README (truncation test).

## Test Cases

### TC-001 — Full URL generates a prompt

**Title:** `/reverse` with a full GitHub URL produces a synthetic creation prompt

**Preconditions:**
- GitHub authentication is configured (TC prerequisites 2 and 3).
- A model is selected via `/model`.

**Instructions:**
1. Launch ragent in the project root: `cargo run`
2. Wait for the TUI to initialise and the input cursor to appear.
3. Type `/reverse https://github.com/octocat/Hello-World` and press `Enter`.
4. Observe the status bar at the bottom of the screen.
5. Wait for the command to complete (status bar returns to "ready").

**Test data to enter:**
- Input: `/reverse https://github.com/octocat/Hello-World`

**Expected results:**
- The status bar shows `⏳ reverse: octocat/Hello-World…` while the command is
  running.
- An assistant message appears in the chat panel starting with
  `From: /reverse`.
- The assistant message contains a generated prompt — a block of text that
  someone might have typed into an AI coding assistant to create the
  repository. It should reference the repo name, the README content, and the
  file structure.
- No error messages appear.
- The status bar returns to "ready" after completion.

---

### TC-002 — Shorthand `owner/repo` works

**Title:** `/reverse` accepts the `owner/repo` shorthand

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. In the TUI input field, type `/reverse octocat/Hello-World` and press
   `Enter`.
2. Wait for completion.

**Test data to enter:**
- Input: `/reverse octocat/Hello-World`

**Expected results:**
- The behaviour is identical to TC-001. The status bar, assistant message, and
  generated prompt all appear as expected.
- No error about URL parsing is shown.

---

### TC-003 — SSH-style URL is parsed

**Title:** `/reverse` accepts `git@github.com:owner/repo.git`

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. In the TUI input field, type
   `/reverse git@github.com:octocat/Hello-World.git` and press `Enter`.
2. Wait for completion.

**Test data to enter:**
- Input: `/reverse git@github.com:octocat/Hello-World.git`

**Expected results:**
- The `.git` suffix is stripped and the command proceeds as in TC-001.
- No error about URL parsing is shown.

---

### TC-004 — scaffold flags constrain the prompt

**Title:** `/spec reverse --language rust --type cmdline` injects the target scaffold into the prompt

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. In the TUI input field, type
   `/spec reverse https://github.com/thawkins/ragent --language rust --type cmdline` and press `Enter`.
2. Wait for completion.

**Test data to enter:**
- Input: `/spec reverse https://github.com/thawkins/ragent --language rust --type cmdline`

**Expected results:**
- The status bar shows `⏳ reverse: thawkins/ragent…`.
- The generated prompt explicitly mentions Rust and a command-line app, either
  in the opening sentence or in a dedicated section.
- The prompt references the repo's actual file structure and README.

---

### TC-005 — `--create <name>` chains into `/spec create`

**Title:** `/reverse --create my-test-spec` generates a spec

**Preconditions:**
- Same as TC-001.
- The `specs/` directory exists in the working directory (or will be created).

**Instructions:**
1. In the TUI input field, type
   `/reverse --create my-test-spec https://github.com/octocat/Hello-World` and
   press `Enter`.
2. Wait for the `/reverse` command to finish (status returns to "ready" or
  changes to a `spec:` status).
3. Wait for the subsequent `/spec create` command to finish.
4. Check the filesystem for the created spec directory.

**Test data to enter:**
- Input: `/reverse --create my-test-spec https://github.com/octocat/Hello-World`

**Expected results:**
- The `/reverse` command runs first and generates a prompt (as in TC-001).
- After the prompt is generated, the status bar changes to
  `spec: writing specs/my-test-spec/...`.
- A new directory `specs/my-test-spec/` appears on disk containing `SPEC.md`,
  `PLAN.md`, and `TESTPLAN.md`.
- The `SPEC.md` content is based on the generated prompt, not on the raw
  GitHub metadata.

---

### TC-006 — scaffold flags and `--create` combined

**Title:** `/spec reverse --language python --type cmdline --create py-spec` works together

**Preconditions:**
- Same as TC-001 and TC-005.

**Instructions:**
1. In the TUI input field, type
   `/spec reverse https://github.com/octocat/Hello-World --language python --type cmdline --create py-spec`
   and press `Enter`.
2. Wait for both the reverse and spec-create phases to finish.
3. Check the filesystem.

**Test data to enter:**
- Input: `/spec reverse https://github.com/octocat/Hello-World --language python --type cmdline --create py-spec`

**Expected results:**
- The generated prompt mentions Python and a command-line app.
- `specs/py-spec/` is created with `SPEC.md`, `PLAN.md`, and `TESTPLAN.md`.

---

### TC-007 — `/reverse help` shows usage

**Title:** `/reverse help` displays a usage message

**Preconditions:**
- Ragent is running.

**Instructions:**
1. In the TUI input field, type `/reverse help` and press `Enter`.

**Test data to enter:**
- Input: `/reverse help`

**Expected results:**
- An assistant message appears showing the usage of `/reverse`, including:
  - The required repo URL or `owner/repo` argument.
  - The optional `/new` scaffold flags (`--language` / `--type` / `--stack`).
  - The optional `--create <name>` flag.
- No API calls are made (no `⏳` status appears).

---

### TC-008 — Missing argument shows usage error

**Title:** `/reverse` with no argument displays a usage error

**Preconditions:**
- Ragent is running.

**Instructions:**
1. In the TUI input field, type `/reverse` and press `Enter`.

**Test data to enter:**
- Input: `/reverse`

**Expected results:**
- An assistant message appears stating that a repository URL is required.
- No API calls are made.
- No `⏳` status appears.

---

### TC-009 — Invalid identifier is rejected

**Title:** `/reverse not-a-valid-identifier` shows an error

**Preconditions:**
- Ragent is running.

**Instructions:**
1. In the TUI input field, type `/reverse justoneword` and press `Enter`.

**Test data to enter:**
- Input: `/reverse justoneword`

**Expected results:**
- An assistant message appears stating that the identifier must be in
  `owner/repo` format or a full GitHub URL.
- No API calls are made.

---

### TC-010 — Non-existent repository shows 404 error

**Title:** `/reverse owner/repo-that-does-not-exist` shows a not-found error

**Preconditions:**
- GitHub authentication is configured.

**Instructions:**
1. In the TUI input field, type
   `/reverse octocat/this-repo-does-not-exist-12345` and press `Enter`.
2. Wait for the error to appear.

**Test data to enter:**
- Input: `/reverse octocat/this-repo-does-not-exist-12345`

**Expected results:**
- An assistant message appears stating that the repository
  `octocat/this-repo-does-not-exist-12345` was not found or is private.
- The message includes the HTTP 404 status reference.
- No file-tree or README fetch is attempted (no additional error messages about
  those endpoints).

---

### TC-011 — No GitHub token shows authentication error

**Title:** `/reverse` without a configured token shows an auth error

**Preconditions:**
- GitHub authentication is **not** configured. Run `/github logout` first, and
  ensure `GITHUB_TOKEN` is not set in the environment.

**Instructions:**
1. Verify `/github status` shows no token configured.
2. In the TUI input field, type
   `/reverse https://github.com/octocat/Hello-World` and press `Enter`.

**Test data to enter:**
- Input: `/reverse https://github.com/octocat/Hello-World`

**Expected results:**
- An assistant message appears instructing the user to run `/github login`.
- No API calls are made.
- No `⏳` status appears.

---

### TC-012 — Large README is truncated

**Title:** `/reverse torvalds/linux` completes without context overflow

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. In the TUI input field, type `/reverse https://github.com/torvalds/linux`
   and press `Enter`.
2. Wait for completion (this may take longer due to the large repo).

**Test data to enter:**
- Input: `/reverse https://github.com/torvalds/linux`

**Expected results:**
- The command completes without an LLM context-overflow error.
- The generated prompt references the repo's file structure.
- No panic or crash occurs.

---

### TC-013 — Autocomplete shows `/reverse`

**Title:** Typing `/rev` in the input shows `reverse` in the autocomplete menu

**Preconditions:**
- Ragent is running.

**Instructions:**
1. In the TUI input field, type `/rev` (do **not** press `Enter`).

**Test data to enter:**
- Input: `/rev` (no Enter)

**Expected results:**
- The slash-command autocomplete menu appears below the input field.
- One of the entries is `reverse` with a description matching the
  `SLASH_COMMANDS` registration.
- Selecting `reverse` from the menu inserts `/reverse ` into the input field.

---

### TC-014 — Trailing slash and `.git` are stripped

**Title:** `/reverse https://github.com/octocat/Hello-World/` works

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. In the TUI input field, type
   `/reverse https://github.com/octocat/Hello-World/` (note the trailing slash)
   and press `Enter`.
2. Wait for completion.

**Test data to enter:**
- Input: `/reverse https://github.com/octocat/Hello-World/`

**Expected results:**
- The trailing slash is stripped and the command proceeds as in TC-001.
- No URL-parsing error is shown.

---

### TC-015 — Cancel mid-execution

**Title:** Pressing `Esc` during `/reverse` cancels the running task

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. In the TUI input field, type
   `/reverse https://github.com/torvalds/linux` and press `Enter`.
2. While the status bar shows `⏳ reverse: …`, press `Esc` (or `Ctrl+X`).

**Test data to enter:**
- Input: `/reverse https://github.com/torvalds/linux`
- Key press: `Esc`

**Expected results:**
- The status bar changes to indicate the agent is halting.
- No further assistant messages from the `/reverse` task appear.
- The TUI remains responsive and the user can type a new command.

---

### TC-016 — scaffold-only flags scaffold a project folder

**Title:** `/spec reverse ... --language rust --type cmdline --folder <dir>` scaffolds a runnable project

**Preconditions:**
- Same as TC-001.
- `<dir>` does not exist (or exists and is empty apart from `.ragent/`, `log/`, `target/`).

**Instructions:**
1. In the TUI input field, type
   `/spec reverse https://github.com/thawkins/ragent --language rust --type cmdline --folder <dir>` and press `Enter`.
2. Wait for completion.

**Test data to enter:**
- Input: `/spec reverse https://github.com/thawkins/ragent --language rust --type cmdline --folder <dir>`

**Expected results:**
- An `[ ok ] project scaffolded in <dir>` summary appears before the fetch status,
  listing the git init and (when requested) remote outcome.
- `<dir>` contains the `/new` workspace (`src/main.rs`, `Cargo.toml`, `.ragent/`, `docs/`)
  and a git repository with an initial commit.
- The generated prompt still follows in the chat window.

---

### TC-017 — non-empty `--folder` target is reported without stopping

**Title:** a non-empty `--folder` target reports `[err]` and the prompt still generates

**Preconditions:**
- Same as TC-001.
- `<dir>` exists and contains at least one file that is not `.ragent/`, `log/`, or `target/`.

**Instructions:**
1. Run `/spec reverse https://github.com/thawkins/ragent --language rust --type cmdline --folder <dir>`.
2. Wait for completion.

**Expected results:**
- The message window shows `[err] target folder is not empty` with the blocking
  entries and a `[note] nothing was scaffolded` line.
- The repository fetch and prompt generation continue and produce the prompt.

---

### TC-019 — chained `--create` writes the spec into the scaffolded project

**Title:** `/spec reverse ... --folder <dir> --create <name>` leaves `<dir>/specs/<name>/` populated

**Preconditions:**
- Same as TC-001.
- `<dir>` does not exist (or is empty apart from `.ragent/`, `log/`, `target/`).

**Instructions:**
1. Run `/spec reverse https://github.com/thawkins/ragent --language rust --type cmdline \
   --folder <dir> --create port-spec`.
2. Wait for the generated prompt and the chained `/spec create` to finish.

**Expected results:**
- The scaffold summary reports `[ ok ] project scaffolded in <dir>`.
- The chaining notice names the target as `<dir>/specs/port-spec/`.
- `<dir>/specs/port-spec/` contains `SPEC.md`, `PLAN.md`, and `TESTPLAN.md`.
- The invoking directory does **not** gain a `specs/port-spec/` directory.

---

### TC-020 — a first-token flag reports the missing `<repo>` positional

**Title:** `/spec reverse --language rust --type cmdline` reports a usage error instead of doing nothing

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. Run `/spec reverse --language rust --type cmdline --stack gtk4` (no repo positional).
2. Run `/spec reverse --bogus`.

**Expected results:**
- The first invocation reports
  ``the first argument after `/spec reverse` must be `<repo>`, got '--language'``.
- The second reports the same wording for `'--bogus'`.
- Both leave the status line at `spec: reverse usage`; neither makes an API call.

---

### TC-018 — `--github` / `--gitlab` create a remote

**Title:** `/spec reverse ... --folder <dir> --github` creates a private remote and pushes

**Preconditions:**
- Same as TC-016.
- A GitHub token is configured (`/github login`) and `git` is on `PATH`.

**Instructions:**
1. Run
   `/spec reverse https://github.com/thawkins/ragent --language rust --type cmdline --folder <dir> --github`.
2. Wait for completion.

**Expected results:**
- The scaffold summary reports the created remote URL and the push outcome.
- `git -C <dir> remote -v` shows the created repository as `origin`.
- Supplying `--github --gitlab` together reports
  `--github and --gitlab are mutually exclusive; supply at most one`.
- Supplying `--folder` (or `--github`) without `--language`/`--type` reports a
  usage error and scaffolds nothing.

---

## Cleanup

After completing the manual tests:

1. **Remove generated specs** — delete any `specs/my-test-spec/` and
   `specs/py-spec/` directories created during TC-005 and TC-006:
   ```bash
   rm -rf specs/my-test-spec specs/py-spec
   ```
2. **Re-authenticate GitHub** — if TC-011 logged you out, run `/github login`
   again to restore your token for normal use.
3. **Clear the chat** — run `/clear` to remove test messages from the session
   history.
4. **Verify no stray files** — check the working directory for any unexpected
   output files from the `/reverse` command and remove them.