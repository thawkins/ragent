# ragent-tools-vcs

GitHub, GitLab, and local git version-control tools for ragent. Provides API
clients, OAuth device-flow auth, 39 VCS tools, and a VCS-agnostic provider
parser for the `/reverse` command.

## Workspace Dependencies

- ragent-config
- ragent-types
- ragent-storage

## External Dependencies

- tokio, async-trait, serde, serde_json, anyhow, reqwest, dirs, zip

Dev-dependencies: tempfile, wiremock.

## Public API (crate root)

- **ToolOutput** (struct) — Result of a tool execution with optional structured metadata.
- **ToolContext** (struct) — Execution context (session_id, working_dir, storage, config).
- **Tool** (trait) — Core trait for agent-invokable tools.
- **ToolRegistry** (struct) — Thread-safe registry; methods: `new`, `register`, `get`, `list`, `set_hidden`, `definitions`.
- **create_vcs_registry** (fn) — Creates a `ToolRegistry` and registers all 39 VCS tools.

## Module: storage

- **StorageBackend** (trait) — Storage backend abstraction for provider auth and settings.
- **Storage** (type alias) — Compatibility alias for `dyn StorageBackend`.

## Module: git

Local git workspace tools — execute the `git` CLI in the agent's working directory.

- Tools: `GitAddTool`, `GitBranchTool`, `GitCheckoutTool`, `GitCherryPickTool`, `GitCloneTool`, `GitCommitTool`, `GitDiffTool`, `GitFetchTool`, `GitLogTool`, `GitMergeTool`, `GitPullTool`, `GitPushTool`, `GitRemoteTool`, `GitResetTool`, `GitShowTool`, `GitStashTool`, `GitStatusTool`, `GitTagTool`.
- **run_git** (fn) — Runs a git command, returns `(stdout, stderr)`.
- **run_git_or_error** (fn) — Runs a git command, returns stdout only.

## Module: github

- Auth: **load_token**, **save_token**, **delete_token**, **device_flow_login** (fns).
- **GitHubClient** (struct) — Authenticated GitHub API client; methods: `new`, `with_token`, `with_base_url`, `client_id`, `get`, `post`, `put`, `patch`, `get_bytes`, `current_user`, `detect_repo`, `parse_repo_url`, `validate_repo_input`, `fetch_repo_metadata`, `fetch_root_tree`, `fetch_tree_recursive`, `fetch_readme`.
- **RepoMetadata** (struct) — Typed repo metadata with `from_response()`.
- **README_MAX_CHARS** (const) — Max README chars in reverse-engineering context (8000).
- **build_reverse_prompt**, **classify_api_error**, **extract_download_url**, **extract_rate_limit_reset**, **format_reset_time**, **parse_root_tree** (fns).
- **DeviceFlowState** (struct), **start_device_flow**, **poll_device_flow** (fns) — OAuth device flow.
- Tools: `GithubGetActionsTool`, `GithubListIssuesTool`, `GithubGetIssueTool`, `GithubCreateIssueTool`, `GithubCommentIssueTool`, `GithubCloseIssueTool`, `GithubListPrsTool`, `GithubGetPrTool`, `GithubCreatePrTool`, `GithubMergePrTool`, `GithubReviewPrTool`.
- **extract_context_ranges** (fn) — Extracts log line ranges around error keywords.

## Module: gitlab

- Auth: **GitLabConfig** (struct), **load_token**, **save_token**, **delete_token**, **load_config**, **save_config**, **delete_config**, **migrate_legacy_files** (fns).
- **GitLabClient** (struct) — Authenticated GitLab API client; methods: `new`, `with_credentials`, `get`, `post`, `put`, `detect_project`, `instance_url`, `fetch_project_metadata`, `fetch_repository_tree`, `fetch_repository_tree_recursive`, `fetch_readme`.
- Tools: `GitlabListIssuesTool`, `GitlabGetIssueTool`, `GitlabCreateIssueTool`, `GitlabCommentIssueTool`, `GitlabCloseIssueTool`, `GitlabListMrsTool`, `GitlabGetMrTool`, `GitlabCreateMrTool`, `GitlabMergeMrTool`, `GitlabApproveMrTool`, `GitlabListPipelinesTool`, `GitlabGetPipelineTool`, `GitlabListJobsTool`, `GitlabGetJobTool`, `GitlabGetJobLogTool`, `GitlabRetryJobTool`, `GitlabCancelJobTool`, `GitlabRetryPipelineTool`, `GitlabCancelPipelineTool`.

## Module: vcs_provider

- **VcsProvider** (enum) — Resolved VCS provider; variants: `GitHub { owner, repo }`, `GitLab { host, project_path }`.
- **parse_reverse_repo** (fn) — Parses a repository identifier into a `VcsProvider`.