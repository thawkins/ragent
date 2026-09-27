# Web source

- URL: https://www.linkedin.com/posts/tarun-bagga-866a4012_dgx-spark-activity-7456916252540903425-gFBK
- Title: Running 70B Model Locally with NVIDIA DGX Spark | Tarun Bagga posted on the topic | LinkedIn
- Author(s): Tarun Bagga
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:34:11.957665036+00:00
- Relevance: High — title + snippet match query


```text
In this LinkedIn post, Tarun Bagga reports on running large language models locally on the NVIDIA DGX Spark, a desktop system with 128GB unified memory and 1 petaFLOP of AI compute that draws under 100W at full load (versus 575W for an RTX 5090, which he notes cannot load a 70B model). He identifies a practical model sweet spot of 14B–70B parameters: smaller models like DeepSeek-R1 14B and GPT-OSS 20B reach 80+ tokens/second, while mid-range models like Qwen3-80B and Llama 3.3 70B run a full RAG stack at ~45 tokens/second, with 120B+ models feasible only for prototyping. Key optimizations he cites include NVFP4 quantization (2.6x throughput gains), EAGLE3 speculative decoding, batching agent calls, and software updates (a CES 2026 update alone delivered 2.5x performance improvements), and he highlights running multi-agent orchestration entirely locally as a shift toward "local AI sovereignty," recommending build.nvidia.com/spark, LM Studio, and NVIDIA's Developer Forums as starting points.
```
