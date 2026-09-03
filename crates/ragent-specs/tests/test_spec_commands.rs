#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-specs/src/commands.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_specs::commands::SpecCommand;

#[test]
fn parse_help() {
    assert_eq!(SpecCommand::parse("help"), SpecCommand::Help);
    assert_eq!(SpecCommand::parse(""), SpecCommand::Help);
}

#[test]
fn parse_create() {
    let cmd = SpecCommand::parse("create my-spec Add auth");
    assert!(
        matches!(cmd, SpecCommand::Create { specname, feature, from_research: None } if specname == "my-spec" && feature == "Add auth")
    );
}

#[test]
fn parse_create_missing_feature() {
    assert!(
        matches!(SpecCommand::parse("create my-spec"), SpecCommand::Unknown(s) if s == "create")
    );
}

#[test]
fn parse_validate() {
    assert_eq!(
        SpecCommand::parse("validate my-spec"),
        SpecCommand::Validate {
            spec_id: Some("my-spec".to_string()),
        }
    );
}

#[test]
fn parse_validate_all() {
    assert_eq!(
        SpecCommand::parse("validate"),
        SpecCommand::Validate { spec_id: None }
    );
}

#[test]
fn parse_list() {
    assert_eq!(
        SpecCommand::parse("list --status draft"),
        SpecCommand::List {
            args: "--status draft".to_string(),
        }
    );
}

#[test]
fn parse_search() {
    assert_eq!(
        SpecCommand::parse("search websocket"),
        SpecCommand::Search {
            query: "websocket".to_string(),
        }
    );
}

#[test]
fn parse_status_show() {
    assert_eq!(
        SpecCommand::parse("status my-spec"),
        SpecCommand::Status {
            spec_id: "my-spec".to_string(),
            new_status: None,
        }
    );
}

#[test]
fn parse_status_transition() {
    assert_eq!(
        SpecCommand::parse("status my-spec in_review"),
        SpecCommand::Status {
            spec_id: "my-spec".to_string(),
            new_status: Some("in_review".to_string()),
        }
    );
}

#[test]
fn parse_status_missing_id() {
    assert!(matches!(SpecCommand::parse("status"), SpecCommand::Unknown(s) if s == "status"));
}

#[test]
fn parse_task_list() {
    assert_eq!(
        SpecCommand::parse("task my-spec"),
        SpecCommand::Task {
            spec_id: "my-spec".to_string(),
            task_id: None,
            new_status: None,
        }
    );
}

#[test]
fn parse_task_show() {
    assert_eq!(
        SpecCommand::parse("task my-spec T-001"),
        SpecCommand::Task {
            spec_id: "my-spec".to_string(),
            task_id: Some("T-001".to_string()),
            new_status: None,
        }
    );
}

#[test]
fn parse_task_update() {
    assert_eq!(
        SpecCommand::parse("task my-spec T-001 completed"),
        SpecCommand::Task {
            spec_id: "my-spec".to_string(),
            task_id: Some("T-001".to_string()),
            new_status: Some("completed".to_string()),
        }
    );
}

#[test]
fn parse_task_missing_id() {
    assert!(matches!(SpecCommand::parse("task"), SpecCommand::Unknown(s) if s == "task"));
}

#[test]
fn parse_activate() {
    assert_eq!(
        SpecCommand::parse("activate my-spec"),
        SpecCommand::Activate {
            spec_id: "my-spec".to_string(),
        }
    );
}

#[test]
fn parse_deactivate() {
    assert_eq!(SpecCommand::parse("deactivate"), SpecCommand::Deactivate);
}

#[test]
fn parse_coverage() {
    assert_eq!(
        SpecCommand::parse("coverage my-spec"),
        SpecCommand::Coverage {
            spec_id: "my-spec".to_string(),
        }
    );
}

#[test]
fn parse_unknown() {
    assert!(matches!(SpecCommand::parse("foobar"), SpecCommand::Unknown(s) if s == "foobar"));
}

#[test]
fn help_message_contains_expected_fragments() {
    let help = SpecCommand::build_help_message();
    assert!(help.contains("/spec create"));
    assert!(help.contains("/spec validate"));
    assert!(help.contains("/spec list"));
    assert!(help.contains("/spec search"));
    assert!(help.contains("/spec status"));
    assert!(help.contains("/spec task"));
    assert!(help.contains("/spec activate"));
    assert!(help.contains("/spec deactivate"));
    assert!(help.contains("/spec coverage"));
    assert!(help.contains("/spec impl"));
    assert!(help.contains("/spec implement"));
}

