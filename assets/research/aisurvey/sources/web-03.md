# Web source

- URL: https://www.anthropic.com/research/self-replicating-ai
- Title: AI models may be capable of self-replicating without access to a datacenter
- Author(s): @AnthropicAI
- Language: English
- Published (UTC): 2026-10-12T00:00:00+00:00
- Captured (UTC): 2026-10-27T13:09:33.352983860+00:00
- Relevance: High - title matches query


```text
In an Anthropic research post, William Zhou and Daniel Freeman report that small AI models may be months away from being capable enough to self-replicate on commercial IoT devices, with a 9B model getting inference running on 8 of 9 tested hardware types; models of 30B parameters and under already complete many individual replication steps but make end-to-end mistakes, while GLM-5.2 and stronger models achieved full end-to-end replication in simplified evaluations. They demonstrated Gemma4-26B-A4B distributed across 15 ~$55 off-brand Android TV boxes (4GB RAM each) at 0.39 tokens/s, and only GLM-5.2 completed autonomous replication across their GCP fleet, reaching 8/8 nodes in 4 of 5 trials. Using the >2 million-device Kimwolf IoT botnet as a reference, they estimate ~133k replicas and 80k tokens/s, at most ~40 H100s-worth of inference, with 1–2 days per 9B replication, and note ISP quarantine cleared 92% of Mirai infections within 14 days versus 74% without notification.
```
