# Web source

- URL: https://libraries.io/npm/@tpxipsterliu%2Fmcp-connect
- Title: @tpxipsterliu/mcp-connect on NPM
- Author(s): -
- Language: English
- Published (UTC): 2026-07-28T08:55:08+00:00
- Captured (UTC): 2026-10-02T13:27:55.547183457+00:00
- Relevance: Medium - partial query match


```text
mcp-connect (@tpxipsterliu/mcp-connect on npm, CLI binary `mcp-connect`) is a client-side connection layer for desktop AI agents such as Codex, WorkBuddy, and Claude Desktop that manages MCP server connection profiles and bridges remote Streamable HTTP MCP servers to local stdio MCP connectors, without containing domain business logic or acting as an agent runtime. Its planned CLI includes `init`, `create`, `connector add/list/show/update/remove`, `run`, `config path`, `doctor`, and `agent codex/claude/workbuddy`, resolving config via explicit `--config`, project `.mcp-connect/config.json`, then user-global `~/.mcp-connect/config.json`; OAuth, cloud sync, GUI management, and marketplace discovery are planned but not MVP. It was smoke-tested against `bot-ai-bridge/local-reader-bridge` at `http://localhost:3010/mcp`, and the recommended stack is TypeScript/Node.js LTS with commander or clipanion, zod, official MCP TypeScript SDK, pino, vitest, tsup, ESLint, and Prettier. The package is designed to publish from GitHub Actions via npm Trusted Publishing for owner `tarogoing`, repo `mcp-connect`, workflow `npm-publish.yml`, with dry run then `tag=beta`.
```
