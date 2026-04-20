---
title: "DeepSeek R1"
entity_type: "product"
type: entity
generated: "2026-04-19T15:38:01.120125377+00:00"
---

# DeepSeek R1

**Type:** product

### From: huggingface

DeepSeek R1 is a large language model developed by DeepSeek AI, a Chinese artificial intelligence company, and included in HuggingFaceProvider's default model catalog. Released in early 2025, DeepSeek R1 gained significant attention for its reasoning capabilities and cost efficiency, representing a notable achievement in open-weight model development. The model is based on a Mixture-of-Experts (MoE) architecture with 671 billion total parameters, though only 37 billion are activated per token during inference. This architectural choice enables high performance while reducing computational costs compared to dense models of similar capability.

In the ragent implementation, DeepSeek R1 is configured with distinctive capabilities compared to other default models. While most default models (Llama 3.1 variants, Qwen 2.5) have reasoning: false and tool_use: true, DeepSeek R1 inverts this pattern with reasoning: true and tool_use: false. This reflects the model's specialization: it excels at extended chain-of-thought reasoning and complex problem-solving but was not trained with tool-calling capabilities. The configuration includes a 128,000 token context window and notably higher max_output of 8,192 tokens compared to the 4,096 standard for other models, accommodating its verbose reasoning traces.

The inclusion of DeepSeek R1 in the default catalog alongside Western models like Llama 3.1 demonstrates ragent's commitment to model diversity and the global nature of the open-source AI ecosystem. DeepSeek AI's release of R1 as an open-weight model—with fully open training details—contributed to debates about AI development economics and the feasibility of efficient large model training. The model's presence in this Rust implementation, developed likely by a Western team, illustrates how open-source AI infrastructure transcends geopolitical boundaries, though users should note that access to Chinese-developed models through HuggingFace may be subject to additional scrutiny or restrictions depending on regulatory environments.

## Sources

- [huggingface](../sources/huggingface.md)
