# Web source

- URL: https://temporal.io/blog/using-coding-agents-on-a-migration-three-practices-that-mattered
- Title: Using coding agents on a migration: Three practices that mattered
- Author(s): Paul Oh, Chandler Ortman
- Language: English
- Published (UTC): 2026-09-02T00:00:00+00:00
- Captured (UTC): 2026-10-02T21:30:17.861098776+00:00
- Relevance: High - title matches query


```text
Temporal’s post describes migrating the store behind Temporal Cloud’s usage/billing data to ClickHouse with heavy coding-agent use, finding agents helped most with cleanup rather than construction: the final teardown touched several dozen mostly deployment-config files in one working session, guided by `CLEANUP-NOTE` comments written when temporary migration code was created; durable rules were placed next to code and tied to regression tests after a ported query’s fixed-width integer accumulator overflowed, producing negative totals for large accounts. The author cannot claim dramatic speedup: prompt history starts over a week into the project, there is no hand-work baseline, and the honest estimate is most cleanup would have remained another year; unattended CI/review loops mostly produced hygiene, and the most effective rules embedded years of billing-pipeline-specific carve-outs. One instruction file still wrongly directs agents to deleted files and an obsolete data structure, showing prose instructions can rot unnoticed.
```
