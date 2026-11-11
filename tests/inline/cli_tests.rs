//! Inline tests for `cli.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use clap::Parser;

use super::ResearchCommands;

/// Wrapper so `ResearchCommands` can be parsed as a standalone CLI in tests.
#[derive(Parser, Debug)]
struct TestCli {
    #[command(subcommand)]
    command: ResearchCommands,
}

/// Wrapper so `SpecCommands` can be parsed as a standalone CLI in tests.
#[derive(Parser, Debug)]
struct SpecCli {
    #[command(subcommand)]
    command: super::SpecCommands,
}

#[test]
fn govcreate_to_token_string_rebuilds_positionals_and_flags() {
    let cli = SpecCli::parse_from([
        "spec",
        "govcreate",
        "my-spec",
        "docs/arch.md",
        "./target-dir",
        "--language",
        "rust",
        "--type",
        "cmdline",
        "--stack",
        "axum",
        "--github",
        "--force",
    ]);
    let super::SpecCommands::GovCreate(args) = cli.command;
    assert_eq!(
        args.to_token_string(),
        "my-spec docs/arch.md ./target-dir --language rust --type cmdline \
         --stack axum --github --force"
    );
}

#[test]
fn govcreate_to_token_string_omits_unset_optional_flags() {
    let cli = SpecCli::parse_from([
        "spec",
        "govcreate",
        "my-spec",
        "https://example.gov/docs",
        "/tmp/out",
        "--language",
        "python",
        "--type",
        "library",
    ]);
    let super::SpecCommands::GovCreate(args) = cli.command;
    assert_eq!(
        args.to_token_string(),
        "my-spec https://example.gov/docs /tmp/out --language python --type library"
    );
}

#[test]
fn govcreate_github_gitlab_conflict_is_rejected_by_clap() {
    let err = SpecCli::try_parse_from([
        "spec",
        "govcreate",
        "my-spec",
        "docs/arch.md",
        "./out",
        "--language",
        "rust",
        "--type",
        "cmdline",
        "--github",
        "--gitlab",
    ])
    .expect_err("conflicting hosting flags must be rejected");
    assert!(
        err.to_string().contains("gitlab"),
        "error should name the conflicting flag: {err}"
    );
}

#[test]
fn govcreate_token_round_trip_through_shared_parser() {
    // FR-020: the rebuilt tokens must parse through the single shared
    // `SpecCommand::parse` into the same GovCreate variant fields.
    use ragent_specs::SpecCommand;
    let cli = SpecCli::parse_from([
        "spec",
        "govcreate",
        "payments-arch",
        "https://docs.example.gov/payments",
        "./payments-svc",
        "--language",
        "rust",
        "--type",
        "cmdline",
        "--force",
    ]);
    let super::SpecCommands::GovCreate(args) = cli.command;
    let parsed = SpecCommand::parse(&format!("govcreate {}", args.to_token_string()));
    let SpecCommand::GovCreate {
        spec_id,
        content_ref,
        target_folder,
        scaffold,
        force,
    } = parsed
    else {
        panic!("expected GovCreate, got {parsed:?}");
    };
    assert_eq!(spec_id, "payments-arch");
    assert_eq!(content_ref, "https://docs.example.gov/payments");
    assert_eq!(target_folder, "./payments-svc");
    assert!(force);
    assert_eq!(
        scaffold.language().as_str(),
        "rust",
        "shared parser must accept the rebuilt language flag"
    );
}

#[test]
fn govcreate_invalid_spec_id_surfaces_usage_variant_via_shared_parser() {
    use ragent_specs::SpecCommand;
    let cli = SpecCli::parse_from([
        "spec",
        "govcreate",
        "bad$id",
        "docs/arch.md",
        "./out",
        "--language",
        "rust",
        "--type",
        "cmdline",
    ]);
    let super::SpecCommands::GovCreate(args) = cli.command;
    let parsed = SpecCommand::parse(&format!("govcreate {}", args.to_token_string()));
    assert!(
        matches!(parsed, SpecCommand::GovCreateUsage(_)),
        "invalid spec id must produce the usage variant: {parsed:?}"
    );
}

