# Web source

- URL: https://claude-code.hexdocs.pm/ClaudeCode.Plugin.Marketplace.html
- Title: ClaudeCode.Plugin.Marketplace — ClaudeCode v0.36.5
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:08:25.299443169+00:00
- Relevance: Medium - multiple title terms match query


```text
The `ClaudeCode.Plugin.Marketplace` module wraps `claude plugin marketplace` CLI commands for managing catalogs that define where plugins can be discovered and installed from, including GitHub repos, git URLs, npm packages, and more. Functions resolve the CLI binary via the internal `Adapter.Port.Resolver` and execute synchronously; remote node support is not yet implemented, so commands run locally only. Its API includes `list/0` (returns `%ClaudeCode.Plugin.Marketplace{}` structs parsed from `claude plugin marketplace list --json`), `add/2` (accepts GitHub shorthand `"owner/repo"`, full git URLs, or local paths, with `scope` options `:user` default, `:project`, `:local`, and `sparse` paths for monorepos), `remove/1` by name, and `update/1` for all marketplaces or one named marketplace.
```
