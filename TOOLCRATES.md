# TOOLCRATES - third-party crates per tool

Generated report of the **external (third-party) crate count for every
registered tool** in the ragent tool layer, plus the crates that are
**unique to a single tool** (used by that one tool and no other).

`TOOLREP.md` groups tools and lists the crates a *group of files* mentions.
`TOOLCRATES.md` is finer-grained: it answers "how many third-party crates
does *this one tool* need?" by following the tool module's intra-crate
reference closure, and which of those crates no other tool pulls in.

## Method

1. Parse each tool crate's `lib.rs`/`registry.rs` `mod` tree to map every
   source file to its module path.
2. A file that contains `impl Tool for X` plus the `fn name(&self) -> "..."`
   string literal is the file that defines tool `name`.
3. Build the module's **intra-crate reference closure** - the module's own
   `crate::`/`super::`/`self::` references, transitively. This captures
   shared helpers (`guard`, `path_util`, `replace`, ...) the tool needs to
   compile.
4. From that closure, collect every external crate identifier used either as
   a `use <crate>::...` import or a path-qualified `crate::item` reference.
5. "External" = every non-`ragent-*` name in the workspace
   `[workspace.dependencies]` table (plus a few directly-declared crate
   deps such as `which`, `libc`, `zip`). `std`, `core`, and all
   `ragent-*` workspace crates are excluded.
6. A crate is **unique to a tool** when exactly one of the 112 tools
   references it. Shared crates (`anyhow`, `serde_json`, ...) are excluded
   by definition, so the "Unique crates" column isolates the domain crates
   a single tool owns.

### Caveats

* Counts are **compile-time reach**, not runtime behaviour: `anyhow` and
  `async-trait` appear for almost every tool because every tool impls the
  async `Tool` trait and returns `anyhow::Result`. The interesting signal is
  the *additional* domain crates (regex, reqwest, pdf-extract, ...).
* The intra-crate closure pulls in the crate root (`lib.rs`) for tools that
  `use super::...`, so a tool inherits the root module's crates too.
* Counts do not include *transitive* crates-of-crates (e.g. `reqwest` pulls
  hyper/tls). Only directly-named dependencies in the tool's source closure
  are listed.
* "Unique" is scoped to the **tool layer only** (these 112 tools). A crate
  marked unique here may still be used elsewhere in ragent, outside the tool
  crates.
* `multiedit` appears twice - once as the real tool in `ragent-tools-core`
  and once as `ExtractedCoreToolAdapter` in `ragent-agent`; both are listed.

---

## Summary

| Crate | Tools | Distinct external crates | Unique-to-tool crates | Min | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| ragent-tools-core | 24 | 18 | 7 | 3 | 9 |
| ragent-tools-extended | 22 | 22 | 5 | 3 | 19 |
| ragent-tools-vcs | 24 | 7 | 1 | 4 | 5 |
| ragent-agent | 42 | 11 | 0 | 3 | 7 |
| **All** | **112** | **35** | - | - | - |

Total distinct external crates referenced across all tools: **35**.

---

## ragent-tools-core

**Tools:** 24  
**Distinct external crates:** 18  
**Crates unique to one tool in this crate:** 7 (globset, grep-regex, grep-searcher, ignore, lru, rayon, similar)

**Crate-level deps:** see `ragent-tools-core/Cargo.toml`

