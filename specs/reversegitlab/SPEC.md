---
status: draft
audit:
  - { time: 1787388741, from: "none", to: "draft", actor: "system" }
---
# Specification: reversegitlab — GitLab Support for the `/reverse` Command

## Overview

The `/reverse` slash command reverse-engineers a repository by fetching its
metadata, root file tree, and README, then asking the LLM to generate a
synthetic creation prompt. Today it only works with GitHub repositories:
`parse_reverse_args` produces a bare repo string, `GitHubClient::validate_repo_input`
parses it into `(owner, repo)`, and `GitHubClient` fetches the three artifacts
via the GitHub REST API.

ragent already ships a full GitLab client (`ragent_tools_vcs::gitlab::client::GitLabClient`),
GitLab auth (`ragent_tools_vcs::gitlab::auth`), a `/gitlab setup` TUI dialog,
and a `GitLabIntegrationConfig` in `ragent-config`. However, the GitLab client
only has methods for merge requests, pipelines, jobs, and issues — it has no
methods for fetching project metadata, the repository tree, or the README,
and `/reverse` has no dispatch logic to route to it.

This specification adds support for both public `gitlab.com` repositories and
private self-hosted GitLab instances in the `/reverse` command by introducing
provider-prefixed repository identifiers (`github:`, `gitlab:`), adding the
missing GitLab fetch methods, and wiring a VCS-agnostic dispatch layer into
`handle_reverse_command`.

## Requirements

### FR-001

The system shall accept provider-prefixed repository identifiers in the
`/reverse` command, where the prefix `github:` routes to the GitHub API and
the prefix `gitlab:` routes to the GitLab API. (Ubiquitous)

### FR-002

The system shall accept `gitlab:<namespace>/<project>` identifiers that
resolve against the configured GitLab instance URL (loaded from `GITLAB_URL`
env, `ragent.json`, or the encrypted database via `/gitlab setup`), defaulting
to `https://gitlab.com` when no instance URL is configured. (State-driven)

### FR-003

The system shall accept `gitlab:<host>/<namespace>/<project>` identifiers
where `<host>` overrides the configured GitLab instance URL for that
invocation, targeting a self-hosted GitLab instance at `https://<host>`.
(Ubiquitous)

### FR-004

The system shall accept full GitLab URLs (`https://gitlab.com/<namespace>/<project>`
and `https://<host>/<namespace>/<project>`) and `git@<host>:<namespace>/<project>.git`
SSH URLs as repository identifiers, routing them to the GitLab API at the
detected host. (Event-driven)

### FR-005

The system shall accept bare `owner/repo` identifiers (no provider prefix)
and route them to the GitHub API, preserving backward compatibility with
existing `/reverse` usage. (Ubiquitous)

### FR-006

The system shall accept bare `https://github.com/owner/repo` and
`git@github.com:owner/repo.git` URLs (no provider prefix) and route them to
the GitHub API, preserving backward compatibility. (Ubiquitous)

### FR-007

When the repository identifier is prefixed `gitlab:` or resolves to a GitLab
URL, the system shall authenticate using the GitLab Personal Access Token
resolved from `GITLAB_TOKEN` env, `ragent.json`, or the encrypted database
via `/gitlab setup`, and shall surface a clear error message directing the
user to run `/gitlab setup` when no token is found. (Event-driven)

### FR-008

The system shall add `fetch_project_metadata`, `fetch_repository_tree`, and
`fetch_readme` async methods to `GitLabClient` that call the GitLab REST API
v4 endpoints `GET /projects/:id`, `GET /projects/:id/repository/tree`, and
`GET /projects/:id/readme` respectively, where `:id` is the URL-encoded
`namespace/project` path. (Ubiquitous)

### FR-009

The `fetch_project_metadata` method shall return a `RepoMetadata` struct
populated from the GitLab project JSON response, mapping the `description`,
`language` (from `languages` endpoint or empty), `topics` (from `topics`
array), `star_count` (to `stargazers_count`), and `default_branch` fields.
(Ubiquitous)

### FR-010

The `fetch_repository_tree` method shall return a `Vec<String>` of root-level
file and directory names from the GitLab `GET /projects/:id/repository/tree`
endpoint, extracting the `name` field from each entry in the response array.
(Ubiquitous)

### FR-011

