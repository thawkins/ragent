# Web source

- URL: https://www.creativeainews.com/articles/bun-rust-rewrite-claude-code-anthropic-2026
- Title: Inside Bun&#x27;s 1M-Line Rust Rewrite by Claude Code
- Author(s): Vannarot Roeung
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:35:47.191263752+00:00
- Relevance: Medium - multiple title terms match query


```text
On May 14, Bun PR 30412 ported Bun’s entire Zig codebase to Rust in nine days, adding 1,009,257 lines, removing 4,000, and touching 2,188 files; the new tree passes 99.8% of Bun’s existing tests on Linux x64, with the remaining 0.2% involving platform-specific issues like glibc version detection and Linux capability flags. The rewrite was driven by Anthropic’s Claude Code, shipped after Anthropic acquired Bun in December 2025, using a four-phase loop: parallel translation, compile-error fixup (16,000+ errors), test-suite bisection (from ~70% to 99.8%), and a cleanup PR removing ~600,000 lines of legacy Zig, titled “ai slop” by Jarred Sumner and flagged by GitHub’s anti-AI-slop detection. The largest production rewrite credited to an AI agent to date left 13,000-plus unsafe blocks—about 181 times the density of uv—mostly at the JavaScriptCore FFI boundary but also from direct Zig-idiom translation, making the tree a release candidate rather than stable LTS. Key takeaways: agent-driven ports work for mechanical translation, not redesign; language-switching costs have collapsed; and the bottleneck shifts to senior review of architectural/unsafe boundaries and integration tests, with Bun users advised to pin 1.3.14 for now while watching issue volume, comparable ports, and competitors like Grok Build and Cursor Composer 2.5.
```
