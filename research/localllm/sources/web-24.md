# Web source

- URL: https://community.frame.work/t/amd-strix-halo-llama-cpp-installation-guide-for-fedora-42/75856
- Title: AMD Strix Halo Llama.cpp Installation Guide for Fedora 42
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:37:25.599408903+00:00
- Relevance: Medium — multiple title terms match query


```text
This Framework community guide details how to run llama.cpp LLM inference on AMD Ryzen AI Max "Strix Halo" integrated GPUs under Fedora 42 (128 GB RAM recommended). Key setup steps include passing kernel parameters via grubby (`amd_iommu=off amdgpu.gttsize=131072 ttm.pages_limit=33554432` to enable ~128 GiB unified GPU memory, though commenters note the `GGML_CUDA_ENABLE_UNIFIED_MEMORY=ON` environment variable can replace the GTT changes), setting BIOS GPU memory to 512 MB, and running everything inside prebuilt Toolbx containers (docker.io/kyuz0/amd-strix-halo-toolboxes) offering three backends: Vulkan RADV (most stable, fastest token generation), Vulkan AMDVLK (fastest prompt processing but a 2 GiB single-buffer limit), and ROCm 6.4.4 + ROCWMMA (best for BF16 models, prone to crashes mitigable via `amdgpu.cwsr_enable=0`). Models (GGUF, e.g., Unsloth's Qwen3-Coder-30B-A3B BF16) are run with `--no-mmap -ngl 999 -fa on`, and memory can be checked with gguf-vram-estimator.py—e.g., Qwen3-235B Q3_K_M (~104.72 GiB) fits 128 GB systems up to ~131k context (130.22 GiB) but not its full 262k context (153.72 GiB). One user reported ~384 tokens/s prompt processing and ~44.5 tokens/s generation on a 120B model via Vulkan RADV, and a commenter recommended building llama.cpp with `AMDGPU_TARGETS="gfx1151"`.
```
