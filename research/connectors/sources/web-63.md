# Web source

- URL: https://note.com/masa_wunder/n/n53f45b621510?hl=en
- Title: [Must-Read for Developers] A Thorough Explanation of the Codex app-server! The Difference from the Codex SDK and a...
- Author(s): -
- Language: English
- Published (UTC): 2026-05-06T10:25:32+00:00
- Captured (UTC): 2026-10-02T13:31:29.952847576+00:00
- Relevance: Medium - partial query match


```text
`codex app-server` is described as a JSON-RPC 2.0-over-stdio server bundled with the Codex CLI and spawned as a child process, making it language-agnostic for GUI apps such as the author's Tauri 2 + React `codex-image-editor`; it is categorized alongside `exec` for CI one-shots and the SDK for typed TS/Python server use, not as a competitor. Its authentication is a local trust model in `~/.codex/auth.json` with no per-request identity or authorization, so BYO ChatGPT subscription distribution relies on each user's local codex login, the app author should not handle keys, and VPS exposure requires Tailscale/mTLS or a thin auth bridge; the author recommends mandatory `approvalPolicy: "on-request"` and `sandbox: "workspace-write"`. The article also warns that official documentation drifts from the actual schema—use `codex app-server generate-json-schema` as the source of truth—and reports gpt-image-2 1024x1024 pricing of $0.006 low, $0.053 medium, and $0.211 high (as of 2026-05-06), with ChatGPT Plus ($20/month) breaking even at about 3,333 low, 377 medium, or 95 high images and Pro ($200/month) at about 947 high images.
```
