# Web source

- URL: https://www.linkedin.com/posts/martin-monperrus-369300a4_most-coding-agents-describe-tools-with-json-activity-7495020448573857792-3cfT
- Title: Most coding agents describe tools with JSON Schema. The Lark agent takes an alternative route: every tool call is...
- Author(s): Martin Monperrus
- Language: English
- Published (UTC): 2026-08-17T07:35:25.655+00:00
- Captured (UTC): 2026-10-02T13:31:45.484507374+00:00
- Relevance: High - title + snippet match query


```text
The Lark agent, a "concept car" for tool calls as languages, departs from the usual JSON Schema approach by emitting every tool call as text constrained by a Lark grammar, passed to the inference endpoint as custom tool specs of the form `{ type: "grammar", syntax: "lark", ... }` so the model directly produces each tool's language. Its four tools are `apply_patch` (Codex's patch grammar), `exec` (Codex code-mode grammar), `read_file` (accepting `PATH` or `PATH?lines=N-M`), and `write_file` (path on the first line, file contents after). The author argues JSON is a useful universal envelope but not always the most natural interface, and that grammars can make the tool surface concise, readable, and structurally explicit while retaining ordinary coding agent capabilities; a live trajectory shows the model's own explanation of the decoding → translation → local-validation path.
```
