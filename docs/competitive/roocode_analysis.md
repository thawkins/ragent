# Roocode vs ragent: Strengths and Gaps

## Strengths of Roocode
- **Multiple Interaction Modes**: Code, Architect, Ask, Debug, Orchestrator (Boomerang) with mode‑specific permissions.
- **Customizable Profiles & Modes**: Users can define system prompts, assign different AI models per mode, and share custom modes via marketplace.
- **Advanced Context Management**: Intelligent context condensation, configurable limits, and automatic summarisation to stay within model windows.
- **Semantic Codebase Indexing**: User‑controlled embedding providers (Gemini, OpenAI, Ollama) and vector DBs (Qdrant) for meaning‑based search.
- **Integrated Terminal & Command Execution**: Real‑time output, error detection, and ability to install dependencies, run builds/tests, etc.
- **Real‑time Diff & File Ops**: Read/write, concurrent file reads, experimental multi‑file edits with previewed diffs.
- **MCP (Model Context Protocol) Integration**: Extensible external tool connections.
- **Git‑like Checkpoint System**: Version‑control style checkpoints for safe roll‑backs.

## Missing Capabilities Compared to ragent
- **Unified Tooling Framework**: ragent ships with a rich set of built‑in tools (file edit/create, bash, grep, glob, webfetch, etc.) that are immediately usable without extra extensions.
- **Client/Server Architecture & TUI**: ragent provides a terminal UI, HTTP API (Axum), and WebSocket streaming for headless automation.
- **Workspace‑wide Session Management**: Persistent session history stored in SQLite, permission system, and multi‑agent teamwork features.
- **Built‑in Multi‑Provider LLM Orchestration**: ragent can seamlessly switch between Anthropic, OpenAI, Copilot, and Ollama without manual profile juggling.
- **Automatic Codebase Indexing**: ragent does not currently offer semantic vector search; indexing is left to external tools.
- **Checkpoint System**: ragent lacks a native Git‑like checkpoint/rollback mechanism.
- **Mode System**: While Roocode’s mode architecture enables read‑only planning, ragent relies on agents (coder, debug, architect) but without per‑mode permission granularity.
- **Marketplace for Extensions**: Roocode’s community‑driven mode marketplace is not present in ragent.

*This analysis is stored in `docs/competitive/roocode_analysis.md` for further reference.*