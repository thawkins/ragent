//! `ragent acp-server` - serve the ACP endpoint on stdio for IDE clients (spec
//! `openhands` T-008; FR-022, FR-028).
//!
//! This is the CLI entry point for the ACP **server**. It is the mirror of the
//! `--no-tui` one-shot run: ragent owns the terminal's stdin/stdout for the
//! process lifetime and speaks JSON-RPC 2.0 one frame per line to an
//! ACP-capable editor (Zed, VS Code, JetBrains).
//!
//! The endpoint is **disabled by default** and requires two gates to run:
//!
//! 1. the binary must be built with the `acp-server` Cargo feature; and
//! 2. `acp.server_enabled` must be `true` in the loaded configuration.
//!
//! When either gate is closed this module returns a typed
//! [`CliExit`](crate::cli::CliExit) so the process exits with a usage error
//! instead of silently doing nothing.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::session::processor::SessionProcessor;

use crate::Config;

/// The actionable reminder shown when the ACP endpoint cannot run because the
/// `acp-server` feature was not compiled into this binary.
const ACP_FEATURE_HELP: &str = "this build has no `acp-server` feature; rebuild with \
                                 `cargo build --features acp-server` and set \
                                 `acp.server_enabled: true` in the configuration";

/// Run the ACP server endpoint on stdio for an editor client.
///
/// The `processor` and `event_bus` are the initialised runtime pieces `main`
/// already builds for every mode; the agent the endpoint drives is resolved from
/// [`Config::acp_server_agent`](ragent_config::Config::acp_server_agent).
///
/// # Errors
///
/// Returns a [`CliExit`](crate::cli::CliExit) usage error when the endpoint is
/// disabled (feature not compiled in, or `acp.server_enabled` false) or when the
/// configured agent cannot be resolved, and propagates any stdio transport error
/// from the endpoint.
pub async fn run(
    config: Arc<Config>,
    processor: Arc<SessionProcessor>,
    event_bus: Arc<EventBus>,
    working_dir: std::path::PathBuf,
) -> anyhow::Result<()> {
    if !config.acp_server_enabled() {
        // Name the specific closed gate so the message is actionable: the
        // `acp-server` feature must be compiled in *and* `acp.server_enabled`
        // must be true (FR-028). The default build fails both, so it names the
        // feature; a feature build with the key unset names the config key.
        if cfg!(feature = "acp-server") {
            eprintln!(
                "ragent acp-server: the ACP server endpoint is disabled; enable it with \
                 `acp.server_enabled: true` in the configuration"
            );
        } else {
            eprintln!("ragent acp-server: {ACP_FEATURE_HELP}");
        }
        return Err(crate::cli::CliExit::usage().into());
    }

    #[cfg(feature = "acp-server")]
    {
        use ragent_agent::acp::serve_acp_stdio;
        use ragent_agent::agent;

        let agent_name = config.acp_server_agent().to_string();
        let agent =
            agent::resolve_agent_with_model(&agent_name, &config, &processor.provider_registry)?;
        tracing::info!(
            agent = %agent.name,
            "Serving ACP endpoint on stdio"
        );
        serve_acp_stdio(processor, event_bus, agent, working_dir).await
    }

    #[cfg(not(feature = "acp-server"))]
    {
        // Reached only when `acp.server_enabled` is true in a build with no
        // `acp-server` feature; the `if` above already handles the disabled case.
        let _ = (processor, event_bus, working_dir);
        eprintln!("ragent acp-server: {ACP_FEATURE_HELP}");
        Err(crate::cli::CliExit::usage().into())
    }
}
