# Web source

- URL: https://ai.rundatarun.io/practical-applications/dgx-lab-benchmarks-vs-reality-day-4
- Title: DGX Spark Benchmarks vs Reality: 82,739 tok/s on Paper
- Author(s): Justin Johnson
- Language: English
- Published (UTC): 2025-10-26T00:00:00+00:00
- Captured (UTC): 2026-09-18T05:34:00.697649186+00:00
- Relevance: Medium — multiple title terms match query


```text
In this Day 4 "DGX Lab Chronicles" post (published Oct 26, 2025, updated Oct 27–28), Justin Johnson reports on 6+ days of ML workloads on an NVIDIA DGX Spark (ARM64, GB10 Blackwell GPU with 128GB unified memory, CUDA 13.0, driver 580.95.05), finding NVIDIA's benchmark claims largely accurate—training throughput matched claims (e.g., 53,657–82,739 tok/sec), Q4_K_M 4-bit quantization showed no noticeable quality loss, and his eight LoRA fine-tuning runs of Gemma-3-4b-it on 10,000 medical Q&A pairs achieved 70–84% accuracy—but with significant caveats: FP16 GPU inference produced inf/nan errors and empty outputs (costing him 15 hours of misdirected debugging), and GPU memory fragmentation caused a hard system freeze 7.5 hours into training, requiring mitigations like cache clearing every 50 steps, checkpointing every 200 steps, and 2–3 hour maximum session limits. After Hacker News feedback, he corrected his overclaim that "GPU inference is fundamentally broken"—the issue appeared FP16-specific (BF16 inference untested), Ollama was confirmed GPU-accelerated (96% utilization, ~80 tok/sec on Phi-3.5-mini, consistent with NVIDIA's size-scaled numbers), and official llama.cpp benchmarks contradict his failed local attempts; a later update (Oct 28) identified CUDA version mismatches as the root cause of most issues, yielding 3.6x speedups. His verdict: the Blackwell + ARM64 + CUDA 13.0 stack is powerful but not plug-and-play—cautiously recommended for experts willing to implement workarounds, while others should wait 6–12 months for maturity or use TensorRT-LLM for production inference.
```
