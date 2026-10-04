# Web source

- URL: https://github.com/franzos/claude-plugins
- Title: GitHub - franzos/claude-plugins: Claude Code plugin marketplace: a set of domain experts (subagents) grouped into...
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:12:14.287088227+00:00
- Relevance: Medium - multiple title terms match query


```text
The GitHub repo franzos/claude-plugins is a small Claude Code plugin marketplace (named gofranz) that groups domain-expert subagents into installable plugins, added via `/plugin marketplace add franzos/claude-plugins` and installed by name, with `@gofranz` to disambiguate collisions and local-directory checkout support. Its plugins include engineers (engineer:cpp, go, java, nextjs, qt, react, rust, typescript), identity (specialist:keycloak, oauth-oidc, oid4vc, node-oidc-provider), guix, security, sql, iota, iced, plan, forseti, stackpit, and infra (haproxy, nginx, caddy, traefik, docker, podman, systemd). It recommends declaring environment facts in `~/.claude/CLAUDE.md` so agents know build/test/format commands; otherwise agents probe and ask, and never install system-wide packages without asking. The `plan` plugin depends on Anthropic’s separately installed `feature-dev` plugin from `anthropics/claude-plugins-official`, and agents are portable Markdown with YAML frontmatter, with skills encouraged to delegate to existing agents rather than duplicate them.
```