| Tool | Tool class | # ext | Unique crates | External crates |
| --- | --- | ---: | --- | --- |
| `bash` | `bash.rs` | 9 | - | anyhow, async-trait, dirs, libc, serde, serde_json, tokio, tracing, which |
| `bash_reset` | `bash_reset.rs` | 9 | - | anyhow, async-trait, dirs, libc, serde, serde_json, tokio, tracing, which |
| `grep` | `grep.rs` | 9 | **grep-regex**, **grep-searcher**, **ignore** | anyhow, async-trait, grep-regex, grep-searcher, ignore, serde, serde_json, tokio, tracing |
| `apply_patch` | `apply_patch.rs` | 8 | - | anyhow, async-trait, chrono, regex, serde, serde_json, tokio, tracing |
| `edit` | `edit.rs` | 8 | - | anyhow, async-trait, chrono, regex, serde, serde_json, tokio, tracing |
| `glob` | `glob.rs` | 8 | **globset**, **rayon** | anyhow, async-trait, globset, rayon, serde, serde_json, tokio, tracing |
| `multi_edit` | `multiedit.rs` | 8 | - | anyhow, async-trait, chrono, regex, serde, serde_json, tokio, tracing |
| `diff_files` | `diff.rs` | 7 | **similar** | anyhow, async-trait, serde, serde_json, similar, tokio, tracing |
| `file_info` | `file_info.rs` | 7 | - | anyhow, async-trait, chrono, serde, serde_json, tokio, tracing |
| `read` | `read.rs` | 7 | **lru** | anyhow, async-trait, lru, serde, serde_json, tokio, tracing |
| `append_to_file` | `append_file.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `copy_file` | `copy_file.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `create` | `create.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `list` | `list.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `make_directory` | `mkdir.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `move_file` | `move_file.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `open` | `open.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `patch` | `patch.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `rm` | `rm.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `write` | `write.rs` | 6 | - | anyhow, async-trait, serde, serde_json, tokio, tracing |
| `agent_complete` | `agent_complete.rs` | 5 | - | anyhow, async-trait, serde, serde_json, tracing |
| `think` | `think.rs` | 5 | - | anyhow, async-trait, serde, serde_json, tracing |
| `calculator` | `calculator.rs` | 3 | - | anyhow, async-trait, serde_json |
| `get_env` | `get_env.rs` | 3 | - | anyhow, async-trait, serde_json |

## ragent-tools-extended

**Tools:** 22  
**Distinct external crates:** 22  
**Crates unique to one tool in this crate:** 5 (lingua, printpdf, quick-xml, rusqlite, rustc-hash)

**Crate-level deps:** see `ragent-tools-extended/Cargo.toml`

| Tool | Tool class | # ext | Unique crates | External crates |
| --- | --- | ---: | --- | --- |
| `mf_fetch` | `fetch.rs` | 19 | **lingua**, **quick-xml**, **rusqlite** | anyhow, async-trait, chrono, futures, html2text, lingua, lopdf, pdf-extract, quick-xml, readability, regex, reqwest, rusqlite, serde, serde_json, thiserror, tokio, tracing, url |
| `mf_crawl` | `crawl_tool.rs` | 12 | - | anyhow, async-trait, chrono, html2text, readability, regex, reqwest, serde, serde_json, thiserror, tracing, url |
| `mf_search` | `search_tool.rs` | 10 | **rustc-hash** | anyhow, async-trait, futures, reqwest, rustc-hash, serde, serde_json, thiserror, tokio, tracing |
| `webfetch` | `webfetch.rs` | 9 | - | anyhow, async-trait, futures, readability, reqwest, serde, serde_json, tracing, url |
| `pdf_read` | `pdf_read.rs` | 7 | - | anyhow, async-trait, lopdf, pdf-extract, serde_json, tokio, tracing |
| `http_request` | `http_request.rs` | 5 | - | anyhow, async-trait, reqwest, serde, serde_json |
| `pdf_write` | `pdf_write.rs` | 5 | **printpdf** | anyhow, async-trait, printpdf, serde_json, tokio |
| `task_create` | `task.rs` | 5 | - | anyhow, async-trait, serde, serde_json, uuid |
| `codeindex_communities` | `codeindex_communities.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_dependencies` | `codeindex_dependencies.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_explain` | `codeindex_explain.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_godnodes` | `codeindex_godnodes.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_path` | `codeindex_path.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_references` | `codeindex_references.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_reindex` | `codeindex_reindex.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_search` | `codeindex_search.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_status` | `codeindex_status.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `codeindex_symbols` | `codeindex_symbols.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `mf_version` | `version.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `websearch` | `websearch.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `mf_cache_clear` | `cache_clear.rs` | 3 | - | anyhow, async-trait, serde_json |
| `mf_screenshot` | `screenshot.rs` | 3 | - | anyhow, async-trait, serde_json |

## ragent-tools-vcs

**Tools:** 24  
**Distinct external crates:** 7  
**Crates unique to one tool in this crate:** 1 (zip)

**Crate-level deps:** see `ragent-tools-vcs/Cargo.toml`

| Tool | Tool class | # ext | Unique crates | External crates |
| --- | --- | ---: | --- | --- |
| `github_get_actions` | `github_actions.rs` | 5 | **zip** | anyhow, async-trait, serde, serde_json, zip |
| `github_list_prs` | `github_prs.rs` | 5 | - | anyhow, async-trait, serde, serde_json, tokio |
| `gitlab_list_mrs` | `gitlab_mrs.rs` | 5 | - | anyhow, async-trait, serde, serde_json, tokio |
| `gitlab_list_pipelines` | `gitlab_pipelines.rs` | 5 | - | anyhow, async-trait, serde, serde_json, tracing |
| `git_add` | `git_add.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_branch` | `git_branch.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_checkout` | `git_checkout.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_cherry_pick` | `git_cherry_pick.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_clone` | `git_clone.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_commit` | `git_commit.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_diff` | `git_diff.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_fetch` | `git_fetch.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_log` | `git_log.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_merge` | `git_merge.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_pull` | `git_pull.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_push` | `git_push.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_remote` | `git_remote.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_reset` | `git_reset.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_show` | `git_show.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_stash` | `git_stash.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_status` | `git_status.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `git_tag` | `git_tag.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `github_list_issues` | `github_issues.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `gitlab_list_issues` | `gitlab_issues.rs` | 4 | - | anyhow, async-trait, serde, serde_json |

