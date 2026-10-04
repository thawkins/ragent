# Web source

- URL: https://www.alphaxiv.org/abs/2604.25850
- Title: Agentic Harness Engineering: Observability-Driven Automatic Evolution of Coding-Agent Harnesses
- Author(s): https://www.alphaxiv.org/@jiahang-lin
- Language: English
- Published (UTC): 2026-05-07T11:25:51.725+00:00
- Captured (UTC): 2026-10-02T21:31:43.074317476+00:00
- Relevance: Medium - multiple title terms match query


```text
Agentic Harness Engineering (AHE) is a closed-loop framework for autonomously evolving coding-agent harnesses via three observability pillars: component observability (file-level, revertible editable components), experience observability (distilling millions of raw trajectory tokens into a layered drill-down evidence corpus), and decision observability (pairing each edit with a self-declared prediction verified against next-round task outcomes). In ten iterations on Terminal-Bench 2, AHE raised pass@1 from 69.7% to 77.0%, surpassing human-designed Codex-CLI (71.9%) and self-evolving baselines ACE and TF-GRPO. The frozen harness transferred without re-evolution: on SWE-bench-verified it reached 75.6% success with 461k vs. 526k tokens (12% fewer than the seed), outperforming ACE (74.6%) and TF-GRPO (74.2%), and on Terminal-Bench 2 it produced +5.1 to +10.1pp cross-family gains across three alternate model families. Ablations localized gains to tools, middleware, and long-term memory rather than the system prompt, suggesting factual harness structure transfers while prose-level strategy does not.
```
