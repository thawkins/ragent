#![allow(clippy::assert_is_empty)]
//! Tests for `/spec` slash-command parsing.

use ragent_specs::SpecCommand;
use regex::Regex;

#[test]
fn test_slash_spec_no_args_shows_help() {
    let cmd = SpecCommand::parse("");
    assert!(
        matches!(cmd, SpecCommand::Help),
        "empty args should show help"
    );
    let help = SpecCommand::build_help_message();
    assert!(help.contains("spec help"), "should show spec help: {help}");
    assert!(
        help.contains("spec create"),
        "should mention spec create: {help}"
    );
    assert!(help.contains("specs/"), "should mention specs/ dir: {help}");
    assert!(help.contains("PLAN.md"), "should mention PLAN.md: {help}");
}

#[test]
fn test_slash_spec_create_starts_generation() {
    let cmd = SpecCommand::parse("create websocket Add real-time collaborative editing");
    let SpecCommand::Create {
        specname,
        feature,
        from_research: _,
    } = cmd
    else {
        panic!("expected Create command, got {cmd:?}");
    };
    assert_eq!(specname, "websocket");
    assert_eq!(feature, "Add real-time collaborative editing");

    let status = SpecCommand::build_create_status(&specname);
    assert_eq!(
        status,
        "spec: writing specs/websocket/SPEC.md + specs/websocket/PLAN.md + \
         specs/websocket/TESTPLAN.md…",
        "status should indicate generation"
    );

    let msg = SpecCommand::build_create_message(&specname, &feature);
    assert!(
        msg.contains("specs/websocket/SPEC.md"),
        "message should contain spec file path"
    );
    assert!(
        msg.contains("specs/websocket/PLAN.md"),
        "message should contain plan file path"
    );
    assert!(
        msg.contains("specs/websocket/TESTPLAN.md"),
        "message should contain testplan file path"
    );

    let log = SpecCommand::build_create_log(&specname, &feature);
    assert!(
        log.contains("specs/websocket/SPEC.md"),
        "log should contain spec file path"
    );
    assert!(
        log.contains("specs/websocket/TESTPLAN.md"),
        "log should contain testplan file path"
    );

    let prompt = SpecCommand::build_create_prompt(&specname, &feature, None);
    assert!(
        prompt.contains("specification writer"),
        "task should contain spec writer prompt"
    );
    assert!(
        prompt.to_lowercase().contains("ears"),
        "task should mention EARS"
    );
    assert!(
        prompt.contains("specs/websocket/SPEC.md"),
        "task should contain spec file path"
    );
    assert!(
        prompt.contains("specs/websocket/PLAN.md"),
        "task should contain plan file path"
    );
    assert!(
        prompt.contains("specs/websocket/TESTPLAN.md"),
        "task should contain testplan file path"
    );
}

#[test]
fn test_slash_spec_create_prompt_requests_testplan() {
    let specname = "websocket";
    let feature = "Add real-time collaborative editing";
    let prompt = SpecCommand::build_create_prompt(specname, feature, None);

    // FR-001 / FR-003: the prompt must instruct writing a TESTPLAN.md file
    assert!(
        prompt.contains("specs/websocket/TESTPLAN.md"),
        "prompt should instruct writing TESTPLAN.md, got: {prompt}"
    );

    // FR-005: the prompt must request a '## Test Cases' section
    assert!(
        prompt.contains("## Test Cases"),
        "prompt should request a '## Test Cases' section, got: {prompt}"
    );

    // FR-005: the prompt must instruct using TC-NNN test case IDs
    assert!(
        prompt.contains("TC-001"),
        "prompt should instruct using TC-NNN test case IDs, got: {prompt}"
    );

    // FR-005: the prompt must request preconditions, instructions, and expected
    // results in each test case
    assert!(
        prompt.contains("preconditions"),
        "prompt should request preconditions, got: {prompt}"
    );
    assert!(
        prompt.contains("expected results"),
        "prompt should request expected results, got: {prompt}"
    );

    // FR-006: the prompt must instruct enumerating UI navigation steps and
    // exact data to enter when the feature involves user-interface navigation
    assert!(
        prompt.contains("user-interface navigation"),
        "prompt should instruct enumerating UI navigation steps, got: {prompt}"
    );
    assert!(
        prompt.contains("exact data to enter"),
        "prompt should instruct specifying exact data to enter, got: {prompt}"
    );
}

#[test]
fn test_slash_spec_create_prompt_excludes_automated_test_code_from_testplan() {
    let specname = "websocket";
    let feature = "Add real-time collaborative editing";
    let prompt = SpecCommand::build_create_prompt(specname, feature, None);

    // FR-009: the prompt must explicitly tell the agent NOT to include
    // automated test code, `#[test]` functions, or `cargo test` references
    assert!(
        prompt.contains("Do NOT include automated test code"),
        "prompt should instruct excluding automated test code, got: {prompt}"
    );
    assert!(
        prompt.contains("#[test]"),
        "prompt should mention #[test] as excluded, got: {prompt}"
    );
    assert!(
        prompt.contains("cargo test"),
        "prompt should mention cargo test as excluded, got: {prompt}"
    );
    assert!(
        prompt.contains("manual test plan only"),
        "prompt should emphasise manual-only test plan, got: {prompt}"
    );
}

#[test]
fn test_spec_command_unknown_subcommand() {
    let cmd = SpecCommand::parse("foobar something");
    assert!(
        matches!(cmd, SpecCommand::Unknown(s) if s == "foobar"),
        "unexpected subcommand should return Unknown"
    );
}

#[test]
fn test_spec_command_create_missing_feature() {
    let cmd = SpecCommand::parse("create websocket");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "create"),
        "create without feature should return Unknown('create')"
    );
    assert!(cmd.is_usage_error(), "should be a usage error");
}

// ── Update subcommand tests ──────────────────────────────────────────────

#[test]
fn test_spec_command_update_parses() {
    let cmd = SpecCommand::parse("update myspec");
    let SpecCommand::Update { spec_id } = cmd else {
        panic!("expected Update command, got {cmd:?}");
    };
    assert_eq!(spec_id, "myspec");
}

#[test]
fn test_spec_command_update_missing_spec_id() {
    let cmd = SpecCommand::parse("update");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "update"),
        "update without spec-id should return Unknown('update'), got {cmd:?}"
    );
    assert!(cmd.is_usage_error(), "should be a usage error");
}

#[test]
fn test_spec_help_contains_update() {
    let help = SpecCommand::build_help_message();
    assert!(
        help.contains("/spec update"),
        "help should mention /spec update: {help}"
    );
}

#[test]
fn test_spec_update_status_message() {
    let status = SpecCommand::build_update_status("myspec");
    assert!(
        status.contains("specs/myspec/PLAN.md"),
        "status should contain PLAN.md path: {status}"
    );
    assert!(
        status.contains("specs/myspec/TESTPLAN.md"),
        "status should contain TESTPLAN.md path: {status}"
    );
}

#[test]
fn test_spec_update_message() {
    let msg = SpecCommand::build_update_message("myspec");
    assert!(
        msg.contains("SPEC.md"),
        "message should mention SPEC.md: {msg}"
    );
    assert!(
        msg.contains("PLAN.md"),
        "message should mention PLAN.md: {msg}"
    );
    assert!(
        msg.contains("TESTPLAN.md"),
        "message should mention TESTPLAN.md: {msg}"
    );
}

