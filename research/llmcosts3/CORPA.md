# Corpus Analysis Companion (CORPA.md)

Quality-assurance companion document for `The captured sources contain no vendor price lists or per-token dollar rates…`. Generated together with `RESEARCH.md`; the `[#N]` source indices reference the Sources Reference table at the bottom of this file.

## Contradiction Graph

_(no contradictions detected among the gathered sources)_

## Loci Analysis

| Locus | Sources | Mentions | Representative Snippets |
|-------|---------|----------|-------------------------|
| Cost | #1, #4, #5, #6 | 4 | LLMs-as-the-Judge as an alternative to costly human evaluation for biomedical relat; cial role in performance, security, |
| Accessibility | #8 | 1 | nd rigid control pipelines, which limit accessibility for non-expert users. The work introduc |
| Adoption | #7 | 1 | ata sources and tools, noting its rapid adoption across major AI providers and developer |
| Performance | #4 | 1 | as HTTP proxies, play a crucial role in performance, security, and cost-effectiveness. The |
| Risk | #3 | 1 | t selection/hiring, information-sharing risks like GDPR/CCPA violations), and compro |

## Depth Investigation

| Locus | Depth | Sources | Note |
|-------|-------|---------|------|
| Cost | deep | #1, #4, #5 | Detected in 4 sources (depth: deep). |
| Accessibility | surface | #8 | Detected in 1 source (depth: surface). |
| Adoption | surface | #7 | Detected in 1 source (depth: surface). |
| Performance | surface | #4 | Detected in 1 source (depth: surface). |
| Risk | surface | #3 | Detected in 1 source (depth: surface). |

## Source Tensions

| Kind | Label | Sources | Note |
|------|-------|---------|------|
| shallow evidence | Accessibility | #8 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Adoption | #7 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Performance | #4 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Risk | #3 | surface evidence: only 1 source(s) mention this dimension. |
| isolated source | Accessibility | #8 | Source #8 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Adoption | #7 | Source #7 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Cost | #1 | Source #1 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Cost | #5 | Source #5 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Cost | #6 | Source #6 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Risk | #3 | Source #3 only supports one dimension and may represent an outlier or niche view. |

## Synthesis Audit

**Overall score:** 100/100

**Recommendation:** Proceed — the synthesis passes the deterministic 4-critic audit.

Synthesis audit for 'LLM API pricing comparison 2025' scored 100/100 across critics [coverage=100 logic=100 evidence=100 readability=100]; 8/9 sources cited.

| Critic | Score | Status | Issue / Gap Summary |
|--------|-------|--------|---------------------|
| coverage | 100 | pass | none |
| logic | 100 | pass | No contradictions detected; no logic conflicts to resolve. |
| evidence | 100 | pass | none |
| readability | 100 | pass | Finding 1 contains a paragraph longer than 1200 characters |

## Corpus Critic

**Overall score:** 39/100 (review)

**Subscores:** coverage 50 | evidence 20 | balance 0 | tension 100

**Issues:**
- Dimension 'Accessibility' has only surface-level support (1 source(s))
- Dimension 'Adoption' has only surface-level support (1 source(s))
- Dimension 'Performance' has only surface-level support (1 source(s))
- Dimension 'Risk' has only surface-level support (1 source(s))
- Corpus is dominated by one perspective; adversarial sources may be missing.
- 6 source(s) only support a single dimension and may be outliers

**Evidence gaps:**
- Find additional evidence on 'Accessibility' for 'LLM API pricing comparison 2025'
- Find additional evidence on 'Adoption' for 'LLM API pricing comparison 2025'
- Find additional evidence on 'Performance' for 'LLM API pricing comparison 2025'
- Find additional evidence on 'Risk' for 'LLM API pricing comparison 2025'
- Add sources with an opposing view on 'LLM API pricing comparison 2025'

**Recommendations:**
- Re-run the width sweep with explicitly skeptical sub-queries.

**Shallow dimensions:** Accessibility, Adoption, Performance, Risk

**Isolated sources:** #1, #6, #7, #3, #8, #5

## Sources Reference

| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|
| 1 | web | page | English | [https://doi.org/10.18653/v1/2025.acl-long.1238](https://doi.org/10.18653/v1/2025.acl-long.1238) | Proceedings of the 63rd Annual Meeting of the Association for Computational Linguistics (Volume 1: Long Papers),… | — | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:37:34.898097403+00:00 |
| 2 | web | page | English | [https://doi.org/10.1016/j.smhl.2025.100552](https://doi.org/10.1016/j.smhl.2025.100552) | Privacy-preserving LLM-based chatbots for hypertensive patient self-management | [Sara Montagna, Stefano Ferretti, Lorenz Cuno Klopfenstein, Michelangelo Ungolo, Martino F. Pengo, Gianluca Aguzzi, Matteo Magnini] | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:37:49.146422292+00:00 |
| 3 | web | page | English | [https://doi.org/10.18653/v1/2025.realm-1.9](https://doi.org/10.18653/v1/2025.realm-1.9) | Proceedings of the 1st Workshop for Research on Agent Language Models (REALM 2025), pages 109–130 | — | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:37:58.347681121+00:00 |
| 4 | web | page | English | [http://arxiv.org/abs/2410.11857](http://arxiv.org/abs/2410.11857) | LLMBridge: Reducing Costs to Access LLMs in a Prompt-Centric Internet | [Noah Martin, Abdullah Bin Faisal, Hiba Eltigani, Rukhshan Haroon, Swaminathan Lamelas, Fahad R. Dogar] | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:38:02.920766880+00:00 |
| 5 | web | page | English | [https://doi.org/10.1609/aaai.v40i43.40970](https://doi.org/10.1609/aaai.v40i43.40970) | Breaking Model Lock-in: Cost-Efficient Zero-Shot LLM Routing via a Universal Latent Space | — | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:38:16.267730487+00:00 |
| 6 | web | page | English | [https://doi.org/10.1145/3725356](https://doi.org/10.1145/3725356) | SpareLLM: Automatically Selecting Task-Specific Minimum-Cost Large Language Models under Equivalence Constraint | — | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:38:27.944893800+00:00 |
| 7 | web | page | English | [http://arxiv.org/abs/2508.14704](http://arxiv.org/abs/2508.14704) | MCP-Universe: Benchmarking Large Language Models with Real-World Model Context Protocol Servers | [Luo, Ziyang, Zhiqi Shen, Wenzhuo Yang, Zirui Zhao, Prathyusha Jwalapuram, Amrita Saha, Doyen Sahoo, Silvio Savarese, Caiming Xiong, Junnan Li] | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:38:30.381252986+00:00 |
| 8 | web | page | English | [https://doi.org/10.3390/s26020608](https://doi.org/10.3390/s26020608) | Latency-Aware Benchmarking of Large Language Models for Natural-Language Robot Navigation in ROS 2 | [Manjulika Das, Zawar Hussain, Muhammad Nawaz] | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:38:33.938567779+00:00 |
| 9 | web | page | English | [https://doi.org/10.1016/j.ecoinf.2025.103278](https://doi.org/10.1016/j.ecoinf.2025.103278) | Challenges of passive citizen science in ecology within a shifting social media landscape | [Pablo Otero, Javier Menéndez‐Blázquez, David March] | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-13T01:38:38.414320678+00:00 |
