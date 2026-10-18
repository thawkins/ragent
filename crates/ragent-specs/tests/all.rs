//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#[path = "test_backward_compat.rs"]
mod test_backward_compat;
#[path = "test_git_branch.rs"]
mod test_git_branch;
#[path = "test_govcreate_authoring.rs"]
mod test_govcreate_authoring;
#[path = "test_govcreate_helpers.rs"]
mod test_govcreate_helpers;
#[path = "test_govcreate_parse.rs"]
mod test_govcreate_parse;
#[path = "test_id_scanner.rs"]
mod test_id_scanner;
#[path = "test_manager.rs"]
mod test_manager;
#[path = "test_ms04_spec_id_validation.rs"]
mod test_ms04_spec_id_validation;
#[path = "test_slash_spec.rs"]
mod test_slash_spec;
#[path = "test_spec.rs"]
mod test_spec;
#[path = "test_spec_commands.rs"]
mod test_spec_commands;
#[path = "test_spec_impl.rs"]
mod test_spec_impl;
#[path = "test_spec_io.rs"]
mod test_spec_io;
#[path = "test_spec_reverse_flags.rs"]
mod test_spec_reverse_flags;
#[path = "test_t017_govcreate_parser.rs"]
mod test_t017_govcreate_parser;
#[path = "test_templates.rs"]
mod test_templates;
#[path = "test_validation.rs"]
mod test_validation;
#[path = "test_validation_perf.rs"]
mod test_validation_perf;
