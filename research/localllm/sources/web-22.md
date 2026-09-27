# Web source

- URL: https://community.frame.work/t/quickstart-guide-ollama-with-gpu-support-no-rocm-needed/79186
- Title: Quickstart Guide: Ollama With GPU Support (No ROCM Needed)
- Author(s): —
- Language: English
- Published (UTC): 2025-12-26T04:20:44+00:00
- Captured (UTC): 2026-09-18T05:37:01.671458988+00:00
- Relevance: High — title + snippet match query


```text
This Framework community guide shows how to quickly run an Ollama LLM server with GPU acceleration on AMD Strix Halo systems (e.g., the Framework Desktop with Ryzen AI Max 395) using Vulkan instead of ROCm, which the author considers unstable and crash-prone on this hardware while Vulkan "just works" with equal or better performance. It recommends Ubuntu 25.10 (newer kernels for immature Strix Halo support), skipping full-disk encryption to avoid slower model loading, updating the BIOS via fwupdmgr (with a warning that UEFI v3.04 has a known long-boot-time bug traced to AMD PI and acknowledged by Framework), and configuring iGPU memory in BIOS (e.g., a custom 96 GB allocation). After installing curl, nvtop, and libfuse2 and installing Ollama via its curl script (tested by pulling and running IBM's granite4:tiny-h), users enable GPU, LAN, and flash-attention support via systemd override environment variables—OLLAMA_HOST=0.0.0.0:11434, OLLAMA_VULKAN=1, OLLAMA_FLASH_ATTENTION=1, and OLLAMA_CONTEXT_LENGTH=32768 (default is 4096 tokens; the author typically uses 50k)—verify GPU use with nvtop, and optionally install the AnythingLLM GUI. Hardening suggestions include a static IP, changing the default port 11434, monthly updates, disabling USB boot, and treating untrusted models like untrusted software; commenters add alternatives such as GRUB-based memory allocation on Ubuntu 24.04 (ttm.pages_limit=27648000 for 108 GB), extending OLLAMA_KEEP_ALIVE beyond 5 minutes and setting OLLAMA_MAX_LOADED_MODELS (gpt-oss:120b is 66 GB; gpt-oss:20b takes 9 seconds to load), while criticizing running internet shell scripts as root and noting clamav cannot detect LLM data poisoning.
```
