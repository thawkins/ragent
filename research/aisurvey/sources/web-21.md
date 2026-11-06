# Web source

- URL: https://alphasignal.ai/news/google-s-sleep-framework-lets-ai-models-keep-learning-without-forgetting
- Title: Google&#x27;s Sleep Framework Lets AI Models Keep Learning Without Forgetting | AlphaSignal
- Author(s): AlphaSignal Newsroom
- Language: English
- Published (UTC): 2026-10-27T17:40:42+00:00
- Captured (UTC): 2026-11-03T21:06:59.034225647+00:00
- Relevance: Medium - multiple title terms match query


```text
Google Research proposed a continual-learning “Sleep” framework in which a deployed LLM alternates between a wake phase for processing inputs and a sleep phase for consolidating information into weights. It uses a Mixture-of-Experts architecture with pre-allocated low-rank experts, NREM-inspired parameter expansion, knowledge distillation, and synaptic pruning, plus REM-inspired synthetic “dream” training data filtered by a gradient-based scorer. Google reports lower forgetting and substantial gains over baselines on continual translation, long-context recall, factual integration, and mathematical reasoning, including a BABILong result involving up to 10 million tokens processed across wake/sleep rather than one inference window. The work remains research with no announced production offering, and deployment would require reserved capacity, offline compute, reliable evaluators, versioning/rollback, data controls, and fleet coordination.
```
