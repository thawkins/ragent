# Tools — GitHub

Native GitHub tools for issues, pull requests, reviews, and Actions. All
tools require GitHub authentication (`/github login`) and a GitHub-backed git
remote. Issue/PR numbers are repository-scoped.

| Tool | Description |
|------|-------------|
| `github_list_issues` | List issues (open/closed/all). |
| `github_get_issue` | Get full issue details. |
| `github_create_issue` | Create a new issue. |
| `github_comment_issue` | Comment on an issue. |
| `github_close_issue` | Close an issue. |
| `github_list_prs` | List pull requests. |
| `github_get_pr` | Get PR details. |
| `github_create_pr` | Create a new pull request. |
| `github_merge_pr` | Merge an open PR. |
| `github_review_pr` | Submit a PR review. |
| `github_get_actions` | List recent Actions workflow runs. |

**Visibility switch:** `github`. Requires `/github login` first. See
`docs/howtos/tutorial.md` Section 7.

---

## github_list_issues

List issues in the repository detected from the current working directory.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `state` | enum | no | `open` (default), `closed`, `all` | `"open"` |
| `labels` | string | no | Comma-separated label names (must already exist) | `"bug"` |
| `limit` | integer | no | Max issues (max 100) | `20` |

---

## github_get_issue

Get full issue details: title, body, state, labels, assignees, and the first
10 comments. Pull requests also appear via the issues endpoint but with
incomplete details — use `github_get_pr` for PR-specific data.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `number` | integer | yes | Issue number |

---

## github_create_issue

Create a new issue.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `title` | string | yes | Issue title | `"Panic in config loader"` |
| `body` | string | no | Description in markdown | — |
| `labels` | string | no | Comma-separated existing label names | `"bug,help wanted"` |
| `assignees` | string | no | Comma-separated GitHub usernames | `"octocat"` |

---

## github_comment_issue

Add a comment to an issue.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `number` | integer | yes | Issue number |
| `body` | string | yes | Comment text (markdown supported) |

---

## github_close_issue

Close an issue, optionally posting a closing comment first. Closing an
already-closed issue is a no-op; requires collaborator permissions.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `number` | integer | yes | Issue number |
| `comment` | string | no | Closing note posted before closing |

---

## github_list_prs

List pull requests.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `state` | enum | no | `open` (default), `closed`, `all` | `"open"` |
| `base` | string | no | Filter by exact base branch name | `"main"` |
| `limit` | integer | no | Max PRs (max 100) | `20` |

---

## github_get_pr

Get PR details: title, description, state, source and base branches, and the
first batch of review comments (not the full diff).

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `number` | integer | yes | PR number |

---

## github_create_pr

Create a new pull request. The head branch must already be pushed to the
remote.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `title` | string | yes | PR title | `"Add greet subcommand"` |
| `base` | string | no | Target branch | `"main"` |
| `head` | string | no | Source branch (default: current git branch) | `"feature/greet"` |
| `body` | string | no | Description in markdown | `"Implements greet"` |
| `draft` | boolean | no | Create as draft | `false` |

---

## github_merge_pr

Merge an open PR. The PR must be mergeable and required checks must pass.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `number` | integer | yes | PR number | `42` |
| `method` | enum | no | `merge` (default), `squash`, `rebase` | `"squash"` |
| `message` | string | no | Custom merge commit message | — |

---

## github_review_pr

Submit a review on a PR. Note: `APPROVE` cannot include a body in the GitHub
API — use a separate `COMMENT` for explanatory text.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `number` | integer | yes | PR number |
| `event` | enum | yes | `APPROVE`, `REQUEST_CHANGES`, or `COMMENT` |
| `body` | string | yes for `COMMENT`/`REQUEST_CHANGES` | Review comment text |

---

## github_get_actions

List recent GitHub Actions workflow runs with status summaries and, for
failed runs, filtered log excerpts around `error`/`failed` lines (log
extraction downloads a zip archive and may be slow for large workflows).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `limit` | integer | no | Recent runs to inspect (max 30) | `1` |

**Example:**
```text
github_create_pr title="Add greet subcommand" base="main" body="Implements greet"
github_merge_pr number=42 method="squash"
```
