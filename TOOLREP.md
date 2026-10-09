# TOOLREP - dependencies per tool group

Generated report of the unique dependency crates used by each tool group in
the ragent tool layer. Groups map to the three tool crates that actually
register tools - `ragent-tools-core`, `ragent-tools-extended`, `ragent-tools-vcs`
- plus the agent-local tool modules in `ragent-agent/src/tool/`.

Crate lists are derived from the `use`/qualified-path references in each group's
source files. `ragent-*` entries are workspace crates; all others are external
(third-party) crates.

Tool counts are from `create_core_registry` / `create_extended_registry` /
`create_vcs_registry` / `create_default_registry`.

---

## ragent-tools-core :: File operations

**Tools:** apply_patch, read, write, create, edit, multiedit, patch, copy_file, move_file, rm, mkdir (make_directory), append_to_file, file_info, diff_files

**Files:** 20

**External crates (10):** anyhow, async-trait, chrono, lru, regex, serde, serde_json, similar, tokio, tracing
**Workspace crates (2):** ragent_config, ragent_tools_core

Files:
- `crates/ragent-tools-core/src/append_file.rs`
- `crates/ragent-tools-core/src/apply_patch.rs`
- `crates/ragent-tools-core/src/copy_file.rs`
- `crates/ragent-tools-core/src/create.rs`
- `crates/ragent-tools-core/src/diff.rs`
- `crates/ragent-tools-core/src/edit.rs`
- `crates/ragent-tools-core/src/edit_common.rs`
- `crates/ragent-tools-core/src/edit_log.rs`
- `crates/ragent-tools-core/src/file_info.rs`
- `crates/ragent-tools-core/src/file_lock.rs`
- `crates/ragent-tools-core/src/mkdir.rs`
- `crates/ragent-tools-core/src/move_file.rs`
- `crates/ragent-tools-core/src/multiedit.rs`
- `crates/ragent-tools-core/src/patch.rs`
- `crates/ragent-tools-core/src/path_util.rs`
- `crates/ragent-tools-core/src/read.rs`
- `crates/ragent-tools-core/src/replace.rs`
- `crates/ragent-tools-core/src/rm.rs`
- `crates/ragent-tools-core/src/truncate.rs`
- `crates/ragent-tools-core/src/write.rs`

## ragent-tools-core :: Search

**Tools:** glob, list, grep

**Files:** 3

**External crates (10):** anyhow, async-trait, globset, grep-regex, grep-searcher, ignore, rayon, serde_json, tokio, tracing

Files:
- `crates/ragent-tools-core/src/glob.rs`
- `crates/ragent-tools-core/src/grep.rs`
- `crates/ragent-tools-core/src/list.rs`

## ragent-tools-core :: Shell

**Tools:** bash, bash_reset, open

**Files:** 4

**External crates (8):** anyhow, async-trait, dirs, libc, serde_json, tokio, tracing, which
**Workspace crates (1):** ragent_config

Files:
- `crates/ragent-tools-core/src/askpass.rs`
- `crates/ragent-tools-core/src/bash.rs`
- `crates/ragent-tools-core/src/bash_reset.rs`
- `crates/ragent-tools-core/src/open.rs`

## ragent-tools-core :: Interaction

**Tools:** agent_complete, think

**Files:** 2

**External crates (3):** anyhow, async-trait, serde_json

Files:
- `crates/ragent-tools-core/src/agent_complete.rs`
- `crates/ragent-tools-core/src/think.rs`

## ragent-tools-core :: Utility

**Tools:** get_env, calculator, (xlsx helpers)

**Files:** 5

**External crates (7):** anyhow, async-trait, chrono, rust_xlsxwriter, serde, serde_json, tracing

Files:
- `crates/ragent-tools-core/src/calculator.rs`
- `crates/ragent-tools-core/src/cron_log.rs`
- `crates/ragent-tools-core/src/get_env.rs`
- `crates/ragent-tools-core/src/schema.rs`
- `crates/ragent-tools-core/src/xlsx.rs`

## ragent-tools-extended :: PDF & document extraction

