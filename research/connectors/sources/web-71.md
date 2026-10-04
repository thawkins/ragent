# Web source

- URL: https://www.promptfoo.dev/docs/providers/openai-codex-app-server
- Title: OpenAI Codex App Server | Promptfoo
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:32:06.378901261+00:00
- Relevance: Medium - partial query match


```text
Promptfoo's `openai:codex-app-server` provider launches the Codex app-server as a local child process and drives its experimental JSON-RPC protocol to evaluate rich client behavior—streamed agent items, approvals, skills, plugins, app connector events, and thread lifecycle; `openai:codex-desktop` is an alias, and promptfoo starts its own process rather than attaching to Codex Desktop. Provider IDs include `openai:codex-app-server:gpt-6-astra`, `:gpt-6-sol`, `:gpt-6-luna`, and `openai:codex-desktop:gpt-6-sol`; Astra requires Codex 0.153.1+, Sol/Luna require 0.156.1+, and GPT-5.6 models require 0.144.0+ (ultra reasoning is available for Sol but not Luna). Supported evals cover final text, text/image/local_image/skill/mention inputs, JSON schema output, token usage and cost (standard-rate estimates only; fast mode is not exactly billable), thread/turn IDs, deterministic server_request_policy responses, normalized streamed item metadata, and deep OTEL tracing, while live partial output, Desktop attachment, and WebSocket transport are unsupported. Safe defaults include `sandbox_mode: read-only`, `approval_policy: never`, `ephemeral: true`, `reuse_server: true`, and `inherit_process_env: false`, and Bedrock execution is available via `model_provider: amazon-bedrock`.
```
