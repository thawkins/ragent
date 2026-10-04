# Web source

- URL: https://thomas-wiegold.com/blog/claude-api-structured-output
- Title: Claude API Structured Output: Complete Guide to Schema-Guaranteed Responses
- Author(s): Thomas Wiegold
- Language: English
- Published (UTC): 2025-11-15T00:00:00+00:00
- Captured (UTC): 2026-10-02T13:33:37.912495894+00:00
- Relevance: High - title + snippet match query


```text
Anthropic released structured outputs in public beta on November 14, 2025, using constrained decoding to compile a JSON schema into a grammar and restrict token generation, with beta header `anthropic-beta: structured-outputs-2025-11-13`; it supports Claude Sonnet 4.5 and Opus 4.1, with Haiku 4.5 coming and older 3.x models unsupported. It offers JSON outputs via `output_format` (guaranteed-valid JSON in `response.content[0].text`) and strict tool use with `strict: true`, with SDK examples using Python/Pydantic `.parse()` and TypeScript/Zod. First schema use adds 100–300 ms and is cached 24 hours; failures include safety refusals (`stop_reason: refusal`, 200 and billed), max_tokens truncation, and 400 schema errors; it is incompatible with citations and JSON-output message prefilling but works with batch processing, token counting, streaming, and combined JSON+strict tool use. System-prompt overhead is 50–200 tokens (2–3% cost increase at scale), numerical min/max constraints are not enforced, and recursive schemas should be avoided.
```
