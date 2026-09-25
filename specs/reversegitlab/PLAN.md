# Implementation Plan: reversegitlab

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `fetch_project_metadata` to `GitLabClient` | FR-008, FR-009 | M | Critical | completed | — |
| T-002 | Add `fetch_repository_tree` to `GitLabClient` | FR-008, FR-010 | S | Critical | completed | — |
| T-003 | Add `fetch_readme` to `GitLabClient` | FR-008, FR-011 | M | Critical | completed | — |
| T-004 | Define `VcsProvider` enum and `parse_reverse_repo` function | FR-012, FR-013, FR-022 | L | Critical | completed | — |
| T-005 | Add `github:` prefix parsing to `parse_reverse_repo` (delegate to existing `GitHubClient::parse_repo_url`) | FR-001, FR-005, FR-006 | S | High | completed | T-004 |
| T-006 | Add `gitlab:` prefix parsing to `parse_reverse_repo` with configured-instance resolution | FR-002, FR-022 | M | Critical | completed | T-004 |
| T-007 | Add `gitlab:<host>/…` self-hosted parsing to `parse_reverse_repo` | FR-003 | S | High | completed | T-006 |
| T-008 | Add full GitLab URL and SSH URL parsing to `parse_reverse_repo` | FR-004 | M | High | completed | T-006 |
| T-009 | Update `parse_reverse_args` to delegate repo validation to `parse_reverse_repo` | FR-024 | S | Critical | completed | T-004 |
| T-010 | Update `handle_reverse_command` to dispatch fetch based on `VcsProvider` | FR-014 | L | Critical | completed | T-001, T-002, T-003, T-004 |
| T-011 | Add GitLab token validation and error path in `handle_reverse_command` | FR-007, FR-020, FR-021 | M | Critical | completed | T-010 |
| T-012 | Extend `build_reverse_prompt` with optional provider label and `## Repository Source` section | FR-016 | S | Medium | completed | — |
| T-013 | Update status messages and log entries to include provider label | FR-017 | S | Medium | completed | T-010 |
| T-014 | Update `--create` chaining notice to include provider label | FR-018 | S | Low | completed | T-010 |
| T-015 | Update `reverse_help_message` with `github:`, `gitlab:`, and `--depth` documentation | FR-019, FR-025 | S | High | completed | T-004, T-021 |
| T-016 | Verify the `/new` scaffold flags, `--create`, and `--depth` work with GitLab repos end-to-end | FR-023, FR-025 | S | High | completed | T-010, T-021 |
| T-017 | Add unit tests for `parse_reverse_repo` covering all input formats | FR-012, FR-013 | M | High | completed | T-004, T-005, T-006, T-007, T-008 |
| T-018 | Add unit tests for GitLab fetch methods with mock responses | FR-008, FR-009, FR-010, FR-011 | M | High | completed | T-001, T-002, T-003 |
| T-019 | Add unit tests for `build_reverse_prompt` with provider label | FR-015, FR-016 | S | Medium | completed | T-012 |
| T-020 | Verify backward compatibility: bare `owner/repo` and GitHub URLs still work | FR-005, FR-006 | S | Critical | completed | T-010 |
| T-021 | Add `--depth <N>` flag parsing to `parse_reverse_args` with default 1 and range validation (1–10) | FR-025, FR-026, FR-029 | S | High | completed | T-009 |
| T-022 | Add `fetch_tree_recursive` to `GitHubClient` — depth-controlled recursive contents fetch | FR-027, FR-028 | M | High | completed | T-010 |
| T-023 | Add depth-controlled recursive tree fetch to `GitLabClient` (path-based `repository/tree` calls) | FR-027, FR-028 | M | High | completed | T-002, T-010 |
| T-024 | Wire `--depth` value through `handle_reverse_command` dispatch to provider fetch calls | FR-025, FR-026, FR-027 | S | High | completed | T-021, T-022, T-023 |
| T-025 | Add unit tests for `--depth` parsing (default, valid, zero, negative, >10) | FR-025, FR-029 | S | Medium | completed | T-021 |
| T-026 | Add unit tests for recursive tree fetch with mocked directory structures | FR-027, FR-028 | M | Medium | completed | T-022, T-023 |