//! Core library for the ragent AI coding agent.
//!
//! This crate provides the foundational types and logic for ragent, including
//! agent definitions ([`agent`]), configuration loading ([`config`]),
//! error handling ([`error`]), event streaming ([`event`]), LLM provider
//! integration ([`llm`], [`provider`]), MCP server support ([`mcp`]),
//! message types ([`message`]), permission management ([`permission`]),
//! session orchestration ([`session`]), state snapshots ([`snapshot`]),
//! persistent storage ([`storage`]), and tool execution ([`tool`]).

pub mod agent;
pub mod config;
pub mod error;
pub mod event;
pub mod llm;
pub mod mcp;
pub mod message;
pub mod permission;
pub mod provider;
pub mod sanitize;
pub mod session;
pub mod snapshot;
pub mod storage;
pub mod tool;
