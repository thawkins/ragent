# Web source

- URL: https://dev.to/bytecalculators/the-math-behind-local-llms-how-to-calculate-exact-vram-requirements-before-you-crash-your-gpu-12n5
- Title: The Math Behind Local LLMs: How to Calculate Exact VRAM Requirements Before You Crash Your GPU
- Author(s): @
- Language: English
- Published (UTC): 2026-05-02T20:23:42+00:00
- Captured (UTC): 2026-09-18T05:40:08.388500668+00:00
- Relevance: High — title + snippet match query


```text
This article explains how to calculate VRAM requirements for running LLMs locally. The baseline rule is that each parameter in an unquantized model is stored as a 16-bit float (FP16/BF16, 2 bytes), so VRAM in GB equals billions of parameters × 2—e.g., Meta's Llama-3-8B needs 16GB just for its weights. Quantization reduces per-parameter precision: 8-bit (INT8) uses 1 byte (8GB for an 8B model) and 4-bit (INT4/GGUF/AWQ) uses 0.5 bytes (4GB), enabling large models to fit on consumer GPUs like the 24GB RTX 3090/4090. The article warns that the KV cache—which stores prompt and generation context and is computed roughly as 2 × context length × layers × hidden size × 2 bytes—grows linearly with context length and multiplies per concurrent user (10 users with 4k-token prompts could add ~10GB), causing unexpected OOM crashes. The author also promotes a client-side "LLM VRAM Calculator" tool (taking model size, quantization level, and context length as inputs) and argues that doing this math upfront avoids overspending, e.g., renting an A100 (80GB) at ~$2/hour when an RTX 4090 at ~$0.30/hour would suffice.
```