The `fetch_readme` method shall return `Ok(None)` when the GitLab project has
no README (HTTP 404), and shall return `Ok(Some(text))` with the raw README
content fetched via the `readme_url` field or the
`GET /projects/:id/repository/files/:file_path/raw` endpoint on success.
(Event-driven)

### FR-012

The system shall define a `VcsProvider` enum with variants `GitHub` and
`GitLab` that carries the resolved provider, host (for self-hosted GitLab),
and parsed project path, and a `parse_reverse_repo` function that parses any
supported input format into a `VcsProvider` value. (Ubiquitous)

### FR-013

The `parse_reverse_repo` function shall reject identifiers that do not match
any supported format and return a human-readable error message listing all
accepted formats. (Unwanted)

### FR-014

The `handle_reverse_command` function shall dispatch to the GitHub or GitLab
fetch path based on the `VcsProvider` resolved from the parsed repo
identifier, replacing the current direct call to
`GitHubClient::validate_repo_input`. (Event-driven)

### FR-015

The system shall reuse the existing `build_reverse_prompt` function for both
GitHub and GitLab, passing the `RepoMetadata`, tree, README, and optional
tech constraint regardless of which provider supplied the data. (Ubiquitous)

### FR-016

The system shall include the provider name and host in the LLM context block
by extending `build_reverse_prompt` to accept an optional provider label
(e.g. "GitHub" or "GitLab (gitlab.example.com)") and emitting a
`## Repository Source` section before the existing `## Repository Metadata`
section. (Optional)

### FR-017

The system shall include the provider label in the user-facing status
messages and log entries (e.g. `⏳ reverse: gitlab:group/project…` instead of
`⏳ reverse: owner/repo…`). (Ubiquitous)

### FR-018

The system shall include the provider and host in the `--create` chaining
notice so the user can see which VCS provider was used when chaining into
`/spec create`. (Ubiquitous)

### FR-019

The system shall update the `/reverse help` message to document the
`github:` and `gitlab:` prefixes, the self-hosted `gitlab:<host>/…` form, full
URL formats for both providers, and the prerequisite that GitLab
repositories require `/gitlab setup` (or `GITLAB_TOKEN` + `GITLAB_URL` env
vars) before use. (Ubiquitous)

### FR-020

When a GitLab API call fails with HTTP 401, the system shall surface an error
message directing the user to run `/gitlab setup` to update their Personal
Access Token. (Unwanted)

### FR-021

When a GitLab API call fails with HTTP 404, the system shall surface an error
message stating that the project was not found on the target GitLab instance
and suggesting the user verify the namespace and project path. (Unwanted)

### FR-022

The system shall support GitLab nested namespaces (e.g.
`gitlab:group/subgroup/project`) by URL-encoding the full
`group/subgroup/project` path as the project `:id`, not just the last two
segments. (State-driven)

### FR-023

The system shall accept the `--tech` and `--create` flags for both GitHub
and GitLab repositories without modification to their parsing or behavior.
(Ubiquitous)

### FR-024

The `parse_reverse_args` function in `crates/ragent-tui/src/app/reverse.rs`
shall pass the raw `repo_input` string to the new `parse_reverse_repo`
function instead of calling `GitHubClient::validate_repo_input` directly, so
that provider dispatch happens in one place. (Event-driven)

### FR-025

The system shall accept a `--depth <N>` flag in the `/reverse` command, where
`N` is a positive integer controlling how many directory levels of the file
tree are fetched, defaulting to 1 when the flag is omitted. (Ubiquitous)

### FR-026

The system shall fetch only the root-level file tree when `--depth 1` is
specified or the flag is omitted, preserving the current behavior of the
existing `fetch_root_tree` and `fetch_repository_tree` methods. (State-driven)

### FR-027

The system shall fetch subdirectory contents recursively up to the specified
depth when `--depth <N>` (where N > 1) is specified, by calling the provider's
contents/tree endpoint for each directory discovered at the current level,
stopping when the requested depth is reached or no more directories exist.
(Event-driven)

### FR-028

The system shall format multi-level tree entries with path separators (e.g.
`src/`, `src/main.rs`, `src/models/user.rs`) so the LLM can infer the full
directory structure from the context block, and shall append a trailing
slash to directory names to distinguish them from files. (Ubiquitous)

### FR-029

The system shall reject `--depth` values that are zero, negative, or exceed
a maximum of 10, and shall surface a human-readable error message indicating
the valid range (1–10) without making any API calls. (Unwanted)