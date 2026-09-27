# Web source

- URL: https://github.com/lhl/strix-halo-testing/tree/main
- Title: GitHub - lhl/strix-halo-testing
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:34:27.001671041+00:00
- Relevance: High — title matches query


```text
This GitHub repo (lhl/strix-halo-testing) documents testing and development for the AMD Strix Halo (Ryzen AI Max+ 395) APU with its gfx1151 RDNA 3.5 GPU, conducted on a pre-production Framework Desktop to assess Strix Halo's viability for local AI; it includes hardware-test (memory bandwidth), llm-bench (LLM performance sweeps across llama.cpp backends), rpc-test (llama.cpp RPC clustering), torch-therock (a script to build PyTorch + AOTriton, since as of 2025-10-15 no AOTriton/Flash Attention is built automatically), and a pioneering vLLM build, with current documentation hosted on the Strix Halo HomeLab Wiki. The repo also compares Strix Halo (characterized as "a Radeon RX 7600 XT with 128GB of LPDDR5X") against the Nvidia DGX Spark ("a very low power RTX 5070 with 128GB of LPDDR5X") using gpt-oss-120b (Q8/MXFP4) on llama.cpp build 6792 with ROCm nightly 7.10.0a20251017 and Vulkan drivers (RADV 25.2.4-2, AMDVLK 2025.Q2.1-1): token generation/decode is essentially even at short context via Vulkan (Spark +5.6% at 2K), though Spark leads with ROCm (+13.6% at 2K, +117.5% at 32K, where Vulkan tg is 1.8× ROCm), while for prefill Strix Halo lags substantially—ROCm +67.8% slower at 2K rising to +445.6% at 32K, and Vulkan +131.7% to +790.9% slower.
```
