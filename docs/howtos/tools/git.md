# Tools — Git

Local and remote git operations. Remote operations (`git_push`, destructive
resets) follow the Version Control Safety rules: never push unless the user
explicitly asks; never rewind files with `git_checkout`.

| Tool | Description |
|------|-------------|
| `git_add` | Stage files for commit. |
| `git_branch` | List local and remote branches. |
| `git_checkout` | Switch branch or restore files. |
| `git_cherry_pick` | Apply changes from existing commits. |
| `git_clone` | Clone a repository. |
| `git_commit` | Create a commit from staged changes. |
| `git_diff` | Show working tree, staged, or commit diff. |
| `git_fetch` | Fetch from remote without merging. |
| `git_log` | Show commit history. |
| `git_merge` | Merge another branch. |
| `git_pull` | Fetch and integrate from remote. |
| `git_push` | Push branches and tags to remote. |
| `git_remote` | List, add, remove, update remotes. |
| `git_reset` | Unstage or reset to a commit. |
| `git_show` | Show commit, tag, or object details. |
| `git_stash` | Stash and unstash changes. |
| `git_status` | Show working tree status. |
| `git_tag` | List, create, show, or delete tags. |

---

## git_add

Stage files for the next commit. At least one of `paths`, `all`, or `update`
must be provided.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `paths` | array | one of three | Specific files or directories to stage | `["src/main.rs"]` |
| `all` | boolean | one of three | Stage all changes (`git add -A`) | `true` |
| `update` | boolean | one of three | Stage changes to tracked files only (`git add -u`) | `true` |

---

## git_branch

List local and remote branches, including the checked-out branch.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `all` | boolean | no | Include remote-tracking branches | `true` |
| `format` | enum | no | `short` (terse names) or `verbose` (with upstream tracking info) | `"short"` |

---

## git_checkout

Switch branches, or restore working-tree files. Provide exactly one mode.
**Do not use to rewind files** — it risks losing work.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `branch` | string | branch mode | Branch to switch to |
| `create_branch` | boolean | no | Create and switch (`git checkout -b`) |
| `paths` | array | restore mode | Files/directories to restore |
| `source` | string | no | Ref to restore from (default `HEAD`; only with `paths`) |

---

## git_cherry_pick

Apply changes from existing commits onto the current branch.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `commits` | array | yes | Commit hashes or refs to cherry-pick (first must be valid) |
| `no_commit` | boolean | no | Apply to working tree/index without committing (`-n`) |

---

## git_clone

Clone a repository into a new directory inside the working directory.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `url` | string | yes | Repository URL | `"https://github.com/user/repo.git"` |
| `directory` | string | no | Target folder name (default: inferred from URL) | — |
| `branch` | string | no | Branch to check out (`--branch`) | `"main"` |
| `depth` | integer | no | Shallow clone depth (`--depth`) | `1` |
| `bare` | boolean | no | Create a bare repository | `false` |

---

## git_commit

Create a commit from staged changes.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `message` | string | yes | Commit message | `"Fix config loader panic"` |
| `all` | boolean | no | Also stage modified tracked files (`-a`) | `false` |
| `amend` | boolean | no | Amend the previous commit | `false` |
| `no_verify` | boolean | no | Bypass pre-commit hooks | `false` |

---

## git_diff

Show differences for the working tree, the staged index, or a commit.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `target` | string | no | `working` (default), `staged`/`cached`, or a commit ref | `"staged"` |
| `path` | string | no | Limit to one file or directory | `"src/cli.rs"` |
| `stat` | boolean | no | Compact change summary only | `true` |

---

## git_fetch

Fetch updates from a remote without merging.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `remote` | string | no | Remote name (default `origin`) |
| `branch` | string | no | Specific branch or ref |
| `prune` | boolean | no | Remove stale remote-tracking branches |
| `all` | boolean | no | Fetch every remote (ignores `remote`/`branch`) |

---

## git_log

Show commit history for the current branch or a ref.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `limit` | integer | no | Max commits | `20` |
| `branch` | string | no | Ref to log (default: current branch/HEAD) | `"main"` |
| `oneline` | boolean | no | Compact one-line format | `true` |
| `author` | string | no | Filter by author name or email | — |
| `since` | string | no | Only newer commits | `"2024-01-01"`, `"1.week"` |

---

## git_merge

Merge another branch into the current branch.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `branch` | string | yes | Branch to merge in |
| `message` | string | no | Custom merge commit message |
| `no_ff` | boolean | no | Always create a merge commit |
| `ff_only` | boolean | no | Abort unless fast-forward (mutually exclusive with `no_ff`) |
| `squash` | boolean | no | Squash incoming changes into one commit |

---

## git_pull

Fetch and integrate changes into the current branch.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `remote` | string | no | Remote name (default `origin`) |
| `branch` | string | no | Remote branch to pull (default: tracking branch) |
| `rebase` | boolean | no | Rebase instead of merge |

---

## git_push

Push local branches and tags to a remote. **Only call with explicit user
instruction.**

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `remote` | string | no | Remote name (default `origin`) |
| `branch` | string | no | Branch to push (default: current) |
| `force` | boolean | no | Force with lease (destructive; requires explicit confirmation) |
| `tags` | boolean | no | Push all tags |

---

## git_remote

List, add, remove, or set URLs for remotes.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `action` | enum | no | `list` (default), `add`, `remove`, `set-url` |
| `name` | string | yes for non-list | Remote name |
| `url` | string | yes for `add`/`set-url` | Remote URL |

---

## git_reset

Unstage files or reset the branch to a commit. Choose one mode. **`hard`
discards all local working-tree changes.**

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `paths` | array | unstage mode | Files to unstage | `["src/x.rs"]` |
| `target` | string | reset mode | Commit hash, tag, or ref (default `HEAD`) | `"HEAD~1"` |
| `mode` | enum | reset mode | `mixed` (default, keeps changes unstaged), `soft` (keeps staged), `hard`, `keep` | `"mixed"` |

---

## git_show

Show details of a commit, tag, or other git object.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `ref` | string | no | Commit hash, tag, or ref (default `HEAD`) | `"abc123"` |
| `stat` | boolean | no | Include file change statistics (default true) | `true` |

---

## git_stash

Stash and unstash working-tree changes.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `action` | enum | no | `push` (default), `pop`, `apply`, `drop`, `list`, `clear` |
| `message` | string | no | Message for `push` |
| `index` | integer | no | Stash index for `pop`/`apply`/`drop` (default 0) |

---

## git_status

Show working-tree status: modified, staged, untracked, conflicted files.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `short` | boolean | no | Compact one-line-per-file format |
| `branch` | boolean | no | Include current branch name (default true) |

---

## git_tag

List, show, create, or delete tags. Annotated tags require both `name` and
`message`. **Do not tag releases unless explicitly asked.**

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `action` | enum | no | `list` (default), `show`, `create`, `delete` |
| `name` | string | yes for show/create/delete | Tag name |
| `message` | string | annotated create | Tag message |
| `ref` | string | no | Target commit or ref (default HEAD) |

**Example:**
```text
git_status
git_add paths=["src/main.rs"]
git_commit message="Add greet subcommand"
```
