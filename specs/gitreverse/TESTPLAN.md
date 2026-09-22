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

### TC-004 — `--tech` flag constrains the prompt

**Title:** `/reverse --tech rust` injects the tech stack into the prompt

**Preconditions:**
- Same as TC-001.

**Instructions:**
1. In the TUI input field, type
   `/reverse --tech rust https://github.com/thawkins/ragent` and press `Enter`.
2. Wait for completion.

**Test data to enter:**
- Input: `/reverse --tech rust https://github.com/thawkins/ragent`

**Expected results:**
- The status bar shows `⏳ reverse: thawkins/ragent…`.
- The generated prompt explicitly mentions Rust as the technology stack, either
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

### TC-006 — `--tech` and `--create` combined

**Title:** `/reverse --tech python --create py-spec` works together

**Preconditions:**
- Same as TC-001 and TC-005.

**Instructions:**
1. In the TUI input field, type
   `/reverse --tech python --create py-spec https://github.com/octocat/Hello-World`
   and press `Enter`.
2. Wait for both the reverse and spec-create phases to finish.
3. Check the filesystem.

**Test data to enter:**
- Input: `/reverse --tech python --create py-spec https://github.com/octocat/Hello-World`

**Expected results:**
- The generated prompt mentions Python as the tech stack.
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
  - The optional `--tech <stack>` flag.
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