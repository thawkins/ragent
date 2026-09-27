# Web source

- URL: https://apertus.ai/en/blog/nvidia-dgx-spark-review-vs-ai-box
- Title: Local LLMs on the NVIDIA DGX Spark: Performance Test and Alternatives | Apertus - EU-Hosted Apps & AI
- Author(s): —
- Language: English
- Published (UTC): 2025-10-14T00:00:00+00:00
- Captured (UTC): 2026-09-18T05:33:34.826108646+00:00
- Relevance: High — title matches query


```text
This apertus.ai review (Oct 14, 2025) of the NVIDIA DGX Spark finds that the desktop "supercomputer"—built on a Blackwell GB10 chip delivering up to 1 Petaflop (Sparse FP4) with 128 GB of coherent LPDDR5X Unified Memory—is bottlenecked by its low 273 GB/s memory bandwidth. While it excels at prefill and models up to ~20B parameters and can hold large models like Llama 3.1 70B entirely in memory, its token generation (decode) performance collapses on 70B+ models: Llama 3.1 70B runs at ~2.7 tokens/second versus 240+ tps on a well-optimized RTX 6000 with a 120B model, and on GPT-OSS 120B it achieves only 11.66 tps (batch 1) compared to ~100 tps for a multi-GPU "AI-Box" with 3–4 RTX 3090s (72–96 GB VRAM) and ~60 tps for a Mac Studio M4 Max, whose Apple Silicon offers far higher bandwidth. The review concludes the Spark is best understood as a developer kit for replicating NVIDIA's data center stack (SGLang, DGX OS) locally before scaling to DGX servers—not as a price-to-performance choice for local LLM inference, where multi-GPU x86 setups, an RTX 5090 (32 GB GDDR7), or Apple M-chip machines deliver roughly tenfold better decode performance at similar cost.
```
