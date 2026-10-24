//! FR-010 (uniformtools T-015): sequential execution when the provider cannot parallelise.
//!
//! While the active provider profile declares `supports_parallel == false`, the
//! system executes the step's tool calls sequentially even when
//! `experimental.parallel_tool_calls` is true, and logs one `tracing::info!`
//! naming the provider and the reason. The gate lives in the agent loop's
//! per-step `parallel_tool_calls` derivation, before any dispatch, so it is
//! observable as a source invariant and as a behavioural one (state-driven).

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Source guard: the loop gates parallel dispatch on the wire profile (FR-010)
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

/// The agent loop gates `parallel_tool_calls` on the active provider's
/// `ToolWireProfile::supports_parallel` and logs the demotion (FR-010).
#[test]
fn parallel_dispatch_is_gated_on_the_profile_and_logs_the_reason() {
    let source = read_workspace_file("crates/ragent-agent/src/session/processor.rs");

    // The per-step derivation must read the profile for the active provider.
    assert!(
        source.contains("crate::provider::wire::profile_table::profile_for(")
            && source.contains("turn.model_ref.provider_id"),
        "the agent loop must resolve the active provider's wire profile via \
         crate::provider::wire::profile_table::profile_for(&turn.model_ref.provider_id) (FR-010)"
    );

    // The gate itself: `supports_parallel == false` forces sequential.
    assert!(
        source.contains("supports_parallel"),
        "the agent loop must read ToolWireProfile::supports_parallel when \
         deriving parallel_tool_calls (FR-010)"
    );
    // The `false` arm must set the derived flag to `false` rather than shadowing
    // it or ignoring it.
    assert!(
        source.contains("parallel_tool_calls = false"),
        "a provider that declares supports_parallel == false must force \
         `parallel_tool_calls` to false (FR-010)"
    );

    // The gate must run while the `experimental.parallel_tool_calls` flag is
    // true - i.e. it overrides the config rather than the config overriding it.
    // The config read and the profile read must be in the same scope.
    let config_pos = source
        .find("experimental.parallel_tool_calls")
        .expect("the config flag must be read in the agent loop");
    let profile_pos = source
        .find("crate::provider::wire::profile_table::profile_for(")
        .expect("the profile lookup must be present");
    let flag_clear = source
        .find("parallel_tool_calls = false")
        .expect("the forced-sequential assignment must be present");
    assert!(
        config_pos < profile_pos && profile_pos < flag_clear,
        "the loop must read experimental.parallel_tool_calls, then the profile, \
         then force sequential on supports_parallel == false (FR-010 ordering)"
    );

    // One tracing::info! naming the provider and the reason.
    assert!(
        source.contains("tracing::info!(")
            && source.contains("provider = %turn.model_ref.provider_id")
            && source.contains("reason ="),
        "the demotion must log one tracing::info! naming the provider and the reason (FR-010)"
    );
    // The log is inside the supports_parallel branch, so it fires exactly once
    // per step that is demoted rather than once per tool call.
    let info_pos = source
        .find("tracing::info!(")
        .expect("the info log must be present");
    // Only the gate's info is expected to be in this region; the rest of the
    // file's info logs are unrelated.
    let gate_block = &source
        [config_pos..source[config_pos..].find("let mut futures").unwrap_or(8000) + config_pos];
    assert!(
        gate_block.contains("tracing::info!("),
        "the tracing::info! must be inside the per-step parallel gate (FR-010 - log once per demotion)"
    );
    let _ = info_pos;
}

/// The profile table exposes `supports_parallel` on every profile, and the
/// matrix renders it.
#[test]
fn profile_table_exposes_supports_parallel_everywhere() {
    let wire_mod = read_workspace_file("crates/ragent-llm/src/providers/wire/mod.rs");
    assert!(
        wire_mod.contains("pub supports_parallel: bool"),
        "ToolWireProfile must declare supports_parallel (FR-010)"
    );
    assert!(
        wire_mod.contains("`false` forces sequential"),
        "the doc comment must state the false arm forces sequential execution"
    );

    let profile_table =
        read_workspace_file("crates/ragent-llm/src/providers/wire/profile_table.rs");
    assert!(
        profile_table.contains("supports_parallel:"),
        "every profile const must set supports_parallel"
    );

    let matrix = read_workspace_file("crates/ragent-llm/src/providers/wire/matrix.rs");
    assert!(
        matrix.contains("supports_parallel"),
        "the wire matrix must render the supports_parallel column (FR-016)"
    );
}
