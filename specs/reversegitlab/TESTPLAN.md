---
status: draft
---

# Manual Test Plan: reversegitlab

This is a manual test plan for verifying GitLab support and depth-controlled
tree fetching in the `/reverse` command. All tests are performed by a human in
the ragent TUI unless noted otherwise.

## Prerequisites

### Environment setup

1. A working ragent build from the current branch: run `cargo build` in the
   project root and confirm `target/debug/ragent` exists.
2. At least one LLM provider configured (Anthropic, OpenAI, or Ollama) so
   the synthetic prompt generation step can complete. Run `/models` in the
   TUI to confirm a model is available.
3. A GitHub token configured via `/github login` or `GITHUB_TOKEN` env var
   (required for backward-compatibility tests and GitHub-prefixed tests).
4. A GitLab Personal Access Token with `read_api` scope for a public
   `gitlab.com` account (required for public GitLab tests).
5. Access to a self-hosted GitLab instance (private or community edition) for
   the self-hosted tests. If no self-hosted instance is available, skip the
   self-hosted test cases and note the skip in the test report.
6. The following sample repositories identified for testing:
   - GitHub: `octocat/Hello-World` (public, minimal README, shallow tree)
   - GitHub: a repo with subdirectories (e.g. `rust-lang/rust` or any project
     with `src/` subdirectories) for depth > 1 tests
   - GitLab.com public: `gitlab-org/gitlab-runner` (public, has README and
     a root file tree with subdirectories)
   - GitLab.com nested namespace: any public project under a group, e.g.
     `gitlab-org/security-products/analyzers/bandit` (if accessible)
   - Self-hosted: a known public or accessible project on the self-hosted
     GitLab instance

### GitLab configuration

Before running GitLab tests, configure GitLab credentials:

1. In the ragent TUI, type `/gitlab setup` and press Enter.
2. In the **Instance URL** field, enter `https://gitlab.com` (or the
   self-hosted instance URL for self-hosted tests).
3. In the **Token** field, enter the GitLab Personal Access Token.
4. Press Tab to highlight **Save** and press Enter.
5. Run `/gitlab status` and confirm the output shows:
   ```
   From: /gitlab
   ✅ GitLab configured
   Instance: https://gitlab.com
   Token: ✅ configured
   ```

Alternatively, set `GITLAB_TOKEN` and `GITLAB_URL` environment variables
before launching ragent.

## Test Cases

### TC-001: Bare GitHub `owner/repo` backward compatibility

**Title:** `/reverse octocat/Hello-World` still works after the change

**Preconditions:**
- GitHub token configured
- LLM provider available
- ragent running in TUI

**Steps:**
1. Type `/reverse octocat/Hello-World` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse octocat/Hello-World`

**Expected results:**
- A status line appears: `⏳ reverse: octocat/Hello-World…`
- After a few seconds, the chat panel shows a generated synthetic creation
  prompt describing the Hello-World repository.
- The log panel shows: `reverse: fetching octocat/Hello-World`
- No error messages appear.

### TC-002: GitHub URL backward compatibility

**Title:** `/reverse https://github.com/octocat/Hello-World` still works

**Preconditions:**
- Same as TC-001

**Steps:**
1. Type `/reverse https://github.com/octocat/Hello-World` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse https://github.com/octocat/Hello-World`

**Expected results:**
- Same as TC-001 — a synthetic creation prompt is generated for
  `octocat/Hello-World`.
- No error messages appear.

### TC-003: `github:` prefixed identifier

**Title:** `/reverse github:octocat/Hello-World` routes to GitHub

**Preconditions:**
- Same as TC-001

**Steps:**
1. Type `/reverse github:octocat/Hello-World` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse github:octocat/Hello-World`

**Expected results:**
- A status line appears including `github:octocat/Hello-World`.
- A synthetic creation prompt is generated for the repository.
- The context block includes a `## Repository Source` section mentioning
  GitHub.

