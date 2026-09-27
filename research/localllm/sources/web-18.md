# Web source

- URL: https://forum.level1techs.com/t/strix-halo-ryzen-ai-max-395-llm-benchmark-results/233796
- Title: Strix Halo (Ryzen AI Max+ 395) LLM Benchmark Results
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:36:26.341632542+00:00
- Relevance: Medium — multiple title terms match query


```text
In a July 2025 Level1Techs thread, user lhl published rigorous llama.cpp LLM benchmarks run on pre-production Framework Desktop systems with an AMD Ryzen AI Max+ 395 (Strix Halo, gfx1151/RDNA 3.5) and 128GB LPDDR5x-8000, using Linux kernel 6.15.x, TheRock/ROCm 7.0 nightlies, and llama.cpp build b5863; the hardware achieves ~215 GB/s of a theoretical 256 GB/s memory bandwidth (256-bit, 8000 MT/s) and a theoretical 59 FP16 TFLOPS. Across the tested models, prompt processing (pp512) reached ~998 tok/s for Llama 2 7B Q4_0 (Vulkan) but fell to 63 tok/s for the 142B dots1 MoE, while text generation (tg128) hit ~72 tok/s for Qwen 3 30B-A3B, ~20.6 for dots1, 17–19 for the 109B Llama 4 Scout and 80B Hunyuan-A13B MoEs, and only ~4.5–5 tok/s for 70B dense models—results showing large-MoE models suit this high-capacity but bandwidth-limited APU, with the optimal backend (Vulkan vs HIP/rocWMMA) varying per model and between prompt processing and token generation. An August update reported nearly a 50% pp improvement for llama2-7b-q4_0 in Vulkan (884 → 1294 tok/s over three months of driver/software progress, plus 5–10% from a tuned profile), alongside RPC clustering tests and PyTorch Flash Attention/vLLM results, while lhl advised that used EPYC servers with an inexpensive GPU (e.g., via k-transformers CPU/GPU interleaving) currently offer far better value than dedicated Strix Halo hardware for running very large MoE models locally.
```