#[test]
fn test_spec_update_log() {
    let log = SpecCommand::build_update_log("myspec");
    assert!(log.contains("myspec"), "log should contain spec id: {log}");
}

#[test]
fn test_spec_update_prompt() {
    let prompt = SpecCommand::build_update_prompt(
        "myspec",
        "# Existing Plan\n| ID | Title |\n|---|---|\n| T-001 | Done |",
    );
    assert!(
        prompt.contains("specs/myspec/SPEC.md"),
        "prompt should contain SPEC.md path: {prompt}"
    );
    assert!(
        prompt.contains("specs/myspec/PLAN.md"),
        "prompt should contain PLAN.md path: {prompt}"
    );
    assert!(
        prompt.contains("specs/myspec/TESTPLAN.md"),
        "prompt should contain TESTPLAN.md path: {prompt}"
    );
    assert!(
        prompt.contains("## Test Cases"),
        "prompt should request ## Test Cases section: {prompt}"
    );
}

// ── Specify subcommand tests (T-001, FR-001) ───────────────────────────────

#[test]
fn test_spec_command_specify_parses() {
    let cmd = SpecCommand::parse("specify myspec Add a real-time notification system");
    let SpecCommand::Specify {
        specname,
        feature,
        from_research: _,
    } = cmd
    else {
        panic!("expected Specify command, got {cmd:?}");
    };
    assert_eq!(specname, "myspec");
    assert_eq!(feature, "Add a real-time notification system");
}

#[test]
fn test_spec_command_specify_missing_feature() {
    let cmd = SpecCommand::parse("specify myspec");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "specify"),
        "specify without feature should return Unknown('specify'), got {cmd:?}"
    );
    assert!(cmd.is_usage_error(), "should be a usage error");
}

#[test]
fn test_spec_command_specify_missing_specname() {
    let cmd = SpecCommand::parse("specify");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "specify"),
        "specify without specname should return Unknown('specify'), got {cmd:?}"
    );
    assert!(cmd.is_usage_error(), "should be a usage error");
}

#[test]
fn test_spec_help_contains_specify() {
    let help = SpecCommand::build_help_message();
    assert!(
        help.contains("/spec specify"),
        "help should mention /spec specify: {help}"
    );
}

// ── Specify helper tests (T-002, FR-001) ──────────────────────────────────

#[test]
fn test_specify_status_mentions_only_spec_md() {
    let status = SpecCommand::build_specify_status("myspec");
    assert!(
        status.contains("specs/myspec/SPEC.md"),
        "status should mention SPEC.md: {status}"
    );
    assert!(
        !status.contains("PLAN.md"),
        "status should NOT mention PLAN.md: {status}"
    );
    assert!(
        !status.contains("TESTPLAN.md"),
        "status should NOT mention TESTPLAN.md: {status}"
    );
}

#[test]
fn test_specify_message_mentions_spec_md_and_plan_hint() {
    let msg = SpecCommand::build_specify_message("myspec", "Add notifications");
    assert!(
        msg.contains("specs/myspec/SPEC.md"),
        "message should mention SPEC.md: {msg}"
    );
    assert!(
        !msg.contains("PLAN.md\n"),
        "message should NOT list PLAN.md as an output file: {msg}"
    );
    assert!(
        msg.contains("/spec plan"),
        "message should hint at /spec plan for next stage: {msg}"
    );
}

#[test]
fn test_specify_log_no_plan() {
    let log = SpecCommand::build_specify_log("myspec", "Add notifications");
    assert!(log.contains("SPEC.md"), "log should mention SPEC.md: {log}");
    assert!(
        log.contains("no PLAN.md"),
        "log should note no PLAN.md: {log}"
    );
}

#[test]
fn test_specify_prompt_writes_only_spec_md() {
    let prompt = SpecCommand::build_specify_prompt("myspec", "Add notifications", None);
    assert!(
        prompt.contains("specs/myspec/SPEC.md"),
        "prompt should instruct writing SPEC.md: {prompt}"
    );
    assert!(
        prompt.contains("Do NOT create `PLAN.md`"),
        "prompt should explicitly forbid PLAN.md: {prompt}"
    );
    assert!(
        prompt.contains("[NEEDS CLARIFICATION"),
        "prompt should mention NEEDS CLARIFICATION markers: {prompt}"
    );
}

// ── Plan subcommand tests (T-003, FR-004) ─────────────────────────────────

#[test]
fn test_spec_command_plan_parses() {
    let cmd = SpecCommand::parse(
        "plan myspec WebSocket transport, Redis pub/sub, PostgreSQL for persistence",
    );
    let SpecCommand::Plan {
        spec_id,
        tech_context,
    } = cmd
    else {
        panic!("expected Plan command, got {cmd:?}");
    };
    assert_eq!(spec_id, "myspec");
    assert_eq!(
        tech_context,
        "WebSocket transport, Redis pub/sub, PostgreSQL for persistence"
    );
}

#[test]
fn test_spec_command_plan_missing_tech_context() {
    let cmd = SpecCommand::parse("plan myspec");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "plan"),
        "plan without tech-context should return Unknown('plan'), got {cmd:?}"
    );
    assert!(cmd.is_usage_error(), "should be a usage error");
}

#[test]
fn test_spec_command_plan_missing_spec_id() {
    let cmd = SpecCommand::parse("plan");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "plan"),
        "plan without spec-id should return Unknown('plan'), got {cmd:?}"
    );
    assert!(cmd.is_usage_error(), "should be a usage error");
}

#[test]
fn test_spec_help_contains_plan() {
    let help = SpecCommand::build_help_message();
    assert!(
        help.contains("/spec plan"),
        "help should mention /spec plan: {help}"
    );
}
// ── Plan helper tests (T-004, FR-004) ──────────────────────────────────────

#[test]
fn test_plan_status_mentions_plan_md() {
    let status = SpecCommand::build_plan_status("myspec");
    assert!(
        status.contains("specs/myspec/PLAN.md"),
        "status should mention PLAN.md: {status}"
    );
}

#[test]
fn test_plan_message_mentions_tech_context() {
    let msg = SpecCommand::build_plan_message("myspec", "WebSocket + Redis", false, false, false);
    assert!(
        msg.contains("specs/myspec/SPEC.md"),
        "message should mention reading SPEC.md: {msg}"
    );
    assert!(
        msg.contains("specs/myspec/PLAN.md"),
        "message should mention writing PLAN.md: {msg}"
    );
    assert!(
        msg.contains("WebSocket + Redis"),
        "message should include the tech context: {msg}"
    );
    assert!(
        !msg.contains("data-model.md"),
        "message should NOT mention data-model.md when disabled: {msg}"
    );
}

#[test]
fn test_plan_log_mentions_tech_context() {
    let log = SpecCommand::build_plan_log("myspec", "WebSocket + Redis");
    assert!(log.contains("PLAN.md"), "log should mention PLAN.md: {log}");
    assert!(
        log.contains("WebSocket + Redis"),
        "log should include tech context: {log}"
    );
}

