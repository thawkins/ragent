# Web source

- URL: https://community.frame.work/t/amd-strix-halo-ryzen-ai-max-395-gpu-llm-performance-tests/72521
- Title: AMD Strix Halo (Ryzen AI Max+ 395) GPU LLM Performance Tests
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:36:16.247346112+00:00
- Relevance: Medium — multiple title terms match query


```text
This Framework community thread presents detailed LLM inference benchmarks on pre-production Framework Desktop systems with AMD Ryzen AI Max+ 395 (Strix Halo, gfx1151 GPU) and 128GB LPDDR5x-8000, tested by user lhl using llama.cpp with latest kernels (Linux 6.15.5+), TheRock/ROCm 7.0 nightly builds, and both HIP (rocBLAS/hipBLASLt) and Vulkan backends. Hardware limits measured ~215 GB/s GPU memory bandwidth (vs 256 GB/s theoretical) and ~59 theoretical FP16 TFLOPS on RDNA 3.5; topline results showed prompt processing up to ~998 t/s (Llama 2 7B Q4_0, Vulkan) and token generation of 72 t/s on Qwen 3 30B-A3B MoE, ~17–20 t/s on larger MoE models (Llama 4 Scout 109B, Hunyuan-A13B 80B, dots1 142B), and ~5 t/s on a 70B dense model, with Vulkan often winning token generation while HIP led some prefill cases. A key finding (ROCm issue #4748) was that gfx1151 kernels underperform: gfx1100 rocBLAS code paths ran 2.5–6x faster than gfx1151 rocBLAS and 1.5–3x faster than gfx1151 hipBLASLt. Later posts reported that official ROCm 7.0.1 badly regressed versus TheRock or ROCm 6.4.4 builds (e.g., Qwen3 8B BF16 pp512: ~326 t/s vs ~1132 t/s), recommended TheRock/6.4.4 for llama.cpp, and noted AMD is discontinuing its proprietary Vulkan driver in favor of Mesa RADV.
```
