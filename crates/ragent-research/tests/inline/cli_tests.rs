//! Inline tests for `cli.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn parse_create_basic() {
    let cmd = ResearchCliCommand::parse("create rust-async async/await idioms in stable Rust");
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            sources_dir,
            template,
            use_local,
            use_specs,
            ..
        } => {
            assert_eq!(name, "rust-async");
            assert_eq!(topic, "async/await idioms in stable Rust");
            assert!(sources_dir.is_none());
            assert!(template.is_none());
            assert!(!use_local);
            assert!(!use_specs);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_create_with_sources_dir_and_template() {
    let cmd = ResearchCliCommand::parse(
        "create foo topic words --sources-dir /tmp/notes --template deepdive",
    );
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            sources_dir,
            template,
            use_local,
            use_specs,
            ..
        } => {
            assert_eq!(name, "foo");
            assert_eq!(topic, "topic words");
            assert_eq!(sources_dir.as_deref(), Some("/tmp/notes"));
            assert_eq!(template.as_deref(), Some("deepdive"));
            assert!(!use_local);
            assert!(!use_specs);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_create_with_flags() {
    let cmd = ResearchCliCommand::parse(
        "create bar topic --use-local --use-specs --use-low-relevance --papers --use-pdf",
    );
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            use_local,
            use_specs,
            use_low_relevance,
            papers,
            use_pdf,
            ..
        } => {
            assert_eq!(name, "bar");
            assert_eq!(topic, "topic");
            assert!(use_local);
            assert!(use_specs);
            assert!(use_low_relevance);
            assert!(papers);
            assert!(use_pdf);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_create_papers_defaults_off() {
    // FR-005: scholarly engines are excluded unless `--papers` is supplied.
    match ResearchCliCommand::parse("create default topic") {
        ResearchCliCommand::Create { papers, .. } => assert!(!papers),
        other => panic!("unexpected variant: {other:?}"),
    }
    match ResearchCliCommand::parse("create papers topic --papers") {
        ResearchCliCommand::Create { papers, .. } => {
            assert!(papers, "--papers must opt scholarly engines back in")
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_create_with_oa_flags() {
    // No flag defers to config.
    match ResearchCliCommand::parse("create noflag topic") {
        ResearchCliCommand::Create { oa_recovery, .. } => assert_eq!(oa_recovery, None),
        other => panic!("unexpected variant: {other:?}"),
    }
    match ResearchCliCommand::parse("create on topic --oa-enable") {
        ResearchCliCommand::Create { oa_recovery, .. } => {
            assert_eq!(oa_recovery, Some(true));
        }
        other => panic!("unexpected variant: {other:?}"),
    }
    match ResearchCliCommand::parse("create off topic --no-oa") {
        ResearchCliCommand::Create { oa_recovery, .. } => {
            assert_eq!(oa_recovery, Some(false));
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_create_with_model_flags() {
    let cmd = ResearchCliCommand::parse(
        "create baz topic --research-model openai:gpt-4.1 --compression-model openai:gpt-4.1-mini --final-report-model anthropic:claude-sonnet-4",
    );
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            research_model,
            compression_model,
            final_report_model,
            ..
        } => {
            assert_eq!(name, "baz");
            assert_eq!(topic, "topic");
            assert_eq!(research_model.as_deref(), Some("openai:gpt-4.1"));
            assert_eq!(compression_model.as_deref(), Some("openai:gpt-4.1-mini"));
            assert_eq!(
                final_report_model.as_deref(),
                Some("anthropic:claude-sonnet-4")
            );
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_create_with_max_concurrent_research_units() {
    let cmd = ResearchCliCommand::parse("create qux topic --max-concurrent-research-units 4");
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            max_concurrent_research_units,
            ..
        } => {
            assert_eq!(name, "qux");
            assert_eq!(topic, "topic");
            assert_eq!(max_concurrent_research_units, Some(4));
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_create_with_mode() {
    let cmd = ResearchCliCommand::parse("create comp topic --mode competitive");
    match cmd {
        ResearchCliCommand::Create {
            name, topic, mode, ..
        } => {
            assert_eq!(name, "comp");
            assert_eq!(topic, "topic");
            assert_eq!(mode.as_deref(), Some("competitive"));
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_list_json() {
    assert_eq!(
        ResearchCliCommand::parse("list --json"),
        ResearchCliCommand::List {
            all: false,
            json: true
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("list --all"),
        ResearchCliCommand::List {
            all: true,
            json: false
        }
    );
}

#[test]
fn parse_show_defaults_to_json_false() {
    match ResearchCliCommand::parse("show my-item") {
        ResearchCliCommand::Show { name, json } => {
            assert_eq!(name, "my-item");
            assert!(!json);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_search_joins_positional_words() {
    match ResearchCliCommand::parse("search rust async runtimes") {
        ResearchCliCommand::Search { query, json } => {
            assert_eq!(query, "rust async runtimes");
            assert!(!json);
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_delete_requires_name() {
    match ResearchCliCommand::parse("delete doomed") {
        ResearchCliCommand::Delete { name, .. } => assert_eq!(name, "doomed"),
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_export_with_output() {
    match ResearchCliCommand::parse("export item --output /tmp/out") {
        ResearchCliCommand::Export { name, output } => {
            assert_eq!(name, "item");
            assert_eq!(output.as_deref(), Some("/tmp/out"));
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_import_with_name_override() {
    match ResearchCliCommand::parse("import /tmp/item.md --name renamed") {
        ResearchCliCommand::Import { path, name } => {
            assert_eq!(path, "/tmp/item.md");
            assert_eq!(name.as_deref(), Some("renamed"));
        }
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn parse_unknown_verb() {
    assert_eq!(
        ResearchCliCommand::parse("frobnicate"),
        ResearchCliCommand::Unknown("frobnicate".to_string())
    );
}

#[test]
fn split_args_respects_quotes() {
    assert_eq!(
        split_args("create a \"two words\" --flag value"),
        vec!["create", "a", "two words", "--flag", "value"]
    );
}

#[test]
fn render_list_output_table_has_aligned_header() {
    let out = render_list_output(&[(
        "foo".to_string(),
        "Foo".to_string(),
        "topic".to_string(),
        "complete".to_string(),
        "2026-01-01T00:00:00+00:00".to_string(),
        "2026-01-02T00:00:00+00:00".to_string(),
    )]);
    assert!(out.contains("NAME"));
    assert!(out.contains("STATUS"));
    assert!(out.contains("CREATED"));
    assert!(out.contains("foo"));
    assert!(out.contains("complete"));
    assert!(!out.contains('{'));
}

#[test]
fn render_list_output_empty_shows_message() {
    assert_eq!(render_list_output(&[]), "(no research items)\n");
}

#[test]
fn render_list_output_json_is_valid_json() {
    let json = render_list_output_json(&[(
        "foo".to_string(),
        "Foo".to_string(),
        "topic".to_string(),
        "complete".to_string(),
        "2026-01-01T00:00:00+00:00".to_string(),
        "2026-01-02T00:00:00+00:00".to_string(),
    )]);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed[0]["name"].as_str(), Some("foo"));
    assert_eq!(parsed[0]["topic"].as_str(), Some("topic"));
}

#[test]
fn render_show_output_includes_sources() {
    let out = render_show_output(
        "foo",
        "Foo",
        "topic",
        "complete",
        "2024-01-01",
        "2024-01-02",
        &[(
            "s1".to_string(),
            "https://x".to_string(),
            "X".to_string(),
            "web".to_string(),
            None,
        )],
    );
    assert!(out.contains("Research item: foo"));
    assert!(out.contains("[web] s1: X (https://x)"));
}

#[test]
fn render_search_output_bullet_list() {
    let out =
        render_search_output(&[("foo".to_string(), "Foo".to_string(), "snippet".to_string())]);
    assert!(out.contains("* foo - Foo"));
    assert!(out.contains("snippet"));
}

#[test]
fn render_search_output_empty_shows_message() {
    assert_eq!(render_search_output(&[]), "(no matches)\n");
}

#[test]
fn render_search_output_json_is_valid_json() {
    let json = render_search_output_json(&[(
        "foo".to_string(),
        "Foo".to_string(),
        "snippet".to_string(),
        "research/foo/RESEARCH.md".to_string(),
    )]);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed[0]["path"].as_str(), Some("research/foo/RESEARCH.md"));
}

#[test]
fn truncate_short_string_passes_through() {
    assert_eq!(truncate("abc", 5), "abc");
}

#[test]
fn truncate_long_string_ellipsises() {
    let s = "a".repeat(20);
    let t = truncate(&s, 5);
    assert_eq!(t.chars().count(), 5);
    assert!(t.ends_with("..."));
}

#[test]
fn parse_search_filters_extracts_status_and_name() {
    let f = parse_search_filters("status:complete name:rust async");
    assert_eq!(f.status.as_deref(), Some("complete"));
    assert_eq!(f.name.as_deref(), Some("rust"));
    assert_eq!(f.text, vec!["async"]);
}

#[test]
fn parse_output_format_defaults_unknown_to_report() {
    assert_eq!(parse_output_format("weird"), OutputFormat::Report);
    assert_eq!(parse_output_format("imrad"), OutputFormat::Imrad);
}

#[test]
fn parse_depth_returns_none_for_unknown() {
    assert!(parse_depth("deep").is_some());
    assert!(parse_depth("weird").is_none());
}

#[test]
fn parse_tier_returns_none_for_unknown() {
    assert!(parse_tier("full").is_some());
    assert!(parse_tier("weird").is_none());
}

#[test]
fn parse_mode_returns_none_for_unknown() {
    assert!(parse_mode("supervisor").is_some());
    assert!(parse_mode("weird").is_none());
}

#[test]
fn build_index_name_map_assigns_positions() {
    let map = build_index_name_map("- alpha: title\n- beta: other\n");
    assert_eq!(map.get("alpha"), Some(&0));
    assert_eq!(map.get("beta"), Some(&1));
}

#[test]
fn session_event_json_round_trips() {
    let event = crate::session::SessionEvent::Phase {
        phase: crate::session::SessionPhase::Web,
    };
    let json = session_event_json(&event);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["kind"].as_str(), Some("phase"));
}

#[test]
fn render_session_event_json_adds_prefix() {
    let event = crate::session::SessionEvent::Phase {
        phase: crate::session::SessionPhase::Web,
    };
    let rendered = render_session_event_json(&event);
    assert!(rendered.starts_with("ragent-research: "));
}
