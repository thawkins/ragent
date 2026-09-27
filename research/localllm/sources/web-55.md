# Web source

- URL: https://www.linkedin.com/posts/vineetvashishta_dellpromax-activity-7404523787960008704-9BIG
- Title: I got several DMs about running LLMs locally, and the most common question was about the hardware requirements. | Vin…
- Author(s): Vin Vashishta
- Language: English
- Published (UTC): 2025-12-10T14:14:00.426+00:00
- Captured (UTC): 2026-09-18T05:41:56.446914772+00:00
- Relevance: Medium — multiple title terms match query


```text
In a LinkedIn post framed by the hashtag #DellProMax, data-and-AI consultant Vin Vashishta (self-described as "Monetizing Data & AI For The Global 2K Since 2012," 3X founder and best-selling author) answers the most common question he receives about running LLMs locally by detailing his own hardware and explaining where the inference bottleneck lies. His rig is a Dell Pro Max T2 tower with an NVIDIA RTX Pro 6000 GPU (96GB VRAM), 128GB of system RAM, an Intel Ultra 9 285K CPU, and two 3.5TB SanDisk NVMe SSDs, and he warns that poor component choices can reduce even small models to ~2 tokens per second—unusable for coding or agentic workloads—which matters for users motivated by data privacy or avoiding API costs. Ranking components as a "supply chain" analogy, he identifies VRAM as the single most critical constraint (it must hold full model weights plus the KV cache; a 24GB model on a 16GB GPU forces performance-killing offload to RAM, and capacity should be prioritized over bandwidth), followed by GPU core count (which drives generation speed) and system RAM (128GB suffices for most generative workloads), while the CPU is a mere "traffic controller" where mid-range is adequate and NVMe SSD speed only affects initial model loading—concluding that if the model cannot fit entirely on the GPU, no amount of CPU power or SSD speed will compensate. (The page also surfaces unrelated recommended posts—on TPU vs. GPU architecture, NVIDIA's unreleased TITAN Ada, the NVIDIA Rubin platform, and GPU-vs-TPU infrastructure choices—which are excluded as asides.)
```