**Tools:** pdf_read, pdf_write

**Files:** 4

**External crates (5):** anyhow, lopdf, pdf-extract, printpdf, serde_json
**Workspace crates (2):** ragent_tools_core, ragent_types

Files:
- `crates/ragent-tools-extended/src/document_extract.rs`
- `crates/ragent-tools-extended/src/pdf_common.rs`
- `crates/ragent-tools-extended/src/pdf_read.rs`
- `crates/ragent-tools-extended/src/pdf_write.rs`

## ragent-tools-extended :: Web

**Tools:** webfetch, websearch, http_request

**Files:** 3

**External crates (9):** anyhow, async-trait, futures, readability-rs, reqwest, serde, serde_json, tracing, url
**Workspace crates (1):** ragent_types

Files:
- `crates/ragent-tools-extended/src/http_request.rs`
- `crates/ragent-tools-extended/src/webfetch.rs`
- `crates/ragent-tools-extended/src/websearch.rs`

## ragent-tools-extended :: Task management

**Tools:** task_create, task_update, task_get, task_list

**Files:** 1

**External crates (4):** anyhow, async-trait, serde_json, uuid
**Workspace crates (1):** ragent_storage

Files:
- `crates/ragent-tools-extended/src/task.rs`

## ragent-tools-extended :: Code intelligence

**Tools:** codeindex_search, codeindex_status, codeindex_symbols, codeindex_references, codeindex_dependencies, codeindex_reindex, codeindex_godnodes, codeindex_path, codeindex_explain, codeindex_communities

**Files:** 11

**External crates (4):** anyhow, async-trait, serde_json, tokio
**Workspace crates (2):** ragent_codeindex, ragent_types

Files:
- `crates/ragent-tools-extended/src/codeindex_communities.rs`
- `crates/ragent-tools-extended/src/codeindex_dependencies.rs`
- `crates/ragent-tools-extended/src/codeindex_explain.rs`
- `crates/ragent-tools-extended/src/codeindex_godnodes.rs`
- `crates/ragent-tools-extended/src/codeindex_path.rs`
- `crates/ragent-tools-extended/src/codeindex_references.rs`
- `crates/ragent-tools-extended/src/codeindex_reindex.rs`
- `crates/ragent-tools-extended/src/codeindex_search.rs`
- `crates/ragent-tools-extended/src/codeindex_status.rs`
- `crates/ragent-tools-extended/src/codeindex_symbols.rs`
- `crates/ragent-tools-extended/src/codeindex_utils.rs`

## ragent-tools-extended :: Browser automation

**Tools:** browser

**Files:** 4

**External crates (9):** anyhow, async-trait, futures, serde, serde_json, thiserror, tokio, tokio-tungstenite, tracing
**Workspace crates (1):** ragent_types

Files:
- `crates/ragent-tools-extended/src/browser/actions.rs`
- `crates/ragent-tools-extended/src/browser/cdp.rs`
- `crates/ragent-tools-extended/src/browser/launch.rs`
- `crates/ragent-tools-extended/src/browser/mod.rs`

## ragent-tools-extended :: MasterFetch

**Tools:** mf_fetch, mf_crawl, mf_search, mf_screenshot, mf_cache_clear, mf_version

**Files:** 36

**External crates (20):** anyhow, async-trait, chrono, futures, html2text, lingua, lopdf, pdf-extract, quick-xml, readability-rs, regex, reqwest, rusqlite, rustc-hash, serde, serde_json, thiserror, tokio, tracing, url
**Workspace crates (3):** ragent_config, ragent_tools_extended, ragent_types

