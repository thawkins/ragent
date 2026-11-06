//! Argument parsing (FR-023).
//!
//! The command surface named by FR-023 is large; it is declared incrementally so
//! each subcommand lands with its own module and tests rather than as one
//! unreviewable block.

use clap::Parser;

/// Usage text shown by `hyperresearch help`.
pub const USAGE: &str = "\
hyperresearch — deep-research engine

USAGE:
    hyperresearch <COMMAND>

COMMANDS:
    note      Create, read, update, and list vault notes
    fetch     Fetch a URL into the vault
    search    Query the vault index
    index     Regenerate INDEX.md
    mcp       Run the stdio MCP server
    serve     Run the HTTP server
    help      Show this message
";

/// Parsed command line.
#[derive(Debug, Parser)]
#[command(
    name = "hyperresearch",
    version,
    about = "Deep-research engine for coding agents",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Show usage.
    Help,
}