#[test]
fn govcreate_usage_message_drops_tui_header_and_rewrites_surface_name() {
    let body = super::govcreate_usage_message();
    assert!(
        !body.contains("From: /spec govcreate"),
        "CLI usage must not carry the TUI From header: {body}"
    );
    assert!(body.contains("Usage:"), "usage block kept: {body}");
    let rewritten = body.replace("/spec govcreate", "ragent spec govcreate");
    assert!(
        rewritten.contains("ragent spec govcreate <specid>"),
        "surface name rewritten: {rewritten}"
    );
}

#[test]
fn split_authored_sections_no_markers_treats_body_as_spec() {
    let (spec, plan, testplan) =
        ragent_tools_extended::archdoc::split_authored_sections("# The spec body\n");
    assert_eq!(spec, "# The spec body\n");
    assert!(plan.contains("to be filled"));
    assert!(testplan.contains("to be filled"));
}

#[test]
fn split_authored_sections_three_markers_assign_files() {
    let body = "1. `out/specs/x/SPEC.md`\nspec body\n2. `out/specs/x/PLAN.md`\nplan body\n3. `out/specs/x/TESTPLAN.md`\ntest body\n";
    let (spec, plan, testplan) = ragent_tools_extended::archdoc::split_authored_sections(body);
    assert!(spec.contains("spec body"), "spec section: {spec}");
    assert!(plan.contains("plan body"), "plan section: {plan}");
    assert!(testplan.contains("test body"), "testplan: {testplan}");
}