Files:
- `crates/ragent-tools-extended/src/masterfetch/cache.rs`
- `crates/ragent-tools-extended/src/masterfetch/crawl/classify.rs`
- `crates/ragent-tools-extended/src/masterfetch/crawl/mod.rs`
- `crates/ragent-tools-extended/src/masterfetch/crawl/orchestrator.rs`
- `crates/ragent-tools-extended/src/masterfetch/envelope.rs`
- `crates/ragent-tools-extended/src/masterfetch/extractor.rs`
- `crates/ragent-tools-extended/src/masterfetch/focus.rs`
- `crates/ragent-tools-extended/src/masterfetch/http.rs`
- `crates/ragent-tools-extended/src/masterfetch/language.rs`
- `crates/ragent-tools-extended/src/masterfetch/links.rs`
- `crates/ragent-tools-extended/src/masterfetch/metadata.rs`
- `crates/ragent-tools-extended/src/masterfetch/mod.rs`
- `crates/ragent-tools-extended/src/masterfetch/pdf.rs`
- `crates/ragent-tools-extended/src/masterfetch/robots.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/consensus.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/engine.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/exa.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/mod.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/perplexity.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/serper.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs`
- `crates/ragent-tools-extended/src/masterfetch/search/wikipedia.rs`
- `crates/ragent-tools-extended/src/masterfetch/security.rs`
- `crates/ragent-tools-extended/src/masterfetch/tests/inline/engine_tests.rs`
- `crates/ragent-tools-extended/src/masterfetch/tests/inline/mod_tests.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/cache_clear.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/crawl_tool.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/fetch.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/mod.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/screenshot.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/search_tool.rs`
- `crates/ragent-tools-extended/src/masterfetch/tools/version.rs`
- `crates/ragent-tools-extended/src/masterfetch/urlnorm.rs`
- `crates/ragent-tools-extended/src/masterfetch/youtube.rs`

## ragent-tools-extended :: Project scaffold (/new)

**Tools:** (ragent new / /new scaffolding - not a registry tool)

**Files:** 13

**External crates (4):** axum, reqwest, serde_json, tokio
**Workspace crates (1):** ragent_config

Files:
- `crates/ragent-tools-extended/src/project_scaffold/docs.rs`
- `crates/ragent-tools-extended/src/project_scaffold/emit.rs`
- `crates/ragent-tools-extended/src/project_scaffold/flags.rs`
- `crates/ragent-tools-extended/src/project_scaffold/gitinit.rs`
- `crates/ragent-tools-extended/src/project_scaffold/guard.rs`
- `crates/ragent-tools-extended/src/project_scaffold/help.rs`
- `crates/ragent-tools-extended/src/project_scaffold/layout.rs`
- `crates/ragent-tools-extended/src/project_scaffold/mod.rs`
- `crates/ragent-tools-extended/src/project_scaffold/recipes.rs`
- `crates/ragent-tools-extended/src/project_scaffold/remote.rs`
- `crates/ragent-tools-extended/src/project_scaffold/stack.rs`
- `crates/ragent-tools-extended/src/project_scaffold/summary.rs`
- `crates/ragent-tools-extended/src/project_scaffold/workspace.rs`

## ragent-tools-extended :: Archdoc

**Tools:** (archdoc document pipeline - internal support)

**Files:** 8

**External crates (6):** chrono, serde, serde_json, tokio, tracing, url

Files:
- `crates/ragent-tools-extended/src/archdoc/author_split.rs`
- `crates/ragent-tools-extended/src/archdoc/content_ref.rs`
- `crates/ragent-tools-extended/src/archdoc/extract.rs`
- `crates/ragent-tools-extended/src/archdoc/govcreate_run.rs`
- `crates/ragent-tools-extended/src/archdoc/local_source.rs`
- `crates/ragent-tools-extended/src/archdoc/mod.rs`
- `crates/ragent-tools-extended/src/archdoc/runner.rs`
- `crates/ragent-tools-extended/src/archdoc/url_source.rs`

## ragent-tools-extended :: Memory embedding

**Tools:** (embedding support for memory tools)

**Files:** 1

**External crates (1):** anyhow
**Workspace crates (2):** ragent_tools_extended, ragent_types

Files:
- `crates/ragent-tools-extended/src/memory/embedding.rs`

## ragent-tools-vcs :: Git (local)

**Tools:** git_status, git_log, git_diff, git_branch, git_show, git_remote, git_tag, git_add, git_reset, git_checkout, git_commit, git_stash, git_cherry_pick, git_push, git_pull, git_fetch, git_clone, git_merge

**Files:** 19

