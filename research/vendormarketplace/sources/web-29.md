# Web source

- URL: https://spybara.com/anthropic/claude-code/history/docs/en/2026-01-11-1802..2026-01-12-2102/plugin-marketplaces
- Title: plugin-marketplaces.md — Spybara
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:10:02.570575786+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code plugin marketplaces are catalogs for distributing extensions, offering centralized discovery, version tracking, automatic updates, and support for sources such as git repositories and local paths. Creating one requires plugins plus a `.claude-plugin/marketplace.json` catalog (required fields: `name`, `owner`, `plugins`; each plugin requires `name` and `source`), hosted on GitHub, GitLab, or another git service; users add it with `/plugin marketplace add` and refresh with `/plugin marketplace update`. Private repos use `GITHUB_TOKEN`/`GH_TOKEN`, `GITLAB_TOKEN`/`GL_TOKEN`, or `BITBUCKET_TOKEN`; teams can require marketplaces via `extraKnownMarketplaces`/`enabledPlugins` in `.claude/settings.json`, while admins restrict them with `strictKnownMarketplaces` (undefined = no restrictions, `[]` = complete lockdown, allowlist = exact matching). Validation uses `claude plugin validate` or `/plugin validate`; URL-based marketplaces only download `marketplace.json`, so relative plugin paths fail, and plugins are copied to a cache so paths outside the plugin directory also fail.
```
