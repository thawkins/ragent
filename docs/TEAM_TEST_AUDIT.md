# Team Test-Coverage Audit — Journal-Removal Sprint

**Auditor:** test-reviewer (tm-002)  
**Date:** 2025-01-17  
**Scope:** All `ragent-team`, `ragent-tui`, `ragent-server` crates related to team workflows.  
**Goal:** Identify missing edge cases, flaky patterns, and untested critical paths. Provide exact test names to add.

---

## Executive Summary

| Layer | Tests Found | Critical Gaps | Risk Level |
|---|---|---|---|
| TUI integration (`test_teams_tui.rs`) | ~45 tests, 45.6 KB | Race-condition helpers, swarm panel, blueprints | Medium |
| ragent-team tools (17 tools) | **0** | Every tool execution path | **High** |
| ragent-team core (task/mailbox/store/manager/swarm/config) | **0** | Lock races, corruption, timeouts | **High** |
| Server REST API for teams | **0** | Auth, CRUD, SSE events | **High** |
| SSE event → name mapping | 6 tests | Missing `JournalEntryCreated`/`JournalSearched` removal coverage | Low |

**Total uncovered lines in ragent-team/src:** ~3,200 lines of logic with zero unit tests.

---

## 1. ragent-team Tool Tests (High Priority)

All 17 tools in `crates/ragent-team/src/tools/` are **completely untested** via unit/integration tests.  
Below are the exact test names and scenarios to add, grouped by tool.

### 1.1 `team_create` (`team_create.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 1 | `test_team_create_missing_blueprint_rejected` | Empty / missing `blueprint` param → error |
| 2 | `test_team_create_generates_name_from_blueprint` | Omit `name` → auto-generated with timestamp |
| 3 | `test_team_create_duplicate_name_loads_existing` | Team exists → load and seed blueprint |
| 4 | `test_team_create_project_local_true` | Creates under `[PROJECT]/.ragent/teams/` |
| 5 | `test_team_create_project_local_false` | Creates under `~/.ragent/teams/` |
| 6 | `test_team_create_work_context_injected` | `context` param prepended to spawn prompts |
| 7 | `test_team_create_hook_teammate_spawned_fires` | Blueprint seeding fires `HookEvent::TeammateSpawned` |

### 1.2 `team_spawn` (`team_spawn.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 8 | `test_team_spawn_rejects_multi_item_prompt` | 3+ numbered items → error with guidance |
| 9 | `test_team_spawn_accepts_single_item_prompt` | 1 item → success |
| 10 | `test_team_spawn_allows_and_connective` | "A and B" (2 items, not 3+) → allowed |
| 11 | `test_team_spawn_missing_required_params` | Missing `team_name`/`teammate_name`/`agent_type`/`prompt` → error |
| 12 | `test_team_spawn_publishes_teammate_spawned_event` | EventBus receives `TeammateSpawned` |
| 13 | `test_team_spawn_task_id_pre_claims_task` | `task_id` provided → task claimed on behalf |

### 1.3 `team_wait` (`team_wait.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 14 | `test_team_wait_no_teams_found` | Working dir has no teams → error |
| 15 | `test_team_wait_specific_agent_ids` | `agent_ids` array → waits only for subset |
| 16 | `test_team_wait_timeout_returns_partial` | Exceeds `timeout_secs` → partial result |
| 17 | `test_team_wait_all_members_idle` | All non-failed idle → success summary |
| 18 | `test_team_wait_failed_members_excluded` | `Failed` status members skipped |
| 19 | `test_team_wait_resolved_team_name_from_disk` | No `team_name` param → most recent team |

### 1.4 `team_message` (`team_message.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 20 | `test_team_message_resolves_name_to_agent_id` | Name lookup in `config.json` |
| 21 | `test_team_message_lead_literal_passthrough` | `to: "lead"` → no resolution needed |
| 22 | `test_team_message_unknown_name_fails` | Name not in config → error |
| 23 | `test_team_message_writes_to_mailbox` | Mailbox file contains message after call |
| 24 | `test_team_message_from_lead_when_no_team_context` | Default `from` = `"lead"` |

### 1.5 `team_broadcast` (`team_broadcast.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 25 | `test_team_broadcast_skips_stopped_members` | `MemberStatus::Stopped` excluded |
| 26 | `test_team_broadcast_counts_active_only` | 3 active + 2 stopped → `sent=3` |
| 27 | `test_team_broadcast_empty_team_ok` | Zero active → `sent=0` |

