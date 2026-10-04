# Web source

- URL: https://www.sean-weldon.com/blog/2026-01-06-how-to-install-and-discover-claude-code-plugins-through-mark
- Title: How to Install Claude Code Plugins (Marketplace Guide 2026)
- Author(s): Sean Weldon
- Language: English
- Published (UTC): 2026-01-06T00:00:00+00:00
- Captured (UTC): 2026-10-02T14:07:58.007500874+00:00
- Relevance: Medium - multiple title terms match query


```text
Sean Weldon’s Jan 6, 2026 post explains that Claude Code plugin marketplaces are catalogs registered separately from plugin installation, so adding a marketplace enables browsing but installs nothing; plugins can be installed at user, project, or local scope via `/plugin install plugin-name@marketplace-name` and managed through the `/plugin` interface. The official Anthropic marketplace (`claude-plugins-official`) is preconfigured, while third-party catalogs are added with `/plugin marketplace add` using GitHub `owner/repo`, Git URLs, local paths, or direct JSON URLs. Official plugins include LSP-based code intelligence for 10 languages—e.g., Python `pyright-langserver`, TypeScript `typescript-language-server`, Rust `rust-analyzer`, Go `gopls`—requiring language-server binaries, plus external integrations (GitHub, GitLab, Jira/Confluence, Slack, etc.), workflow plugins, and output styles; plugin commands use namespaced syntax such as `/commit-commands:commit`. Auto-update defaults on for official marketplaces and off for third-party/local, plugins require Claude Code 1.0.33+, and Anthropic does not verify third-party plugin contents.
```
