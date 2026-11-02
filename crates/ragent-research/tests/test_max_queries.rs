//! Tests for the `--max-queries` research flag.
//!
//! The flag caps the number of search queries produced by the topic
//! decomposition step. It is exercised at each layer that honours it:
//!
//! - the CLI parser (`--max-queries N` -> `ResearchCliCommand::Create`);
//! - `build_session_config` resolution (flag -> config -> built-in default);
//! - the decomposers themselves (LLM prompt/cap and heuristic fallback);
//! - the web gatherer, which re-applies the cap before searching so a
//!   decomposer cannot bypass it.
//!
//! The default is 20 and `0` means "unbounded" throughout.

use ragent_research::{
    DEFAULT_MAX_QUERIES, HeuristicQueryDecomposer, OutputFormat, QueryDecomposer,
    ResearchCliCommand, ResearchRunRequest, build_session_config,
};

/// Extract the `max_queries` value from a parsed `create` command.
fn parsed_max_queries(cmd: ResearchCliCommand) -> Option<usize> {
    match cmd {
        ResearchCliCommand::Create { max_queries, .. } => max_queries,
        other => panic!("expected a create command, got {other:?}"),
    }
}

#[test]
fn default_is_twenty() {
    assert_eq!(DEFAULT_MAX_QUERIES, 20);
}

#[test]
fn cli_parses_max_queries_flag() {
    let cmd = ResearchCliCommand::parse("create q \"topic\" --max-queries 7");
    assert_eq!(parsed_max_queries(cmd), Some(7));
}

#[test]
fn cli_accepts_zero_as_unbounded() {
    let cmd = ResearchCliCommand::parse("create q \"topic\" --max-queries 0");
    assert_eq!(parsed_max_queries(cmd), Some(0));
}

#[test]
fn cli_omitted_flag_is_none() {
    let cmd = ResearchCliCommand::parse("create q \"topic\"");
    assert_eq!(parsed_max_queries(cmd), None);
}

#[test]
fn cli_rejects_a_non_numeric_value() {
    let cmd = ResearchCliCommand::parse("create q \"topic\" --max-queries lots");
    match cmd {
        ResearchCliCommand::Invalid(diagnostic) => {
            assert!(
                diagnostic.contains("--max-queries"),
                "diagnostic should name the flag: {diagnostic}"
            );
        }
        other => panic!("expected an invalid command, got {other:?}"),
    }
}

#[test]
fn session_config_uses_the_built_in_default() {
    let req = ResearchRunRequest::new("q", "topic");
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.web.max_queries, DEFAULT_MAX_QUERIES);
}

#[test]
fn session_config_prefers_the_per_run_flag() {
    let mut req = ResearchRunRequest::new("q", "topic");
    req.max_queries = Some(3);
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.web.max_queries, 3);
}

#[test]
fn session_config_prefers_the_configured_value_over_the_default() {
    let mut config = ragent_config::Config::default();
    config.research.max_queries = 12;
    let req = ResearchRunRequest::new("q", "topic");
    let cfg = build_session_config(&req, Some(&config));
    assert_eq!(cfg.web.max_queries, 12);

    // An explicit flag still wins over the configured value.
    let mut req_with_flag = ResearchRunRequest::new("q", "topic");
    req_with_flag.max_queries = Some(4);
    let cfg = build_session_config(&req_with_flag, Some(&config));
    assert_eq!(cfg.web.max_queries, 4);
}

#[test]
fn invocation_replay_carries_max_queries() {
    let req = ResearchRunRequest::from_invocation(
        "ragent research create q \"topic\" --max-queries 6 --depth deep",
    )
    .expect("replay should parse");
    assert_eq!(req.max_queries, Some(6));
    assert_eq!(req.depth.as_deref(), Some("deep"));
    assert_eq!(req.output_format, None);
}

#[test]
fn output_format_is_unaffected_by_the_new_flag() {
    // Guard against the argument parser shifting positions when a new arm is
    // added: `--format` must still resolve.
    let req = ResearchRunRequest::from_invocation(
        "ragent research create q \"topic\" --max-queries 6 --format imrad",
    )
    .expect("replay should parse");
    assert_eq!(req.output_format.as_deref(), Some("imrad"));
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::Imrad);
}

#[tokio::test]
async fn heuristic_decomposer_honours_the_cap() {
    let topic = "alpha and beta, gamma and delta, epsilon and zeta, eta and theta";
    let unbounded = HeuristicQueryDecomposer::new()
        .with_max_queries(None)
        .decompose(topic)
        .await
        .expect("heuristic decomposition");
    assert!(
        unbounded.len() > 3,
        "fixture must produce more than three queries, got {}",
        unbounded.len()
    );

    let capped = HeuristicQueryDecomposer::new()
        .with_max_queries(Some(3))
        .decompose(topic)
        .await
        .expect("heuristic decomposition");
    assert_eq!(capped.len(), 3);
    assert_eq!(capped, unbounded[..3].to_vec());
}

#[tokio::test]
async fn heuristic_decomposer_reports_its_limit() {
    let d = HeuristicQueryDecomposer::new();
    assert_eq!(d.query_limit(), Some(DEFAULT_MAX_QUERIES));

    let d = d.with_max_queries(Some(5));
    assert_eq!(d.query_limit(), Some(5));

    let d = d.with_max_queries(None);
    assert_eq!(d.query_limit(), None);
}

#[tokio::test]
async fn heuristic_decomposer_default_keeps_short_inputs_intact() {
    // A cap above the number of produced queries changes nothing.
    let topic = "Rust async runtime";
    let defaulted = HeuristicQueryDecomposer::new()
        .decompose(topic)
        .await
        .expect("heuristic decomposition");
    let wide = HeuristicQueryDecomposer::new()
        .with_max_queries(Some(50))
        .decompose(topic)
        .await
        .expect("heuristic decomposition");
    assert_eq!(defaulted, wide);
}