**External crates (4):** anyhow, async-trait, serde_json, tokio
**Workspace crates (1):** ragent_types

Files:
- `crates/ragent-tools-vcs/src/git/git_add.rs`
- `crates/ragent-tools-vcs/src/git/git_branch.rs`
- `crates/ragent-tools-vcs/src/git/git_checkout.rs`
- `crates/ragent-tools-vcs/src/git/git_cherry_pick.rs`
- `crates/ragent-tools-vcs/src/git/git_clone.rs`
- `crates/ragent-tools-vcs/src/git/git_commit.rs`
- `crates/ragent-tools-vcs/src/git/git_diff.rs`
- `crates/ragent-tools-vcs/src/git/git_fetch.rs`
- `crates/ragent-tools-vcs/src/git/git_log.rs`
- `crates/ragent-tools-vcs/src/git/git_merge.rs`
- `crates/ragent-tools-vcs/src/git/git_pull.rs`
- `crates/ragent-tools-vcs/src/git/git_push.rs`
- `crates/ragent-tools-vcs/src/git/git_remote.rs`
- `crates/ragent-tools-vcs/src/git/git_reset.rs`
- `crates/ragent-tools-vcs/src/git/git_show.rs`
- `crates/ragent-tools-vcs/src/git/git_stash.rs`
- `crates/ragent-tools-vcs/src/git/git_status.rs`
- `crates/ragent-tools-vcs/src/git/git_tag.rs`
- `crates/ragent-tools-vcs/src/git/mod.rs`

## ragent-tools-vcs :: GitHub

**Tools:** github_list_issues, github_get_issue, github_create_issue, github_comment_issue, github_close_issue, github_list_prs, github_get_pr, github_create_pr, github_merge_pr, github_review_pr, github_get_actions

**Files:** 7

**External crates (7):** anyhow, async-trait, reqwest, serde_json, tokio, tracing, zip
**Workspace crates (2):** ragent_config, ragent_types

Files:
- `crates/ragent-tools-vcs/src/github/auth.rs`
- `crates/ragent-tools-vcs/src/github/client.rs`
- `crates/ragent-tools-vcs/src/github/github_actions.rs`
- `crates/ragent-tools-vcs/src/github/github_issues.rs`
- `crates/ragent-tools-vcs/src/github/github_prs.rs`
- `crates/ragent-tools-vcs/src/github/helpers.rs`
- `crates/ragent-tools-vcs/src/github/mod.rs`

## ragent-tools-vcs :: GitLab

**Tools:** gitlab_list_issues, gitlab_get_issue, gitlab_create_issue, gitlab_comment_issue, gitlab_close_issue, gitlab_list_mrs, gitlab_get_mr, gitlab_create_mr, gitlab_merge_mr, gitlab_approve_mr, gitlab_list_pipelines, gitlab_get_pipeline, gitlab_list_jobs, gitlab_get_job, gitlab_get_job_log, gitlab_retry_job, gitlab_cancel_job, gitlab_retry_pipeline, gitlab_cancel_pipeline

**Files:** 7

**External crates (7):** anyhow, async-trait, reqwest, serde, serde_json, tokio, tracing
**Workspace crates (1):** ragent_config

Files:
- `crates/ragent-tools-vcs/src/gitlab/auth.rs`
- `crates/ragent-tools-vcs/src/gitlab/client.rs`
- `crates/ragent-tools-vcs/src/gitlab/gitlab_issues.rs`
- `crates/ragent-tools-vcs/src/gitlab/gitlab_mrs.rs`
- `crates/ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs`
- `crates/ragent-tools-vcs/src/gitlab/helpers.rs`
- `crates/ragent-tools-vcs/src/gitlab/mod.rs`

## ragent-tools-vcs :: Shared (http/limits/vocab)

**Tools:** (shared HTTP client + provider plumbing)

**Files:** 6

**External crates (6):** anyhow, async-trait, reqwest, serde, serde_json, tracing
**Workspace crates (3):** ragent_config, ragent_storage, ragent_types