### TC-004: `gitlab:` prefixed public gitlab.com repo

**Title:** `/reverse gitlab:gitlab-org/gitlab-runner` routes to GitLab

**Preconditions:**
- GitLab token configured for `https://gitlab.com` (see Prerequisites)
- LLM provider available

**Steps:**
1. Type `/reverse gitlab:gitlab-org/gitlab-runner` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:gitlab-org/gitlab-runner`

**Expected results:**
- A status line appears: `⏳ reverse: gitlab:gitlab-org/gitlab-runner…`
- The log panel shows: `reverse: fetching gitlab:gitlab-org/gitlab-runner`
- After a few seconds, the chat panel shows a generated synthetic creation
  prompt describing the gitlab-runner repository.
- The context block includes a `## Repository Source` section mentioning
  GitLab and the instance URL `https://gitlab.com`.

### TC-005: Full GitLab URL

**Title:** `/reverse https://gitlab.com/gitlab-org/gitlab-runner` routes to GitLab

**Preconditions:**
- Same as TC-004

**Steps:**
1. Type `/reverse https://gitlab.com/gitlab-org/gitlab-runner` in the TUI
   input box.
2. Press Enter.

**Test data:**
- Command: `/reverse https://gitlab.com/gitlab-org/gitlab-runner`

**Expected results:**
- Same as TC-004 — a synthetic creation prompt is generated for
  `gitlab-org/gitlab-runner` from `https://gitlab.com`.

### TC-006: Self-hosted GitLab via `gitlab:host/…` prefix

**Title:** `/reverse gitlab:gitlab.example.com/group/project` targets self-hosted instance

**Preconditions:**
- Access to a self-hosted GitLab instance (e.g. `gitlab.example.com`)
- A GitLab token valid for that instance
- A public or accessible project on that instance (e.g. `group/project`)
- `GITLAB_TOKEN` env var set to the self-hosted instance token, or `/gitlab
  setup` configured with the self-hosted instance URL

**Steps:**
1. Type `/reverse gitlab:gitlab.example.com/group/project` in the TUI input
   box, replacing `gitlab.example.com/group/project` with your actual
   self-hosted host and project path.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:<your-host>/<your-group>/<your-project>`

**Expected results:**
- A status line appears: `⏳ reverse: gitlab:<your-host>/<your-group>/<your-project>…`
- The system fetches metadata, tree, and README from the self-hosted GitLab
  instance at `https://<your-host>`, not from `gitlab.com`.
- A synthetic creation prompt is generated for the project.
- The context block includes a `## Repository Source` section mentioning
  GitLab and the self-hosted host.

### TC-007: Self-hosted GitLab via full URL

**Title:** `/reverse https://gitlab.example.com/group/project` targets self-hosted instance

**Preconditions:**
- Same as TC-006

**Steps:**
1. Type `/reverse https://gitlab.example.com/group/project` in the TUI
   input box, replacing with your actual self-hosted URL and project path.
2. Press Enter.

**Test data:**
- Command: `/reverse https://<your-host>/<your-group>/<your-project>`

**Expected results:**
- Same as TC-006 — the system detects the GitLab host from the URL and
  fetches from the self-hosted instance.

### TC-008: GitLab nested namespace

**Title:** `/reverse gitlab:group/subgroup/project` handles nested namespaces

**Preconditions:**
- Same as TC-004
- A public GitLab.com project with a nested namespace (e.g.
  `gitlab-org/security-products/analyzers/bandit` or similar)

**Steps:**
1. Type `/reverse gitlab:<group>/<subgroup>/<project>` in the TUI input box,
   replacing with an actual nested-namespace project path.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:gitlab-org/security-products/analyzers/bandit`

**Expected results:**
- The system URL-encodes the full `group/subgroup/project` path as the
  project ID (e.g. `gitlab-org%2Fsecurity-products%2Fanalyzers%2Fbandit`).
- A synthetic creation prompt is generated for the project.
- No "project not found" error appears.

### TC-009: GitLab with `--tech` flag

**Title:** `/reverse gitlab:gitlab-org/gitlab-runner --tech Rust` constrains the prompt

**Preconditions:**
- Same as TC-004

**Steps:**
1. Type `/reverse gitlab:gitlab-org/gitlab-runner --tech Rust` in the TUI
   input box.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:gitlab-org/gitlab-runner --tech Rust`

