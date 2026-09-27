# Web source

- URL: https://discuss.huggingface.co/t/how-much-vram-and-how-many-gpus-to-fine-tune-a-70b-parameter-model-like-llama-3-1-locally/150882
- Title: How much VRAM and how many GPUs to fine-tune a 70B parameter model like LLaMA 3.1 locally?
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:40:48.368418359+00:00
- Relevance: Medium — multiple title terms match query


```text
A Hugging Face forum post by user rxdt (July 2026) breaks down VRAM requirements for fine-tuning a 70B-parameter model like Llama 3.1 by method: full fine-tuning (AdamW, mixed precision) requires ~16 bytes/parameter (~1.1 TB before activations), realistically needing 8× H100/A100-80GB GPUs with ZeRO-3/FSDP sharding and activation checkpointing; LoRA with an fp16 base needs ~140 GB for the frozen weights plus adapter/optimizer state (2× 80GB or 4× 48GB GPUs with sharding); and QLoRA with a 4-bit NF4 base fits in ~40–45 GB, running on a single 48GB card (e.g., A6000/RTX PRO 6000), with context length being the main memory constraint since KV/activation memory scales with tokens. The author also built a VRAM calculator (vram.rxdt.dev) covering full fine-tuning, LoRA, and QLoRA with per-method formulas and assumptions.
```
