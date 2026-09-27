# Web source

- URL: https://lambda.ai/blog/demystifying-gpt-3
- Title: OpenAI's GPT-3 Language Model: A Technical Overview
- Author(s): Chuan Li
- Language: English
- Published (UTC): 2020-06-03T04:00:00+00:00
- Captured (UTC): 2026-09-18T05:41:32.213668027+00:00
- Relevance: Medium — multiple title terms match query


```text
This June 2020 Lambda Labs blog post by Chuan Li analyzes OpenAI's GPT-3, the then-largest language model with 175 billion parameters across eight sizes (125M–175B), which the author estimates would require 3.14×10²³ FLOPS—355 GPU-years and ~$4.6M—to train even on the cheapest Tesla V100 cloud instances. Trained via next-word prediction on 300 billion tokens (Common Crawl 180.4B, WebText2 55.1B, Books 22.8B + 23.65B, Wikipedia 10.2B) using model parallelism on Microsoft's V100 cluster (its 700GB FP32 footprint far exceeding any single GPU's 48GB), GPT-3 keeps GPT-2's architecture but performs zero-, one-, and few-shot inference without fine-tuning, with performance scaling as a power law of model size, data, and compute. Notable results include near-chance human detection (~52%) of its 175B-generated articles versus 76% for the 125M model, beating fine-tuned SOTA on WMT Fr→En and De→En translation, lagging ~15% behind SOTA on BoolQ, and scoring 100% on 2-digit but under 10% on 5-digit arithmetic—evidence the author cites in debating whether the model reasons or merely memorizes. The post also notes data contamination checks (13-gram overlap) affected few benchmarks, that SOTA model sizes grow ~10× yearly (BERT 355M → GPT-2 1.5B → T5 11B → GPT-3 175B) outpacing GPU memory, and speculates on trillion-parameter models approaching the human brain's ~100 trillion synapses.
```
