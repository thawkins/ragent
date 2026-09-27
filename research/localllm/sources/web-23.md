# Web source

- URL: https://github.com/hogeheer499-commits/strix-halo-guide
- Title: GitHub - hogeheer499-commits/strix-halo-guide: Evidence-backed AMD Strix Halo local-AI setup and benchmarks: Qwen3.8,…
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:37:34.767858675+00:00
- Relevance: High — title + snippet match query


```text
This GitHub repository is an independent, evidence-focused guide to running large language models locally on AMD Strix Halo / Ryzen AI MAX+ 395 systems (Radeon 8060S iGPU, gfx1151, 96–128GB unified memory), covering BIOS configuration (512MB UMA frame buffer), Ubuntu 24.04 setup, and Ollama, llama.cpp Vulkan/RADV, ROCm/HIP, MTP speculative decoding, and vLLM backends, measured primarily on a Beelink GTR9 Pro with corroborating data from 13 systems and 10 community contributors. Key measured results include: Qwen3-Coder 30B-A3B Q4_K_S at 100.99 t/s direct (b9851), Qwen3-30B-A3B-Instruct-2507 IQ4_XS at 100.04 tg128, Qwen3.6 35B-A3B at 60.57–62.56 t/s as the recommended Ollama 0.31.2 default, an experimental CHADROCK ROCmFP4 MTP server path at 141.37 t/s mean, gpt-oss-120b at 55.57 t/s, and a 284B DeepSeek V4 Flash GGUF capacity proof at 13.27 t/s (90.86GB artifact). As of August 30, 2026, every headline claim is linked to raw logs, CSVs, or explicit caveats (machine-readable via data/headline_claims.csv), with failures and negative results preserved; the maintainer cites 15+ merged upstream contributions (llama.cpp, AMD Lemonade, OpenAI .NET SDK) and states no affiliate links are present, and the guide ships only docs, scripts, and data—not binaries or model weights.
```
