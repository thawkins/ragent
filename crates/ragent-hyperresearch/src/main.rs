//! `hyperresearch` binary entry point.
//!
//! Contains no business logic beyond argument parsing and dispatch to
//! [`ragent_hyperresearch::cli::run`] (FR-002).

#![deny(unsafe_code)]

use clap::Parser;
use ragent_hyperresearch::cli::args::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    ragent_hyperresearch::cli::run(cli)?;
    Ok(())
}
