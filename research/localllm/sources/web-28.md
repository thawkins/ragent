# Web source

- URL: https://dev.to/agustinsacco/frontier-logic-at-local-speed-the-2026-strix-halo-ultimate-benchmark-suite-2cdf
- Title: Frontier Logic at Local Speed: The 2026 Strix Halo Ultimate Benchmark Suite
- Author(s): @
- Language: English
- Published (UTC): 2026-05-31T23:22:02+00:00
- Captured (UTC): 2026-09-18T05:37:56.168463856+00:00
- Relevance: High — title + snippet match query


```text
This dev.to article benchmarks the AMD Strix Halo APU (Ryzen AI Max+ 395 / Radeon 8060S, 128GB LPDDR5X-8000 unified memory, ROCm 7.2.2 with RADV/Mesa) for local LLM inference using a tuned llama.cpp stack, finding the Vulkan backend outperformed ROCm on this unified-memory chip. Key optimizations included native Multi-Token Prediction (MTP) via Unsloth GGUFs, custom register-tile kernels for the 40-CU iGPU, and UMA buffer mapping enabling 128k context, yielding these generation speeds: Qwen 3.6 35B MoE (Q8_K_XL) jumped from 45.5 to 51.0 t/s (+12.1%), Qwen 3.6 27B Dense (Q4_K_XL) from 11.8 to 20.0 t/s (+69.5%), and Qwen 3.5 122B MoE (Q4_K_M) from 23.2 to 24.4 t/s (+5.2%). MTP imposed a ~20% prompt-prefill penalty (~100 t/s baseline vs. ~80 t/s), deemed a negligible trade-off. The author (Tars, sidekick to Agustin Sacco) concludes the 35B MoE—activating only 3B parameters per token—is the optimal local-agent model, delivering "GPT-4o class reasoning" at 51 t/s on edge hardware and outperforming the 27B dense model by ~150%.
```
