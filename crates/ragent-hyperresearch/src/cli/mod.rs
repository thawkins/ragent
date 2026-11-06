//! Command-line surface (FR-023, FR-024).
//!
//! One module per subcommand group. Every command returns `Result<_, Error>`
//! and every result is emitted through [`cli::output`], which supports plain and
//! JSON formats and maps failures to a non-zero exit status without panicking on
//! user input. `println!` is allowed here and nowhere else in the crate (NFR-005).

pub mod args;
pub mod output;

use crate::core::error::Result;

/// Entry point for the `hyperresearch` binary.
///
/// # Errors
///
/// Propagates any command failure; `main` turns it into an exit status.
pub fn run(args: args::Cli) -> Result<()> {
    match args.command {
        args::Command::Help => {
            println!("{}", args::USAGE);
            Ok(())
        }
    }
}
