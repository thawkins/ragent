# Web source

- URL: https://github.com/Arcanada-one/model-connector/commit/212449cd26b0952b66b533a69700b7b9649dcb61
- Title: CONN-0046: surface stderr on empty stdout + classify malformed --outp… · Arcanada-one/model-connector@212449c
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:32:33.554212338+00:00
- Relevance: Medium - partial query match


```text
Commit 212449cd26b0952b66b533a69700b7b9649dcb61 to the `CodexConnector` class (extending `BaseCliConnector`) makes `parseOutput` use its stderr argument, adding an `extractStderrError(stderr)` helper that strips blank lines and "Reading additional input from stdin..." lines, joins the rest, and truncates to 500 characters; stderr error text now replaces the generic "No output" and "Failed to parse Codex JSONL output" messages. `classifyError` now returns `validation_error` when the message contains both "output schema" and "not valid json". New tests and fixtures cover a live-captured P8-d malformed `--output-schema` error ("Output schema file ... is not valid JSON: EOF while parsing a list at line 2 column 0"), a "Not logged in" auth failure (classified as `auth_error` via the base classifier), and the fallback to "No output" when both stdout and stderr are empty; an existing stderr-noise filter references the `codex_core::models_manager::manager` failed-to-refresh-models error dated 2026-04-23.
```
