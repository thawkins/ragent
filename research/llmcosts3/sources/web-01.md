# Web source

- URL: https://doi.org/10.18653/v1/2025.acl-long.1238
- Title: Proceedings of the 63rd Annual Meeting of the Association for Computational Linguistics (Volume 1: Long Papers),…
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T01:37:34.898097403+00:00
- Relevance: Scholarly — engine-ranked abstract
- Open-access recovery: full text fetched from unpaywall (https://aclanthology.org/2025.acl-long.1238.pdf); version=gold, license=cc-by

```text
This ACL 2025 paper (pages 25483–25497; Laskar et al., York University and Vector Institute) investigates LLMs-as-the-Judge as an alternative to costly human evaluation for biomedical relation extraction, benchmarking 8 LLM judges (GPT-4o-Mini, Gemini-1.5-Flash, Claude-3-Haiku, LLaMA-3.1-8B-Instruct, Qwen-2.5-7B-Instruct, Phi-3.5-Mini-3.8B-Instruct, and DeepSeek-R1-Distill-Qwen-7B/Llama-8B) on responses from 5 LLM generators (GPT-3.5, Claude-2, PaLM-2, LLaMA-2-13B, and GPT-4-Turbo) across 3 datasets: BC5CDR (chemical–disease, 500 test samples), KD-DTI (drug–target interaction, 1159), and DDI (drug–drug interaction, 191). LLM judges performed poorly in this task—typically below 50% exact-match accuracy (only GPT-4o-Mini exceeded 50%, yet under 60% per dataset)—largely because LLM-generated responses are unstructured and contain synonyms/abbreviations (e.g., "dexamethasone" vs. gold "dex") that defeat string-matching metrics. Requiring structured (JSON) output formatting improved judge accuracy by about 15% on average (gains statistically significant, p < 0.05), and a proposed domain adaptation/transfer learning technique—fine-tuning on limited human-annotated out-of-domain judgment data—further boosted open-source judges (e.g., fine-tuned Qwen-2.5-7B reached 75.75% on KD-DTI, +13.97, outperforming closed-source models). Scaling analysis showed shrinking Qwen from 7B to 1.5B cut accuracy by 58.70%/45.12%/74.44% on BC5CDR/DDI/KD-DTI, though a fine-tuned 3B model could outperform the zero-shot 7B. Based on over 100 experiments, the authors released 36k judgment samples (4k human-annotated, 32k LLM-annotated) at github.com/tahmedge/llm_judge_biomedical_re, while noting LLM judges still fall short of human evaluators.
```