#[test]
fn test_plan_prompt_includes_spec_md_and_tech_context() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "WebSocket", spec_md, "", false, false, "");
    assert!(
        prompt.contains("specs/myspec/PLAN.md"),
        "prompt should instruct writing PLAN.md: {prompt}"
    );
    assert!(
        prompt.contains("WebSocket"),
        "prompt should include tech context: {prompt}"
    );
    assert!(
        prompt.contains(spec_md),
        "prompt should include SPEC.md content: {prompt}"
    );
    assert!(
        prompt.contains("Do NOT modify `SPEC.md`"),
        "prompt should forbid modifying SPEC.md: {prompt}"
    );
    assert!(
        prompt.contains("## Technology Choices"),
        "prompt should request technology choices section: {prompt}"
    );
    assert!(
        !prompt.contains("data-model.md"),
        "prompt should NOT mention data-model.md when disabled: {prompt}"
    );
}

#[test]
fn test_plan_prompt_with_existing_plan_preservation() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let plan_md = "## Tasks\n| T-001 | Do thing | FR-001 | S | High | — |";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "WebSocket", spec_md, plan_md, false, false, "");
    assert!(
        prompt.contains("Preserve the existing task IDs"),
        "prompt should mention preserving task IDs when plan_md is non-empty: {prompt}"
    );
    assert!(
        prompt.contains(plan_md),
        "prompt should include existing PLAN.md content: {prompt}"
    );
}

#[test]
fn test_plan_prompt_without_existing_plan_no_preservation() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "WebSocket", spec_md, "", false, false, "");
    assert!(
        !prompt.contains("Preserve the existing task IDs"),
        "prompt should NOT mention preservation when no existing plan: {prompt}"
    );
}

// ── Data-model instruction tests (T-021, FR-011) ───────────────────────────

#[test]
fn test_data_model_instruction_contains_spec_id() {
    let instruction = SpecCommand::build_data_model_instruction("myspec");
    assert!(
        instruction.contains("specs/myspec/data-model.md"),
        "instruction should reference the spec's data-model.md path: {instruction}"
    );
}

#[test]
fn test_data_model_instruction_mentions_entities_section() {
    let instruction = SpecCommand::build_data_model_instruction("myspec");
    assert!(
        instruction.contains("## Entities"),
        "instruction should request a Entities section: {instruction}"
    );
}

#[test]
fn test_data_model_instruction_mentions_relationships_section() {
    let instruction = SpecCommand::build_data_model_instruction("myspec");
    assert!(
        instruction.contains("## Relationships"),
        "instruction should request a Relationships section: {instruction}"
    );
}

#[test]
fn test_data_model_instruction_mentions_constraints_section() {
    let instruction = SpecCommand::build_data_model_instruction("myspec");
    assert!(
        instruction.contains("## Constraints"),
        "instruction should request a Constraints section: {instruction}"
    );
}

#[test]
fn test_data_model_instruction_says_optional() {
    let instruction = SpecCommand::build_data_model_instruction("myspec");
    assert!(
        instruction.contains("do NOT create"),
        "instruction should say the file is optional when no data entities: {instruction}"
    );
}

#[test]
fn test_plan_prompt_with_data_model_enabled() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall store user profiles.";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "PostgreSQL", spec_md, "", true, false, "");
    assert!(
        prompt.contains("data-model.md"),
        "prompt should mention data-model.md when enabled: {prompt}"
    );
    assert!(
        prompt.contains("## Entities"),
        "prompt should include Entities section instruction when enabled: {prompt}"
    );
    assert!(
        prompt.contains("## Relationships"),
        "prompt should include Relationships section instruction when enabled: {prompt}"
    );
    assert!(
        prompt.contains("## Constraints"),
        "prompt should include Constraints section instruction when enabled: {prompt}"
    );
}

#[test]
fn test_plan_prompt_without_data_model_no_instruction() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "WebSocket", spec_md, "", false, false, "");
    assert!(
        !prompt.contains("data-model.md"),
        "prompt should NOT mention data-model.md when disabled: {prompt}"
    );
}

#[test]
fn test_plan_prompt_with_data_model_and_existing_plan() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall store user profiles.";
    let plan_md = "## Tasks\n| T-001 | Do thing | FR-001 | S | High | — |";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "PostgreSQL", spec_md, plan_md, true, false, "");
    assert!(
        prompt.contains("Preserve the existing task IDs"),
        "prompt should preserve task IDs with existing plan: {prompt}"
    );
    assert!(
        prompt.contains("data-model.md"),
        "prompt should include data-model instruction when enabled: {prompt}"
    );
}

#[test]
fn test_plan_message_with_data_model_enabled() {
    let msg = SpecCommand::build_plan_message("myspec", "PostgreSQL", true, false, false);
    assert!(
        msg.contains("data-model.md"),
        "message should mention data-model.md when enabled: {msg}"
    );
}

#[test]
fn test_plan_message_without_data_model_no_mention() {
    let msg = SpecCommand::build_plan_message("myspec", "PostgreSQL", false, false, false);
    assert!(
        !msg.contains("data-model.md"),
        "message should NOT mention data-model.md when disabled: {msg}"
    );
}

// ── Contracts instruction tests (T-022, FR-012) ───────────────────────────

#[test]
fn test_contracts_instruction_contains_spec_id() {
    let instruction = SpecCommand::build_contracts_instruction("myspec");
    assert!(
        instruction.contains("specs/myspec/contracts/"),
        "instruction should reference the spec's contracts/ path: {instruction}"
    );
}

#[test]
fn test_contracts_instruction_mentions_api_endpoints() {
    let instruction = SpecCommand::build_contracts_instruction("myspec");
    assert!(
        instruction.contains("API endpoints"),
        "instruction should mention API endpoints: {instruction}"
    );
}

#[test]
fn test_contracts_instruction_mentions_inter_service_contracts() {
    let instruction = SpecCommand::build_contracts_instruction("myspec");
    assert!(
        instruction.contains("inter-service contracts"),
        "instruction should mention inter-service contracts: {instruction}"
    );
}

#[test]
fn test_contracts_instruction_mentions_request_response_schemas() {
    let instruction = SpecCommand::build_contracts_instruction("myspec");
    assert!(
        instruction.contains("Request/response schemas"),
        "instruction should mention request/response schemas: {instruction}"
    );
}

#[test]
fn test_contracts_instruction_mentions_error_codes() {
    let instruction = SpecCommand::build_contracts_instruction("myspec");
    assert!(
        instruction.contains("Error codes"),
        "instruction should mention error codes: {instruction}"
    );
}

#[test]
fn test_contracts_instruction_says_optional() {
    let instruction = SpecCommand::build_contracts_instruction("myspec");
    assert!(
        instruction.contains("do NOT create"),
        "instruction should say the directory is optional when no contracts: {instruction}"
    );
}

#[test]
fn test_plan_prompt_with_contracts_enabled() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall expose a REST API.";
    let prompt = SpecCommand::build_plan_prompt("myspec", "axum", spec_md, "", false, true, "");
    assert!(
        prompt.contains("contracts/"),
        "prompt should mention contracts/ when enabled: {prompt}"
    );
    assert!(
        prompt.contains("API endpoints"),
        "prompt should include API endpoints instruction when enabled: {prompt}"
    );
}

