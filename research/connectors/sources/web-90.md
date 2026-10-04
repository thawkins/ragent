# Web source

- URL: https://docs.anthropic.com/en/docs/build-with-claude/tool-use/implement-tool-use
- Title: Define tools
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:33:29.096895401+00:00
- Relevance: Medium - partial query match


```text
Anthropic’s tool-use implementation guide says client tools are specified in the API request’s top-level `tools` parameter; Anthropic-schema tools such as bash and text editor use date-versioned types, while computer use and browser use are client toolsets declared as one unnamed entry with fixed member tools. When `tools` is passed, the API constructs a special system prompt from tool definitions, tool configuration, and any user system prompt. It recommends detailed descriptions (at least 3–4 sentences per tool), schema-validated `input_examples` for complex inputs (not supported for server-side tools or client toolsets; invalid examples return 400; ~20–50 tokens simple, ~100–200 complex), consolidating related operations, meaningful namespacing, high-signal responses, and asking for short explanations rather than reasoning to avoid `reasoning_extraction` refusals. `tool_choice` supports `auto` (default with tools), `any`, `tool`, and `none` (default without tools); forced use is not supported on all models, and `any`/`tool` prefills the assistant message so no natural-language explanation precedes `tool_use`. Claude may also produce natural commentary before tool calls, so code should not rely on specific formatting.
```
