# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha.0] - Unreleased

### Added
- Initial project scaffolding with Cargo workspace (3 crates)
- Core library (ragent-core): agent, config, event, llm, mcp, message, permission, provider, session, snapshot, storage, tool modules
- Provider adapters for Anthropic and OpenAI with SSE streaming
- 8 built-in tools: read, write, edit, bash, grep, glob, list, question
- Permission system with glob-based rule matching
- SQLite storage for sessions, messages, and provider auth
- HTTP server (ragent-server) with REST + SSE endpoints via axum
- Terminal UI (ragent-tui) with ratatui
- CLI entry point with clap (run, serve, session, auth, models, config commands)
- Event bus for real-time internal pub/sub
- File snapshot/restore for undo support
- Workspace-wide lint configuration (clippy pedantic, nursery, missing_docs)