#[test]
fn test_plan_prompt_without_contracts_no_instruction() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "WebSocket", spec_md, "", false, false, "");
    assert!(
        !prompt.contains("contracts/"),
        "prompt should NOT mention contracts/ when disabled: {prompt}"
    );
}

#[test]
fn test_plan_prompt_with_data_model_and_contracts_both_enabled() {
    let spec_md =
        "## Requirements\n### FR-001\nThe system shall store user profiles via a REST API.";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "axum + PostgreSQL", spec_md, "", true, true, "");
    assert!(
        prompt.contains("data-model.md"),
        "prompt should mention data-model.md when both enabled: {prompt}"
    );
    assert!(
        prompt.contains("contracts/"),
        "prompt should mention contracts/ when both enabled: {prompt}"
    );
}

#[test]
fn test_plan_prompt_with_contracts_and_existing_plan() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall expose a REST API.";
    let plan_md = "## Tasks\n| T-001 | Do thing | FR-001 | S | High | — |";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "axum", spec_md, plan_md, false, true, "");
    assert!(
        prompt.contains("Preserve the existing task IDs"),
        "prompt should preserve task IDs with existing plan: {prompt}"
    );
    assert!(
        prompt.contains("contracts/"),
        "prompt should include contracts instruction when enabled: {prompt}"
    );
}

#[test]
fn test_plan_message_with_contracts_enabled() {
    let msg = SpecCommand::build_plan_message("myspec", "axum", false, true, false);
    assert!(
        msg.contains("contracts/"),
        "message should mention contracts/ when enabled: {msg}"
    );
}

#[test]
fn test_plan_message_without_contracts_no_mention() {
    let msg = SpecCommand::build_plan_message("myspec", "axum", false, false, false);
    assert!(
        !msg.contains("contracts/"),
        "message should NOT mention contracts/ when disabled: {msg}"
    );
}

// ── Tasks subcommand tests (T-005, FR-005) ────────────────────────────────

#[test]
fn test_spec_command_tasks_parses() {
    let cmd = SpecCommand::parse("tasks myspec");
    let SpecCommand::Tasks { spec_id } = cmd else {
        panic!("expected Tasks command, got {cmd:?}");
    };
    assert_eq!(spec_id, "myspec");
}

#[test]
fn test_spec_command_tasks_missing_spec_id() {
    let cmd = SpecCommand::parse("tasks");
    assert!(
        matches!(&cmd, SpecCommand::Unknown(s) if s == "tasks"),
        "tasks without spec-id should return Unknown('tasks'), got {cmd:?}"
    );
    assert!(cmd.is_usage_error(), "should be a usage error");
}

#[test]
fn test_spec_help_contains_tasks() {
    let help = SpecCommand::build_help_message();
    assert!(
        help.contains("/spec tasks"),
        "help should mention /spec tasks: {help}"
    );
}
// ── Edge-case tests for SDD subcommands (T-041, NFR-004) ───────────────────

#[test]
fn test_spec_command_specify_extra_whitespace() {
    let cmd = SpecCommand::parse("specify   myspec    Add a feature");
    let SpecCommand::Specify {
        specname,
        feature,
        from_research: _,
    } = cmd
    else {
        panic!("expected Specify command, got {cmd:?}");
    };
    assert_eq!(specname, "myspec");
    // The feature text should have the internal whitespace preserved after the
    // first split, but leading/trailing whitespace is trimmed.
    assert_eq!(feature, "Add a feature");
}

#[test]
fn test_spec_command_plan_extra_whitespace() {
    let cmd = SpecCommand::parse("plan   myspec   WebSocket transport");
    let SpecCommand::Plan {
        spec_id,
        tech_context,
    } = cmd
    else {
        panic!("expected Plan command, got {cmd:?}");
    };
    assert_eq!(spec_id, "myspec");
    assert_eq!(tech_context, "WebSocket transport");
}

#[test]
fn test_spec_command_tasks_extra_whitespace() {
    let cmd = SpecCommand::parse("tasks   myspec   ");
    let SpecCommand::Tasks { spec_id } = cmd else {
        panic!("expected Tasks command, got {cmd:?}");
    };
    assert_eq!(spec_id, "myspec");
}

#[test]
fn test_spec_help_lists_all_new_subcommands() {
    let help = SpecCommand::build_help_message();
    assert!(help.contains("/spec specify"), "help should list specify");
    assert!(help.contains("/spec plan"), "help should list plan");
    assert!(help.contains("/spec tasks"), "help should list tasks");
}

// ── Feedback command tests (FR-017, T-032) ──────────────────────────────────

#[test]
fn test_spec_feedback_parse_valid() {
    let cmd = SpecCommand::parse("feedback myspec Latency spiked to 500ms in production");
    let SpecCommand::Feedback { spec_id, note } = cmd else {
        panic!("expected Feedback command, got {cmd:?}");
    };
    assert_eq!(spec_id, "myspec");
    assert_eq!(note, "Latency spiked to 500ms in production");
}

#[test]
fn test_spec_feedback_parse_missing_spec_id() {
    let cmd = SpecCommand::parse("feedback");
    assert!(
        matches!(cmd, SpecCommand::Unknown(ref s) if s == "feedback"),
        "missing spec_id should be Unknown, got {cmd:?}"
    );
    assert!(cmd.is_usage_error());
}

#[test]
fn test_spec_feedback_parse_missing_note() {
    let cmd = SpecCommand::parse("feedback myspec");
    assert!(
        matches!(cmd, SpecCommand::Unknown(ref s) if s == "feedback"),
        "missing note should be Unknown, got {cmd:?}"
    );
    assert!(cmd.is_usage_error());
}

#[test]
fn test_spec_feedback_parse_extra_whitespace() {
    let cmd = SpecCommand::parse("feedback   myspec   Some note here   ");
    let SpecCommand::Feedback { spec_id, note } = cmd else {
        panic!("expected Feedback command, got {cmd:?}");
    };
    assert_eq!(spec_id, "myspec");
    assert_eq!(note, "Some note here");
}

#[test]
fn test_spec_feedback_is_usage_error() {
    let cmd = SpecCommand::Unknown("feedback".to_string());
    assert!(cmd.is_usage_error());
}

#[test]
fn test_spec_feedback_status() {
    let status = SpecCommand::build_feedback_status("myspec");
    assert!(
        status.contains("specs/myspec/FEEDBACK.md"),
        "status should mention FEEDBACK.md path: {status}"
    );
}

#[test]
fn test_spec_feedback_message() {
    let msg = SpecCommand::build_feedback_message("myspec", "Latency issue");
    assert!(
        msg.contains("specs/myspec/FEEDBACK.md"),
        "message should mention file path: {msg}"
    );
    assert!(
        msg.contains("Latency issue"),
        "message should contain the note: {msg}"
    );
    assert!(
        msg.contains("advisory"),
        "message should mention advisory nature: {msg}"
    );
}

#[test]
fn test_spec_feedback_log() {
    let log = SpecCommand::build_feedback_log("myspec", "Latency issue");
    assert!(
        log.contains("specs/myspec/FEEDBACK.md"),
        "log should mention file path: {log}"
    );
    assert!(
        log.contains("Latency issue"),
        "log should contain the note: {log}"
    );
}