#[test]
fn create_status_is_well_formed() {
    let s = SpecCommand::build_create_status("foo");
    assert!(s.contains("specs/foo/SPEC.md"));
}

#[test]
fn create_message_contains_paths() {
    let m = SpecCommand::build_create_message("bar", "baz");
    assert!(m.contains("specs/bar"));
}

#[test]
fn create_prompt_contains_feature_and_files() {
    let p = SpecCommand::build_create_prompt("qux", "quux", None);
    assert!(p.contains("quux"));
    assert!(p.contains("specs/qux/SPEC.md"));
}

#[test]
fn parse_impl_basic() {
    assert_eq!(
        SpecCommand::parse("impl myspec"),
        SpecCommand::Impl {
            spec_id: "myspec".to_string(),
            task_id: None,
            dry_run: false,
        }
    );
}

#[test]
fn parse_impl_with_task() {
    assert_eq!(
        SpecCommand::parse("impl myspec --task T-003"),
        SpecCommand::Impl {
            spec_id: "myspec".to_string(),
            task_id: Some("T-003".to_string()),
            dry_run: false,
        }
    );
}

#[test]
fn parse_impl_dry_run() {
    assert_eq!(
        SpecCommand::parse("impl myspec --dry-run"),
        SpecCommand::Impl {
            spec_id: "myspec".to_string(),
            task_id: None,
            dry_run: true,
        }
    );
}

#[test]
fn parse_impl_all_options() {
    assert_eq!(
        SpecCommand::parse("impl myspec --task T-005 --dry-run"),
        SpecCommand::Impl {
            spec_id: "myspec".to_string(),
            task_id: Some("T-005".to_string()),
            dry_run: true,
        }
    );
}

#[test]
fn parse_impl_missing_specname() {
    assert!(matches!(
        SpecCommand::parse("impl"),
        SpecCommand::Unknown(s) if s == "impl"
    ));
}

#[test]
fn parse_implement_basic() {
    assert_eq!(
        SpecCommand::parse("implement myspec"),
        SpecCommand::Impl {
            spec_id: "myspec".to_string(),
            task_id: None,
            dry_run: false,
        }
    );
}

#[test]
fn parse_implement_with_task() {
    assert_eq!(
        SpecCommand::parse("implement myspec --task T-003"),
        SpecCommand::Impl {
            spec_id: "myspec".to_string(),
            task_id: Some("T-003".to_string()),
            dry_run: false,
        }
    );
}

#[test]
fn parse_implement_dry_run() {
    assert_eq!(
        SpecCommand::parse("implement myspec --dry-run"),
        SpecCommand::Impl {
            spec_id: "myspec".to_string(),
            task_id: None,
            dry_run: true,
        }
    );
}

#[test]
fn parse_implement_missing_specname() {
    assert!(matches!(
        SpecCommand::parse("implement"),
        SpecCommand::Unknown(s) if s == "impl"
    ));
}

#[test]
fn parse_delete() {
    let cmd = SpecCommand::parse("delete my-spec --yes");
    assert!(matches!(cmd, SpecCommand::Delete { spec_id, yes } if spec_id == "my-spec" && yes));
}

#[test]
fn parse_delete_without_yes() {
    let cmd = SpecCommand::parse("delete my-spec");
    assert!(matches!(cmd, SpecCommand::Delete { spec_id, yes } if spec_id == "my-spec" && !yes));
}

#[test]
fn parse_delete_missing_spec_id() {
    assert!(matches!(SpecCommand::parse("delete"), SpecCommand::Unknown(s) if s == "delete"));
}

#[test]
fn parse_delete_is_usage_error() {
    let cmd = SpecCommand::Unknown("delete".to_string());
    assert!(cmd.is_usage_error());
}

#[test]
fn help_message_contains_delete() {
    let help = SpecCommand::build_help_message();
    assert!(help.contains("/spec delete"));
}

#[test]
fn parse_add() {
    let cmd = SpecCommand::parse("add my-spec Add new feature for X");
    assert!(
        matches!(cmd, SpecCommand::Add { spec_id, feature } if spec_id == "my-spec" && feature == "Add new feature for X")
    );
}

#[test]
fn parse_add_missing_feature() {
    assert!(matches!(SpecCommand::parse("add my-spec"), SpecCommand::Unknown(s) if s == "add"));
}

