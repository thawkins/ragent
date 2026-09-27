# Web source

- URL: https://www.promptquorum.com/local-llms/how-much-vram-local-llm
- Title: Local LLM VRAM: 7B=4GB, 13B=8GB, 70B=42GB (2026)
- Author(s): Hans Kuepper
- Language: English
- Published (UTC): 2026-04-05T00:00:00+00:00
- Captured (UTC): 2026-09-18T05:40:29.173998216+00:00
- Relevance: Medium — multiple title terms match query


```text
This PromptQuorum guide (by Hans Kuepper, updated August 28, 2026) provides a VRAM rule of thumb for local LLMs: multiply model size in billions by ~0.6 GB at Q4 (4-bit) quantization — yielding ~4 GB for 7B, ~8 GB for 13B, ~13 GB for 22B, and ~42 GB for 70B weights — then add 1–2 GB headroom for KV cache, context, and OS overhead. Costs scale with bytes per parameter: Q5 runs ~15–20% higher (≈0.7), Q8 ~75% higher (≈1.05), and FP32 needs 4× Q4, with figures verified against real GGUF file sizes (e.g., Llama-3.3-70B Q4_K_M = 42.52 GB); Q4 is recommended as the sweet spot at ~87.5% savings versus FP32 with only ~1% accuracy loss. Practical GPU buy-targets are 6–8 GB for 7B, 12 GB for 13B, 16 GB for 22B, and 48 GB for 70B (e.g., 2× RTX 4090 or one H100 80 GB), while CPU-only setups need roughly double in system RAM. MoE models generally require VRAM for all resident weights, not just active parameters — Llama 4 Scout (109B total, 17B active) still needs ~55 GB at Q4 — and CPU offloading via llama.cpp/Ollama is possible but costs 30–50% performance. Batch size affects throughput, not single-inference VRAM, so multi-user servers should allocate batch × model VRAM, and fine-tuning a 7B model requires 12–16 GB with LoRA or 28 GB+ for full training.
```
