# Web source

- URL: https://blog.modelcode.ai/p/can-ai-agents-actually-rewrite-your
- Title: Can AI Agents Actually Rewrite Your Codebase? We Built a Benchmark to Find Out.
- Author(s): modelcode
- Language: English
- Published (UTC): 2026-08-19T17:28:27+00:00
- Captured (UTC): 2026-10-02T21:34:52.703859684+00:00
- Relevance: High - title matches query


```text
The post highlights large-scale AI-agent coding successes such as Bun’s Rust rewrite and Cursor’s SQLite Rust implementation, arguing that detailed specs—documentation or test suites—are key to long-horizon rewrites. It introduces RepoMod-Bench, published at KDD 2026, a dataset/framework for evaluating repository-level migrations by converting a source repo’s existing tests into an implementation-agnostic, hidden test suite that interacts only through high-level interfaces such as REST APIs or CLIs; a four-stage pipeline selects repos, creates pytest suites, configures Docker/benchmark files, and validates suites to 100% on the original before shipping. The benchmark includes 21 real-world repos across 8 programming languages, over 11,600 unique tests, over 1.6M LOC, sizes from 14 LOC to 211K LOC, and tiers Small (<10K LOC), Medium (10K–50K), Large (>50K). Evaluating four agent–model combinations—Claude Code/Opus 4.5 (100% build, 48.2% pass, 38.5 min), OpenCode/GPT-5.2 (100%, 43.0%, 16.4 min), OpenCode/Opus 4.5 (100%, 42.0%, 21.6 min), and Codex CLI/GPT-5.2 (95.2%, 30.4%, 20.3 min)—it found pass rates collapsed from 91.3% on small projects to 15.3% on large projects, with the 211K-LOC project maxing at 19.5%; build success exceeded 95%, harness mattered as much as model (Claude Code beat OpenCode by 6.2 pp with same Opus 4.5), and single misunderstandings cascaded through dependent tests, with the bottleneck being architectural coherence across thousands of interdependent files rather than context management. The framework can act as a feedback signal for agent loops targeting at least 95% functional equivalence; paper: “RepoMod-Bench: A Benchmark for Code Repository Modernization via Implementation-Agnostic Testing”; code: github.com/Modelcode-ai/mcode-benchmark.
```
