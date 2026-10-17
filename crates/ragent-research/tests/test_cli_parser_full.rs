//! Full-surface integration tests for the `ragent research` hand parser and
//! rendering helpers in `crates/ragent-research/src/cli.rs`.
//!
//! These exercise every verb arm, every `create` flag (including the numeric
//! validation failures), the render helpers, and the search/output-format
//! helpers. Pure logic only - no network or filesystem access.

use ragent_research::cli::{
    ResearchCliCommand, build_index_name_map, parse_depth, parse_mode, parse_output_format,
    parse_search_filters, parse_tier, render_list_output, render_list_output_json,
    render_search_output, render_search_output_json, render_show_output, render_show_output_json,
};
use ragent_research::run_config::{Depth, OutputFormat, ResearchMode, Tier};

// -- Verb dispatch ---------------------------------------------------------

#[test]
fn parse_empty_is_unknown() {
    assert_eq!(
        ResearchCliCommand::parse(""),
        ResearchCliCommand::Unknown(String::new())
    );
}

#[test]
fn parse_list_all_and_json() {
    assert_eq!(
        ResearchCliCommand::parse("list --all --json"),
        ResearchCliCommand::List {
            all: true,
            json: true
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("list"),
        ResearchCliCommand::List {
            all: false,
            json: false
        }
    );
}

#[test]
fn parse_show_name_and_json() {
    assert_eq!(
        ResearchCliCommand::parse("show my-item --json"),
        ResearchCliCommand::Show {
            name: "my-item".to_string(),
            json: true
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("show"),
        ResearchCliCommand::Show {
            name: String::new(),
            json: false
        }
    );
}

#[test]
fn parse_search_joins_and_detects_json() {
    assert_eq!(
        ResearchCliCommand::parse("search rust async --json"),
        ResearchCliCommand::Search {
            query: "rust async".to_string(),
            json: true
        }
    );
}

#[test]
fn parse_open_delete_archive_resume() {
    assert_eq!(
        ResearchCliCommand::parse("open thing"),
        ResearchCliCommand::Open {
            name: "thing".to_string()
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("delete thing --yes"),
        ResearchCliCommand::Delete {
            name: "thing".to_string(),
            yes: true
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("archive thing"),
        ResearchCliCommand::Archive {
            name: "thing".to_string()
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("resume thing"),
        ResearchCliCommand::Resume {
            name: "thing".to_string()
        }
    );
}

#[test]
fn parse_continue_with_message() {
    assert_eq!(
        ResearchCliCommand::parse("continue thing --message go on"),
        ResearchCliCommand::Continue {
            name: "thing".to_string(),
            message: Some("go".to_string())
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("continue thing"),
        ResearchCliCommand::Continue {
            name: "thing".to_string(),
            message: None
        }
    );
}

#[test]
fn parse_update_cluster_export_import() {
    assert_eq!(
        ResearchCliCommand::parse("update thing"),
        ResearchCliCommand::Update {
            name: "thing".to_string()
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("cluster thing --force"),
        ResearchCliCommand::Cluster {
            name: "thing".to_string(),
            force: true
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("export thing --output /tmp/x.md"),
        ResearchCliCommand::Export {
            name: "thing".to_string(),
            output: Some("/tmp/x.md".to_string())
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("import /tmp/file.md --name renamed"),
        ResearchCliCommand::Import {
            path: "/tmp/file.md".to_string(),
            name: Some("renamed".to_string())
        }
    );
}

#[test]
fn parse_config_and_help() {
    assert_eq!(
        ResearchCliCommand::parse("config"),
        ResearchCliCommand::Config
    );
    assert_eq!(ResearchCliCommand::parse("help"), ResearchCliCommand::Help);
    assert_eq!(
        ResearchCliCommand::parse("--help"),
        ResearchCliCommand::Help
    );
    assert_eq!(ResearchCliCommand::parse("-h"), ResearchCliCommand::Help);
}

#[test]
fn parse_unknown_verb_reports_name() {
    assert_eq!(
        ResearchCliCommand::parse("frobnicate"),
        ResearchCliCommand::Unknown("frobnicate".to_string())
    );
}

// -- create: every flag ---------------------------------------------------

#[test]
fn parse_create_sets_every_typed_flag() {
    let cmd = ResearchCliCommand::parse(
        "create my-item my topic --iterations 7 --depth deep --tier full --mode supervisor \
         --summarization-model ollama:sum --research-model ollama:res --compression-model ollama:cmp \
         --final-report-model ollama:fin --max-concurrent-research-units 4 --fetch-concurrently 3 \
         --local-concurrently 2 --fetch-timeout-secs 45 --web-phase-timeout-secs 90 \
         --local-phase-timeout-secs 60 --search-max-retries 5 --search-retry-base-delay-ms 300 \
         --max-web-results 40 --max-search-calls 12 --max-local-sources 9 --max-synthesis-sources 8 \
         --max-concepts 6 --max-findings 11 --format imrad --sources-dir /extra --template tpl \
         --brief \"a brief\" --clarify --use-local --use-specs --use-low-relevance --papers \
         --use-pdf --oa-enable --evaluate --url-cloak",
    );
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            iterations,
            depth,
            tier,
            mode,
            summarization_model,
            research_model,
            compression_model,
            final_report_model,
            max_concurrent_research_units,
            clarify,
            format,
            sources_dir,
            template,
            fetch_concurrency,
            fetch_timeout_secs,
            local_concurrency,
            web_phase_timeout_secs,
            local_phase_timeout_secs,
            search_max_retries,
            search_retry_base_delay_ms,
            max_web_results,
            max_search_calls,
            max_local_sources,
            max_synthesis_sources,
            max_concepts,
            max_findings,
            brief,
            use_local,
            use_specs,
            use_low_relevance,
            papers,
            use_pdf,
            oa_recovery,
            evaluate,
            url_cloak,
            ..
        } => {
            assert_eq!(name, "my-item");
            assert_eq!(topic, "my topic");
            assert_eq!(iterations, Some(7));
            assert_eq!(depth.as_deref(), Some("deep"));
            assert_eq!(tier.as_deref(), Some("full"));
            assert_eq!(mode.as_deref(), Some("supervisor"));
            assert_eq!(summarization_model.as_deref(), Some("ollama:sum"));
            assert_eq!(research_model.as_deref(), Some("ollama:res"));
            assert_eq!(compression_model.as_deref(), Some("ollama:cmp"));
            assert_eq!(final_report_model.as_deref(), Some("ollama:fin"));
            assert_eq!(max_concurrent_research_units, Some(4));
            assert_eq!(clarify, Some(true));
            assert_eq!(format.as_deref(), Some("imrad"));
            assert_eq!(sources_dir.as_deref(), Some("/extra"));
            assert_eq!(template.as_deref(), Some("tpl"));
            assert_eq!(fetch_concurrency, Some(3));
            assert_eq!(fetch_timeout_secs, Some(45));
            assert_eq!(local_concurrency, Some(2));
            assert_eq!(web_phase_timeout_secs, Some(90));
            assert_eq!(local_phase_timeout_secs, Some(60));
            assert_eq!(search_max_retries, Some(5));
            assert_eq!(search_retry_base_delay_ms, Some(300));
            assert_eq!(max_web_results, Some(40));
            assert_eq!(max_search_calls, Some(12));
            assert_eq!(max_local_sources, Some(9));
            assert_eq!(max_synthesis_sources, Some(8));
            assert_eq!(max_concepts, Some(6));
            assert_eq!(max_findings, Some(11));
            assert_eq!(brief.as_deref(), Some("a brief"));
            assert!(use_local && use_specs && use_low_relevance && papers && use_pdf);
            assert_eq!(oa_recovery, Some(true));
            assert!(evaluate && url_cloak);
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn parse_create_web_time_alias_sets_timeout() {
    match ResearchCliCommand::parse("create t topic --web-time 120") {
        ResearchCliCommand::Create {
            web_phase_timeout_secs,
            ..
        } => {
            assert_eq!(web_phase_timeout_secs, Some(120));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn parse_create_no_clarify_and_no_oa() {
    match ResearchCliCommand::parse("create t topic --no-clarify --no-oa") {
        ResearchCliCommand::Create {
            clarify,
            oa_recovery,
            ..
        } => {
            assert_eq!(clarify, Some(false));
            assert_eq!(oa_recovery, Some(false));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn parse_create_from_url_and_file_repeatable() {
    match ResearchCliCommand::parse(
        "create t topic --from-url https://a.example --from-urls https://b.example \
         --from-file /a.md --from-files /b.md",
    ) {
        ResearchCliCommand::Create {
            from_urls,
            from_files,
            ..
        } => {
            assert_eq!(from_urls, vec!["https://a.example", "https://b.example"]);
            assert_eq!(from_files, vec!["/a.md", "/b.md"]);
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn parse_create_invalid_numeric_is_rejected() {
    for line in [
        "create t topic --fetch-concurrently x",
        "create t topic --local-concurrently x",
        "create t topic --fetch-timeout-secs x",
        "create t topic --web-phase-timeout-secs x",
        "create t topic --local-phase-timeout-secs x",
        "create t topic --search-max-retries x",
        "create t topic --search-retry-base-delay-ms x",
        "create t topic --max-web-results x",
        "create t topic --max-search-calls x",
        "create t topic --max-local-sources x",
        "create t topic --max-synthesis-sources x",
        "create t topic --iterations x",
        "create t topic --max-concurrent-research-units x",
        "create t topic --max-concepts x",
        "create t topic --max-findings x",
    ] {
        match ResearchCliCommand::parse(line) {
            ResearchCliCommand::Invalid(msg) => {
                assert!(msg.contains("expected an integer"), "line {line}: {msg}");
            }
            other => panic!("line {line}: expected Invalid, got {other:?}"),
        }
    }
}

#[test]
fn parse_create_without_name_is_unknown() {
    assert_eq!(
        ResearchCliCommand::parse("create --depth deep"),
        ResearchCliCommand::Unknown("create".to_string())
    );
}

#[test]
fn parse_create_flag_without_value_is_tolerated() {
    // A trailing value flag with no value must not panic; the flag is ignored.
    match ResearchCliCommand::parse("create t topic --depth") {
        ResearchCliCommand::Create { depth, .. } => assert_eq!(depth, None),
        other => panic!("expected Create, got {other:?}"),
    }
}

// -- split_args quoting (via search query) --------------------------------

#[test]
fn parse_respects_quoted_arguments() {
    match ResearchCliCommand::parse("create \"my item\" \"a topic phrase\"") {
        ResearchCliCommand::Create { name, topic, .. } => {
            assert_eq!(name, "my item");
            assert_eq!(topic, "a topic phrase");
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

// -- render helpers --------------------------------------------------------

#[test]
fn render_list_output_truncates_long_fields() {
    let items = vec![(
        "n".repeat(30),
        "t".repeat(40),
        "topic".to_string(),
        "complete".to_string(),
        "2024-01-01T00:00:00Z".to_string(),
        "2024-01-02T00:00:00Z".to_string(),
    )];
    let out = render_list_output(&items);
    assert!(out.contains("NAME"));
    assert!(
        out.contains("..."),
        "long fields should be ellipsised: {out}"
    );
    assert!(render_list_output(&[]).contains("no research items"));
}

#[test]
fn render_list_output_json_is_parseable_object() {
    let items = vec![(
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
        "f".to_string(),
    )];
    let json = render_list_output_json(&items);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed[0]["name"], "a");
    assert_eq!(parsed[0]["modified"], "f");
}

#[test]
fn render_search_output_lists_and_handles_empty() {
    let results = vec![("id".to_string(), "Title".to_string(), "snip".to_string())];
    let out = render_search_output(&results);
    assert!(out.contains("* id - Title"));
    assert!(out.contains("snip"));
    assert!(render_search_output(&[]).contains("no matches"));
}

#[test]
fn render_search_output_json_includes_path() {
    let results = vec![(
        "id".to_string(),
        "Title".to_string(),
        "snip".to_string(),
        "/p".to_string(),
    )];
    let parsed: serde_json::Value =
        serde_json::from_str(&render_search_output_json(&results)).expect("valid json");
    assert_eq!(parsed[0]["path"], "/p");
}

#[test]
fn render_show_output_includes_and_omits_preview() {
    let sources = vec![
        (
            "1".to_string(),
            "https://a".to_string(),
            "A".to_string(),
            "web".to_string(),
            Some("prev".to_string()),
        ),
        (
            "2".to_string(),
            "/p".to_string(),
            "P".to_string(),
            "local".to_string(),
            None,
        ),
    ];
    let human = render_show_output("n", "t", "top", "complete", "c", "m", &sources);
    assert!(human.contains("Research item: n"));
    assert!(human.contains("preview: prev"));
    let json: serde_json::Value = serde_json::from_str(&render_show_output_json(
        "n", "t", "top", "complete", "c", "m", &sources,
    ))
    .expect("valid json");
    assert_eq!(json["sources"][0]["preview"], "prev");
    assert!(json["sources"][1].get("preview").is_none());
}

// -- small parsers ---------------------------------------------------------

#[test]
fn parse_search_filters_extracts_and_defaults() {
    let f = parse_search_filters("status:archived name:foo bar baz");
    assert_eq!(f.status.as_deref(), Some("archived"));
    assert_eq!(f.name.as_deref(), Some("foo"));
    assert_eq!(f.text, vec!["bar", "baz"]);
    let empty = parse_search_filters("");
    assert!(empty.status.is_none() && empty.name.is_none() && empty.text.is_empty());
}

#[test]
fn build_index_name_map_parses_bullet_lines() {
    let content = "- alpha: desc\nnoise\n- beta: other\n";
    let map = build_index_name_map(content);
    assert_eq!(map.get("alpha"), Some(&0));
    assert_eq!(map.get("beta"), Some(&1));
    assert_eq!(map.len(), 2);
}

#[test]
fn output_format_depth_tier_mode_parsers() {
    assert_eq!(
        parse_output_format("executive-summary"),
        OutputFormat::ExecutiveSummary
    );
    assert_eq!(parse_output_format("nonsense"), OutputFormat::Report);
    assert_eq!(parse_depth("shallow"), Some(Depth::Shallow));
    assert_eq!(parse_depth("nonsense"), None);
    assert_eq!(parse_tier("full"), Some(Tier::Full));
    assert_eq!(parse_tier("nonsense"), None);
    assert_eq!(parse_mode("supervisor"), Some(ResearchMode::Supervisor));
    assert_eq!(parse_mode("nonsense"), None);
}
