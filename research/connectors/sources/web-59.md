# Web source

- URL: https://learn.chatgpt.com/docs/app-server
- Title: Codex App Server | ChatGPT Learn
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:10.210871499+00:00
- Relevance: Medium - partial query match


```text
Codex app-server is the open-source (`openai/codex/codex-rs/app-server`) interface Codex uses to power rich clients such as the VS Code extension, supporting authentication, conversation history, approvals, and streamed agent events; for automation or CI, use the Codex SDK instead. It supports remote CLI connections (`codex app-server --listen ws://127.0.0.1:4500`; `codex --remote` over `ws://`, `wss://`, `unix://`, or `unix://PATH`, with `wss://` and token-env auth for non-local) and a remote Code Mode host via `--code-mode-host wss://...`. The protocol uses bidirectional JSON-RPC 2.0 (with `jsonrpc` omitted) over stdio (default, JSONL), WebSocket (experimental, one message per text frame), Unix socket, or `off`; `ws://IP:PORT` also serves `GET /readyz` and `/healthz`, rejects Origin headers with 403, and WebSocket auth options include capability-token files/SHA-256 or signed-bearer-token shared secrets. WebSocket mode is experimental/unsupported for production, non-loopback listeners may allow unauthenticated connections during rollout, overload returns JSON-RPC error -32001 “Server overloaded; retry later.” with retry via exponential backoff and jitter, and requests use `method`, `params`, and `id`; examples use GPT-6.1 Sol, which requires account/workspace access, with `model/list` available to select an available model.
```
