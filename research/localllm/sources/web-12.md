# Web source

- URL: https://machinelearning.apple.com/research/exploring-llms-mlx-m5
- Title: Exploring LLMs with MLX and the Neural Accelerators in the M5 GPU
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:35:04.070245043+00:00
- Relevance: High — title + snippet match query


```text
Apple's MLX—an open-source, NumPy-like array framework for Apple silicon with the MLX LM package for running, fine-tuning, and quantizing Hugging Face LLMs—now leverages the M5 chip's GPU Neural Accelerators (dedicated matrix-multiplication units accessed via Metal 4's TensorOps and Metal Performance Primitives) when running macOS 26.2 or later. Benchmarks comparing a 24GB M5 MacBook Pro to a comparable M4 across Qwen3 1.7B/8B in BF16, 4-bit Qwen3 8B/14B, the MoE Qwen3-30B-A3B (4-bit), and GPT OSS 20B (MXFP4), using 4096-token prompts and 128 generated tokens, show compute-bound time-to-first-token speedups of 3.3–4.1x (dropping under 10 seconds for a dense 14B model and under 3 seconds for the 30B MoE) and memory-bandwidth-bound generation speed gains of 19–27%, consistent with M5's 153GB/s bandwidth versus M4's 120GB/s (28% higher). All tested models fit in under 18GB of unified memory, and generating a 1024x1024 image with FLUX-dev-4bit (12B parameters) is more than 3.8x faster on M5 than on M4.
```
