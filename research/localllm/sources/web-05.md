# Web source

- URL: https://learn.arm.com/learning-paths/laptops-and-desktops/dgx_spark_llamacpp/1_gb10_introduction
- Title: Unlock quantized LLM performance on Arm-based NVIDIA DGX Spark: Explore Grace Blackwell architecture for efficient…
- Author(s): Odin Shen, @ArmSoftwareDev
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:33:43.774611944+00:00
- Relevance: High — title matches query


```text
This Arm Learning Path introduces the NVIDIA DGX Spark, a compact Arm-based "personal AI supercomputer" built on the GB10 Grace Blackwell Superchip, which combines a Grace CPU (10 Cortex-X925 and 10 Cortex-A725 cores on Armv9) with a Blackwell GPU featuring 5th-generation Tensor Cores optimized for FP8/FP4 workloads, delivering up to 1 petaFLOP (1,000 TFLOPs) of FP4 AI performance. Its 128 GB unified memory, connected via NVLink-C2C with 900 GB/s bidirectional bandwidth, lets CPU and GPU share one address space, eliminating data-transfer bottlenecks that affect x86 platforms. The page highlights benefits for quantized LLM inference (Q4, Q5, Q8): the Grace CPU handles tokenization, orchestration, and memory paging with high IPC and energy efficiency, while the Blackwell GPU (CUDA 13) accelerates quantized transformer layers, enabling models like Qwen2-7B or LLaMA3-8B (Q4_K_M) to run in shared memory with near-real-time inference, positioning DGX Spark as a desktop platform for developing, profiling, and scaling AI models locally.
```
