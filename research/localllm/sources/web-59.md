# Web source

- URL: https://www.geeksforgeeks.org/deep-learning/recommended-hardware-for-running-llms-locally
- Title: Recommended Hardware for Running LLMs Locally - GeeksforGeeks
- Author(s): GeeksforGeeks
- Language: English
- Published (UTC): 2024-09-24T12:06:02+00:00
- Captured (UTC): 2026-09-18T05:41:50.961789788+00:00
- Relevance: Medium — multiple title terms match query


```text
This GeeksforGeeks guide outlines hardware recommendations for running LLMs locally, organized by three model-size tiers: basic (3B–7B parameters), intermediate (13B–30B), and advanced (34B–70B+). The GPU is identified as the most critical component, with VRAM determining which models can run without aggressive quantization—recommendations range from the RTX 3060 (12GB) or RTX 4060 Ti (16GB) for small models, to the RTX 3090 (24GB) for mid-size models, up to the RTX 4090, A6000, or A100 for large models and fine-tuning. Supporting recommendations scale accordingly: CPUs from Core i5/Ryzen 5 up to Threadripper/Xeon; RAM from 32GB to 128–256GB; storage from 512GB–1TB SSD to 2TB+ Gen4 NVMe; power supplies from 650W to 1200W+; and cooling from stock air coolers to 360mm liquid cooling for multi-GPU rigs. The article also recommends Linux (Ubuntu/Pop!_OS) with CUDA-enabled PyTorch as the base software environment, scaling up to DeepSpeed, Megatron-LM, TensorRT, or ROCm for large-scale fine-tuning, plus networking from Gigabit Ethernet to 10Gb Ethernet for multi-node clusters.
```
