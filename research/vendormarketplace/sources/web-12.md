# Web source

- URL: https://claude.yourdocs.dev/docs/claude-code/plugin-marketplaces
- Title: Claude Code Plugin Marketplaces — Claude
- Author(s): Claude
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:08:29.584346969+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code plugin marketplaces are JSON catalogs of plugins that enable discovery, installation, and management of Claude Code extensions, with sources including Git repositories, GitHub repos, local paths, and package managers. Users add marketplaces with `/plugin marketplace add` (e.g., `owner/repo`, Git URLs, or local paths) and install via `/plugin install plugin-name@marketplace-name`; teams can define required marketplaces in `.claude/settings.json` using `extraKnownMarketplaces`, so trusted repos auto-install those marketplaces and `enabledPlugins`. To create one, put `.claude-plugin/marketplace.json` at the repo root with required `name`, `owner`, and `plugins`; each plugin entry requires `name` and `source`, and may include `description`, `version`, `author`, `license`, `category`, `tags`, `commands`, `agents`, `hooks`, and `mcpServers`, with `strict` defaulting to `true`. Marketplaces can be hosted on GitHub (recommended), other Git services, or locally, and managed with `/plugin marketplace list/update/remove` (removal uninstalls plugins from it), with troubleshooting checks for URL access, JSON validity via `claude plugin validate`, and repository permissions.
```
