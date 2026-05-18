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
    assert!(help.contains("spec create"), "should mention spec create: {help}");
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
        "spec: writing specs/websocket/SPEC.md + specs/websocket/PLAN.md…",
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

    let log = SpecCommand::build_create_log(&specname, &feature);
    assert!(
        log.contains("specs/websocket/SPEC.md"),
        "log should contain spec file path"
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