Files:
- `crates/ragent-tools-vcs/src/http_client.rs`
- `crates/ragent-tools-vcs/src/lib.rs`
- `crates/ragent-tools-vcs/src/limits.rs`
- `crates/ragent-tools-vcs/src/percent.rs`
- `crates/ragent-tools-vcs/src/vcs_provider.rs`
- `crates/ragent-tools-vcs/src/vocab.rs`

## ragent-agent :: Planning

**Tools:** plan_enter, plan_exit

**Files:** 1

**External crates (3):** anyhow, async-trait, serde_json

Files:
- `crates/ragent-agent/src/tool/plan.rs`

## ragent-agent :: Sub-agents

**Tools:** new_agent, cancel_agent, list_agents, wait_agents

**Files:** 4

**External crates (6):** anyhow, async-trait, chrono, serde_json, tokio, tracing
**Workspace crates (1):** ragent_types

Files:
- `crates/ragent-agent/src/tool/cancel_agent.rs`
- `crates/ragent-agent/src/tool/list_agents.rs`
- `crates/ragent-agent/src/tool/new_agent.rs`
- `crates/ragent-agent/src/tool/wait_agents.rs`

## ragent-agent :: Memory & search

**Tools:** memory_store, memory_recall, memory_forget, conversation_search, session_search

**Files:** 3

**External crates (3):** anyhow, async-trait, serde_json
**Workspace crates (2):** ragent_storage, ragent_types

Files:
- `crates/ragent-agent/src/tool/conversation_search.rs`
- `crates/ragent-agent/src/tool/session_search.rs`
- `crates/ragent-agent/src/tool/structured_memory.rs`

## ragent-agent :: Teams

**Tools:** team_create, team_spawn, team_message, team_broadcast, team_read_messages, team_status, team_task_list, team_task_create, team_task_claim, team_task_complete, team_assign_task, team_submit_plan, team_approve_plan, team_wait, team_idle, team_shutdown_teammate, team_shutdown_ack, team_cleanup, team_memory_read, team_memory_write

**Files:** 20

**External crates (8):** anyhow, async-trait, chrono, futures, serde_json, tokio, tracing, uuid
**Workspace crates (1):** ragent_config

Files:
- `crates/ragent-agent/src/tool/team_approve_plan.rs`
- `crates/ragent-agent/src/tool/team_assign_task.rs`
- `crates/ragent-agent/src/tool/team_broadcast.rs`
- `crates/ragent-agent/src/tool/team_cleanup.rs`
- `crates/ragent-agent/src/tool/team_create.rs`
- `crates/ragent-agent/src/tool/team_idle.rs`
- `crates/ragent-agent/src/tool/team_memory_read.rs`
- `crates/ragent-agent/src/tool/team_memory_write.rs`
- `crates/ragent-agent/src/tool/team_message.rs`
- `crates/ragent-agent/src/tool/team_read_messages.rs`
- `crates/ragent-agent/src/tool/team_shutdown_ack.rs`
- `crates/ragent-agent/src/tool/team_shutdown_teammate.rs`
- `crates/ragent-agent/src/tool/team_spawn.rs`
- `crates/ragent-agent/src/tool/team_status.rs`
- `crates/ragent-agent/src/tool/team_submit_plan.rs`
- `crates/ragent-agent/src/tool/team_task_claim.rs`
- `crates/ragent-agent/src/tool/team_task_complete.rs`
- `crates/ragent-agent/src/tool/team_task_create.rs`
- `crates/ragent-agent/src/tool/team_task_list.rs`
- `crates/ragent-agent/src/tool/team_wait.rs`

## ragent-agent :: Spec management

**Tools:** spec_read, spec_list, spec_search, spec_task_update, spec_coverage

**Files:** 5

**External crates (5):** anyhow, async-trait, serde_json, tracing, uuid
**Workspace crates (2):** ragent_storage, ragent_types

Files:
- `crates/ragent-agent/src/tool/spec_coverage.rs`
- `crates/ragent-agent/src/tool/spec_list.rs`
- `crates/ragent-agent/src/tool/spec_read.rs`
- `crates/ragent-agent/src/tool/spec_search.rs`
- `crates/ragent-agent/src/tool/spec_task_update.rs`