### 1.6 `team_idle` (`team_idle.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 28 | `test_team_idle_blocked_when_in_progress_task` | Agent has `InProgress` task → rejection |
| 29 | `test_team_idle_hook_rejected_returns_feedback` | Hook returns `Feedback` → stay Working |
| 30 | `test_team_idle_hook_success_sets_idle` | Hook returns `Ok` → `MemberStatus::Idle` |
| 31 | `test_team_idle_no_team_context_uses_session_id` | `agent_id` falls back to `ctx.session_id` |

### 1.7 `team_shutdown_teammate` (`team_shutdown_teammate.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 32 | `test_team_shutdown_sets_shutting_down_status` | Member status updated |
| 33 | `test_team_shutdown_sends_shutdown_request_mailbox` | Mailbox contains `ShutdownRequest` message |
| 34 | `test_team_shutdown_unknown_teammate_fails` | `resolve_agent_id` returns error |

### 1.8 `team_shutdown_ack` (`team_shutdown_ack.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 35 | `test_team_shutdown_ack_sets_stopped_status` | Member → `Stopped` |
| 36 | `test_team_shutdown_ack_publishes_team_cleaned_up` | EventBus `TeamCleanedUp` fired |

### 1.9 `team_task_create` (`team_task_create.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 37 | `test_team_task_create_assigns_auto_increment_id` | `next_task_id()` → `task-001`, `task-002` … |
| 38 | `test_team_task_create_runs_hook_with_stdin` | `HookEvent::TaskCreated` with JSON stdin |
| 39 | `test_team_task_create_missing_title_fails` | Required param missing |

### 1.10 `team_task_claim` (`team_task_claim.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 40 | `test_team_task_claim_next_available` | First `Pending` with satisfied deps |
| 41 | `test_team_task_claim_specific_task_id` | `task_id` param → claim exact task |
| 42 | `test_team_task_claim_already_in_progress_returns_same` | Same agent re-claims → `(task, true)` |
| 43 | `test_team_task_claim_blocked_by_unsatisfied_dependency` | Dep not completed → skip to next |
| 44 | `test_team_task_claim_race_condition_safe` | Two agents claim simultaneously → only one wins |

### 1.11 `team_task_complete` (`team_task_complete.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 45 | `test_team_task_complete_sets_completed_status` | `Completed` + timestamp |
| 46 | `test_team_task_complete_wrong_agent_rejected` | Task assigned to `tm-001`, `tm-002` completes → error |
| 47 | `test_team_task_complete_unblocks_dependent_tasks` | Dependent tasks become claimable |
| 48 | `test_team_task_complete_missing_task_id_fails` | Required param missing |

### 1.12 `team_task_list` (`team_task_list.rs`)

| # | Test Name | Scenario |
|---|---|---|
| 49 | `test_team_task_list_empty_team` | No tasks → empty table |
| 50 | `test_team_task_list_shows_all_statuses` | Pending / InProgress / Completed / Cancelled |

### 1.13 `team_approve_plan` / `team_submit_plan` / `team_assign_task`

| # | Test Name | Scenario |
|---|---|---|
| 51 | `test_team_submit_plan_writes_plan_request_mailbox` | `MessageType::PlanRequest` appended |
| 52 | `test_team_approve_plan_updates_plan_status_approved` | `PlanStatus::Approved` |
| 53 | `test_team_approve_plan_rejected_updates_plan_status` | `PlanStatus::Rejected` |
| 54 | `test_team_assign_task_sets_assigned_to` | Task `assigned_to` updated |

### 1.14 `team_status` / `team_read_messages`

| # | Test Name | Scenario |
|---|---|---|
| 55 | `test_team_status_returns_member_list` | JSON with members, statuses, tasks |
| 56 | `test_team_read_messages_returns_unread_only` | `read=false` messages returned, then marked read |
| 57 | `test_team_read_messages_empty_mailbox_ok` | No messages → empty array |

### 1.15 `team_cleanup`

| # | Test Name | Scenario |
|---|---|---|
| 58 | `test_team_cleanup_removes_team_dir` | Disk state deleted |
| 59 | `test_team_cleanup_blocked_when_teammates_working` | Any `Working` member → error |

---

## 2. ragent-team Core Module Tests (High Priority)

### 2.1 `task.rs` — `TaskStore` file-locking & claim logic

