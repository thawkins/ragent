# Web source

- URL: https://community.frame.work/t/anybody-tried-running-image-generation-e-g-stable-diffusion-xl-3-5-or-similar-on-linux/76932
- Title: Anybody tried running image generation (e.g. Stable Diffusion XL, 3.5 or similar) on Linux?
- Author(s): —
- Language: English
- Published (UTC): 2025-10-17T20:05:10+00:00
- Captured (UTC): 2026-09-18T05:38:10.789724211+00:00
- Relevance: Medium-high — snippet matches query


```text
In this Framework community thread (October–November 2025), users report successfully running image generation on the Framework Desktop's AMD Strix Halo GPU (gfx1151) under Linux, primarily via ComfyUI with ROCm. One user (_alX, on Arch) shared working setup instructions: create a Python venv, install nightly ROCm and torch builds from TheRock repo (e.g., `pip install --index-url https://rocm.nightlies.amd.com/v2/gfx1151/`), clone ComfyUI, and run `python main.py`—SDXL and Flux workloads ran without crashing. The original poster (Ubuntu 25.10) found SD XL Turbo "exceptionally fast," though large models like Qwen-Image crashed his system regardless of setting 64GB shared RAM. Another user runs ComfyUI in a podman container with ROCm nightlies, noting a kernel bug (fixed in 6.18) causing crashes under heavy GPU load, and uses kernel params `amd_iommu=off ttm.pages_limit=33554432` on the 128GB model to make nearly all RAM available as VRAM on demand. User edf documented that ROCm 6.x torch builds on gfx1151 throw "HIP error: Invalid device function" unless `HSA_OVERRIDE_GFX_VERSION=11.0.0` is set, while ROCm 7.x builds (TheRock v2/staging) work without the override—and actually segfault if it is set; Fedora 43/Rawhide system ROCm packages proved unstable ("crash city"), so he reverted to Fedora 42 with ROCm 6.3.1.
```
