# Web source

- URL: https://www.simplified.guide/codex/output-schema-use
- Title: How to use an output schema in Codex
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:16.251058710+00:00
- Relevance: High - title + snippet match query


```text
Codex exec’s `--output-schema` option reads a JSON Schema file from disk and constrains the final assistant message to a predictable object for automation; `--output-last-message` saves that response to a file while stdout still shows it unless `--json` is used for the JSONL event stream. The schema should mark required fields, use enums for closed status values, and set `"additionalProperties": false`, and Codex must run from an authenticated, trusted repository directory because trust and API authentication are checked first. An example schema defines `status` (`"ok"`, `"action_required"`), `summary`, and `next_action` (`"none"`, `"review"`, `"rerun"`); running `codex exec --output-schema schema.json --output-last-message result.json ...` produced `{"status":"ok","summary":"schema verified","next_action":"none"}`, which can be validated with `jq`.
```
