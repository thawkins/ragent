# Web source

- URL: https://research.google/blog/accelerating-code-migrations-with-ai
- Title: Accelerating code migrations with AI
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:34:13.678796987+00:00
- Relevance: Medium - multiple title terms match query


```text
Google’s internal AI migration toolkit fine-tunes a Gemini model on Google’s monorepo using the DIDACT methodology; it splits migrations into targeting, edit generation/validation, and review/rollout, using pre-existing static tools like Kythe and Code Search for targeting and focusing on AI-generated diffs validated by compilation/unit tests plus optional ML-powered repairs. In a Google Ads case study migrating ID types from 32-bit to 64-bit integers across thousands of files and tens of thousands of locations, 80% of code modifications in landed CLs were AI-authored, total migration time was reduced by an estimated 50%, and Java file edit-need prediction accuracy was 91%. The toolkit has created hundreds of change lists, with >75% of AI-generated character changes successfully landing in the monorepo on average. The work was a collaboration among Google Core Developer, Google Ads, and Google DeepMind teams, with next steps including more complex multi-component migrations and IDE/user-experience improvements.
```
