# Web source

- URL: https://www.mindstudio.ai/blog/kolibri-1-aleph-alpha-german-model
- Title: Kolibri-1: Aleph Alpha's Open-Weight German-English MoE Model
- Author(s): Luis Chavez-Mattos
- Language: English
- Published (UTC): 2026-10-04T00:00:00+00:00
- Captured (UTC): 2026-10-27T13:09:43.278742611+00:00
- Relevance: High - title + snippet match query


```text
Aleph Alpha released Kolibri-1 on October 3, 2026, an Apache 2.0 open-weight German-English MoE model with 78B total parameters but only ~3.46B active per token; its 50-layer transformer uses 384 experts per layer (one shared, six routed), supports up to 1,048,576 tokens (with 262,144 recommended for complex or latency-sensitive work), and was trained on 20T bilingual German-English-code tokens plus 3.44T mid-training and 201B long-context tokens. It grew from the unreleased Kolibri Origin in about three months, scaling from ~30B to 78B total parameters while active parameters rose only from 3.27B to 3.46B, training data from 7.5T to 20T tokens, context from 64K to 262,144, and experts from 128 to 384; its knowledge cutoff is June 18, 2026. FP8 weights are ~78GB, but Aleph Alpha lists a minimum of two A100 80GB or two H100 SXM5 GPUs (or one H200/B200/B300), independent testing reported ~140GB VRAM, and deployment uses the aleph-alpha-inference package/vLLM plugin with reasoning and tool-calling support. Hands-on tests included a Blender Python trampoline-park scene, a playable Godot game in one pass, and a self-contained HTML canvas kebab animation, while a German philosophical-prompt test found German output stronger and more idiomatic than constrained English; Aleph Alpha compares it to ~3B-active MoE models such as GLM-4.7 Flash 30B-A3B, Nemotron 3 Nano 30B-A3B, and Qwen3.5 35B-A3B.
```
