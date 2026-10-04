# Web source

- URL: https://hex.pm/packages/codex_sdk/0.18.1/files/guides/14-plugin-marketplaces.md
- Title: guides/14-plugin-marketplaces.md - codex_sdk 0.18.1
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:13:52.437459935+00:00
- Relevance: Medium - multiple title terms match query


```text
`Codex.Plugins.Marketplace` models Codex discovery’s local `.agents/plugins/marketplace.json`; canonical writes are Codex-native, but read/validation flows also accept `.claude-plugin/marketplace.json`. Plugin entries use nested `policy.*` fields such as `installation`, `authentication`, and optional `products`, do not emit legacy top-level `installPolicy`/`authPolicy`, and `add_marketplace_plugin/3` preserves unrelated/unknown keys, refuses duplicate plugin names unless `overwrite: true`, and writes deterministic pretty JSON with a trailing newline. Marketplace source paths must resolve within the marketplace root, reject `..` traversal even if the expanded path would remain under the root, and are checked on read/write/update; repo scope uses `<repo-root>/.agents/plugins/marketplace.json` (alternate: `<repo-root>/.claude-plugin/marketplace.json`) with plugin paths `./plugins/<plugin-name>`, while personal scope uses `~/.agents/plugins/marketplace.json`. Author locally with `Codex.Plugins.*`, use `Codex.CLI.marketplace_add/2` or `Codex.AppServer.marketplace_add/3` for runtime acquisition from a source tree or Git ref, optionally start `codex app-server` for runtime verification, then verify with `plugin/list` and `plugin/read`; local authoring and runtime verification remain separate APIs that share stable validation vocabulary.
```