#[test]
fn test_spec_feedback_log_truncation() {
    let long_note = "A".repeat(100);
    let log = SpecCommand::build_feedback_log("myspec", &long_note);
    assert!(
        log.contains("…"),
        "log should truncate long notes with ellipsis: {log}"
    );
}

#[test]
fn test_spec_feedback_format_row() {
    let row = SpecCommand::format_feedback_row("Test note");
    assert!(
        row.starts_with("| "),
        "row should start with table pipe: {row}"
    );
    assert!(
        row.contains("Test note"),
        "row should contain the note: {row}"
    );
    assert!(
        row.contains("user"),
        "row should contain 'user' source: {row}"
    );
    // Should contain a date in YYYY-MM-DD format
    assert!(
        Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap().is_match(&row),
        "row should contain a date: {row}"
    );
}

#[test]
fn test_spec_feedback_format_row_escapes_pipes() {
    let row = SpecCommand::format_feedback_row("note with | pipe");
    assert!(
        row.contains("\\|"),
        "row should escape pipe characters: {row}"
    );
}

#[test]
fn test_spec_feedback_append_to_empty() {
    let result = SpecCommand::append_feedback_note("", "My Spec", "First note");
    assert!(
        result.contains("# Feedback: My Spec"),
        "should generate template with title: {result}"
    );
    assert!(
        result.contains("First note"),
        "should contain the note: {result}"
    );
    assert!(
        !result.contains("[YYYY-MM-DD]"),
        "placeholder row should be replaced: {result}"
    );
}

#[test]
fn test_spec_feedback_append_to_existing() {
    let existing = "# Feedback: My Spec\n\n## Feedback Notes\n\n| Date | Source | Note |\n|------|--------|------|\n| 2025-01-01 | user | Old note |\n\n---\n\n*Notes are advisory.*\n";
    let result = SpecCommand::append_feedback_note(existing, "My Spec", "New note");
    assert!(
        result.contains("Old note"),
        "should preserve existing notes: {result}"
    );
    assert!(
        result.contains("New note"),
        "should contain the new note: {result}"
    );
    assert!(
        result.contains("---"),
        "should preserve the separator: {result}"
    );
}

#[test]
fn test_spec_feedback_append_multiple_notes() {
    let result1 = SpecCommand::append_feedback_note("", "My Spec", "First note");
    let result2 = SpecCommand::append_feedback_note(&result1, "My Spec", "Second note");
    assert!(
        result2.contains("First note"),
        "should contain first note: {result2}"
    );
    assert!(
        result2.contains("Second note"),
        "should contain second note: {result2}"
    );
}

#[test]
fn test_spec_feedback_help_lists_command() {
    let help = SpecCommand::build_help_message();
    assert!(
        help.contains("/spec feedback"),
        "help should list feedback command: {help}"
    );
    assert!(
        help.contains("FEEDBACK.md"),
        "help should mention FEEDBACK.md: {help}"
    );
}
// ── --from-research parsing tests (FR-010, T-019) ───────────────────────────

#[test]
fn test_spec_command_create_with_from_research() {
    let cmd = SpecCommand::parse("create myspec Add auth --from-research auth-research");
    let SpecCommand::Create {
        specname,
        feature,
        from_research,
    } = cmd
    else {
        panic!("expected Create command, got {cmd:?}");
    };
    assert_eq!(specname, "myspec");
    assert_eq!(feature, "Add auth");
    assert_eq!(from_research.as_deref(), Some("auth-research"));
}

#[test]
fn test_spec_command_create_without_from_research() {
    let cmd = SpecCommand::parse("create myspec Add auth");
    let SpecCommand::Create {
        specname,
        feature,
        from_research,
    } = cmd
    else {
        panic!("expected Create command, got {cmd:?}");
    };
    assert_eq!(specname, "myspec");
    assert_eq!(feature, "Add auth");
    assert!(from_research.is_none());
}

#[test]
fn test_spec_command_specify_with_from_research() {
    let cmd =
        SpecCommand::parse("specify myspec Add notifications --from-research realtime-collab");
    let SpecCommand::Specify {
        specname,
        feature,
        from_research,
    } = cmd
    else {
        panic!("expected Specify command, got {cmd:?}");
    };
    assert_eq!(specname, "myspec");
    assert_eq!(feature, "Add notifications");
    assert_eq!(from_research.as_deref(), Some("realtime-collab"));
}

#[test]
fn test_spec_command_specify_without_from_research() {
    let cmd = SpecCommand::parse("specify myspec Add notifications");
    let SpecCommand::Specify {
        specname,
        feature,
        from_research,
    } = cmd
    else {
        panic!("expected Specify command, got {cmd:?}");
    };
    assert_eq!(specname, "myspec");
    assert_eq!(feature, "Add notifications");
    assert!(from_research.is_none());
}

#[test]
fn test_spec_command_create_from_research_no_name() {
    // --from-research with no name should not set from_research
    let cmd = SpecCommand::parse("create myspec Add auth --from-research");
    let SpecCommand::Create { from_research, .. } = cmd else {
        panic!("expected Create command, got {cmd:?}");
    };
    assert!(
        from_research.is_none(),
        "from_research should be None when no name given"
    );
}

// ── Research frontmatter instruction tests (FR-010, T-019) ──────────────────

#[test]
fn test_build_research_frontmatter_instruction_with_name() {
    let instruction = SpecCommand::build_research_frontmatter_instruction(Some("auth-research"));
    assert!(
        instruction.contains("research:"),
        "instruction should mention research: field: {instruction}"
    );
    assert!(
        instruction.contains("\"auth-research\""),
        "instruction should contain the research name: {instruction}"
    );
}

#[test]
fn test_build_research_frontmatter_instruction_without_name() {
    let instruction = SpecCommand::build_research_frontmatter_instruction(None);
    assert!(
        instruction.is_empty(),
        "instruction should be empty when no research name: {instruction}"
    );
}

#[test]
fn test_build_create_prompt_includes_research_frontmatter() {
    let prompt = SpecCommand::build_create_prompt("myspec", "Add auth", Some("auth-research"));
    assert!(
        prompt.contains("research:"),
        "create prompt should include research frontmatter instruction: {prompt}"
    );
    assert!(
        prompt.contains("\"auth-research\""),
        "create prompt should contain the research name: {prompt}"
    );
}

#[test]
fn test_build_create_prompt_without_research() {
    let prompt = SpecCommand::build_create_prompt("myspec", "Add auth", None);
    assert!(
        !prompt.contains("research:"),
        "create prompt should NOT include research instruction when none: {prompt}"
    );
}

#[test]
fn test_build_specify_prompt_includes_research_frontmatter() {
    let prompt =
        SpecCommand::build_specify_prompt("myspec", "Add notifications", Some("realtime-collab"));
    assert!(
        prompt.contains("research:"),
        "specify prompt should include research frontmatter instruction: {prompt}"
    );
    assert!(
        prompt.contains("\"realtime-collab\""),
        "specify prompt should contain the research name: {prompt}"
    );
}

#[test]
fn test_build_specify_prompt_without_research() {
    let prompt = SpecCommand::build_specify_prompt("myspec", "Add notifications", None);
    assert!(
        !prompt.contains("research:"),
        "specify prompt should NOT include research instruction when none: {prompt}"
    );
}

