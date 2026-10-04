# Web source

- URL: https://dev.to/aicoding-guide/sign-in-to-a-remote-mcp-server-in-claude-code-mcp-and-claude-mcp-login-43i7
- Title: to a remote MCP server in Claude Code: /mcp and claude mcp login
- Author(s): @
- Language: English
- Published (UTC): 2026-09-27T19:21:56+00:00
- Captured (UTC): 2026-10-02T13:30:26.967474654+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code requires browser-based OAuth sign-in for hosted MCP servers such as Sentry, Linear, and Notion; after `claude mcp add --transport http <name> <url>`, `claude mcp list` shows `! Needs authentication` until the user authenticates via `/mcp` → Authenticate or `claude mcp login <name>` (revoke with `claude mcp logout <name>` or “Clear authentication” in `/mcp`), with tokens stored securely and refreshed automatically, and servers flagged when they return 401/403 plus a startup notice since v2.1.193. Headless/SSH users can run `claude mcp login --no-browser` to get an authorization URL and paste the redirect URL back (requiring an interactive terminal such as `ssh -t`), while non-interactive runs (`claude -p`, Agent SDK) have no `/mcp` panel and, as of v2.1.196 with tool search enabled, tell Claude the server’s tools are unavailable until authorized. Without Dynamic Client Registration, users must register an OAuth app and pass `--client-id`, `--callback-port <port>` (Claude Code otherwise picks a random free port), and optionally `--client-secret`/`MCP_CLIENT_SECRET`; JSON config supports `oauth` with `clientId`, `callbackPort`, `scopes` (which pins scopes, takes precedence over `authServerMetadataUrl` and `/.well-known`, and appends `offline_access` if advertised), and `authServerMetadataUrl`, while the client secret is stored in the keychain/credentials file and can only be set at add time (to change it, remove and re-add with the same `--scope`; `claude mcp get <name>` checks credentials). On token expiry, a 401 triggers refresh/reconnect/retry once, a rejected refresh token requires Re-authenticate in `/mcp`, and troubleshooting covers browser-not-opening (copy URL), redirect failure (paste full callback URL), Authorization header 401/403 (no OAuth fallback), and `403 insufficient_scope` (add scope to `oauth.scopes` then re-authenticate); the on-disk location of OAuth access/refresh tokens is not confirmed in official docs.
```
