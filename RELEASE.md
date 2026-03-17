# Release

## Current Version: 0.1.0-alpha.14

### Added (since 0.1.0-alpha.13)
- **MCP server auto-discovery** — new `/mcp discover` command scans PATH, npm global packages, and well-known MCP registry directories for installed MCP servers
  - Recognizes 18 known MCP servers (filesystem, GitHub, git, postgres, sqlite, memory, brave-search, fetch, puppeteer, slack, google-drive, google-maps, sentry, sequential-thinking, everything, time, aws-kb-retrieval, exa)
  - Scans `@modelcontextprotocol` npm scope for installed servers
  - Reads Claude Desktop, Cline, and generic MCP registry directories
  - Discovered servers can be added to `ragent.json` config
- **TUI MCP discovery panel** — F9 key opens discovery panel showing available MCP servers

### Changed (since 0.1.0-alpha.13)
- MCP module structure reorganized with new `discovery.rs` submodule

### Carried from 0.1.0-alpha.13
- LSP test prompts — 5 test prompts for LSP server integration testing
- Office 365 test prompts — 5 test prompts for Office document read/write testing
- LSP workspace folders support
- OpenSkills support for extended skill file formats
- Output file support for `/simplify` skill
- Skills system fully implemented (SPEC §3.19) — 10 phases complete
- 781+ tests passing
