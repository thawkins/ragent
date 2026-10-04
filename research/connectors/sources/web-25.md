# Web source

- URL: https://aiidelist.com/blog/codex-apps-mcp-failed-to-start
- Title: Codex Apps MCP Failed to Start: Why It Happens and How to Fix It
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:28:25.274247020+00:00
- Relevance: Medium - multiple title terms match query


```text
The `codex_apps` MCP startup error means Codex failed during the MCP handshake with its built-in app-connector layer, specifically a remote ChatGPT endpoint at `https://chatgpt.com/backend-api/ps/mcp`, yielding “handshaking with MCP server failed” and “MCP startup incomplete” rather than a local project or custom MCP server failure. Likely causes are terminal network/proxy mismatch, ChatGPT authentication state, TLS interception such as `CODEX_CA_CERTIFICATE`, or app-connector configuration; recommended diagnosis is to test `curl -v` against the endpoint and check `env | grep -i proxy`, then refresh auth with `codex logout && codex login`, update Codex, and optionally disable connectors in `~/.codex/config.toml` via `[apps._default] enabled = false` or `apps.<id>.enabled`. The article treats this as a Codex integration-layer warning and notes Codex supports STDIO and Streamable HTTP MCP transport, ChatGPT/API-key sign-in, and the Codex app on macOS and Windows.
```
