# Web source

- URL: https://platform.claude.com/docs/en/agents-and-tools/tool-use/strict-tool-use
- Title: Strict tool use
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:33:22.487605688+00:00
- Relevance: Medium-high - snippet matches query


```text
Setting `strict: true` on a tool definition uses grammar-constrained sampling to guarantee Claude’s tool inputs match the provided JSON Schema, ensuring correctly typed arguments, valid tool names, and fewer runtime errors or retries in agentic workflows. Strict tool use compiles `input_schema` into grammars via the structured-outputs pipeline and temporarily caches tool schemas for up to 24 hours since last use, though prompts and responses are not retained beyond the API response. It is HIPAA eligible, but PHI must not appear in `input_schema` property names, enum values, const values, or pattern regexes; the `computer_toolset_20260801` and `browser_toolset_20260801` entries reject `strict: true`.
```
