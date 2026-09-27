# Web source

- URL: https://doi.org/10.1145/3725356
- Title: SpareLLM: Automatically Selecting Task-Specific Minimum-Cost Large Language Models under Equivalence Constraint
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T01:38:27.944893800+00:00
- Relevance: Scholarly — engine-ranked abstract
- Open-access recovery: full text fetched from unpaywall (https://dl.acm.org/doi/pdf/10.1145/3725356); version=hybrid, license=cc-by

```text
SpareLLM (Selecting Passable And Resource-Efficient LLMs), by Saehan Jo and Immanuel Trummer of Cornell University (Proc. ACM Manag. Data, SIGMOD 2025), is a framework that minimizes LLM inference costs for large-scale NLP tasks while guaranteeing outputs equivalent to a reference LLM (typically the most powerful model) with user-defined probability and confidence thresholds. It works in two phases: a profiling phase that compares cheaper LLMs' outputs to the reference model using Bernoulli trials and Clopper-Pearson binomial confidence intervals (with early termination when further profiling is estimated to be wasteful), and an application phase that allocates remaining items across multiple LLMs via a mixed integer linear program. Evaluated on five datasets (MMLU, IMDB, SMS-Spam, AgNews, HellaSwag) using OpenAI models (GPT-4-Turbo, GPT-3.5 variants, davinci-002, babbage-002), SpareLLM achieved cost savings up to 8.6× versus GPT-4-Turbo at a ≥90% equivalence constraint, with per-dataset savings of 1.2×, 8.6×, 4.5×, 7.2×, and 1.4× respectively; it dominated 91.1% of Pareto curve points against baselines LLMCascade and FrugalGPT for OpenAI models and 83.8% for Llama models (3B, 8B, 70B, 405B). The framework comes in three incremental variants—ProfileAll, ProfileSmart, and ModelMix—and its equivalence guarantees also yield a provable accuracy bound within 100·δ% of the reference model's accuracy.
```