#[test]
fn parse_add_missing_spec_id() {
    assert!(matches!(SpecCommand::parse("add"), SpecCommand::Unknown(s) if s == "add"));
}

#[test]
fn parse_add_is_usage_error() {
    let cmd = SpecCommand::Unknown("add".to_string());
    assert!(cmd.is_usage_error());
}

#[test]
fn add_status_is_well_formed() {
    let s = SpecCommand::build_add_status("foo");
    assert!(s.contains("specs/foo/SPEC.md"));
    assert!(s.contains("specs/foo/PLAN.md"));
}

#[test]
fn add_message_contains_spec_id_and_feature() {
    let m = SpecCommand::build_add_message("bar", "Add new auth flow");
    assert!(m.contains("specs/bar"));
    assert!(m.contains("Add new auth flow"));
}

#[test]
fn add_log_contains_spec_id_and_feature() {
    let l = SpecCommand::build_add_log("baz", "Add caching");
    assert!(l.contains("baz"));
    assert!(l.contains("Add caching"));
}

#[test]
fn add_prompt_contains_existing_content_and_numbering() {
    let spec_md =
        "---\nstatus: draft\n---\n# My Spec\n\n**FR-001** (Ubiquitous) The system shall work.";
    let plan_md = "# Plan\n\n## Tasks\n\n| ID | Title | Requirement | Effort | Priority | Dependencies |\n|---|---|---|---|---|---|\n| T-001 | Task 1 | FR-001 | S | Critical | --- |";
    let p = SpecCommand::build_add_prompt("myspec", "Add new feature", spec_md, plan_md, 2, 1, 2);
    assert!(p.contains("Add new feature"));
    assert!(p.contains("myspec"));
    // The prompt includes the existing content so the LLM can generate
    // correct incremental additions without reading files.
    assert!(p.contains("FR-001"));
    assert!(p.contains("T-001"));
    assert!(p.contains("FR-002"));
    assert!(p.contains("NFR-001"));
    assert!(p.contains("T-002"));
    // The prompt now instructs the LLM to use the `edit` tool directly,
    // matching /spec create and /spec update. No delimited text blocks.
    assert!(p.contains("`edit` tool"));
    assert!(p.contains("NOT rewrite"));
    assert!(!p.contains("---NEW REQUIREMENTS---"));
    assert!(!p.contains("---END---"));
}

// ── JTBD parser tests ─────────────────────────────────────────────────────────

#[test]
fn parse_jtbd_basic() {
    let cmd = SpecCommand::parse("jtbd my-spec");
    assert!(matches!(cmd, SpecCommand::Jtbd { spec_id, force, agent }
            if spec_id == "my-spec" && !force && agent.is_none()));
}

#[test]
fn parse_jtbd_force() {
    let cmd = SpecCommand::parse("jtbd my-spec --force");
    assert!(matches!(cmd, SpecCommand::Jtbd { spec_id, force, agent }
            if spec_id == "my-spec" && force && agent.is_none()));
}

#[test]
fn parse_jtbd_force_default_false() {
    // FR-003: without --force, force must be false
    let cmd = SpecCommand::parse("jtbd my-spec");
    assert!(matches!(cmd, SpecCommand::Jtbd { force, .. } if !force));
}

#[test]
fn parse_jtbd_agent() {
    let cmd = SpecCommand::parse("jtbd my-spec --agent coder");
    assert!(matches!(cmd, SpecCommand::Jtbd { spec_id, force, agent }
            if spec_id == "my-spec" && !force && agent.as_deref() == Some("coder")));
}

#[test]
fn parse_jtbd_force_and_agent() {
    let cmd = SpecCommand::parse("jtbd my-spec --force --agent explore");
    assert!(matches!(cmd, SpecCommand::Jtbd { spec_id, force, agent }
            if spec_id == "my-spec" && force && agent.as_deref() == Some("explore")));
}

#[test]
fn parse_jtbd_agent_before_force() {
    // Order shouldn't matter — agent before force
    let cmd = SpecCommand::parse("jtbd my-spec --agent general --force");
    assert!(matches!(cmd, SpecCommand::Jtbd { spec_id, force, agent }
            if spec_id == "my-spec" && force && agent.as_deref() == Some("general")));
}

#[test]
fn parse_jtbd_agent_without_name() {
    // --agent with no value following — agent stays None, no panic
    let cmd = SpecCommand::parse("jtbd my-spec --agent");
    assert!(matches!(cmd, SpecCommand::Jtbd { spec_id, force, agent }
            if spec_id == "my-spec" && !force && agent.is_none()));
}

