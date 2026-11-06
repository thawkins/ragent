//! Stdio JSON-RPC MCP server (FR-025).
//!
//! Serves the vault tools over stdio. Tool arguments are validated against each
//! tool's schema, and malformed arguments produce a structured JSON-RPC error —
//! never a panic or a process abort.

pub mod server;
pub mod tools;