**Current state:** Zero tests. This is the most critical module for correctness because it uses `flock` for cross-process safety.

| # | Test Name | Scenario |
|---|---|---|
| 60 | `test_task_store_open_creates_file` | `TaskStore::open` → `tasks.json` exists |
| 61 | `test_task_store_read_empty_returns_default` | Empty file → `TaskList::default()` |
| 62 | `test_task_store_read_corrupted_json_fails` | Invalid JSON → parse error |
| 63 | `test_task_store_add_task_appends` | `add_task` → task in list |
| 64 | `test_task_store_claim_next_basic` | One pending task → claimed, status `InProgress` |
| 65 | `test_task_store_claim_next_respects_dependencies` | Task with unsatisfied dep → skipped |
| 66 | `test_task_store_claim_next_already_in_progress` | Re-claim by same agent → returns same task, `already_had=true` |
| 67 | `test_task_store_claim_specific_success` | Exact ID claimed |
| 68 | `test_task_store_claim_specific_wrong_agent_blocked` | Task in-progress by other → error |
| 69 | `test_task_store_complete_success` | Task → `Completed`, `completed_at` set |
| 70 | `test_task_store_complete_wrong_agent_fails` | Different agent → error |
| 71 | `test_task_store_concurrent_claims_only_one_wins` | Spawn two threads, both claim → one gets task, other gets `None` |
| 72 | `test_task_store_concurrent_complete_unblocks_dependent` | Complete task A, simultaneously claim dependent B → B becomes claimable |

**Flaky-pattern warning:** The existing TUI tests (`test_team_tasks_renders_table_with_status`) exercise `TaskStore` indirectly via `TeamStore`, but do not verify the `flock` serialization. The concurrent tests above must use real OS processes (not just threads) to validate `flock` semantics across `std::process::Command`.

### 2.2 `mailbox.rs` — `Mailbox` read/write/notifier

| # | Test Name | Scenario |
|---|---|---|
| 73 | `test_mailbox_open_creates_file` | `Mailbox::open` → `{agent_id}.json` exists |
| 74 | `test_mailbox_read_all_empty` | New mailbox → empty vec |
| 75 | `test_mailbox_push_appends_message` | Push 1 message → `read_all` returns 1 |
| 76 | `test_mailbox_push_multiple_preserved_order` | Push A, B, C → order preserved |
| 77 | `test_mailbox_push_signals_notifier` | `register_notifier` + `push` → `Notify` triggered |
| 78 | `test_mailbox_corrupted_json_fails` | Invalid JSON in mailbox → parse error |
| 79 | `test_mailbox_concurrent_push_safe` | Two processes push → no data loss |

### 2.3 `store.rs` — `TeamStore` CRUD & discovery

| # | Test Name | Scenario |
|---|---|---|
| 80 | `test_team_store_create_writes_config_json` | `create` → `config.json` valid |
| 81 | `test_team_store_load_reads_config_json` | Round-trip create + load |
| 82 | `test_team_store_save_atomic_rename` | `save` uses temp + rename |
| 83 | `test_team_store_list_teams_dedupes_project_over_global` | Same name in both dirs → project wins |
| 84 | `test_team_store_list_teams_sorted_by_mtime` | Returns most-recent first |
| 85 | `test_team_store_add_member_persists` | `add_member` → `config.json` updated |
| 86 | `test_team_store_next_task_id_increments` | `task-001` → `task-002` |
| 87 | `test_team_store_find_team_dir_project_priority` | Both exist → project returned |

### 2.4 `manager.rs` — `TeamManager` spawn & lifecycle

| # | Test Name | Scenario |
|---|---|---|
| 88 | `test_team_manager_spawn_teammate_starts_session` | `spawn_teammate` → session created in storage |
| 89 | `test_team_manager_shutdown_teammate_sends_event` | `shutdown_teammate` → `TeammateShutdown` event |
| 90 | `test_is_token_overflow_error_detects_anthropic` | `"prompt token count … exceeds"` → `true` |
| 91 | `test_is_token_overflow_error_detects_openai` | `"context_length_exceeded"` → `true` |
| 92 | `test_is_permanent_api_error_4xx` | `"HTTP 404:"` → `true` |
| 93 | `test_is_permanent_api_error_excludes_429` | `"HTTP 429:"` → `false` |
| 94 | `test_compact_teammate_session_replaces_history` | History compacted to summary |

### 2.5 `swarm.rs` — Decomposition parsing