#[test]
fn web_time_alias_parses_to_web_phase_timeout() {
    let cli = TestCli::parse_from([
        "research",
        "create",
        "--web-time",
        "90",
        "my-name",
        "my topic",
    ]);
    match cli.command {
        ResearchCommands::Create {
            web_phase_timeout_secs,
            ..
        } => assert_eq!(web_phase_timeout_secs, Some(90)),
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn web_time_zero_disables_deadline() {
    let cli = TestCli::parse_from([
        "research",
        "create",
        "--web-time",
        "0",
        "my-name",
        "my topic",
    ]);
    match cli.command {
        ResearchCommands::Create {
            web_phase_timeout_secs,
            ..
        } => assert_eq!(web_phase_timeout_secs, Some(0)),
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn long_form_web_phase_timeout_still_parses() {
    let cli = TestCli::parse_from([
        "research",
        "create",
        "--web-phase-timeout-secs",
        "120",
        "my-name",
        "my topic",
    ]);
    match cli.command {
        ResearchCommands::Create {
            web_phase_timeout_secs,
            ..
        } => assert_eq!(web_phase_timeout_secs, Some(120)),
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn oa_flags_parse_independently() {
    // Neither flag leaves the per-run override unset.
    let cli = TestCli::parse_from(["research", "create", "my-name", "my topic"]);
    match cli.command {
        ResearchCommands::Create {
            oa_enable, no_oa, ..
        } => {
            assert!(!oa_enable);
            assert!(!no_oa);
        }
        other => panic!("expected Create, got {other:?}"),
    }

    let cli = TestCli::parse_from(["research", "create", "--oa-enable", "my-name", "my topic"]);
    match cli.command {
        ResearchCommands::Create {
            oa_enable, no_oa, ..
        } => {
            assert!(oa_enable);
            assert!(!no_oa);
        }
        other => panic!("expected Create, got {other:?}"),
    }

    let cli = TestCli::parse_from(["research", "create", "--no-oa", "my-name", "my topic"]);
    match cli.command {
        ResearchCommands::Create {
            oa_enable, no_oa, ..
        } => {
            assert!(!oa_enable);
            assert!(no_oa);
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn papers_flag_defaults_to_excluding_scholarly() {
    // FR-005: scholarly engines are excluded by default; `--papers` opts
    // them back in.
    let cli = TestCli::parse_from(["research", "create", "my-name", "my topic"]);
    match cli.command {
        ResearchCommands::Create { papers, .. } => assert!(!papers),
        other => panic!("expected Create, got {other:?}"),
    }

    let cli = TestCli::parse_from(["research", "create", "--papers", "my-name", "my topic"]);
    match cli.command {
        ResearchCommands::Create { papers, .. } => assert!(papers),
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn output_limit_flags_parse_and_default_to_none() {
    // FR-006 / FR-007: both limits are optional; omitting them leaves the
    // decision to the config/default resolution downstream.
    let cli = TestCli::parse_from(["research", "create", "my-name", "my topic"]);
    match cli.command {
        ResearchCommands::Create {
            max_concepts,
            max_findings,
            ..
        } => {
            assert_eq!(max_concepts, None);
            assert_eq!(max_findings, None);
        }
        other => panic!("expected Create, got {other:?}"),
    }

    let cli = TestCli::parse_from([
        "research",
        "create",
        "--max-concepts",
        "3",
        "--max-findings",
        "9",
        "my-name",
        "my topic",
    ]);
    match cli.command {
        ResearchCommands::Create {
            max_concepts,
            max_findings,
            ..
        } => {
            assert_eq!(max_concepts, Some(3));
            assert_eq!(max_findings, Some(9));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn output_limit_zero_is_preserved_as_unbounded() {
    // FR-016: `0` is a meaningful "unbounded" sentinel, so it must parse
    // as `Some(0)` rather than collapsing to `None`.
    let cli = TestCli::parse_from([
        "research",
        "create",
        "--max-concepts",
        "0",
        "--max-findings",
        "0",
        "my-name",
        "my topic",
    ]);
    match cli.command {
        ResearchCommands::Create {
            max_concepts,
            max_findings,
            ..
        } => {
            assert_eq!(max_concepts, Some(0));
            assert_eq!(max_findings, Some(0));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn output_limit_rejects_non_numeric_value() {
    // FR-019: a malformed value is rejected by clap rather than silently
    // defaulting.
    let err = TestCli::try_parse_from([
        "research",
        "create",
        "--max-concepts",
        "abc",
        "my-name",
        "my topic",
    ])
    .expect_err("non-numeric --max-concepts must be rejected");
    assert!(
        err.to_string().contains("max-concepts"),
        "error should name the offending flag: {err}"
    );
}

#[test]
fn output_limit_help_lists_both_flags() {
    let mut command = <TestCli as clap::CommandFactory>::command();
    let create = command
        .find_subcommand_mut("create")
        .expect("create subcommand exists");
    let help = create.render_long_help().to_string();
    assert!(help.contains("--max-concepts"), "{help}");
    assert!(help.contains("--max-findings"), "{help}");
}

/// Regression (#24): a flag after the topic is parsed as a flag, not folded
/// into the topic text.
#[test]
fn flags_after_the_topic_are_parsed_as_flags() {
    let cli = TestCli::parse_from([
        "research",
        "create",
        "my-name",
        "rust async runtimes",
        "--depth",
        "deep",
    ]);
    match cli.command {
        ResearchCommands::Create { topic, depth, .. } => {
            assert_eq!(topic, vec!["rust async runtimes".to_string()]);
            assert_eq!(depth.as_deref(), Some("deep"));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

/// Regression (#24): an unknown flag after the topic is an error, not topic
/// text.
#[test]
fn unknown_flag_after_the_topic_is_an_error() {
    let result = TestCli::try_parse_from(["research", "create", "my-name", "topic", "--bogusflag"]);
    assert!(result.is_err(), "unknown flag must be rejected: {result:?}");
}

/// Multi-word topics given as separate words still join in order.
#[test]
fn multi_word_topic_words_are_collected() {
    let cli = TestCli::parse_from([
        "research",
        "create",
        "my-name",
        "rust",
        "async",
        "--iterations",
        "2",
    ]);
    match cli.command {
        ResearchCommands::Create {
            topic, iterations, ..
        } => {
            assert_eq!(topic, vec!["rust", "async"]);
            assert_eq!(iterations, Some(2));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}