#[test]
fn parse_jtbd_missing_spec_id() {
    // FR-008: missing spec name → Unknown("jtbd")
    assert!(matches!(SpecCommand::parse("jtbd"), SpecCommand::Unknown(s) if s == "jtbd"));
}

#[test]
fn parse_jtbd_empty_args() {
    // FR-008: empty argument string after subcommand → Unknown
    assert!(matches!(SpecCommand::parse("jtbd   "), SpecCommand::Unknown(s) if s == "jtbd"));
}

#[test]
fn parse_jtbd_is_usage_error() {
    let cmd = SpecCommand::Unknown("jtbd".to_string());
    assert!(cmd.is_usage_error());
}

#[test]
fn help_message_contains_jtbd() {
    let help = SpecCommand::build_help_message();
    assert!(help.contains("/spec jtbd"));
}

// ── JTBD builder helper tests ─────────────────────────────────────────────────

#[test]
fn jtbd_status_is_well_formed() {
    let s = SpecCommand::build_jtbd_status("my-spec");
    assert!(s.contains("spec jtbd"));
    assert!(s.contains("my-spec"));
}

#[test]
fn jtbd_message_contains_spec_id() {
    let s = SpecCommand::build_jtbd_message("my-spec");
    assert!(s.contains("my-spec"));
    assert!(s.contains("JTBD"));
}

#[test]
fn jtbd_log_without_force_or_agent() {
    let s = SpecCommand::build_jtbd_log("my-spec", false, None);
    assert!(s.contains("my-spec"));
    assert!(s.contains("JTBD.md"));
    assert!(!s.contains("--force"));
    assert!(!s.contains("--agent"));
}

#[test]
fn jtbd_log_with_force() {
    let s = SpecCommand::build_jtbd_log("my-spec", true, None);
    assert!(s.contains("--force"));
    assert!(!s.contains("--agent"));
}

#[test]
fn jtbd_log_with_agent() {
    let s = SpecCommand::build_jtbd_log("my-spec", false, Some("coder"));
    assert!(s.contains("--agent"));
    assert!(s.contains("coder"));
    assert!(!s.contains("--force"));
}

#[test]
fn jtbd_log_with_force_and_agent() {
    let s = SpecCommand::build_jtbd_log("my-spec", true, Some("explore"));
    assert!(s.contains("--force"));
    assert!(s.contains("--agent"));
    assert!(s.contains("explore"));
}

#[test]
fn jtbd_prompt_contains_spec_id_and_files() {
    let p = SpecCommand::build_jtbd_prompt("my-spec");
    assert!(p.contains("my-spec"));
    assert!(p.contains("SPEC.md"));
    assert!(p.contains("JTBD.md"));
}

#[test]
fn jtbd_prompt_contains_jtbd_grammar() {
    // FR-006: prompt must instruct the JTBD grammar
    let p = SpecCommand::build_jtbd_prompt("my-spec");
    assert!(p.contains("When <situation>"));
    assert!(p.contains("motivation"));
    assert!(p.contains("expected outcome"));
}

#[test]
fn jtbd_prompt_contains_traceability_instruction() {
    // FR-007: prompt must instruct tracing jobs to FR/NFR IDs
    let p = SpecCommand::build_jtbd_prompt("my-spec");
    assert!(p.contains("FR-NNN"));
    assert!(p.contains("NFR-NNN"));
    assert!(p.contains("untraced"));
}
// ── /spec add prompt tests (edit-tool based, no post-processing) ────────────

#[test]
fn build_add_prompt_includes_spec_md_and_plan_md_content() {
    let spec_md = "## Requirements\n### FR-001\nOld.\n";
    let plan_md = "| ID | Title |\n| T-001 | Old |\n";
    let prompt = SpecCommand::build_add_prompt("my-spec", "add auth", spec_md, plan_md, 19, 5, 40);
    // The prompt must include the existing content for context.
    assert!(prompt.contains("FR-001"));
    assert!(prompt.contains("T-001"));
    // The prompt must instruct the LLM to use the edit tool directly.
    assert!(prompt.contains("`edit` tool"));
    assert!(prompt.contains("FR-019"));
    assert!(prompt.contains("T-040"));
    // No delimited text blocks — the LLM writes files directly.
    assert!(!prompt.contains("---NEW REQUIREMENTS---"));
    assert!(!prompt.contains("---END---"));
}
