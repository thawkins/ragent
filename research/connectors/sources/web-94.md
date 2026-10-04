# Web source

- URL: https://www.linkedin.com/posts/schaunwheeler_im-posting-this-in-case-it-can-help-anyone-activity-7429134169789644800-Tnfd
- Title: Anthropic Claude API concurrent call issues with dynamic JSON schema keys | Schaun Wheeler posted on the topic |...
- Author(s): Schaun Wheeler
- Language: English
- Published (UTC): 2026-02-16T12:06:52.688+00:00
- Captured (UTC): 2026-10-02T13:33:24.751077752+00:00
- Relevance: High - title + snippet match query


```text
Schaun Wheeler reports that Relay’s concurrent LLM structured-output calls worked on OpenAI and Gemini but failed intermittently on Anthropic’s Claude API—especially at 32 simultaneous calls—with an unhelpful “Invalid request” error that was not a rate limit. The cause was dynamic JSON schema keys: random hex property names forced Anthropic to compile a new grammar for each schema, and many concurrent compilations overwhelmed the system. Replacing the random keys with fixed positional names like `alt_1`, `alt_2`, etc., and mapping them back to internal IDs after the response made the schema identical across calls, so the grammar compiled once, was cached, and produced zero errors. His advice: keep schema keys fixed and do ID mapping in your own code after the response; the error message won’t point to grammar compilation.
```
