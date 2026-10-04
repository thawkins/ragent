# Web source

- URL: https://platform.claude.com/docs/en/api/php/beta/organization/plugin_marketplaces
- Title: Plugin Marketplaces - Claude API Reference
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:08:34.440803965+00:00
- Relevance: Medium - multiple title terms match query


```text
The `plugin_marketplace` object always has type `plugin_marketplace` and an ID prefixed `marketplace_`. Its `defaultInstallationPreference` controls the organization-wide setting (`required`, `auto_install`, `available`, or `not_available`) and is null for a member’s personal marketplace; `lastSyncEndedAt` is an RFC 3339 timestamp for the latest completed sync attempt, or creation time if no repository sync has run, and null for non-repository marketplaces. Other fields cover the last-read commit, the organization or member owner, plugin source (`manual`, `github`, `gitlab`, `public_git`, or Anthropic’s `directory` catalog, which this API does not list), and sync status (`success`, `in_progress`, `failed_content`, `failed_transient`, `failed_auth`, `failed_limits`; null until first sync).
```
