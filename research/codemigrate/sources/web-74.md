# Web source

- URL: https://arxiv.org/html/2509.16187v2
- Title: MatchFixAgent: Language-Agnostic Autonomous Repository-Level Code Translation Validation and Repair
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:33:09.878318328+00:00
- Relevance: Medium - multiple title terms match query


```text
MatchFixAgent is an LLM-based, programming-language-agnostic multi-agent framework for validating and repairing repository-level code translations, combining a Semantic Analyzer (six sub-analyses: control flow, data flow, I/O, library API, exception/error handling, and specifications), a Test Generator & Repair Agent, and a Verdict Agent. Evaluated on 2,219 translation pairs covering 6 PL pairs from 24 GitHub projects with over 900K lines of code, it produced verdicts for 99.2% of pairs; on 1,571 pairs where both it and prior tools gave verdicts, it agreed 72.8% of the time, and on disagreements human review found MatchFixAgent correct in 60.7% of cases. It repaired 50.6% of inequivalent translations versus prior work’s 18.5%, required only 1,650 lines of code with about 280 additional lines per supported PL, and used Claude 3.7 Sonnet/Claude Code 1.0.51 with a 1,000-second timeout, averaging 309 seconds and $1.22 per verdict (total $2,710.45). An ablation found removing code analyses and in-the-loop test generation reduced verdict accuracy by 42.3% and increased token usage by 5.2%.
```