#[test]
fn test_spec_help_specify_mentions_from_research() {
    let help = SpecCommand::build_help_message();
    assert!(
        help.contains("--from-research"),
        "help should mention --from-research for specify: {help}"
    );
}

// ── Tasks handler tests (FR-005, T-006) ───────────────────────────────────

#[test]
fn test_tasks_status_mentions_tasks_md() {
    let status = SpecCommand::build_tasks_status("myspec");
    assert!(
        status.contains("specs/myspec/TASKS.md"),
        "status should mention TASKS.md path: {status}"
    );
    assert!(
        status.contains("PLAN.md"),
        "status should mention PLAN.md: {status}"
    );
}

#[test]
fn test_tasks_message_mentions_extraction() {
    let msg = SpecCommand::build_tasks_message("myspec");
    assert!(
        msg.contains("specs/myspec/PLAN.md"),
        "message should mention PLAN.md: {msg}"
    );
    assert!(
        msg.contains("specs/myspec/TASKS.md"),
        "message should mention TASKS.md: {msg}"
    );
}

#[test]
fn test_tasks_log_mentions_both_files() {
    let log = SpecCommand::build_tasks_log("myspec");
    assert!(
        log.contains("specs/myspec/TASKS.md"),
        "log should mention TASKS.md: {log}"
    );
    assert!(log.contains("PLAN.md"), "log should mention PLAN.md: {log}");
}

#[test]
fn test_tasks_completion_message_includes_count() {
    let msg = SpecCommand::build_tasks_completion_message("myspec", 5);
    assert!(
        msg.contains("5 task(s)"),
        "completion message should include task count: {msg}"
    );
    assert!(
        msg.contains("specs/myspec/TASKS.md"),
        "completion message should mention TASKS.md: {msg}"
    );
}

#[test]
fn test_tasks_no_plan_error_message() {
    let msg = SpecCommand::build_tasks_no_plan_error("myspec");
    assert!(
        msg.contains("specs/myspec/PLAN.md"),
        "no-plan error should mention PLAN.md: {msg}"
    );
    assert!(
        msg.contains("/spec plan"),
        "no-plan error should suggest /spec plan: {msg}"
    );
}

#[test]
fn test_tasks_no_tasks_error_message() {
    let msg = SpecCommand::build_tasks_no_tasks_error("myspec");
    assert!(
        msg.contains("specs/myspec/PLAN.md"),
        "no-tasks error should mention PLAN.md: {msg}"
    );
    assert!(
        msg.contains("## Tasks"),
        "no-tasks error should mention ## Tasks table: {msg}"
    );
}

#[test]
fn test_build_tasks_md_extracts_task_table() {
    let plan_md = r"
# Plan

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Define types | FR-003 | S | Critical | — |
| T-002 | Build parser | FR-004 | M | High | T-001 |
| T-003 | Add tests | FR-005 | M | High | T-002 |

## Details
";
    let md = SpecCommand::build_tasks_md("myspec", "My Spec", plan_md)
        .expect("should extract tasks from valid PLAN.md");
    assert!(md.contains("# Tasks"), "TASKS.md should have a title: {md}");
    assert!(
        md.contains("specs/myspec/PLAN.md"),
        "TASKS.md should reference source PLAN.md: {md}"
    );
    assert!(
        md.contains("My Spec"),
        "TASKS.md should include spec title: {md}"
    );
    assert!(
        md.contains("T-001"),
        "TASKS.md should contain task T-001: {md}"
    );
    assert!(
        md.contains("T-002"),
        "TASKS.md should contain task T-002: {md}"
    );
    assert!(
        md.contains("T-003"),
        "TASKS.md should contain task T-003: {md}"
    );
    assert!(
        md.contains("| ID | Title |"),
        "TASKS.md should have a table header: {md}"
    );
    assert!(
        md.contains("pending"),
        "TASKS.md should include default task status: {md}"
    );
}

#[test]
fn test_build_tasks_md_with_status_column() {
    let plan_md = r"
## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|---|---|---|---|---|---|---|
| T-001 | Define types | FR-003 | S | Critical | completed | — |
| T-002 | Build parser | FR-004 | M | High | in_progress | T-001 |
";
    let md = SpecCommand::build_tasks_md("myspec", "", plan_md)
        .expect("should extract tasks with status column");
    assert!(
        md.contains("completed"),
        "TASKS.md should preserve completed status: {md}"
    );
    assert!(
        md.contains("in_progress"),
        "TASKS.md should preserve in_progress status: {md}"
    );
}

#[test]
fn test_build_tasks_md_returns_none_for_empty_plan() {
    let plan_md = "# Plan\n\nNo tasks here.\n";
    let result = SpecCommand::build_tasks_md("myspec", "My Spec", plan_md);
    assert!(
        result.is_none(),
        "should return None when no task table exists"
    );
}

#[test]
fn test_build_tasks_md_returns_none_for_empty_string() {
    let result = SpecCommand::build_tasks_md("myspec", "My Spec", "");
    assert!(
        result.is_none(),
        "should return None for empty PLAN.md content"
    );
}

#[test]
fn test_build_tasks_md_includes_footer() {
    let plan_md = r"
## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Task one | FR-001 | S | High | — |
";
    let md =
        SpecCommand::build_tasks_md("myspec", "My Spec", plan_md).expect("should extract tasks");
    assert!(
        md.contains("/spec tasks"),
        "TASKS.md footer should mention /spec tasks: {md}"
    );
    assert!(
        md.contains("PLAN.md"),
        "TASKS.md footer should mention PLAN.md: {md}"
    );
}

#[test]
fn test_build_tasks_md_handles_dependencies() {
    let plan_md = r"
## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | First | FR-001 | S | High | — |
| T-002 | Second | FR-002 | M | Medium | T-001 |
| T-003 | Third | FR-003 | L | Low | T-001, T-002 |
";
    let md =
        SpecCommand::build_tasks_md("myspec", "My Spec", plan_md).expect("should extract tasks");
    assert!(
        md.contains("T-001, T-002"),
        "TASKS.md should join multiple dependencies: {md}"
    );
    assert!(
        md.contains("—"),
        "TASKS.md should show em-dash for no dependencies: {md}"
    );
}

#[test]
fn test_build_tasks_md_without_title() {
    let plan_md = r"
## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|---|---|---|---|---|---|
| T-001 | Task one | FR-001 | S | High | — |
";
    let md = SpecCommand::build_tasks_md("myspec", "", plan_md)
        .expect("should extract tasks even without title");
    assert!(
        !md.contains("**Spec:**"),
        "TASKS.md should not include Spec line when title is empty: {md}"
    );
    assert!(
        md.contains("T-001"),
        "TASKS.md should still contain tasks: {md}"
    );
}
// ── Feedback surfacing tests (FR-017, T-033) ───────────────────────────────

