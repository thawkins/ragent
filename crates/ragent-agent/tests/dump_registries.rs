//! One-off generator (not a real test gate): dumps the `tool_info` and
//! `commands_info` JSON payloads to `target/temp/` so they can be bulk-loaded
//! into MongoDB (`ragent.tools` / `ragent.slashcommands`).
//!
//! Run with: `cargo test -p ragent-agent --test dump_registries -- --nocapture --include-ignored`

use ragent_agent::tool::{ToolContext, create_default_registry};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn dump_ctx() -> ToolContext {
    ToolContext {
        session_id: "dump".to_string(),
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        event_bus: Arc::new(ragent_agent::event::EventBus::new(8)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: Arc::new(create_default_registry()),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "generator, not a gate test"]
async fn dump_registries_to_target_temp() {
    let ctx = dump_ctx();
    let registry = create_default_registry();

    let tool_info = registry.get("tool_info").expect("tool_info registered");
    let ti = tool_info
        .execute(json!({}), &ctx)
        .await
        .expect("tool_info run");
    std::fs::create_dir_all("target/temp").expect("mkdir target/temp");
    std::fs::write("target/temp/tools_dump.json", &ti.content).expect("write tools dump");

    let cmds_info = registry
        .get("commands_info")
        .expect("commands_info registered");
    let ci = cmds_info
        .execute(json!({}), &ctx)
        .await
        .expect("commands_info run");
    std::fs::write("target/temp/commands_dump.json", &ci.content).expect("write commands dump");

    let t: serde_json::Value = serde_json::from_str(&ti.content).expect("parse tools json");
    let c: serde_json::Value = serde_json::from_str(&ci.content).expect("parse commands json");
    println!(
        "tools total={} visible={} | commands total={} builtin={} plugin={}",
        t["total"],
        t["visible"],
        c["total"],
        c["builtin"].as_array().map_or(0, Vec::len),
        c["plugin_commands"].as_array().map_or(0, Vec::len)
    );
}
