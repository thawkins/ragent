//! Diagnostic: replicate `/websearch search` exactly for the reported failing
//! query and dump every per-engine report (blocked/error/count) plus the
//! merged distribution.
//!
//! `cargo test -p ragent-tools-extended --test test_mf_orchestrator_diag -- --nocapture`

use ragent_config::Config;
use ragent_tools_extended::ToolContext;
use ragent_tools_extended::masterfetch::search::SearchOptions;
use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;

fn tui_ctx() -> ToolContext {
    // Mirrors websearch_diag_ctx() in crates/ragent-tui/src/app/slash.rs:
    // Config::load() + cwd-based working dir. cargo test runs with cwd set to
    // the crate dir, so hop up to the workspace root where .ragent/ragent.json
    // lives.
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let _ = std::env::set_current_dir(&workspace);
    let config = Config::load().expect("config should load");
    eprintln!("serper key present: {:?}", config.serper_api_key.is_some());
    eprintln!(
        "langsearch key present: {:?}",
        config.langsearch_api_key.is_some()
    );
    ToolContext {
        session_id: String::new(),
        working_dir: workspace.clone(),
        event_bus: std::sync::Arc::new(ragent_types::EventBus::new(16)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[tokio::test]
async fn test_replicate_websearch_search_agentic_loop() {
    let ctx = tui_ctx();
    let orchestrator = MfSearchTool::build_orchestrator(&ctx);
    eprintln!("engines wired: {:?}", orchestrator.engine_names());

    let query = "agentic loop architecture components LLM agents";
    let opts = SearchOptions::new(100).with_per_engine_results(25);

    // Per-engine reports via search_per_engine for full visibility.
    let reports = orchestrator.search_per_engine(query, &opts).await;
    std::thread::sleep(std::time::Duration::from_millis(1500)); // avoid serper rate-limit
    for r in &reports {
        eprintln!(
            "engine={} count={} blocked={} error={:?}",
            r.engine, r.result_count, r.engine_blocked, r.error
        );
        for hit in r.results.iter().take(3) {
            eprintln!("    hit: {:.70} src={}", hit.url, hit.source);
        }
    }

    // Merged output exactly as /websearch search renders it.
    let out = orchestrator.search(query, &opts).await;
    eprintln!(
        "merged: total={} raw={} engines_with_results={} blocked={:?}",
        out.merge.total_merged_results,
        out.merge.total_raw_results,
        out.merge.engines_with_results,
        out.merge.blocked_engines
    );
    for r in &out.merge.results {
        eprintln!(
            "  {:2}. [{:.2}] {:.60} src={}",
            r.position, r.relevance_score, r.url, r.source
        );
    }
}