#[test]
fn test_build_feedback_instruction_contains_content() {
    let instruction = SpecCommand::build_feedback_instruction(
        "myspec",
        "# Feedback: My Spec\n\n| Date | Note |\n|---|---|\n| 2026-01-01 | Latency spike |",
    );
    assert!(
        instruction.contains("Production feedback notes"),
        "instruction should mention feedback notes: {instruction}"
    );
    assert!(
        instruction.contains("specs/myspec/FEEDBACK.md"),
        "instruction should reference FEEDBACK.md path: {instruction}"
    );
    assert!(
        instruction.contains("Latency spike"),
        "instruction should embed the feedback content: {instruction}"
    );
    assert!(
        instruction.contains("Consider the above production feedback notes"),
        "instruction should tell agent to consider feedback: {instruction}"
    );
}

#[test]
fn test_build_feedback_instruction_empty_content() {
    let instruction = SpecCommand::build_feedback_instruction("myspec", "");
    // Even with empty content, the instruction structure should be present
    assert!(
        instruction.contains("Production feedback notes"),
        "instruction should mention feedback notes even with empty content: {instruction}"
    );
}

#[test]
fn test_build_feedback_instruction_includes_spec_id() {
    let instruction = SpecCommand::build_feedback_instruction("my-awesome-spec", "some note");
    assert!(
        instruction.contains("specs/my-awesome-spec/FEEDBACK.md"),
        "instruction should contain spec id: {instruction}"
    );
}

#[test]
fn test_plan_prompt_with_feedback_includes_content() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let feedback =
        "# Feedback: Test\n\n| Date | Note |\n|---|---|\n| 2026-01-01 | Latency spike to 500ms |";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "WebSocket", spec_md, "", false, false, feedback);
    assert!(
        prompt.contains("Production feedback notes"),
        "prompt should include feedback section: {prompt}"
    );
    assert!(
        prompt.contains("Latency spike to 500ms"),
        "prompt should embed feedback content: {prompt}"
    );
    assert!(
        prompt.contains("Consider the above production feedback notes"),
        "prompt should instruct agent to consider feedback: {prompt}"
    );
}

#[test]
fn test_plan_prompt_without_feedback_excludes_section() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "WebSocket", spec_md, "", false, false, "");
    assert!(
        !prompt.contains("Production feedback notes"),
        "prompt should NOT include feedback section when feedback_md is empty: {prompt}"
    );
}

#[test]
fn test_plan_prompt_with_whitespace_only_feedback_excludes_section() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let prompt = SpecCommand::build_plan_prompt(
        "myspec",
        "WebSocket",
        spec_md,
        "",
        false,
        false,
        "   \n  \n",
    );
    assert!(
        !prompt.contains("Production feedback notes"),
        "prompt should NOT include feedback section when feedback_md is only whitespace: {prompt}"
    );
}

#[test]
fn test_plan_prompt_feedback_and_data_model_both_enabled() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let feedback = "| 2026-01-01 | Memory leak in worker pool |";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "PostgreSQL", spec_md, "", true, false, feedback);
    assert!(
        prompt.contains("data-model.md"),
        "prompt should include data-model instruction: {prompt}"
    );
    assert!(
        prompt.contains("Production feedback notes"),
        "prompt should include feedback section: {prompt}"
    );
    assert!(
        prompt.contains("Memory leak in worker pool"),
        "prompt should embed feedback content: {prompt}"
    );
}

#[test]
fn test_plan_prompt_feedback_and_contracts_both_enabled() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let feedback = "| 2026-01-01 | API timeout on /export endpoint |";
    let prompt =
        SpecCommand::build_plan_prompt("myspec", "axum", spec_md, "", false, true, feedback);
    assert!(
        prompt.contains("contracts/"),
        "prompt should include contracts instruction: {prompt}"
    );
    assert!(
        prompt.contains("Production feedback notes"),
        "prompt should include feedback section: {prompt}"
    );
}

#[test]
fn test_plan_prompt_all_three_artifacts_enabled() {
    let spec_md = "## Requirements\n### FR-001\nThe system shall do X.";
    let feedback = "| 2026-01-01 | Crash under high load |";
    let prompt = SpecCommand::build_plan_prompt(
        "myspec",
        "axum + PostgreSQL",
        spec_md,
        "",
        true,
        true,
        feedback,
    );
    assert!(prompt.contains("data-model.md"));
    assert!(prompt.contains("contracts/"));
    assert!(prompt.contains("Production feedback notes"));
    assert!(prompt.contains("Crash under high load"));
}

#[test]
fn test_plan_message_with_feedback_enabled_mentions_feedback() {
    let msg = SpecCommand::build_plan_message("myspec", "PostgreSQL", false, false, true);
    assert!(
        msg.contains("FEEDBACK.md"),
        "message should mention FEEDBACK.md when feedback_enabled: {msg}"
    );
    assert!(
        msg.contains("feedback notes"),
        "message should mention feedback notes when feedback_enabled: {msg}"
    );
}

#[test]
fn test_plan_message_without_feedback_does_not_mention_feedback() {
    let msg = SpecCommand::build_plan_message("myspec", "PostgreSQL", false, false, false);
    assert!(
        !msg.contains("FEEDBACK.md"),
        "message should NOT mention FEEDBACK.md when feedback disabled: {msg}"
    );
    assert!(
        !msg.contains("feedback notes"),
        "message should NOT mention feedback notes when feedback disabled: {msg}"
    );
}

#[test]
fn test_plan_message_feedback_and_data_model_both_enabled() {
    let msg = SpecCommand::build_plan_message("myspec", "PostgreSQL", true, false, true);
    assert!(msg.contains("data-model.md"));
    assert!(msg.contains("FEEDBACK.md"));
}

#[test]
fn test_plan_message_feedback_and_contracts_both_enabled() {
    let msg = SpecCommand::build_plan_message("myspec", "axum", false, true, true);
    assert!(msg.contains("contracts/"));
    assert!(msg.contains("FEEDBACK.md"));
}
// ── Related Research section instruction tests (FR-010, T-020) ─────────────

#[test]
fn test_build_research_section_instruction_with_name() {
    let instruction = SpecCommand::build_research_section_instruction(Some("auth-research"));
    assert!(
        instruction.contains("## Related Research"),
        "instruction should mention ## Related Research section: {instruction}"
    );
    assert!(
        instruction.contains("auth-research"),
        "instruction should contain the research name: {instruction}"
    );
    assert!(
        instruction.contains("../research/auth-research/RESEARCH.md"),
        "instruction should contain the research path link: {instruction}"
    );
}

#[test]
fn test_build_research_section_instruction_without_name() {
    let instruction = SpecCommand::build_research_section_instruction(None);
    assert!(
        instruction.is_empty(),
        "instruction should be empty when no research name: {instruction}"
    );
}

#[test]
fn test_build_create_prompt_includes_research_section() {
    let prompt = SpecCommand::build_create_prompt("myspec", "Add auth", Some("auth-research"));
    assert!(
        prompt.contains("## Related Research"),
        "create prompt should include ## Related Research section instruction: {prompt}"
    );
    assert!(
        prompt.contains("../research/auth-research/RESEARCH.md"),
        "create prompt should contain the research path link: {prompt}"
    );
}

#[test]
fn test_build_create_prompt_without_research_section() {
    let prompt = SpecCommand::build_create_prompt("myspec", "Add auth", None);
    assert!(
        !prompt.contains("## Related Research"),
        "create prompt should NOT include ## Related Research section when no research: {prompt}"
    );
}

