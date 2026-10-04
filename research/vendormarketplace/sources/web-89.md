# Web source

- URL: https://github.com/openai/codex/commit/66b0781502be5de3b1909525c987643b9e5e407d
- Title: /plugins: add marketplace install flow (#18704) · openai/codex@66b0781
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:13:25.728461257+00:00
- Relevance: Medium - multiple title terms match query


```text
This commit to OpenAI's Codex repo adds marketplace-add support to the TUI: it introduces `fetch_marketplace_add`, which sends a typed `MarketplaceAdd` request (with a UUID request ID) over the app-server protocol and emits `MarketplaceAddLoaded`, plus new `AppEvent` variants (`OpenMarketplaceAddPrompt`, `OpenMarketplaceAddLoading`, `FetchMarketplaceAdd`) and a `newly_installed_marketplace_tab_id` field on `ChatWidget`. A new `marketplace_add_source_for_request` helper resolves relative local paths (`./`, `../`, `.\`, `..\`) against the cwd while preserving `#ref`/`@ref` suffixes, and on successful add the app refetches the plugins list if the cwd matches. The change also rewords error messages (reserved curated marketplace "cannot be added from this source"; already-added source; invalid format now specifies "expected owner/repo, a git URL, or a local marketplace path") and adds tests covering relative-path resolution.
```