**Expected results:**
- The generated synthetic creation prompt targets the Rust technology stack.
- The context block includes a `## Technology Stack Constraint` section with
  `Rust`.

### TC-010: GitLab with `--create` flag chains to `/spec create`

**Title:** `/reverse gitlab:gitlab-org/gitlab-runner --create runner-clone` chains to spec creation

**Preconditions:**
- Same as TC-004

**Steps:**
1. Type `/reverse gitlab:gitlab-org/gitlab-runner --create runner-clone` in
   the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:gitlab-org/gitlab-runner --create runner-clone`

**Expected results:**
- The synthetic creation prompt is generated.
- A notice appears: `reverse: generated prompt for gitlab:gitlab-org/gitlab-runner. Chaining into /spec create runner-clone…`
- A new spec is created at `specs/runner-clone/` containing `SPEC.md`,
  `PLAN.md`, and `TESTPLAN.md`.
- The `SPEC.md` contains content derived from the gitlab-runner repository.

### TC-011: GitLab without token — error message

**Title:** `/reverse gitlab:…` with no GitLab token shows setup guidance

**Preconditions:**
- No `GITLAB_TOKEN` env var set
- No GitLab token in the database (run `/gitlab logout` to clear)
- LLM provider available

**Steps:**
1. Type `/reverse gitlab:gitlab-org/gitlab-runner` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:gitlab-org/gitlab-runner`

**Expected results:**
- The chat panel shows an error message containing:
  `❌ No GitLab token configured.`
- The message directs the user to run `/gitlab setup`.
- No API call is made; no LLM generation starts.

### TC-012: Invalid GitLab project — 404 error

**Title:** `/reverse gitlab:nonexistent/fake-repo-12345` shows project-not-found error

**Preconditions:**
- GitLab token configured
- LLM provider available

**Steps:**
1. Type `/reverse gitlab:nonexistent/fake-repo-12345` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:nonexistent/fake-repo-12345`

**Expected results:**
- The chat panel shows an error message stating the project was not found on
  the GitLab instance.
- The error suggests verifying the namespace and project path.
- No synthetic creation prompt is generated.

### TC-013: Invalid repository identifier — usage error

**Title:** `/reverse justoneword` shows usage message with all formats

**Preconditions:**
- ragent running in TUI

**Steps:**
1. Type `/reverse justoneword` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse justoneword`

**Expected results:**
- The chat panel shows the help/usage message.
- The usage message lists `github:` and `gitlab:` prefixes, the self-hosted
  `gitlab:host/…` form, and full URL formats for both providers.
- The usage message mentions that GitLab requires `/gitlab setup`.
- The usage message mentions the `--depth <N>` flag and its default of 1.

### TC-014: `/reverse help` shows updated documentation

**Title:** `/reverse help` lists both providers and --depth flag

**Preconditions:**
- ragent running in TUI