| # | Test Name | Scenario |
|---|---|---|
| 95 | `test_parse_decomposition_strips_markdown_fences` | ` ```json … ``` ` → parsed |
| 96 | `test_parse_decomposition_strips_trailing_commas` | JSON with trailing comma → parsed |
| 97 | `test_parse_decomposition_invalid_json_fails` | Non-JSON → error |
| 98 | `test_build_decomposition_user_prompt_includes_goal` | Goal string in prompt |

### 2.6 `config.rs` — `TeamConfig` serialization

| # | Test Name | Scenario |
|---|---|---|
| 99 | `test_team_config_serde_roundtrip` | Serialize + deserialize |
| 100 | `test_member_status_serde_lowercase` | `"working"` → `MemberStatus::Working` |
| 101 | `test_plan_status_serde_lowercase` | `"approved"` → `PlanStatus::Approved` |
| 102 | `test_resolve_memory_dir_project_local` | Returns project `.ragent/memory/` |
| 103 | `test_resolve_memory_dir_global` | Returns `~/.ragent/memory/` |

---

## 3. Server REST API Tests (High Priority)

There are **zero** tests for team REST endpoints. The server tests only cover memory endpoints (`test_memory_api.rs`) and auth (`test_integration.rs`).

Suggested new file: `crates/ragent-server/tests/test_team_api.rs`

| # | Test Name | Endpoint | Scenario |
|---|---|---|---|
| 104 | `test_team_create_requires_auth` | `POST /teams` | No token → 401 |
| 105 | `test_team_create_success` | `POST /teams` | Valid token + body → 201, team dir exists |
| 106 | `test_team_list_requires_auth` | `GET /teams` | No token → 401 |
| 107 | `test_team_list_returns_teams` | `GET /teams` | 200 with team array |
| 108 | `test_team_get_requires_auth` | `GET /teams/{name}` | No token → 401 |
| 109 | `test_team_get_not_found` | `GET /teams/{name}` | Unknown team → 404 |
| 110 | `test_team_get_returns_members` | `GET /teams/{name}` | 200 with member list |
| 111 | `test_team_tasks_requires_auth` | `GET /teams/{name}/tasks` | No token → 401 |
| 112 | `test_team_tasks_returns_task_list` | `GET /teams/{name}/tasks` | 200 with task table |
| 113 | `test_team_delete_requires_auth` | `DELETE /teams/{name}` | No token → 401 |
| 114 | `test_team_delete_removes_dir` | `DELETE /teams/{name}` | 204, dir gone |
| 115 | `test_team_delete_active_blocked` | `DELETE /teams/{name}` | Working members → 409 |

---

## 4. SSE Event Tests (Low Priority — mostly covered)

`test_event_to_sse.rs` already maps 6 team-related events. Missing:

| # | Test Name | Event | Why Needed |
|---|---|---|---|
| 116 | `test_journal_entry_created_removed` | `Event::JournalEntryCreated` | Verify removal doesn't break SSE dispatch |
| 117 | `test_journal_searched_removed` | `Event::JournalSearched` | Verify removal doesn't break SSE dispatch |

*(These are relevant because the journal-removal team is also deleting journal events; the SSE dispatch table in `sse.rs` must still compile and dispatch correctly after removal.)*

---

## 5. TUI Integration — Missing Edge Cases (Medium Priority)

The existing `test_teams_tui.rs` is solid (45+ tests) but has gaps:

| # | Test Name | Scenario |
|---|---|---|
| 118 | `test_team_create_with_blueprint_spawns_auto` | Blueprint provided → teammates auto-spawned |
| 119 | `test_team_create_no_ragent_dir_creates_it` | `/team create` when `.ragent/` missing → auto-create |
| 120 | `test_team_message_to_lead_from_teammate` | `/team message lead hello` from teammate context |
| 121 | `test_team_delete_nonexistent_team` | `/team delete ghost` → error |
| 122 | `test_team_clear_preserves_team_dir` | `/team clear` removes tasks.json but keeps team dir |
| 123 | `test_team_tasks_renders_cancelled_status` | Task status `Cancelled` visible in table |
| 124 | `test_team_panel_updates_on_teammate_message_event` | `TeammateMessage` event increments sent/recv counters |
| 125 | `test_team_panel_closes_on_escape_key` | `Esc` when `show_teams_window=true` → closes |
| 126 | `test_team_swarm_command_renders_decomposition` | `/team swarm <goal>` → decomposition table |
| 127 | `test_team_swarm_no_goal_shows_usage` | `/team swarm` → usage hint |
| 128 | `test_team_open_existing_team` | `/team open <name>` → loads from disk |
| 129 | `test_team_open_nonexistent_team` | `/team open ghost` → error |

