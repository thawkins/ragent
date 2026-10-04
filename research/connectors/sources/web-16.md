# Web source

- URL: https://dev.to/aicoding-guide/codex-cli-v0158-mcp-oauth-client-secrets-and-approval-for-elevated-commands-52o7
- Title: Codex CLI v0.158: MCP OAuth client secrets and approval for elevated commands
- Author(s): @
- Language: English
- Published (UTC): 2026-09-30T19:10:03+00:00
- Captured (UTC): 2026-10-02T13:27:50.886946411+00:00
- Relevance: Medium - multiple title terms match query


```text
Codex CLI v0.158.0 enables terminal input approval by default for commands with elevated permissions (while runtime-only grants no longer cause repeat reviews) and adds MCP server support for pre-registered OAuth client secrets via `codex mcp add --oauth-client-secret`, though that flag was absent from the official configuration reference as of September 30, 2026. The release also fixes sandbox issues on Windows (Windows 10 paths, stored credentials, large permission policies), Linux (nested writable roots startup), and Linux/macOS (git metadata protections across writable roots), plus macOS path-alias approvals and bearer-token-secured exec-server WebSocket connections. v0.159.0 adds opt-in `instant_interrupt`, TUI/Mermaid changes, and `.aws` protected by default under writable roots; v0.159.2 is a Windows-only patch preventing console-window flashes during background or sandboxed command launches.
```
