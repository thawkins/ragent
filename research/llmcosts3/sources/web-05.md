# Web source

- URL: https://doi.org/10.1609/aaai.v40i43.40970
- Title: Breaking Model Lock-in: Cost-Efficient Zero-Shot LLM Routing via a Universal Latent Space
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T01:38:16.267730487+00:00
- Relevance: Scholarly — engine-ranked abstract
- Open-access recovery: full text fetched from unpaywall (https://ojs.aaai.org/index.php/AAAI/article/download/40970/44931); version=gold, license=unspecified

```text
The AAAI-26 paper "Breaking Model Lock-in: Cost-Efficient Zero-Shot LLM Routing via a Universal Latent Space" (Yan et al.; USTC, Hefei Comprehensive National Science Center, and iFLYTEK) introduces ZeroRouter, a zero-shot LLM routing framework that overcomes "model lock-in"—the need for costly full-scale retraining when adding new models to routing systems like HybridLLM, RouteLLM, MixLLM, GraphRouter, and FORC. ZeroRouter decouples query characterization from model profiling via a universal latent space built on a multidimensional 2PL Item Response Theory model (latent dimension D=20, trained on Open LLM Leaderboard data from 200 models), selects an informative anchor set using D-optimality (Fisher information) for lightweight new-model profiling (~200 queries), predicts latent coordinates from DistilBERT semantic embeddings plus 11 structural linguistic features, and formulates routing as an Integer Linear Program balancing accuracy, cost, and latency with user-specified weights. Evaluated on 9 datasets (6 in-distribution: IFEval, BBH, MATH, GPQA, MuSR, MMLU-PRO; 3 OOD: ARC-C, TruthfulQA, HumanEval) over 60 LLMs (10 core models spanning 1B–235B parameters, plus 50 models released after the router's training cutoff), it consistently beat baselines on all objectives—e.g., achieving higher Max-Acc (0.45 small-model ID; 0.68 vs. 0.62 OOD for large models) alongside lower cost (−0.17) and latency (−0.25); a D-optimality ablation lifted Max-Acc from 0.27 (random sampling) to 0.39. Training used a single NVIDIA A800 (80GB), and code is released at github.com/Codeffun3/ZeroRouter.
```