---

## 6. Flaky Patterns Detected

### 6.1 CWD-sensitive tests
Several tests in `test_teams_tui.rs` mutate `std::env::set_current_dir()` but use a `Mutex<()>` lock (`CWD_LOCK`). This is **process-wide** and serializes tests, but:
- **Risk:** If a test panics while holding the lock, subsequent tests may deadlock or run with wrong CWD.
- **Fix:** Wrap each CWD test in a `scopeguard` or `defer!` that restores CWD even on panic. The existing `CwdGuard` pattern in `test_tools_visibility_command.rs` is better and should be adopted.

### 6.2 Time-dependent assertions
`unique_team_name()` uses `SystemTime::now().as_nanos()`. Under extreme parallel execution, nanosecond collisions are theoretically possible (though unlikely).  
- **Fix:** Append a random `uuid` or thread-local counter instead.

### 6.3 File-lock tests not using real processes
The `TaskStore` `flock` mechanism is only tested indirectly via single-threaded TUI tests.  
- **Fix:** Add cross-process claim-race tests using `std::process::Command` to spawn two `cargo test` sub-processes that compete for the same `tasks.json`.

### 6.4 `TempDir` dropped before assertions complete
Some tests set `original_dir` after creating the temp dir, but if an assertion fails mid-test, the `TempDir` may drop and delete files before the CWD is restored.  
- **Fix:** Use `let _guard = CwdGuard(original_dir);` at the top of every CWD test.

---

## 7. Untested Critical Paths

### 7.1 Hook execution failure
`run_team_hook()` in `manager.rs` can return `HookOutcome::Error`. No test covers:
- Hook script missing
- Hook script exits non-zero
- Hook stdout parsing failure

### 7.2 Mailbox notifier registry memory leak
`register_notifier()` inserts into a global `OnceLock<RwLock<HashMap>>`. There is no test that `deregister_notifier()` actually removes entries, which could leak memory across many team lifecycles.

### 7.3 `TeamManager` compaction path
`compact_teammate_session()` is an async function with complex error handling. No test covers:
- Token-overflow detection → compaction → retry
- Permanent API error → stop retrying
- Compaction agent resolution failure

### 7.4 Blueprint seeding
When `team_create` receives a blueprint name, it reads `.ragent/blueprints/teams/{name}/spawn-prompts.json` and spawns teammates automatically. This entire I/O path is untested.

### 7.5 Team memory read/write
`team_memory_read` and `team_memory_write` tools interact with the memory system. No tests verify round-trip persistence.

---

## 8. Suggested Test File Layout

```
crates/ragent-team/tests/
├── test_team_tools.rs          # Tests 1–59 (tool execution)
├── test_task_store.rs          # Tests 60–72 (flock + claim logic)
├── test_mailbox.rs             # Tests 73–79 (mailbox I/O)
├── test_team_store.rs          # Tests 80–87 (store CRUD)
├── test_team_manager.rs        # Tests 88–94 (manager lifecycle)
├── test_swarm.rs               # Tests 95–98 (decomposition)
├── test_team_config.rs         # Tests 99–103 (config serde)

crates/ragent-server/tests/
├── test_team_api.rs            # Tests 104–115 (REST endpoints)

crates/ragent-tui/tests/
├── test_teams_tui.rs           # Already exists; add tests 118–129
```

---

## 9. Immediate Action Items (for journal-removal team)

Since this audit was performed **during** the journal-removal sprint, the following tests are most urgent to prevent regressions:

1. **Add tests 116–117** (SSE journal event removal) to confirm `sse.rs` still dispatches correctly after `JournalEntryCreated`/`JournalSearched` are deleted.
2. **Add tests 60–72** (TaskStore concurrent claims) — the task system is core to team coordination and currently untested.
3. **Add tests 104–115** (Team REST API) — the server tests currently only cover memory/journal; team endpoints are completely dark.
4. **Refactor CWD guard** in `test_teams_tui.rs` to use `CwdGuard` struct (adopt pattern from `test_tools_visibility_command.rs`) to fix flaky CWD tests.

---

*End of audit.*
