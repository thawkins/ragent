# Web source

- URL: https://stal.blogspot.com/2025/11/nvidia-dgx-spark-supercomputer-for-your.html
- Title: NVIDIA DGX Spark: A Supercomputer for Your Desk?
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:33:50.862286324+00:00
- Relevance: High — title + snippet match query


```text
This blog post is a hands-on review of the NVIDIA DGX Spark, a compact desktop AI workstation purchased for €4,300 for local LLM inference and fine-tuning. The system runs NVIDIA's custom Blackwell-based GB10 chip (10 ARM Cortex-X925 + 10 Cortex-A725 cores, up to 1 petaFLOPS sparse FP4) with 128 GB of LPDDR5X unified CPU/GPU memory (~120 GB usable), whose 273 GB/s shared bandwidth is the key performance bottleneck. Benchmarks using SGLang and Ollama showed strong small/medium-model performance—Llama 3.1 8B hit ~8,000 tokens/s prefill and ~20 tokens/s decode (scaling to 368 tokens/s at batch 32), and GPT-OSS 20B managed ~2,000/50 tokens/s—while large models exposed bandwidth limits, with Llama 3.1 70B at ~800 tokens/s prefill and ~2.7 tokens/s decode, roughly 8× slower than an RTX Pro 6000; Eagle-3 speculative decoding nearly doubled decode throughput. Hardware highlights include silent operation, thermal stability, four USB-C ports (240 W power delivery), 10 GbE, and dual 200 Gbps InfiniBand ports enabling two-unit clusters that NVIDIA claims can serve models up to 405B parameters at FP4 (untested by the author), and it ships with a CUDA-enabled custom Ubuntu "DGX OS" supporting SGLang, Ollama, and Open WebUI out of the box. The author concludes the DGX Spark excels for researchers and developers wanting quiet, private, local AI development with small-to-medium models, but its memory bandwidth caps large-model inference and its ARM architecture limits some software (e.g., gaming) compatibility.
```
