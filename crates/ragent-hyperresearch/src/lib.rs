//! Deep-research engine for coding agents (spec `hyperesearch`).
//!
//! This crate is a Rust port of the Python project `jordan-gibbs/hyperresearch`.
//! It turns an external agent (Claude Code, Codex, or ragent itself) into a
//! deep-research agent: it decomposes a question, searches and fetches web and
//! scholarly sources, deduplicates and ranks them, extracts claims, builds a
//! contradiction and citation graph across notes, and synthesises long-form,
//! fully cited output — all persisted in a wiki-style vault that grows across
//! sessions.
//!
//! Four surfaces sit over one shared domain layer:
//!
//! 1. a library API ([`core`], [`models`], …),
//! 2. a CLI binary named `hyperresearch`,
//! 3. a stdio MCP server (`hyperresearch mcp`), and
//! 4. a small HTTP server (`hyperresearch serve`).
//!
//! There is no LLM client here (non-goal): synthesis steps are embedded
//! skill/prompt text consumed by the host agent.
//!
//! ## Modules
//!
//! - [`cli`] — one module per subcommand group; argument parsing and dispatch.
//! - [`core`] — domain logic with no stdout I/O (config, vault, untrusted
//!   content fencing, migrations, errors).
//! - [`models`] — serde-only data types (notes, sources, claims, records).
//! - [`mcp`] — stdio JSON-RPC MCP server exposing the vault tools.
//! - [`serve`] — HTTP server and HTML renderer for vault notes and reports.
//! - [`web`] — fetch providers (builtin safe HTTP lane plus third parties).
//! - [`scholar`] — scholarly providers, registry, and cross-provider dedup.
//! - [`search`] — full-text index and evidence filters.
//! - [`graph`] — link and contradiction graph over the vault.
//! - [`export`] — report export formats.
//! - [`indexgen`] — `INDEX.md` vault index generation.
//! - [`skills`] — the embedded, golden-tested skill pipeline.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

pub mod cli;
pub mod core;
pub mod export;
pub mod graph;
pub mod indexgen;
pub mod mcp;
pub mod models;
pub mod scholar;
pub mod search;
pub mod serve;
pub mod skills;
pub mod web;

pub use crate::core::error::{Error, Result};
