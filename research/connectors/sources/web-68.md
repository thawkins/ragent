# Web source

- URL: https://github.com/openai/codex/commit/464ab40dfa1fd5058ea52512c29f38d2e4f6b204
- Title: feat: best-effort compact large tool schemas (#23904) · openai/codex@464ab40
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:38.574262544+00:00
- Relevance: High - title + snippet match query


```text
Commit 464ab40dfa1fd5058ea52512c29f38d2e4f6b204 in openai/codex adds best-effort compaction for unusually large tool input schemas in Rust: `parse_tool_input_schema` now calls `compact_large_tool_schema` after sanitization and unreachable-definition pruning. It uses a 4,000-byte compact normalized JSON budget (`MAX_COMPACT_TOOL_SCHEMA_BYTES`) and depth limit 2 (`MAX_COMPACT_TOOL_SCHEMA_DEPTH`), applying lossy passes—`strip_schema_descriptions` then `collapse_deep_schema_objects_from_root`—while preserving the top-level argument surface. The commit also adds helpers to rewrite local `$ref`s to empty schemas before dropping root definition tables and to collapse complex schema objects at depth ≥2; the 4,000-byte check is described as a cheap local proxy for a 1k-token limit.
```