## ragent-agent :: Cron scheduler

**Tools:** cron_add, cron_remove, cron_list, cron_enable, cron_disable

**Files:** 1

**External crates (5):** anyhow, async-trait, chrono, serde_json, tokio
**Workspace crates (2):** ragent_storage, ragent_types

Files:
- `crates/ragent-agent/src/tool/cron.rs`

## ragent-agent :: Background tasks

**Tools:** bg

**Files:** 1

**External crates (3):** anyhow, async-trait, serde_json

Files:
- `crates/ragent-agent/src/tool/bg.rs`

## ragent-agent :: Introspection & utility

**Tools:** model_info, ragent_info, os_info, tool_info, commands_info, initiative, skill_manage

**Files:** 8

**External crates (9):** anyhow, async-trait, chrono, serde, serde_json, sysinfo, tokio, tracing, uuid
**Workspace crates (3):** ragent_agent, ragent_config, ragent_types

Files:
- `crates/ragent-agent/src/tool/command_catalog.rs`
- `crates/ragent-agent/src/tool/initiative.rs`
- `crates/ragent-agent/src/tool/metadata.rs`
- `crates/ragent-agent/src/tool/model_info.rs`
- `crates/ragent-agent/src/tool/os_info.rs`
- `crates/ragent-agent/src/tool/ragent_info.rs`
- `crates/ragent-agent/src/tool/skill_manage.rs`
- `crates/ragent-agent/src/tool/tool_info.rs`

## ragent-agent :: Aliases

**Tools:** update_file, ask_user (alias layer)

**Files:** 1

**External crates (5):** anyhow, async-trait, serde_json, tokio, uuid
**Workspace crates (1):** ragent_tools_core

Files:
- `crates/ragent-agent/src/tool/aliases.rs`

---

## Summary

| Tool group | External crates | Workspace crates | Files |
| --- | ---: | ---: | ---: |
| ragent-tools-core :: File operations | 10 | 2 | 20 |
| ragent-tools-core :: Search | 10 | 0 | 3 |
| ragent-tools-core :: Shell | 8 | 1 | 4 |
| ragent-tools-core :: Interaction | 3 | 0 | 2 |
| ragent-tools-core :: Utility | 7 | 0 | 5 |
| ragent-tools-extended :: PDF & document extraction | 5 | 2 | 4 |
| ragent-tools-extended :: Web | 9 | 1 | 3 |
| ragent-tools-extended :: Task management | 4 | 1 | 1 |
| ragent-tools-extended :: Code intelligence | 4 | 2 | 11 |
| ragent-tools-extended :: Browser automation | 9 | 1 | 4 |
| ragent-tools-extended :: Gmail & messaging | 8 | 2 | 2 |
| ragent-tools-extended :: MasterFetch | 20 | 3 | 36 |
| ragent-tools-extended :: Project scaffold (/new) | 4 | 1 | 13 |
| ragent-tools-extended :: Archdoc | 6 | 0 | 8 |
| ragent-tools-extended :: Memory embedding | 1 | 2 | 1 |
| ragent-tools-vcs :: Git (local) | 4 | 1 | 19 |
| ragent-tools-vcs :: GitHub | 7 | 2 | 7 |
| ragent-tools-vcs :: GitLab | 7 | 1 | 7 |
| ragent-tools-vcs :: Shared (http/limits/vocab) | 6 | 3 | 6 |
| ragent-agent :: Planning | 3 | 0 | 1 |
| ragent-agent :: Sub-agents | 6 | 1 | 4 |
| ragent-agent :: Memory & search | 3 | 2 | 3 |
| ragent-agent :: Teams | 8 | 1 | 20 |
| ragent-agent :: Spec management | 5 | 2 | 5 |
| ragent-agent :: Cron scheduler | 5 | 2 | 1 |
| ragent-agent :: Background tasks | 3 | 0 | 1 |
| ragent-agent :: Introspection & utility | 9 | 3 | 8 |
| ragent-agent :: Aliases | 5 | 1 | 1 |
