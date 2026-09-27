# Web source

- URL: https://llmcheck.net/benchmarks
- Title: Apple Silicon LLM Benchmarks — 248 tok/s Figures, M1 to M6
- Author(s): LLM Check
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:35:33.183150981+00:00
- Relevance: Medium — multiple title terms match query


```text
LLMCheck's benchmarks page lists tokens-per-second (tok/s) generation speeds and time-to-first-token (TTFT) for dozens of local LLMs—including Phi-4, Qwen, Gemma, Llama, Mistral, DeepSeek, GLM, and others—tested across Apple Silicon chips ranging from M1 (8–16 GB) to M5 Ultra (512 GB), using the Ollama, LM Studio, and MLX engines, mostly at Q4_K_M quantization. Measured rows follow a reference protocol (256-token prompt, 512 output tokens, default context, 3-run average), while many rows are flagged as estimates computed from model memory footprint and chip bandwidth, and some are community- or vendor-sourced; the full dataset is downloadable in CSV/JSON under CC BY 4.0. As of August 2026, the fastest listed entry is Maple Preview 20B-A1B at 281 tok/s on an M5 Pro (vendor-reported, ternary quantization), followed by LFM2.5-2.6B at 220 tok/s on M5 Max, with Gemma 4 E2B the fastest pure estimate at ~261 tok/s; among large models, DeepSeek V4 Flash (284B-A13B MoE) reportedly reaches ~39 tok/s at 2-bit on a 128 GB M5 Max. The page reports a near-linear bandwidth-to-speed relationship (M5 Max at ~600 GB/s generates roughly 3× faster than a base M3 at ~200 GB/s), engine performance gaps of typically 5–15% (MLX often fastest, as Apple's native Metal framework), and chip guidance: M1 16 GB suits 3–9B models, M4 Pro 24 GB is the "sweet spot" for 14–35B, and M5 Max 128 GB is ideal for 70B+. Its 0–100 LLMCheck Score weights model capability (50), Mac-specific speed (25), accessibility/minimum RAM (15), and license openness (10).
```
