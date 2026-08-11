use ragent_specs::SpecCommand;

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
    let SpecCommand::Create { specname, feature } = cmd else {
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

    let prompt = SpecCommand::build_create_prompt(&specname, &feature);
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
    let prompt = SpecCommand::build_create_prompt(specname, feature);

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
    let prompt = SpecCommand::build_create_prompt(specname, feature);

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