#[test]
fn test_build_specify_prompt_includes_research_section() {
    let prompt =
        SpecCommand::build_specify_prompt("myspec", "Add notifications", Some("realtime-collab"));
    assert!(
        prompt.contains("## Related Research"),
        "specify prompt should include ## Related Research section instruction: {prompt}"
    );
    assert!(
        prompt.contains("../research/realtime-collab/RESEARCH.md"),
        "specify prompt should contain the research path link: {prompt}"
    );
}

#[test]
fn test_build_specify_prompt_without_research_section() {
    let prompt = SpecCommand::build_specify_prompt("myspec", "Add notifications", None);
    assert!(
        !prompt.contains("## Related Research"),
        "specify prompt should NOT include ## Related Research section when no research: {prompt}"
    );
}

#[test]
fn test_build_create_prompt_with_research_includes_both_frontmatter_and_section() {
    let prompt = SpecCommand::build_create_prompt("myspec", "Add auth", Some("auth-research"));
    // FR-010: both frontmatter link AND body section should be present
    assert!(
        prompt.contains("research:"),
        "create prompt should include frontmatter research field: {prompt}"
    );
    assert!(
        prompt.contains("## Related Research"),
        "create prompt should include body section instruction: {prompt}"
    );
}

#[test]
fn test_build_specify_prompt_with_research_includes_both_frontmatter_and_section() {
    let prompt =
        SpecCommand::build_specify_prompt("myspec", "Add notifications", Some("realtime-collab"));
    assert!(
        prompt.contains("research:"),
        "specify prompt should include frontmatter research field: {prompt}"
    );
    assert!(
        prompt.contains("## Related Research"),
        "specify prompt should include body section instruction: {prompt}"
    );
}
// ── Quickstart.md generation tests (FR-013, T-023) ───────────────────────

#[test]
fn test_build_quickstart_md_with_requirements() {
    let spec_md = r"
## Requirements

### FR-001 — User Login
`The system shall allow users to log in with email and password.`

### FR-002 — Password Reset
`The system shall send a password reset email when requested.`
";
    let md = SpecCommand::build_quickstart_md("myspec", "My Spec", spec_md)
        .expect("should generate quickstart from requirements");
    assert!(
        md.contains("# Quickstart Validation Scenarios"),
        "quickstart should have title heading: {md}"
    );
    assert!(
        md.contains("specs/myspec/SPEC.md"),
        "quickstart should reference source SPEC.md: {md}"
    );
    assert!(
        md.contains("**Spec:** My Spec"),
        "quickstart should include spec title: {md}"
    );
    assert!(
        md.contains("### FR-001"),
        "quickstart should include FR-001 scenario: {md}"
    );
    assert!(
        md.contains("### FR-002"),
        "quickstart should include FR-002 scenario: {md}"
    );
    assert!(
        md.contains("Verify that: The system shall allow users to log in"),
        "quickstart should include EARS verification text: {md}"
    );
    assert!(
        md.contains("Verify that: The system shall send a password reset"),
        "quickstart should include FR-002 EARS text: {md}"
    );
}

#[test]
fn test_build_quickstart_md_without_title() {
    let spec_md = r"
### FR-001 — Basic Feature
`The system shall do something useful.`
";
    let md = SpecCommand::build_quickstart_md("myspec", "", spec_md)
        .expect("should generate even without title");
    assert!(
        !md.contains("**Spec:**"),
        "quickstart should not include Spec line when title is empty: {md}"
    );
    assert!(
        md.contains("### FR-001"),
        "quickstart should still contain scenarios: {md}"
    );
}

#[test]
fn test_build_quickstart_md_returns_none_for_no_requirements() {
    let spec_md = "# Specification: Empty\n\nNo requirements here.";
    let result = SpecCommand::build_quickstart_md("myspec", "Empty", spec_md);
    assert!(
        result.is_none(),
        "quickstart should be None when no requirements found"
    );
}

#[test]
fn test_build_quickstart_md_returns_none_for_empty_spec() {
    let result = SpecCommand::build_quickstart_md("myspec", "Empty", "");
    assert!(
        result.is_none(),
        "quickstart should be None for empty SPEC.md"
    );
}

#[test]
fn test_build_quickstart_md_includes_intro_text() {
    let spec_md = r"
### FR-001 — Feature One
`The system shall provide feature one.`
";
    let md = SpecCommand::build_quickstart_md("myspec", "My Spec", spec_md)
        .expect("should generate quickstart");
    assert!(
        md.contains("smoke-test scenarios"),
        "quickstart should contain intro explaining purpose: {md}"
    );
    assert!(
        md.contains("TESTPLAN.md"),
        "quickstart should reference full TESTPLAN.md: {md}"
    );
}

#[test]
fn test_build_quickstart_md_includes_footer() {
    let spec_md = r"
### FR-001 — Feature One
`The system shall provide feature one.`
";
    let md = SpecCommand::build_quickstart_md("myspec", "My Spec", spec_md)
        .expect("should generate quickstart");
    assert!(
        md.contains("generated by `/spec tasks`"),
        "quickstart should contain footer noting generation source: {md}"
    );
    assert!(
        md.contains("Edit `SPEC.md`"),
        "quickstart footer should mention editing SPEC.md: {md}"
    );
}

#[test]
fn test_build_quickstart_md_handles_inline_requirement_format() {
    let spec_md = r"
FR-001.  The system shall validate user input before processing.
";
    let md = SpecCommand::build_quickstart_md("myspec", "My Spec", spec_md)
        .expect("should parse inline format");
    assert!(
        md.contains("### FR-001"),
        "quickstart should include FR-001 from inline format: {md}"
    );
    assert!(
        md.contains("validate user input"),
        "quickstart should include EARS text from inline format: {md}"
    );
}

#[test]
fn test_build_tasks_completion_message_mentions_quickstart() {
    let msg = SpecCommand::build_tasks_completion_message("myspec", 5);
    assert!(
        msg.contains("quickstart.md"),
        "completion message should mention quickstart.md: {msg}"
    );
    assert!(
        msg.contains("5 task(s)"),
        "completion message should mention task count: {msg}"
    );
    assert!(
        msg.contains("validation scenarios"),
        "completion message should mention validation scenarios: {msg}"
    );
}

#[test]
fn test_build_tasks_message_mentions_quickstart() {
    let msg = SpecCommand::build_tasks_message("myspec");
    assert!(
        msg.contains("quickstart.md"),
        "tasks message should mention quickstart.md: {msg}"
    );
    assert!(
        msg.contains("validation scenarios"),
        "tasks message should mention validation scenarios: {msg}"
    );
}

#[test]
fn test_build_quickstart_md_multiple_requirements_all_included() {
    let spec_md = r"
### FR-001 — First
`The system shall do first thing.`

### FR-002 — Second
`The system shall do second thing.`

### FR-003 — Third
`The system shall do third thing.`
";
    let md = SpecCommand::build_quickstart_md("myspec", "Multi", spec_md)
        .expect("should generate quickstart");
    for id in &["FR-001", "FR-002", "FR-003"] {
        assert!(
            md.contains(&format!("### {id}")),
            "quickstart should include {id}: {md}"
        );
    }
    for text in &["first thing", "second thing", "third thing"] {
        assert!(
            md.contains(text),
            "quickstart should contain '{text}': {md}"
        );
    }
}
