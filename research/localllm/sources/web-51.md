# Web source

- URL: https://dev.to/pat9000/gguf-quantization-explained-q4km-vs-q5km-vs-q8-which-to-pick-2026-31pl
- Title: GGUF Quantization Explained: Q4_K_M vs Q5_K_M vs Q8 — Which to Pick (2026)
- Author(s): @
- Language: English
- Published (UTC): 2026-05-13T14:00:08+00:00
- Captured (UTC): 2026-09-18T05:40:15.667144659+00:00
- Relevance: Medium — partial query match


```text
This guide explains GGUF quantization for running local LLMs via llama.cpp, Ollama, or LM Studio: compressing 16-bit FP16 weights to 4–8 bits shrinks a 7B model from ~14 GB to as little as ~4.4 GB, with the article ranking levels by size and measured accuracy—Q8_0 (~7.7 GB, ~99.5% quality), Q6_K (~5.9 GB, ~99%), Q5_K_M (~5.1 GB, ~98%), Q4_K_M (~4.4 GB, ~96.5%), Q4_K_S, Q3_K_M (~92%), and Q2_K (~85%, "experimental only")—and recommending Q4_K_M for 8 GB VRAM, Q5_K_M for 12 GB, Q6_K/Q8_0 for 16+ GB, and Q8_0 for 24+ GB, noting that K_M variants justify their slightly larger size by allocating more bits to attention layers. Task sensitivity varies: summarization, classification, and chat tolerate Q4, while code generation and multi-step reasoning call for Q5+, and math, precise extraction, and structured output (JSON/XML) demand Q6+ or Q8—consistent with early-2026 research finding commonsense reasoning resilient to quantization but arithmetic reasoning hitting a "quality cliff" below 4 bits. The article also highlights imatrix-calibrated quants like IQ4_XS (~4.0 GB, ~96%), which can match or beat Q4_K_M when properly calibrated (naming TheBloke, bartowski, and mradermacher as reliable uploaders), and warns against common mistakes: always quantize from F16/F32 source weights, budget 1–3 GB extra VRAM for the KV cache, and for CPU-only inference expect 5–15 tok/s with 32 GB RAM minimum for 7B models.
```
