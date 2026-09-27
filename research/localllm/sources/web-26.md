# Web source

- URL: https://forum.level1techs.com/t/strix-halo-llm-inference-notes/249466
- Title: Strix Halo LLM inference notes
- Author(s): —
- Language: English
- Published (UTC): 2026-04-27T19:42:38+00:00
- Captured (UTC): 2026-09-18T05:37:44.643132288+00:00
- Relevance: High — title matches query


```text
A Level1Techs forum thread documents tuning AMD "Strix Halo" (Ryzen AI Max+ 395, Radeon 8060S) machines for long-context local LLM inference. The original poster's Minisforum MS-S1 MAX (128GB LPDDR5X unified memory, CachyOS, llama.cpp b8890 over Vulkan RADV with the `radv_enable_unified_heap_on_apu` driconf fix) ran Qwen3.6-35B-A3B UD-Q8_K_XL at ~24–25 tok/s with a live observed context of 153,562 tokens under 100W, served via llama-swap as an OpenAI-compatible endpoint. A Framework Strix Halo user's llama-bench results showed Vulkan generally beating ROCm for token generation (e.g., Qwen3.6 35B-A3B Q8_0: ~53.5 vs ~44.6 tok/s; prompt processing ~1049 vs ~1014 tok/s), with BF16 models performing poorly. Later testing (llama.cpp b9188, ROCm 7.13 on gfx1151, after MTP merged into mainline) found ROCm decode drops 64% at full context (46.2→16.6 tok/s on the 35B MoE with a 76k-token prompt) but MTP recovers it to 37.5 tok/s, while Vulkan is more stable (32.7→28.9, 34.3 with MTP); a 122B MoE hit ~22–24 tok/s (Q4 on Vulkan), a dense 27B managed only 6–9 tok/s, and ROCm's empty-context lead (2.3x) narrows to 1.3x at full context. Practical takeaways: use Q8 for 35B and Q4 for 122B (BF16 fails at full context), and a two-node Hermes agent topology—27B on a 7900 XTX foreground box plus the Strix Halo as a 262k-context, multi-slot parallel worker—proved more useful than one large model, with ROCm+MTP on the 35B MoE (37.5 tok/s at full context, <100W) as the production choice.
```
