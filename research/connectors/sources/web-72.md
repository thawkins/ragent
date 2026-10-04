# Web source

- URL: https://github.com/openai/codex/commit/e639e8c4bd9b6a65cc5170fe3e236558637d55f8
- Title: connectors: own app metadata types (#29723) · openai/codex@e639e8c
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:56.845491439+00:00
- Relevance: Medium - multiple title terms match query


```text
Commit e639e8c4 in openai/codex adds a `codex-connectors` workspace dependency and introduces an explicit `app_info_to_api` conversion layer that maps connector-domain types (`AppInfo`, `AppBranding`, `AppMetadata`, `AppReview`, `AppScreenshot`) from `codex-connectors` into their app-server wire counterparts in `codex-app-server-protocol` (via sub-converters `app_branding_to_api`, `app_review_to_api`, `app_screenshot_to_api`, and `app_metadata_to_api`). The comment notes that an explicit function is required instead of a `From` impl due to Rust's orphan rules, and that the types remain separate so app-server protocol ownership does not leak into the connector domain crate. The change updates `paginate_apps` to map the sliced connector range through `app_info_to_api` and applies the same mapping in `send_app_list_updated_notification`; several files (including CLI and related crates) swap `codex_app_server_protocol::AppInfo` imports for `codex_connectors::AppInfo`, and `codex-app-server-protocol` is removed from some dependency lists. Referenced fields include `first_party_requires_install` and `show_in_composer_when_unlinked`.
```
