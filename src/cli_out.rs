//! Broken-pipe-safe stdout/stderr output for the CLI presentation surface.
//!
//! The implementation and its documentation live in
//! [`ragent_types::cli_out`]: a closed pipe on user-facing output means the
//! consumer left early (`ragent config show | head -1`), which is a normal
//! condition rather than a crash, so these helpers log the failure through
//! `tracing` and return instead of panicking.
//!
//! This module re-exports that one implementation so the binary's call sites
//! (`crate::cli_out::…`, `use crate::cli_out::cliprintln;`) keep working, and so
//! a library crate's own CLI entry point can share the exact same code.
//!
//! These helpers exist only for the CLI presentation surface
//! (`src/main.rs`, `src/cli.rs`, `src/connectors.rs`, `src/plugins.rs`,
//! `src/acp_server.rs` - the documented `println!` exception in
//! `AGENTS-LANGUAGE.md`). Application code keeps using `tracing`.

pub use ragent_types::cli_out::{write_io_log, writeln_io_log};
pub use ragent_types::{cliprint, cliprint_err, cliprintln, cliprintln_err};
