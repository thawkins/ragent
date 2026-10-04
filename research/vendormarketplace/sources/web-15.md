# Web source

- URL: https://claude-wiki.com/create-and-distribute-a-plugin-marketplace.html
- Title: Create and distribute a plugin marketplace - Claude Wiki
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:08:27.077919093+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code plugin marketplaces are catalogs (defined by a `.claude-plugin/marketplace.json` file at the repo root) that distribute plugins with centralized discovery, version tracking, and automatic updates; each plugin entry requires a `name` and a `source`, which may be a relative path (`./plugins/foo`), `github` repo (pinnable via 40-character `sha` or `ref`), generic `url` git repo, `git-subdir` (sparse clone for monorepos), or `npm` package (with optional `version` and private `registry`). Users add marketplaces via `/plugin marketplace add owner/repo` and install with `/plugin install plugin@marketplace`, while `claude plugin marketplace add/list/remove/update` subcommands and `claude plugin validate` support scripting; plugins are copied to `~/.claude/plugins/cache`, so they cannot reference files outside their directory except via symlinks. Hosting is recommended on GitHub (or GitLab/Bitbucket/self-hosted), private repos work through git credential helpers with `GITHUB_TOKEN`/`GITLAB_TOKEN`/`BITBUCKET_TOKEN` for background auto-updates, and enterprise control is available via `strictKnownMarketplaces` (empty array = lockdown; supports `hostPattern` and `pathPattern` regex). Reserved names blocked for third-party use include `claude-code-marketplace`, `claude-plugins-official`, `anthropic-marketplace`, `agent-skills`, and `life-sciences`; the `strict` field defaults to true, and release channels can be set up by pointing separate marketplaces at different refs.
```
