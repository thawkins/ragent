# Web source

- URL: https://gist.github.com/clairernovotny/89587e4932d854b10bbab913b95ecb5c
- Title: Codex plugins and marketplaces developer notes from source analysis
- Author(s): 262588213843476
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:13:34.090300184+00:00
- Relevance: Medium - multiple title terms match query


```text
Scanned `openai/codexmain` at commit `dae0608c06bf61a356209fd11243aec1ef816547` on April 17, 2026, the Codex plugin feature is present and stable/default-enabled. Plugins are local bundles of skills, MCP servers, ChatGPT app connector IDs, and manifest metadata installed under `$CODEX_HOME/plugins/cache/<marketplace>/<plugin>/<version>` and enabled via user config; marketplaces are discovered from `.agents/plugins/marketplace.json` or `.claude-plugin/marketplace.json`, with the official `openai-curated` marketplace mirrored under `$CODEX_HOME/.tmp/plugins` and its revision recorded in `$CODEX_HOME/.tmp/plugins.sha`. The implementation exposes `marketplace/add`, `plugin/list`, `plugin/read`, `plugin/install`, and `plugin/uninstall`, supports Git/local sources, sparse checkout, product restrictions (default `CODEX`), OAuth, automatic Git marketplace updates, plugin mentions like `plugin://<plugin-name>@<marketplace-name>`, and a suggestion allowlist including GitHub, Notion, Slack, Gmail, Google Calendar, Google Drive, Linear, Figma, and Computer Use.
```