**Steps:**
1. Type `/reverse help` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse help`

**Expected results:**
- The help message documents `github:owner/repo` and
  `gitlab:namespace/project` formats.
- The help message documents the self-hosted form
  `gitlab:host/namespace/project`.
- The help message documents full URL and SSH URL formats for both
  providers.
- The help message states that GitLab requires `/gitlab setup` (or
  `GITLAB_TOKEN` + `GITLAB_URL` env vars).
- The help message documents the `--depth <N>` flag, its default value of 1,
  and the valid range of 1–10.
- The help message includes examples for both providers and for `--depth`.

### TC-015: GitLab SSH URL

**Title:** `/reverse git@gitlab.com:gitlab-org/gitlab-runner.git` routes to GitLab

**Preconditions:**
- Same as TC-004

**Steps:**
1. Type `/reverse git@gitlab.com:gitlab-org/gitlab-runner.git` in the TUI
   input box.
2. Press Enter.

**Test data:**
- Command: `/reverse git@gitlab.com:gitlab-org/gitlab-runner.git`

**Expected results:**
- The system detects the `gitlab.com` host from the SSH URL.
- A synthetic creation prompt is generated for `gitlab-org/gitlab-runner`.
- No error messages appear.

### TC-016: Default depth (omitted) — root level only

**Title:** `/reverse github:octocat/Hello-World` (no --depth) fetches root level only

**Preconditions:**
- GitHub token configured
- LLM provider available

**Steps:**
1. Type `/reverse github:octocat/Hello-World` in the TUI input box (no
   `--depth` flag).
2. Press Enter.

**Test data:**
- Command: `/reverse github:octocat/Hello-World`

**Expected results:**
- A synthetic creation prompt is generated.
- The context block's `## Root File Tree` section lists only root-level
  files and directories (no subdirectory contents).
- This matches the pre-existing behavior before the `--depth` feature was
  added.

### TC-017: `--depth 1` explicit — root level only

**Title:** `/reverse github:octocat/Hello-World --depth 1` fetches root level only

**Preconditions:**
- Same as TC-016

**Steps:**
1. Type `/reverse github:octocat/Hello-World --depth 1` in the TUI input
   box.
2. Press Enter.

**Test data:**
- Command: `/reverse github:octocat/Hello-World --depth 1`

**Expected results:**
- Same as TC-016 — only root-level files and directories appear in the
  tree section.
- No error messages appear.

### TC-018: `--depth 2` — root plus one subdirectory level

**Title:** `/reverse github:rust-lang/rust --depth 2` fetches two levels

**Preconditions:**
- GitHub token configured
- LLM provider available
- A GitHub repository with subdirectories (e.g. `rust-lang/rust` or any
  project with a `src/` directory containing files)

**Steps:**
1. Type `/reverse github:<owner>/<repo> --depth 2` in the TUI input box,
   replacing with a repo that has subdirectories.
2. Press Enter.

**Test data:**
- Command: `/reverse github:rust-lang/rust --depth 2`

**Expected results:**
- A synthetic creation prompt is generated.
- The context block's file tree section includes root-level entries AND
  the contents of root-level subdirectories (one level deep).
- Directory names in the tree have a trailing slash (e.g. `src/`).
- Subdirectory entries include path separators (e.g. `src/main.rs`,
  `src/lib.rs`).
- Entries at depth 3 or deeper do NOT appear.

### TC-019: `--depth 3` — three levels deep

**Title:** `/reverse github:<owner>/<repo> --depth 3` fetches three levels

**Preconditions:**
- Same as TC-018
- A repository with at least 3 levels of nested directories

**Steps:**
1. Type `/reverse github:<owner>/<repo> --depth 3` in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse github:<owner>/<repo> --depth 3`

**Expected results:**
- The file tree section includes entries up to 3 levels deep.
- Entries at depth 4 or deeper do NOT appear.
- All directory names have trailing slashes.
- All nested entries use path separators.

### TC-020: `--depth 2` with GitLab

**Title:** `/reverse gitlab:gitlab-org/gitlab-runner --depth 2` fetches two levels from GitLab

**Preconditions:**
- GitLab token configured
- LLM provider available

**Steps:**
1. Type `/reverse gitlab:gitlab-org/gitlab-runner --depth 2` in the TUI
   input box.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:gitlab-org/gitlab-runner --depth 2`

**Expected results:**
- A synthetic creation prompt is generated.
- The file tree section includes root-level entries and the contents of
  root-level subdirectories (one level deep).
- Directory names have trailing slashes.
- Subdirectory entries include path separators.

### TC-021: `--depth 0` — rejected with error

