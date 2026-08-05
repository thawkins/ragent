# ragent — An AI coding agent for the terminal, built in Rust

Hey r/rust,

I wanted to share a project I've been building: **ragent** — a terminal-native AI coding agent written in Rust. Think of it as a self-contained, statically-linked alternative to tools like RooCode, Claude Code, Copilot CLI, or OpenCode, with zero runtime dependencies.

## What it does

- **Multi-provider LLM support** out of the box: Anthropic, OpenAI, Google Gemini, Hugging Face, GitHub Copilot, Ollama (local/cloud), generic OpenAI-compatible endpoints, Azure AI Foundry, Azure Resource (File), Amazon Bedrock, Microsoft Foundry Local, plus a built-in Model Router.
- **Local-first defaults** — if you don't configure a model, it tries local/self-hosted providers first (e.g. Ollama) instead of forcing a cloud API.
- **~150 built-in tools** across file ops, shell execution (with a 7-layer security model), search, web fetch/search/crawl, browser automation, code intelligence, memory, teams/swarm coordination, GitHub/GitLab, office/PDF, sub-agents, planning, MCP, and more.
- **Full-screen TUI** with streaming chat, markdown/syntax highlighting, slash commands, agent switching, and a live permission countdown.
- **HTTP server mode** with REST + SSE so any frontend can drive the agent.
- **Session management, memory system, specs, skills, research mode, autopilot, and code indexing** — all baked into one binary.

## Get it

- **GitHub repo:** https://github.com/thawkins/ragent
- **Releases:** https://github.com/thawkins/ragent/releases
- **Author:** Tim Hawkins (@thawkins)

## Installation

Pre-built packages are available on the releases page:

- `.deb` for Debian/Ubuntu
- `.rpm` for Fedora/RHEL/openSUSE

For other platforms, you can build from source with Cargo (Rust 1.85+ / edition 2024):

```bash
git clone https://github.com/thawkins/ragent.git
cd ragent
cargo build --release
# Binary is at target/release/ragent
```

The binary is statically linked and has no runtime dependencies, so it should run pretty much anywhere after you build or install it.

## Quick start

read QUICKSTART.md for cli use

read TUI-QUICKSTART.md for working in the TUI CLI

Feedback, issues, and contributions are very welcome!
