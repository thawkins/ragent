//! Domain layer: configuration, vault storage, untrusted-content fencing,
//! schema migrations, and the crate error type.
//!
//! Everything here is I/O-bound but stdout-free: this module never writes to
//! the terminal, so it is safe to call from the CLI, the MCP server, and the
//! HTTP server alike. Presentation belongs to [`crate::cli`].

pub mod config;
pub mod error;
pub mod migrations;
pub mod untrusted;
pub mod vault;

pub use crate::core::error::{Error, Result};
