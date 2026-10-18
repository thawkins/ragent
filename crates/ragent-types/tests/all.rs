//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#[path = "test_activity_event.rs"]
mod test_activity_event;

#[path = "test_activity_projection.rs"]
mod test_activity_projection;

#[path = "test_activity_projection_branches.rs"]
mod test_activity_projection_branches;

#[path = "test_automation_run.rs"]
mod test_automation_run;

#[path = "test_cron_every_no_start.rs"]
mod test_cron_every_no_start;

#[path = "test_cron_from_past_every.rs"]
mod test_cron_from_past_every;

#[path = "test_cron_module.rs"]
mod test_cron_module;

#[path = "test_cron_schedule_description.rs"]
mod test_cron_schedule_description;

#[path = "test_duration_parser.rs"]
mod test_duration_parser;

#[path = "test_event_bus_counters.rs"]
mod test_event_bus_counters;

#[path = "test_event_publish_move.rs"]
mod test_event_publish_move;

#[path = "test_event_variants.rs"]
mod test_event_variants;

#[path = "test_finish_reason.rs"]
mod test_finish_reason;

#[path = "test_format_size.rs"]
mod test_format_size;

#[path = "test_guard_file_helpers.rs"]
mod test_guard_file_helpers;

#[path = "test_html_plain_text.rs"]
mod test_html_plain_text;

#[path = "test_id_embedding_strutil_helpers.rs"]
mod test_id_embedding_strutil_helpers;

#[path = "test_image_part_serde.rs"]
mod test_image_part_serde;

#[path = "test_llm_serde.rs"]
mod test_llm_serde;

#[path = "test_message_display.rs"]
mod test_message_display;

#[path = "test_panic_guard.rs"]
mod test_panic_guard;

#[path = "test_resource.rs"]
mod test_resource;

#[path = "test_sanitize_helpers.rs"]
mod test_sanitize_helpers;

#[path = "test_sanitize_redact.rs"]
mod test_sanitize_redact;

#[path = "test_sanitize_terminal_module.rs"]
mod test_sanitize_terminal_module;

#[path = "test_schedule_parser.rs"]
mod test_schedule_parser;

#[path = "test_secret_registry_cap.rs"]
mod test_secret_registry_cap;

#[path = "test_shared_guards.rs"]
mod test_shared_guards;

#[path = "test_spool_debug_and_errors.rs"]
mod test_spool_debug_and_errors;

#[path = "test_spool_module.rs"]
mod test_spool_module;

#[path = "test_startup_timings.rs"]
mod test_startup_timings;

#[path = "test_structure_types.rs"]
mod test_structure_types;

#[path = "test_strutil.rs"]
mod test_strutil;

#[path = "test_strutil_boundaries.rs"]
mod test_strutil_boundaries;

#[path = "test_strutil_unittest.rs"]
mod test_strutil_unittest;

#[path = "test_thinking.rs"]
mod test_thinking;

#[path = "test_trigger_module.rs"]
mod test_trigger_module;

#[path = "test_trigger_types.rs"]
mod test_trigger_types;

#[path = "test_unified_redaction.rs"]
mod test_unified_redaction;
