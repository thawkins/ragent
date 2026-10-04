# Web source

- URL: https://dev.to/nguyen_jesse_8602dc05abd6/making-claude-code-concise-without-making-it-dumber-the-engineering-behind-two-open-source-plugins-3ll9
- Title: Making Claude Code concise without making it dumber: the engineering behind two open-source plugins
- Author(s): @
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:09:28.800356277+00:00
- Relevance: Medium - multiple title terms match query


```text
clear-claude (MIT; github.com/jessebldr/clear-claude) is a three-layer Claude Code marketplace: Clear Partner for concise communication, Clear UI for the statusline, and experimental Clear Transcript for redrawing the transcript via undocumented function hooks. Clear Partner, a ~4.6 KB output style, measured 524→258 words (−51%) across four runs per arm on three questions (no plugin answer was as long as the shortest stock answer), and its 12-case eval suite scored 6/6 on the first six basics with and without the style on Claude Code 2.1.274, while documenting regression cases such as table-prone case k and one-sentence-after-tool-use case l plus small-sample limits. Clear UI is a zero-dependency statusline with a pure render core showing model, project/git branch with dirty dot, context percentage, 5-hour/weekly usage and an opt-in weekly-limit chip; it benchmarked 38 ms cached and 46 ms on cache miss on an M4 Mac mini, and its usage provider reads a cache file refreshed at most every 10 minutes by a detached worker after a Git Bash incident in which /usage became a path prompt and cost $0.136. Clear Transcript is not in the marketplace, runs only from a clone with --plugin-dir and CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1, and its 1.0 waits for Anthropic to document, stabilise, and enable function hooks by default; Clear Partner and Clear UI install separately via claude plugin install clear-partner@clear-claude and clear-ui@clear-claude.
```
