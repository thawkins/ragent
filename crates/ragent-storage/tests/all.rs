//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#[path = "test_activity_branch.rs"]
mod test_activity_branch;
#[path = "test_activity_checkpoint.rs"]
mod test_activity_checkpoint;
#[path = "test_activity_concurrent.rs"]
mod test_activity_concurrent;
#[path = "test_activity_event_id.rs"]
mod test_activity_event_id;
#[path = "test_activity_jsonl_export.rs"]
mod test_activity_jsonl_export;
#[path = "test_activity_log.rs"]
mod test_activity_log;
#[path = "test_activity_model_message.rs"]
mod test_activity_model_message;
#[path = "test_activity_mutation_rejected.rs"]
mod test_activity_mutation_rejected;
#[path = "test_activity_permission_decision.rs"]
mod test_activity_permission_decision;
#[path = "test_activity_pruning.rs"]
mod test_activity_pruning;
#[path = "test_activity_replay.rs"]
mod test_activity_replay;
#[path = "test_activity_resume.rs"]
mod test_activity_resume;
#[path = "test_activity_retention.rs"]
mod test_activity_retention;
#[path = "test_activity_rollback.rs"]
mod test_activity_rollback;
#[path = "test_activity_termination.rs"]
mod test_activity_termination;
#[path = "test_activity_tool_call.rs"]
mod test_activity_tool_call;
#[path = "test_automation_runs_table.rs"]
mod test_automation_runs_table;
#[path = "test_background_tasks_crud.rs"]
mod test_background_tasks_crud;
#[path = "test_compaction_role.rs"]
mod test_compaction_role;
#[path = "test_cron_automation_lifecycle.rs"]
mod test_cron_automation_lifecycle;
#[path = "test_cron_events_table.rs"]
mod test_cron_events_table;
#[path = "test_cron_storage_round_trip.rs"]
mod test_cron_storage_round_trip;
#[path = "test_discovered_models.rs"]
mod test_discovered_models;
#[path = "test_embedding_store.rs"]
mod test_embedding_store;
#[path = "test_format_version_cache.rs"]
mod test_format_version_cache;
#[path = "test_initiative_lifecycle.rs"]
mod test_initiative_lifecycle;
#[path = "test_initiatives.rs"]
mod test_initiatives;
#[path = "test_knowledge_graph_crud.rs"]
mod test_knowledge_graph_crud;
#[path = "test_memory_crud.rs"]
mod test_memory_crud;
#[path = "test_message_embeddings.rs"]
mod test_message_embeddings;
#[path = "test_message_parts_update.rs"]
mod test_message_parts_update;
#[path = "test_message_search.rs"]
mod test_message_search;
#[path = "test_ms04_diff_boundary.rs"]
mod test_ms04_diff_boundary;
#[path = "test_provider_auth_store.rs"]
mod test_provider_auth_store;
#[path = "test_reader_concurrency.rs"]
mod test_reader_concurrency;
#[path = "test_run_cost_summaries.rs"]
mod test_run_cost_summaries;
#[path = "test_schema_version_fast_path.rs"]
mod test_schema_version_fast_path;
#[path = "test_settings_and_file_storage.rs"]
mod test_settings_and_file_storage;
#[path = "test_snapshot_expand.rs"]
mod test_snapshot_expand;
#[path = "test_storage_transactions.rs"]
mod test_storage_transactions;
#[path = "test_task_crud.rs"]
mod test_task_crud;
#[path = "test_task_cycle.rs"]
mod test_task_cycle;
#[path = "test_task_dag.rs"]
mod test_task_dag;
#[path = "test_todos_migration.rs"]
mod test_todos_migration;
#[path = "test_wal_warmup.rs"]
mod test_wal_warmup;
