# Web source

- URL: https://claude-wiki.com/discover-and-install-prebuilt-plugins-through-marketplaces.html
- Title: Discover and install prebuilt plugins through marketplaces - Claude Wiki
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:11:25.219349460+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code supports prebuilt plugins through marketplaces—catalogs that must be added before browsing/installing—with Anthropic’s official `claude-plugins-official` marketplace automatically available and installable via `/plugin install <name>@claude-plugins-official` (e.g., `github`). Marketplaces can be added from GitHub `owner/repo` repos, other git URLs, local paths, or remote `marketplace.json` URLs; plugins install at user, project, or local scope, are managed through `/plugin`, and activate with `/reload-plugins`. Official categories include code intelligence LSP plugins requiring language-server binaries (e.g., `pyright-lsp` requires `pyright-langserver`), external integrations (`github`, `gitlab`, `atlassian`, `asana`, `linear`, `notion`, `figma`, `vercel`, `firebase`, `supabase`, `slack`, `sentry`), development workflows (`commit-commands`, `pr-review-toolkit`, `agent-sdk-dev`, `plugin-dev`), and output styles (`explanatory-output-style`, `learning-output-style`). Anthropic also maintains the demo `anthropics/claude-code` marketplace; auto-update is enabled by default for official Anthropic marketplaces but disabled by default for third-party/local marketplaces, controlled by `DISABLE_AUTOUPDATER` and `FORCE_AUTOUPDATE_PLUGINS=1`, and because plugins execute arbitrary code, only trusted sources should be installed.
```
