# Web source

- URL: https://www.mindstudio.ai/blog/running-local-ai-amd-rocm-ollama-lm-studio
- Title: Running Local AI on AMD: ROCm, Ollama, and LM Studio Performance in 2026
- Author(s): Luis Chavez-Mattos
- Language: English
- Published (UTC): 2026-05-28T00:00:00+00:00
- Captured (UTC): 2026-09-18T05:39:27.808852182+00:00
- Relevance: Medium — multiple title terms match query


```text
This MindStudio guide (2026) argues that AMD's ROCm 6.x has meaningfully closed the software gap with NVIDIA's CUDA for local AI inference, with PyTorch, Ollama, LM Studio, and ComfyUI now running properly on supported AMD hardware—RDNA3 (RX 7000), RDNA4 (RX 9000), Instinct MI200/MI300, and Radeon PRO W7800 (32GB)/W7900 (48GB) cards, with Linux strongly preferred over Windows (where LM Studio's Vulkan backend is the easiest path and Ollama requires WSL2). Key technical details: ROCm exposes itself as CUDA to PyTorch via the HIP compatibility layer, the HSA_OVERRIDE_GFX_VERSION=11.0.0 environment variable helps unrecognized RDNA3 cards, and LM Studio's ROCm/HIP backend offers 10–20% better throughput than Vulkan on Linux. Benchmarks on a 32GB W7800 running Ollama on Ubuntu 24.04 show practical large-model performance: Llama 3.3 70B Q4_K_M at ~18–22 tokens/sec generation (~2,800 tok/s prefill, entirely in VRAM), Qwen 2.5 32B Q6_K at ~28–34 tok/s, Mistral 7B Q8 at ~85–95 tok/s, FLUX.1-dev at ~12–15 sec/image, and SDXL at ~4–6 sec/image—roughly on par with an RTX 3090 for inference. The guide's main takeaways: VRAM capacity matters more than raw compute for LLM inference (70B models need ~32GB to avoid CPU offloading that drops speeds to 2–5 tok/s; 16GB handles 13B–14B models; an 8GB RX 7600 minimally runs 7B), AMD's price-to-VRAM ratio is its key advantage for inference, but NVIDIA retains meaningful advantages for training and fine-tuning through cuDNN and CUDA ecosystem maturity.
```
