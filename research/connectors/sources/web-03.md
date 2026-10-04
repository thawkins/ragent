# Web source

- URL: https://mem.nowledge.co/docs/integrations/codex-cli
- Title: Codex
- Author(s): Nowledge Labs, @nowledgemem
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:26:42.827407876+00:00
- Relevance: High - title matches query


```text
Nowledge Mem’s Codex integration is a hybrid setup for the Codex desktop app and CLI, which share `~/.codex` config, plugin cache, hooks, and MCP settings: install the plugin, keep the bundled local MCP server at `http://127.0.0.1:14242/mcp/` enabled, and run the hook setup once so SessionStart injects Context Bundle/Working Memory, UserPromptSubmit routes continuation/history queries to Nowledge search, and the Stop hook captures transcripts via `nmem t save --from codex`. Recommended marketplace-first install uses `codex plugin marketplace add nowledge-co/community --sparse .agents --sparse nowledge-mem-codex-plugin` and `codex plugin add nowledge-mem@nowledge-community`, with `plugins = true`, `hooks = true`, and the plugin enabled in `~/.codex/config.toml`; remote Mem is set via `nmem config client set url/api-key` and `nmem config mcp show --host codex`. Skills (`working-memory`, `search-memory`, `save-thread`, `distill-memory`, `status`) replace older custom prompts, and to prevent duplicate learning the page recommends disabling Codex’s “Allow memory generation from tool-assisted tasks” / setting `disable_on_external_context = true`; troubleshooting notes package 0.1.19+ for hook-schema errors, 0.1.26+ for routing-boundary behavior, and WSL plugin updates must run in the same WSL distro.
```
