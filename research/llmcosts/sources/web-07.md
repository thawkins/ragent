# Web source

- URL: https://doi.org/10.18653/v1/2024.acl-long.850
- Title: Proceedings of the 62nd Annual Meeting of the Association for Computational Linguistics (Volume 1: Long Papers),…
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T01:23:52.837558575+00:00
- Relevance: Scholarly — engine-ranked abstract
- Open-access recovery: full text fetched from unpaywall (https://aclanthology.org/2024.acl-long.850.pdf); version=gold, license=cc-by

```text
**AppWorld** (Trivedi et al., Stony Brook University, Allen Institute for AI, Saarland University; ACL 2024 Long Papers, pp. 16022–16076) is a framework for benchmarking interactive coding agents on autonomous, day-to-day digital tasks, addressing the inadequacy of existing tool-use benchmarks that require only 1–4 simple API calls. It comprises (1) **AppWorld Engine**, a ~60K-line execution environment simulating 9 real-world apps (e.g., Amazon, Gmail, Venmo, Spotify, SimpleNote, Todoist) via 457 APIs and 101 database tables (~370K rows), populated with data simulating the lives of ~100 fictitious users, and (2) **AppWorld Benchmark**, a suite of 750 tasks (250 scenarios × 3 variations) split into Train (105), Dev (60), Test-N (168, "normal"), and Test-C (417, "challenge," requiring unseen apps Amazon and Gmail). Tasks require multiple apps (avg 1.8, max 6), many APIs (avg 9.5, max 26), and rich code (avg ~50 lines, max 134), evaluated programmatically via state-based unit tests (avg 8, max ~24 per task) that check database changes while detecting "collateral damage," using Task Goal Completion (TGC) and Scenario Goal Completion (SGC) metrics. Results show the benchmark is highly difficult: the best model, GPT4O with ReAct, achieves only 48.8 TGC on Test-N and 30.2 on Test-C (SGC 32.1/13.0), while GPT4Trb scores 32.7/17.5 and the best open model (FullCodeRefl + LLaMA3) 24.4/7.0; CodeAct and ToolLLaMA failed on all tasks. Oracle-API experiments (up to +9.8 TGC) show API retrieval is not the main bottleneck; rather, challenges lie in interactive code generation, adapting to errors, and instruction following. The ~100K-line system was hand-built over 14 months (benchmark: 5 months, 40K lines) rather than crowdsourced.
```