**Title:** `/reverse github:octocat/Hello-World --depth 0` shows range error

**Preconditions:**
- ragent running in TUI

**Steps:**
1. Type `/reverse github:octocat/Hello-World --depth 0` in the TUI input
   box.
2. Press Enter.

**Test data:**
- Command: `/reverse github:octocat/Hello-World --depth 0`

**Expected results:**
- The chat panel shows an error message stating that `--depth` must be
  between 1 and 10.
- No API call is made; no fetch or LLM generation starts.

### TC-022: `--depth -1` — rejected with error

**Title:** `/reverse github:octocat/Hello-World --depth -1` shows range error

**Preconditions:**
- ragent running in TUI

**Steps:**
1. Type `/reverse github:octocat/Hello-World --depth -1` in the TUI input
   box.
2. Press Enter.

**Test data:**
- Command: `/reverse github:octocat/Hello-World --depth -1`

**Expected results:**
- The chat panel shows an error message stating that `--depth` must be
  between 1 and 10.
- No API call is made.

### TC-023: `--depth 11` — rejected with error

**Title:** `/reverse github:octocat/Hello-World --depth 11` shows range error

**Preconditions:**
- ragent running in TUI

**Steps:**
1. Type `/reverse github:octocat/Hello-World --depth 11` in the TUI input
   box.
2. Press Enter.

**Test data:**
- Command: `/reverse github:octocat/Hello-World --depth 11`

**Expected results:**
- The chat panel shows an error message stating that `--depth` must be
  between 1 and 10.
- No API call is made.

### TC-024: `--depth` combined with `--tech` and `--create`

**Title:** `/reverse gitlab:gitlab-org/gitlab-runner --depth 2 --tech Rust --create runner-deep` works end-to-end

**Preconditions:**
- GitLab token configured
- LLM provider available

**Steps:**
1. Type `/reverse gitlab:gitlab-org/gitlab-runner --depth 2 --tech Rust --create runner-deep`
   in the TUI input box.
2. Press Enter.

**Test data:**
- Command: `/reverse gitlab:gitlab-org/gitlab-runner --depth 2 --tech Rust --create runner-deep`

**Expected results:**
- A synthetic creation prompt is generated with:
  - Two levels of file tree entries
  - A `## Technology Stack Constraint` section with `Rust`
  - A `## Repository Source` section mentioning GitLab
- A notice appears: `reverse: generated prompt for gitlab:gitlab-org/gitlab-runner. Chaining into /spec create runner-deep…`
- A new spec is created at `specs/runner-deep/` containing `SPEC.md`,
  `PLAN.md`, and `TESTPLAN.md`.

### TC-025: `--depth 10` — maximum allowed value

**Title:** `/reverse github:octocat/Hello-World --depth 10` accepts maximum depth

**Preconditions:**
- GitHub token configured
- LLM provider available

**Steps:**
1. Type `/reverse github:octocat/Hello-World --depth 10` in the TUI input
   box.
2. Press Enter.

**Test data:**
- Command: `/reverse github:octocat/Hello-World --depth 10`

**Expected results:**
- No range error appears — depth 10 is accepted.
- A synthetic creation prompt is generated.
- The file tree section may include entries up to 10 levels deep (or fewer
  if the repository is shallower than 10 levels).

## Cleanup

After all test cases are complete:

1. Remove any specs created during testing:
   ```
   /spec list
   ```
   For each spec created during testing (e.g. `runner-clone`, `runner-deep`),
   delete the directory: `rm -rf specs/runner-clone/ specs/runner-deep/`.
2. Remove any research directories created during testing:
   ```
   rm -rf research/runner-clone-context/
   ```
3. If you changed GitLab configuration during testing (e.g. pointed it at a
   self-hosted instance), restore it by running `/gitlab setup` and entering
   the original instance URL, or run `/gitlab logout` to clear it.
4. Clear any `GITLAB_TOKEN` or `GITLAB_URL` environment variables if they
   were set for testing only.