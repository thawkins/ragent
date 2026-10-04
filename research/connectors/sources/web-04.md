# Web source

- URL: https://dev.to/themesberg/build-apps-for-chatgpt-claude-and-gemini-using-mcp-ui-components-from-flowbite-316b
- Title: Build apps for ChatGPT, Claude, and Gemini using MCP UI components from Flowbite
- Author(s): @themesberg
- Language: English
- Published (UTC): 2026-02-06T14:07:46+00:00
- Captured (UTC): 2026-10-02T13:26:36.742232741+00:00
- Relevance: High - title + snippet match query


```text
MCP UI is a standard SDK for building MCP apps that run in ChatGPT, Gemini, Claude, and other MCP clients like Cursor or Windsurf; the Model Context Protocol is part of the Agentic AI Foundation donated by Anthropic in 2025 and has over 100 million monthly SDK downloads. The guide uses Flowbite UI components and the Skybridge framework to build an app from the themesberg/mcp-ui-starter repo, run it locally with `npm run dev --use-forwarded-host` on `http://localhost:3000` with the MCP server at `/mcp` and Flowbite/React widgets as tools, then expose it via ngrok at a forwarding URL ending in `/mcp`. It documents adding the server to ChatGPT, Claude Web, Gemini CLI (`gemini mcp add --transport http`), Cursor (`mcp.json`), VS Code (`.vscode/mcp.json`), Claude Code (`~/.claude.json`), Mistral AI, and Codex (`~/.codex/config.toml`). It also shows creating a `basic-text` widget with a Zod server-side tool and React front end using `skybridge/web`’s `mountWidget`/`useToolInfo`, registering it in `server.ts` for `mcp-ui-components` v0.0.1, and customizing Flowbite themes (default, minimal, enterprise, playful, mono) via `index.css`.
```
