//! Core library for the ragent AI coding agent.
//!
//! This crate provides the foundational types and logic for ragent, including
//! agent definitions ([`agent`]), configuration loading ([`config`]),
//! error handling ([`error`]), event streaming ([`event`]), LLM provider
//! integration ([`llm`], [`provider`]), MCP server support ([`mcp`]),
//! message types ([`message`]), permission management ([`permission`]),
//! session orchestration ([`session`]), skill management ([`skill`]),
//! state snapshots ([`snapshot`]), persistent storage ([`storage`]),
//! and tool execution ([`tool`]).

pub mod agent;
pub mod config;
pub mod error;
pub mod event;
pub mod file_ops;
pub mod id;
pub mod llm;
/// Language Server Protocol client for code-intelligence queries.
pub mod lsp;
pub mod mcp;
pub mod message;
pub mod orchestrator;
pub mod permission;
pub mod provider;
/// @ file reference parsing, resolution, and fuzzy matching (SPEC §3.34).
pub mod reference;
/// Input sanitization and secret redaction utilities.
pub mod sanitize;
/// Process resource limits — bounded concurrency for child process spawns.
pub mod resource;
pub mod session;
/// Skill discovery, loading, argument substitution, and invocation.
pub mod skill;
pub mod snapshot;
pub mod storage;
/// Sub-agent task management for spawning and tracking sub-agents.
pub mod task;
/// Agent team coordination — shared task list, mailboxes, and team config.
pub mod team;
pub mod tool;
/// YOLO mode — bypass all command validation and tool restrictions.
pub mod yolo;
