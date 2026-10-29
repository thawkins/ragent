# Tools — Shell

Tools for running shell commands and managing shell tasks. `Bash`
commands pass through a 7-layer security model: safe-command whitelist,
banned-command checks, denied patterns, directory-escape prevention, syntax
validation, obfuscation detection, and user allowlist/denylist.

| Tool | Description |
|------|-------------|
| `Bash` | Execute a shell command (7-layer security). |
| `bash_reset` | Reset persistent shell state. |
| `bg` | Manage shell tasks (spawn, list, wait, cancel). |
| `open` | Open a file/folder/URL via the desktop handler. |

**Use cases:** running builds, tests, git operations, long-running tasks.

**System instruction:** "For simple commands use `Bash` with the `command`
parameter immediately. Do not describe what you will run."

---

## Bash

Execute a shell command and return its stdout and stderr. Commands run in the
agent's working directory in a persistent shell.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `command` | string | yes | Shell command to execute | `"cargo test"` |
| `timeout` | integer | no | Timeout in seconds | `120` (default); up to `1500` for builds/tests |

**Example:**
```text
Bash command="cargo test -- --nocapture" timeout=1500
Bash command="cargo fmt"
```

---

## bash_reset

Reset the persistent shell state for the current session, clearing saved
working-directory changes and environment variables. No-op if no persistent
state exists.

**Arguments:** none.

```text
bash_reset
```

---

## bg

Manage **shell tasks**: spawn long-running commands that continue while the
agent does other work, then inspect, follow, or stop them.

A shell task is not a sub-agent. Its rows live in the background-task store, in
a different id space from the sub-agent task registry, so a shell task never
appears in `list_agents` and can never be reached with `cancel_agent` or
`wait_agents` - use `new_agent` to start a sub-agent and `cancel_agent` /
`wait_agents` to control one.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `action` | enum | yes | `spawn`, `list`, `status`, `output`, `tail`, `cancel`, `wait`, `cleanup` | `"spawn"` |
| `command` | string | yes for `spawn` | Shell command to run in background | `"cargo build"` |
| `task_id` | string | yes for `status`, `output`, `tail`, `cancel`, `wait` | Shell task id | `"bg-3"` |
| `lines` | integer | no | Lines returned by `tail` | `20` |
| `status` | enum | no | `list` filter: `running` / `completed` / `failed` / `cancelled` | — |
| `limit` | integer | no | Max tasks for `list` | `50` |
| `timeout` | integer | no | `wait` timeout in seconds | `60` |
| `working_dir` | string | no | Working directory for `spawn` | session cwd |
| `completed_only` | boolean | no | `cleanup`: only completed/failed/cancelled tasks | `true` |
| `older_than_minutes` | integer | no | `cleanup`: minimum task age | `60` |
| `session_id` | string | no | `list` / `cleanup` scope. It must name the calling session and then behaves exactly as omitting it; a foreign id is refused | `"sess-42"` |

**Session scope (FR-019, FR-037):** `bg` reads and deletes only the calling
session's shell tasks. `session_id` naming any other session is refused with
`reason_code: wrong_session`, naming both sessions; the refusal deletes
nothing. Omit the parameter, or pass the caller's own id - the two produce an
identical result.

**Reason codes:** `unknown_action`, `parameter_not_accepted`,
`service_unavailable`, `wrong_session`.

**Example:**
```text
bg action="spawn" command="cargo build"
bg action="tail" task_id="bg-1" lines=20
bg action="wait" task_id="bg-1" timeout=300
```

---

## open

Open or reveal a file, folder, or URL using the desktop default handler
(`xdg-open` on Linux, `open` on macOS, `start` on Windows). May fail in
headless or sandboxed contexts.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `target` | string | yes | File path, folder path, or URL | `"report.pdf"` |
| `action` | enum | no | `open` (default), `reveal` (open parent directory), `url` (validate and open a URL) | `"open"` |

**Example:**
```text
open target="docs/howtos/tools/pdf/file-operations.md.pdf"
```
