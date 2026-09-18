# Tools — GitLab

Native GitLab tools for issues, merge requests, pipelines, and jobs. Requires
`/gitlab setup` authentication and a GitLab-backed git remote.

| Tool | Description |
|------|-------------|
| `gitlab_list_issues` | List issues. |
| `gitlab_get_issue` | Get issue details. |
| `gitlab_create_issue` | Create an issue. |
| `gitlab_comment_issue` | Comment on an issue. |
| `gitlab_close_issue` | Close an issue. |
| `gitlab_list_mrs` | List merge requests. |
| `gitlab_get_mr` | Get MR details. |
| `gitlab_create_mr` | Create a merge request. |
| `gitlab_merge_mr` | Merge an MR. |
| `gitlab_approve_mr` | Approve an MR. |
| `gitlab_list_pipelines` | List pipelines. |
| `gitlab_get_pipeline` | Get pipeline details. |
| `gitlab_list_jobs` | List jobs in a pipeline. |
| `gitlab_get_job` | Get job details. |
| `gitlab_get_job_log` | Get job log output. |
| `gitlab_retry_job` | Retry a failed job. |
| `gitlab_cancel_job` | Cancel a running job. |
| `gitlab_retry_pipeline` | Retry a pipeline. |
| `gitlab_cancel_pipeline` | Cancel a pipeline. |

**Visibility switch:** `gitlab`.

---

## gitlab_list_issues

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `state` | enum | no | `opened` (default), `closed`, `all` | `"opened"` |
| `labels` | string | no | Comma-separated label names | `"bug"` |

---

## gitlab_get_issue / gitlab_close_issue

Get full issue details, or close an issue.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `iid` | integer | yes | Issue internal ID (project-scoped) |
| `comment` | string | no (close) | Closing note posted before closing |

---

## gitlab_create_issue

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `title` | string | yes | Issue title | `"Pipeline failing on main"` |
| `description` | string | no | Description in markdown | — |
| `labels` | string | no | Comma-separated labels | `"bug"` |

---

## gitlab_comment_issue

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `iid` | integer | yes | Issue internal ID |
| `body` | string | yes | Comment text |

---

## gitlab_list_mrs

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `state` | enum | no | `opened` (default), `merged`, `closed`, `all` | `"opened"` |
| `target_branch` | string | no | Filter by target branch | `"main"` |

---

## gitlab_get_mr

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `iid` | integer | yes | Merge request internal ID |

---

## gitlab_create_mr

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `title` | string | yes | MR title | `"Add greet subcommand"` |
| `source_branch` | string | yes | Source branch | `"feature/greet"` |
| `target_branch` | string | yes | Target branch | `"main"` |
| `description` | string | no | Description in markdown | — |

---

## gitlab_merge_mr / gitlab_approve_mr

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `iid` | integer | yes | Merge request internal ID |

---

## gitlab_list_pipelines / gitlab_get_pipeline

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `status` | enum | no (`list`) | `running`, `pending`, `success`, `failed`, `canceled` | `"failed"` |
| `id` | integer | yes (`get`) | Pipeline ID | — |

---

## gitlab_list_jobs / gitlab_get_job / gitlab_get_job_log

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `pipeline_id` | integer | yes (`list_jobs`) | Pipeline to list jobs for |
| `job_id` | integer | yes (`get_job`, `get_job_log`) | Job ID |

`gitlab_get_job_log` returns the job's trace output (may be large).

---

## gitlab_retry_job / gitlab_cancel_job / gitlab_retry_pipeline / gitlab_cancel_pipeline

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `job_id` | integer | yes (job tools) | Job ID |
| `pipeline_id` | integer | yes (pipeline tools) | Pipeline ID |

**Example:**
```text
gitlab_list_pipelines status="failed"
gitlab_list_jobs pipeline_id=12345
gitlab_get_job_log job_id=67890
gitlab_retry_job job_id=67890
```