## ragent-agent

**Tools:** 42  
**Distinct external crates:** 11  
**Crates unique to one tool in this crate:** 0

**Crate-level deps:** see `ragent-agent/Cargo.toml`

| Tool | Tool class | # ext | Unique crates | External crates |
| --- | --- | ---: | --- | --- |
| `initiative` | `initiative.rs` | 7 | - | anyhow, async-trait, chrono, serde_json, tokio, tracing, uuid |
| `update_file` | `aliases.rs` | 7 | - | anyhow, async-trait, parking-lot, serde, serde_json, tokio, uuid |
| `multiedit` | `mod.rs` | 6 | - | anyhow, async-trait, parking-lot, serde, serde_json, tokio |
| `ragent_info` | `ragent_info.rs` | 6 | - | anyhow, async-trait, chrono, serde, serde_json, sysinfo |
| `team_spawn` | `team_spawn.rs` | 6 | - | anyhow, async-trait, serde_json, tokio, tracing, uuid |
| `cron_add` | `cron.rs` | 5 | - | anyhow, async-trait, chrono, serde_json, tokio |
| `os_info` | `os_info.rs` | 5 | - | anyhow, async-trait, chrono, serde_json, sysinfo |
| `spec_task_update` | `spec_task_update.rs` | 5 | - | anyhow, async-trait, serde_json, tracing, uuid |
| `team_create` | `team_create.rs` | 5 | - | anyhow, async-trait, chrono, serde_json, tracing |
| `team_shutdown_teammate` | `team_shutdown_teammate.rs` | 5 | - | anyhow, async-trait, serde_json, tracing, uuid |
| `team_wait` | `team_wait.rs` | 5 | - | anyhow, async-trait, serde_json, tokio, tracing |
| `wait_agents` | `wait_agents.rs` | 5 | - | anyhow, async-trait, serde_json, tokio, tracing |
| `list_agents` | `list_agents.rs` | 4 | - | anyhow, async-trait, chrono, serde_json |
| `model_info` | `model_info.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `team_assign_task` | `team_assign_task.rs` | 4 | - | anyhow, async-trait, chrono, serde_json |
| `team_broadcast` | `team_broadcast.rs` | 4 | - | anyhow, async-trait, futures, serde_json |
| `team_read_messages` | `team_read_messages.rs` | 4 | - | anyhow, async-trait, serde_json, tracing |
| `team_submit_plan` | `team_submit_plan.rs` | 4 | - | anyhow, async-trait, serde_json, uuid |
| `team_task_claim` | `team_task_claim.rs` | 4 | - | anyhow, async-trait, serde_json, tracing |
| `team_task_complete` | `team_task_complete.rs` | 4 | - | anyhow, async-trait, serde_json, tracing |
| `tool_info` | `tool_info.rs` | 4 | - | anyhow, async-trait, serde, serde_json |
| `cancel_agent` | `cancel_agent.rs` | 3 | - | anyhow, async-trait, serde_json |
| `conversation_search` | `conversation_search.rs` | 3 | - | anyhow, async-trait, serde_json |
| `memory_store` | `structured_memory.rs` | 3 | - | anyhow, async-trait, serde_json |
| `new_agent` | `new_agent.rs` | 3 | - | anyhow, async-trait, serde_json |
| `plan_enter` | `plan.rs` | 3 | - | anyhow, async-trait, serde_json |
| `session_search` | `session_search.rs` | 3 | - | anyhow, async-trait, serde_json |
| `skill_manage` | `skill_manage.rs` | 3 | - | anyhow, async-trait, serde_json |
| `spec_coverage` | `spec_coverage.rs` | 3 | - | anyhow, async-trait, serde_json |
| `spec_list` | `spec_list.rs` | 3 | - | anyhow, async-trait, serde_json |
| `spec_read` | `spec_read.rs` | 3 | - | anyhow, async-trait, serde_json |
| `spec_search` | `spec_search.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_approve_plan` | `team_approve_plan.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_cleanup` | `team_cleanup.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_idle` | `team_idle.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_memory_read` | `team_memory_read.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_memory_write` | `team_memory_write.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_message` | `team_message.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_shutdown_ack` | `team_shutdown_ack.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_status` | `team_status.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_task_create` | `team_task_create.rs` | 3 | - | anyhow, async-trait, serde_json |
| `team_task_list` | `team_task_list.rs` | 3 | - | anyhow, async-trait, serde_json |

---

## Appendix A - crates used by exactly one tool

| Unique crate | Sole tool | Crate |
| --- | --- | --- |
| `globset` | `glob` | ragent-tools-core |
| `grep-regex` | `grep` | ragent-tools-core |
| `grep-searcher` | `grep` | ragent-tools-core |
| `ignore` | `grep` | ragent-tools-core |
| `lingua` | `mf_fetch` | ragent-tools-extended |
| `lru` | `read` | ragent-tools-core |
| `printpdf` | `pdf_write` | ragent-tools-extended |
| `quick-xml` | `mf_fetch` | ragent-tools-extended |
| `rayon` | `glob` | ragent-tools-core |
| `rusqlite` | `mf_fetch` | ragent-tools-extended |
| `rustc-hash` | `mf_search` | ragent-tools-extended |
| `similar` | `diff_files` | ragent-tools-core |
| `zip` | `github_get_actions` | ragent-tools-vcs |

---

## Appendix B - distinct external crates across the tool layer

| External crate | Tools using it |
| --- | ---: |
| `anyhow` | 112 |
| `async-trait` | 112 |
| `serde_json` | 112 |
| `serde` | 69 |
| `tracing` | 38 |
| `tokio` | 33 |
| `chrono` | 13 |
| `uuid` | 7 |
| `regex` | 5 |
| `reqwest` | 5 |
| `futures` | 4 |
| `readability` | 3 |
| `thiserror` | 3 |
| `url` | 3 |
| `dirs` | 2 |
| `html2text` | 2 |
| `libc` | 2 |
| `lopdf` | 2 |
| `parking-lot` | 2 |
| `pdf-extract` | 2 |
| `sysinfo` | 2 |
| `which` | 2 |
| `globset` | 1 |
| `grep-regex` | 1 |
| `grep-searcher` | 1 |
| `ignore` | 1 |
| `lingua` | 1 |
| `lru` | 1 |
| `printpdf` | 1 |
| `quick-xml` | 1 |
| `rayon` | 1 |
| `rusqlite` | 1 |
| `rustc-hash` | 1 |
| `similar` | 1 |
| `zip` | 1 |

