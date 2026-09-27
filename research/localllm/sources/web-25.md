# Web source

- URL: https://github.com/Gygeek/Framework-strix-halo-llm-setup
- Title: GitHub - Gygeek/Framework-strix-halo-llm-setup: Complete guide to running large language models locally on AMD Ryzen…
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:37:17.133622255+00:00
- Relevance: High — title + snippet match query


```text
This GitHub repository provides a complete guide to running large language models locally on AMD's Ryzen AI Max+ 395 ("Strix Halo") APU, which pairs 128GB of unified memory with an integrated Radeon 8060S GPU (gfx1151 architecture, RDNA 3.5, 40 compute units, rocWMMA support). Unlike discrete GPUs, the APU shares system RAM with the GPU via GTT (Graphics Translation Table), yielding roughly 115–120GB usable for LLM inference—enough to run 70B-parameter models at Q4 quantization at ~15–20 tokens/second, while Llama-2-7B achieves ~50–52 t/s, comparable to Apple M4 Max. Tested on the Framework Desktop (and applicable to systems like the GMKTEC EVO-X2) running Ubuntu 24.04+/25.10, the setup requires kernel 6.16.9 or later, BIOS changes (512MB UMA framebuffer, IOMMU disabled for ~6% memory read improvement, optional 85W TDP), GRUB parameters `amd_iommu=off amdgpu.gttsize=117760`, udev rules for GPU access, and ROCm installed either from AMD's repository or via a Podman/Distrobox container (kyuz0/amd-strix-halo-toolboxes:rocm-6.4.4-rocwmma). Finally, llama.cpp is built with HIP support (`-DGGML_HIP=ON -DAMDGPU_TARGETS="gfx1151"`) and run with critical flags `--no-mmap` and `-ngl 99` for full GPU offloading (llama-bench uses `-mmp 0` instead).
```
