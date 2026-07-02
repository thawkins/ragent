---
name: lcms
title: "\"large"
topic: "\"large Concept Models, using AI models that are based not on transformers, but on diffusion of tokens based on concepts/sentences and not words, find relationships with conceptnet and wordnet, describe the beinifits of Large Concept Models (LCMs). describe how the models are largely language independent, avoid descriptions of diffusion models that are not related to concept models. How could structured knowledge graphs such as ConceptNet or WordNet be explicitly integrated with LCMs to improve commonsense reasoning and interpretability? What tasks that require fine-grained token- or word-level precision (e.g., named-entity recognition, legal text analysis, word-level translation) are likely to underperform in current LCMs?How robust are frozen SONAR embeddings to domain-specific terminology, noise, and low-resource-language input quality? Can diffusion-based LCMs be made efficient enough for latency-sensitive, real-world applications, and how do inference costs scale compared with autoregressive token-level models? What is the optimal granularity of a “concept” beyond a single sentence, and how should segmentation adapt to languages and genres with different sentence boundaries?\""
status: complete
created: 2026-06-29T16:35:51.774741945+00:00
modified: 2026-06-29T16:35:51.833124846+00:00
sources: 0 # see sources/ subdirectory
queries:
  - "Large Concept Model LCM diffusion concepts sentences SONAR embeddings"
  - "ConceptNet WordNet integration Large Concept Models commonsense reasoning"
  - "Large Concept Models language independent multilingual cross-lingual"
  - "LCMs fine-grained token word tasks NER legal text translation underperformance"
  - "Frozen SONAR embeddings robustness domain terminology noise low-resource languages"
  - "Diffusion-based LCMs inference efficiency latency cost autoregressive scaling"
  - "Optimal concept granularity segmentation LCMs sentence boundaries languages genres"
  - "Large Concept Models diffusion concepts vs transformers"
---

# Title: "large

## Topic

"large Concept Models, using AI models that are based not on transformers, but on diffusion of tokens based on concepts/sentences and not words, find relationships with conceptnet and wordnet, describe the beinifits of Large Concept Models (LCMs). describe how the models are largely language independent, avoid descriptions of diffusion models that are not related to concept models. How could structured knowledge graphs such as ConceptNet or WordNet be explicitly integrated with LCMs to improve commonsense reasoning and interpretability? What tasks that require fine-grained token- or word-level precision (e.g., named-entity recognition, legal text analysis, word-level translation) are likely to underperform in current LCMs?How robust are frozen SONAR embeddings to domain-specific terminology, noise, and low-resource-language input quality? Can diffusion-based LCMs be made efficient enough for latency-sensitive, real-world applications, and how do inference costs scale compared with autoregressive token-level models? What is the optimal granularity of a “concept” beyond a single sentence, and how should segmentation adapt to languages and genres with different sentence boundaries?"

## Search Queries

- Large Concept Model LCM diffusion concepts sentences SONAR embeddings
- ConceptNet WordNet integration Large Concept Models commonsense reasoning
- Large Concept Models language independent multilingual cross-lingual
- LCMs fine-grained token word tasks NER legal text translation underperformance
- Frozen SONAR embeddings robustness domain terminology noise low-resource languages
- Diffusion-based LCMs inference efficiency latency cost autoregressive scaling
- Optimal concept granularity segmentation LCMs sentence boundaries languages genres
- Large Concept Models diffusion concepts vs transformers

## Summary

Large Concept Models (LCMs), introduced by Meta’s FAIR team, shift language modeling from predicting the next token to predicting the next sentence-level “concept” in a shared, language- and modality-agnostic embedding space (SONAR). By keeping SONAR encoders/decoders frozen and training only the concept-level model, the same reasoning sequence can be decoded into many languages and modalities without re-running generation. Sources highlight strong zero-shot multilingual summarization, better long-context scaling, and explicit high-level planning as key benefits, while also flagging serious open issues: loss of token-level precision, dependence on the robustness of frozen SONAR embeddings, the unclear optimality of sentence-level concepts across languages and genres, and the unproven real-time efficiency of diffusion-based concept generation compared with autoregressive token models.

## Findings

### Finding 1

**LCMs model language in a sentence/concept embedding space rather than token-by-token.**

**Observation:**
Meta’s “Large Concept Models: Language Modeling in a Sentence Representation Space” defines a concept as a language- and modality-agnostic higher-level semantic unit; as a proof of feasibility, the authors assume a concept corresponds to a sentence and use the existing SONAR embedding space, which supports up to 200 languages in text and speech [#9][#46]. The input text is segmented into sentences, each encoded into a fixed-size SONAR embedding; the LCM processes the sequence of embeddings and produces new embeddings, which a frozen SONAR decoder turns back into text or speech in any supported language [#3][#36].

**Analysis:**
This architecture explicitly separates high-level semantic reasoning from surface-language generation, which is the core mechanism behind the model’s claimed language independence and cross-modal flexibility.

**Cross-reference / Dependencies:**
No direct dependencies.

**Implication:**
Because the encoder/decoder are modular, new languages or modalities can theoretically be added without retraining the core LCM, but overall quality is bounded by whatever SONAR can represent.

**Additional evidence:**
A summary notes that SONAR embeddings are 1,024-dimensional floats and that storing them for 1 TB of raw text requires roughly 15–20 TB of embedding storage, while SONAR’s text encoder outperforms LASER3 and LaBSE on multilingual similarity tasks and includes a decoder for 200 languages [#3][#69].

### Finding 2

**LCMs show strong zero-shot multilingual generalization and better long-context scaling than token-level LLMs.**

**Observation:**
The 7B-parameter LCM “exhibits impressive zero-shot generalization performance to many languages, outperforming existing LLMs of the same size” on summarization, including low-resource languages such as Pashto, Burmese, and Hausa [#9][#36]. Paper-club summaries and LinkedIn distillations report that LCM inference efficiency scales better with context length than traditional autoregressive token models, and that the same concept sequence can be decoded into different languages without repeating the reasoning process [#14][#36].

**Analysis:**
Reasoning over a short sequence of sentence embeddings mitigates the quadratic-attention cost of transformer LLMs and reduces dependence on large per-language corpora, directly addressing the language-agnostic reasoning objective.

**Cross-reference / Dependencies:**
Builds on Finding 1 (SONAR concept space and modular decoder).

**Implication:**
LCMs are promising for multilingual summarization, cross-lingual content reuse, and global content moderation, but the reported gains are so far limited to summarization and the related “summary expansion” task.

**Caveat:**
Most of these claims are secondary summaries of the Meta paper; the primary quantitative evidence is the paper abstract and results section [#9][#46].

### Finding 3

**ConceptNet or WordNet are not yet explicitly integrated with LCMs, but related work shows clear integration pathways.**

**Observation:**
The LCM paper itself does not describe a ConceptNet or WordNet module, although one overview states that LCMs are “designed to incorporate structured knowledge — such as knowledge graphs and ontologies — to reason over complex abstractions” [#1]. Separate sources demonstrate that ConceptNet can be combined with contextual embeddings (e.g., BERT) and graph attention networks to improve commonsense question answering [#21]; that ConceptNet is an open multilingual semantic network with aligned word vectors and ExternalURL links to WordNet and DBPedia [#22]; and that organizing commonsense knowledge around conceptual primitives supports reasoning [#23].

**Analysis:**
To improve LCM commonsense reasoning, one could retrieve a ConceptNet or WordNet subgraph for each input or generated concept and inject graph constraints, relation embeddings, or attention biases into the diffusion denoising process. Mapping predicted concept embeddings to named knowledge-graph nodes would also improve interpretability by exposing explicit semantic relations rather than opaque vector transitions.

**Cross-reference / Dependencies:**
Builds on Finding 1 and Finding 7 (the need for well-defined concept nodes).

**Implication:**
This remains a research proposal rather than an evaluated system: a hybrid LCM with retrieval-augmented or graph-regularized generation should be built and tested on multilingual commonsense benchmarks.

### Finding 4

**Fine-grained token- or word-level tasks are likely to underperform in current LCMs.**

**Observation:**
A limitations analysis states that by skipping token-level processing, LCMs “lose the ability to capture fine-grained semantic and syntactic variations” and may underperform on named-entity recognition, legal text analysis, and word-level translation [#30]. The same source notes that LCMs have not been evaluated on dialogue generation, question answering, document classification, or other tasks requiring token-level comprehension [#30]. Independent clinical-NER work further shows that even token-level LLMs struggle with precise entity-span extraction [#48].

**Analysis:**
A sentence-level concept aggregates words into a single embedding, so the model has no explicit representation of token boundaries, exact lexical items, or morphological detail; the decoder must reconstruct these. This makes LCMs unsuitable as the sole model for tasks where a single word can change legal or medical meaning.

**Cross-reference / Dependencies:**
Depends on Finding 1 (concept definition) and Finding 8 (narrow evaluation scope).

**Implication:**
For legal, clinical, or NER applications, LCMs should be paired with token-level models or extended with token-level refinement layers.

### Finding 5

**Frozen SONAR embeddings are a robustness bottleneck for domain-specific terminology, noise, and low-resource input.**

**Observation:**
The LCM paper itself includes a section on the “fragility of SONAR space” and experiments with a “finetuned robust decoder” [#11]. A limitations review warns that if SONAR embeddings fail to capture domain-specific nuances (medical, legal, technical terms) or low-resource linguistic patterns, LCM performance suffers, and fixed embedding spaces may hinder adaptation [#30]. Broader multilingual-LLM surveys also report persistent performance drops on low-resource languages due to imbalanced training data and tokenization difficulties [#4][#15]. The later OmniSONAR work improves coverage to thousands of languages, but it is not the frozen SONAR used in the original LCM [#62].

**Analysis:**
Because the LCM freezes the encoder and decoder, any weakness in SONAR’s representation of rare terminology, noisy text, or low-resource syntax propagates directly into generation. Domain adaptation through the fixed bottleneck is therefore limited.

**Cross-reference / Dependencies:**
Depends on Finding 1.

**Implication:**
Before deploying in specialized domains, SONAR quality must be validated on in-domain corpora; domain adapters or more robust omnilingual encoders may be needed.

### Finding 6

**Diffusion-based LCM inference cost scales more gently with context length, but absolute latency/throughput may still lag autoregressive models.**

**Observation:**
Meta explored MSE regression, diffusion-based (One-Tower and Two-Tower), and quantized LCM variants; the diffusion variants performed best [#9][#43]. The paper includes an analysis of “inference efficiency of LCMs,” and paper-club notes claim that LCM efficiency scales better with context length than token-level transformers because the concept sequence is shorter [#11][#36]. However, a systematic study of diffusion language models finds that current open-source implementations generally achieve lower throughput than autoregressive models and that acceleration strategies mainly help small-batch settings [#78].

**Analysis:**
Concept-level diffusion reduces effective sequence length, but each generated concept requires multiple denoising forward passes and classifier-free guidance steps. Therefore, while cost growth with context length may be gentler than in AR transformers, per-sample latency and overall throughput may still be worse than optimized AR inference.

**Cross-reference / Dependencies:**
Builds on Finding 1 and Finding 9 (continuous/discrete representation choices).

**Implication:**
Diffusion LCMs are not yet proven efficient enough for latency-sensitive real-time applications; research should focus on reducing sampling steps, caching guidance, and benchmarking wall-clock cost across batch sizes and output lengths.

### Finding 7

**There is no established optimal concept granularity beyond a single sentence, and segmentation must be genre- and language-dependent.**

**Observation:**
The LCM feasibility study assumes a concept equals a sentence, and descriptions note that input text is segmented into sentences of roughly 10–20 tokens [#45][#36]. The paper lists “Concept granularity” as a limitation and includes a “Sentence segmentation analysis” section, but no source identifies an optimal general granularity or provides adaptive segmentation for languages/genres with different boundaries [#11][#30].

**Analysis:**
The sentence-level choice is a simplifying proof-of-concept. It may be too coarse for dense legal or academic prose, too short for languages with different punctuation or discourse structure, and too uniform for multimodal inputs such as speech utterances.

**Cross-reference / Dependencies:**
Depends on Finding 1 and Finding 4.

**Implication:**
Future work should explore hierarchical or learned segmentation (clauses, sentences, paragraphs) and evaluate its impact across typologically diverse languages and long-form genres.

### Finding 8

**The available LCM evaluation is too narrow to support broad claims about reasoning or interpretability.**

**Observation:**
LCMs were evaluated primarily on summarization and the new “summary expansion” task; the limitations analysis calls this a narrow scope and says future evaluations should cover logical reasoning, semantic similarity, narrative continuity, dialogue generation, question answering, and document classification [#9][#30].

**Analysis:**
Claims that LCMs achieve deeper conceptual understanding rest on a small set of high-level generation tasks. Without benchmarks for commonsense reasoning and fine-grained tasks, it is unclear whether LCMs are broadly better or merely better at condensation.

**Cross-reference / Dependencies:**
Depends on Finding 2 and Finding 4.

**Implication:**
The community needs standardized multilingual and cross-modal benchmarks covering commonsense, NER, legal/clinical text, and word-level translation before deployment decisions.

### Finding 9

**Continuous SONAR diffusion outperforms quantized and MSE-regression LCM variants, but quantization may matter for efficiency and knowledge integration.**

**Observation:**
The LCM paper studies continuous targets via MSE and diffusion in SONAR space, and discrete targets via residual vector quantization (Quant LCM), in which a vector is decomposed into 64 codebooks each with 8,192 entries [#11][#43]. The diffusion approach significantly outperformed both the Base MSE and Quantized variants [#43].

**Analysis:**
Continuous spaces preserve semantic smoothness but require iterative sampling; discrete spaces could enable faster autoregressive generation and easier integration with structured symbolic knowledge, but the current Quant LCM was less effective.

**Cross-reference / Dependencies:**
Depends on Finding 1.

**Implication:**
While diffusion on continuous concepts is currently superior, improving quantized representations remains relevant for speed and for explicit integration with knowledge graphs such as ConceptNet.

### Finding 10

**LCMs carry large embedding-storage and training-scale overheads.**

**Observation:**
Storing SONAR sentence embeddings for 1 TB of raw text requires approximately 15–20 TB of embeddings because every sentence is represented by a 1,024-dimensional float vector [#3]. The 7B LCM was trained on about 7.7 trillion tokens according to the Meta abstract, or about 2.7 trillion tokens according to the arXiv abstract [#9][#46].

**Analysis:**
The sentence-embedding bottleneck adds memory, storage, and I/O costs on top of the LCM itself, which affects both training infrastructure and serving economics.

**Cross-reference / Dependencies:**
Builds on Finding 1.

**Implication:**
Practical deployments need embedding compression, approximate nearest-neighbor retrieval, or on-the-fly encoding strategies to manage storage and throughput.

## In-Project Cross-References

| Path | Relevance |
|------|-----------|
| `https://github.com/facebookresearch/large_concept_model` | Meta’s open-sourced LCM training code referenced in summaries of the paper. |
| `https://github.com/facebookresearch/SONAR` | The frozen multilingual/multimodal sentence embedding space that underpins LCMs. |
| `https://github.com/commonsense/conceptnet5/wiki/FAQ` | ConceptNet documentation referenced as a source of structured commonsense knowledge. |

## Open Questions

- What is the right concept granularity beyond a single sentence, and how should segmentation adapt to low-resource languages and long-form genres with different sentence boundaries?
- How can ConceptNet or WordNet be fused into the continuous SONAR/diffusion generation process, and does such integration actually improve multilingual commonsense reasoning and interpretability?
- How robust are frozen SONAR embeddings to domain-specific terminology, noisy input, and extremely low-resource languages, and which adaptation methods preserve zero-shot gains?
- Can diffusion-based LCMs be accelerated enough to match or beat optimized autoregressive token-level models in latency-sensitive settings, and how do costs scale with batch size and output length?
- Which token-level tasks can be recovered by hybrid LCM–LLM pipelines without losing language-agnostic benefits?
- What multilingual and cross-modal benchmarks are needed to fairly evaluate LCMs on commonsense reasoning, fine-grained extraction, legal/clinical text, and word-level translation?

## References Index

| # | Type | Path/URL | Title | Relevance | Captured |
|---|------|----------|-------|-----------|----------|
| 1 | web | https://ernesenorelus.medium.com/large-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d | [Sitemap][1] | — | 2026-06-29T16:29:27.437133707+00:00 |
| 2 | web | https://aimlapi.com/blog/meta-large-concept-model-lcm-the-future-of-language-agnostic-reasoning-multilingual-multimodal-llms-with-conceptual-embeddings | [ | — | 2026-06-29T16:29:29.656559449+00:00 |
| 3 | web | https://gonzoml.substack.com/p/lcm-large-concept-model | [ | — | 2026-06-29T16:29:30.069782011+00:00 |
| 4 | web | https://blog.premai.io/multilingual-llms-progress-challenges-and-future-directions | link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/prism/1.28.0/themes/prism-tomorrow.min.css" | — | 2026-06-29T16:29:31.309676057+00:00 |
| 5 | web | https://ajithp.com/2025/01/05/large-concept-model-lcm-redefining-language-understanding-with-multilingual-and-modality-agnostic-ai | [Skip to content][1] | — | 2026-06-29T16:29:31.775414+00:00 |
| 6 | web | https://www.projectpro.io/article/large-concept-models/1114 | [ [projectpro logo] ][1] | — | 2026-06-29T16:29:35.250855617+00:00 |
| 7 | web | https://www.datacamp.com/blog/large-concept-models | [Skip to main content][1] | — | 2026-06-29T16:29:35.766720783+00:00 |
| 8 | web | https://www.analyticsvidhya.com/blog/2025/03/large-concept-models | [ | — | 2026-06-29T16:29:37.324433462+00:00 |
| 9 | web | https://ai.meta.com/research/publications/large-concept-models-language-modeling-in-a-sentence-representation-space | [[Meta]][1] | — | 2026-06-29T16:29:39.551162623+00:00 |
| 10 | web | https://ai.plainenglish.io/llm-tutorial-28-multilingual-and-cross-lingual-language-models-0bbc475733a6 | [Sitemap][1] | — | 2026-06-29T16:29:41.966303110+00:00 |
| 11 | web | https://arxiv.org/html/2412.08821v2 | 1. Large Concept Models: Language Modeling in a Sentence Representation Space | — | 2026-06-29T16:29:42.854314175+00:00 |
| 12 | web | https://deepchecks.com/glossary/cross-lingual-language-models | [[Deepchecks]][1] | — | 2026-06-29T16:29:46.034597451+00:00 |
| 13 | web | https://www.giskard.ai/glossary/cross-lingual-language-models | [Live session] From LLM vulnerabilities to AI agent red teaming & continuous evaluation 🚀 | — | 2026-06-29T16:29:47.172194272+00:00 |
| 14 | web | https://www.linkedin.com/pulse/large-concept-models-language-modeling-sentence-space-vlad-bogolin-wdige | Agree & Join LinkedIn | — | 2026-06-29T16:29:48.458973783+00:00 |
| 15 | web | https://scale.stanford.edu/ai/repository/multilingual-large-language-models-do-not-comprehend-all-natural-languages-equal | [ Skip to main content ][1] | — | 2026-06-29T16:29:51.036219202+00:00 |
| 16 | web | https://www.youtube.com/watch?v=qN_u_SmtQzw | [][1][][2] | — | 2026-06-29T16:29:51.799851217+00:00 |
| 17 | web | https://aclanthology.org/2025.naacl-long.72.pdf | %PDF-1.5 | — | 2026-06-29T16:29:56.215650331+00:00 |
| 18 | web | https://github.com/LightChen233/Awesome-Multilingual-LLM | [Skip to content][1] | — | 2026-06-29T16:29:57.530237071+00:00 |
| 19 | web | https://www.media.mit.edu/publications/bttj/Paper23Pages211-226.pdf | %PDF-1.4 %���� | — | 2026-06-29T16:30:03.828955856+00:00 |
| 20 | web | https://pypi.org/project/ConceptNet-A-Practical-Commonsense-Reasoning-Toolkit | JavaScript is disabled in your browser. | — | 2026-06-29T16:30:04.227795249+00:00 |
| 21 | web | https://pmc.ncbi.nlm.nih.gov/articles/PMC12824272 | [ Skip to main content ][1] | — | 2026-06-29T16:30:05.719844304+00:00 |
| 22 | web | https://conceptnet.io | # ConceptNet | — | 2026-06-29T16:30:07.859226098+00:00 |
| 23 | web | https://link.springer.com/article/10.1007/s12559-024-10345-6 | [Skip to main content][1] | — | 2026-06-29T16:30:10.900279026+00:00 |
| 24 | web | https://github.com/commonsense/conceptnet5/wiki/FAQ | [Skip to content][1] | — | 2026-06-29T16:30:11.940010226+00:00 |
| 25 | web | https://www.linkedin.com/pulse/rise-large-concept-models-artificial-intelligence-dr-ivan-del-valle-nqgbe | Agree & Join LinkedIn | — | 2026-06-29T16:30:13.621935994+00:00 |
| 26 | web | https://www.salesforce.com/blog/leveraging-language-models-for-commonsense | [ | — | 2026-06-29T16:30:16.951422448+00:00 |
| 27 | web | https://cdn.aaai.org/ocs/7955/7955-36718-1-PB.pdf | %PDF-1.5 %���� | — | 2026-06-29T16:30:19.361897763+00:00 |
| 28 | web | https://adasci.org/blog/a-deep-dive-into-large-concept-models-lcms | # Association of Data Scientists | — | 2026-06-29T16:30:22.871217898+00:00 |
| 29 | web | https://multi3generation.inesc-id.pt/wp-content/uploads/2021/11/STSM-Alicante-University-Lisbon.pdf | %PDF-1.7 | — | 2026-06-29T16:30:29.844867554+00:00 |
| 30 | web | https://dr-arsanjani.medium.com/limitations-of-large-concept-models-critical-analysis-and-recommendations-for-improvement-64318f9b428e | [Sitemap][1] | — | 2026-06-29T16:30:30.860885330+00:00 |
| 31 | web | http://kv-emptypages.blogspot.com/2021/01/adding-commonsense-reasoning-to-natural.html | # [ eMpTy Pages ][1] | — | 2026-06-29T16:30:32.627594091+00:00 |
| 32 | web | https://www.reddit.com/r/singularity/comments/1hkwmo0/large_concept_models_language_modeling_in_a | [ Skip to main content ][1] | — | 2026-06-29T16:30:33.730479132+00:00 |
| 33 | web | https://inklab.usc.edu/CommonGen/commongen_arxiv.pdf | %PDF-1.5 | — | 2026-06-29T16:30:35.247346811+00:00 |
| 34 | web | https://aclanthology.org/2021.eacl-demos.15.pdf | %PDF-1.3 | — | 2026-06-29T16:30:40.275619953+00:00 |
| 35 | web | https://rusillini.github.io/docs/kotov-wsdm12.pdf | %PDF-1.4 | — | 2026-06-29T16:30:40.748289235+00:00 |
| 36 | web | https://www.cloudwalk.io/ai/paper-club-sessions-large-concept-models-language-modeling-in-a-sentence-representation-space | [ | — | 2026-06-29T16:30:41.325309432+00:00 |
| 37 | web | https://www.youtube.com/watch?v=5QOtJ1_2MZE | [][1][][2] | — | 2026-06-29T16:30:42.093464640+00:00 |
| 38 | web | https://www.digitalocean.com/community/tutorials/large-concept-models | * [Blog][1] | — | 2026-06-29T16:30:44.083306904+00:00 |
| 39 | web | https://ainexxo.com/large-concept-models-lcms | [Skip to content][1] | — | 2026-06-29T16:30:46.585757598+00:00 |
| 40 | web | https://www.youtube.com/watch?v=l8Rzc4NeTiw | [][1][][2] | — | 2026-06-29T16:30:47.304736362+00:00 |
| 41 | web | https://www.reddit.com/r/LocalLLaMA/comments/1hdkh7k/metas_large_concept_model | [ Skip to main content ][1] | — | 2026-06-29T16:30:51.090191336+00:00 |
| 42 | web | https://www.linkedin.com/pulse/large-concept-models-thinking-beyond-tokens-amita-kapoor-ymq9c | Agree & Join LinkedIn | — | 2026-06-29T16:30:53.298009985+00:00 |
| 43 | web | https://www.lesswrong.com/posts/7Dtyhdkp5m6p4mquC/distillation-of-meta-s-large-concept-models-paper | x | — | 2026-06-29T16:30:55.090754817+00:00 |
| 44 | web | https://aimultiple.com/large-concept-models | [[AIMultiple][AIMultiple]][1] | — | 2026-06-29T16:30:56.279501042+00:00 |
| 45 | web | https://www.intoai.pub/p/metas-large-concept-models-lcms-are | [ | — | 2026-06-29T16:30:56.889007912+00:00 |
| 46 | web | https://arxiv.org/abs/2412.08821 | [Skip to main content][1] | — | 2026-06-29T16:30:57.107463362+00:00 |
| 47 | web | https://pdfs.semanticscholar.org/5e74/2909ed3ff311c498f89d55021cf1788bcf04.pdf | %PDF-1.3 | — | 2026-06-29T16:31:02.892119573+00:00 |
| 48 | web | https://pmc.ncbi.nlm.nih.gov/articles/PMC12099373 | [ Skip to main content ][1] | — | 2026-06-29T16:31:04.374330276+00:00 |
| 49 | web | https://arxiv.org/abs/2410.20941 | [Skip to main content][1] | — | 2026-06-29T16:31:05.136505005+00:00 |
| 50 | web | https://aclanthology.org/2025.naacl-srw.1 | [[ACL Logo] ACL Anthology ][1] | — | 2026-06-29T16:31:06.727283424+00:00 |
| 51 | web | https://www.facebook.com/groups/2600net/posts/4256572901232469 | https://www.facebook.com/groups/2600net/posts/4256572901232469 | — | 2026-06-29T16:31:07.389072244+00:00 |
| 52 | web | https://openreview.net/pdf/35edb035a4b5b49d806d011714e261de35bc51ff.pdf | %PDF-1.5 | — | 2026-06-29T16:31:09.165263392+00:00 |
| 53 | web | https://www.microsoft.com/en-us/research/publication/fine-grained-coordinated-cross-lingual-text-stream-alignment-for-endless-language-knowledge-acquisition | [Skip to main content][1] [ [Microsoft] ][2] [ Research ][3] [Publications][4] [Code & data][5] [People][6] [Microsoft | — | 2026-06-29T16:31:13.275453815+00:00 |
| 54 | web | https://www.linkedin.com/posts/nishantha-ruwan-15b301b2_languages-are-modalities-cross-lingual-alignment-activity-7399621492797313024-IP4a | Agree & Join LinkedIn | — | 2026-06-29T16:31:16.251403322+00:00 |
| 55 | web | https://www.youtube.com/watch?v=8efKuAWVCMs | [][1][][2] | — | 2026-06-29T16:31:17.206550375+00:00 |
| 56 | web | https://arxiv.org/html/2405.11357v1 | 1. [1 Introduction][1] | — | 2026-06-29T16:31:17.479226831+00:00 |
| 57 | web | https://assets.amazon.science/6b/72/85118aac4805b6520d6a53699d04/fine-tuned-machine-translation-metrics-struggle-in-unseen-domains.pdf | %PDF-1.5 | — | 2026-06-29T16:31:19.505306068+00:00 |
| 58 | web | https://huggingface.co/papers?q=Long-context+ASR | [[Hugging Face's logo] Hugging Face][1] | — | 2026-06-29T16:31:22.135762458+00:00 |
| 59 | web | https://aclanthology.org/2026.loreslm-1.31.pdf | %PDF-1.5 | — | 2026-06-29T16:31:24.848476656+00:00 |
| 60 | web | https://arxiv.org/html/2603.16606v3 | ##### Report GitHub Issue | — | 2026-06-29T16:31:25.984562696+00:00 |
| 61 | web | https://www.youtube.com/watch?v=rTRLzqHIT8A | [][1][][2] | — | 2026-06-29T16:31:26.909871277+00:00 |
| 62 | web | https://ai.meta.com/research/publications/omnilingual-sonar-cross-lingual-and-cross-modal-sentence-embeddings-bridging-massively-multilingual-text-and-speech | [[Meta]][1] | — | 2026-06-29T16:31:30.856819155+00:00 |
| 63 | web | https://medium.com/@roseserene/sonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31 | [Sitemap][1] | — | 2026-06-29T16:31:32.031973961+00:00 |
| 64 | web | https://www.raillab.org/publication/nigatu-2024-zenos/nigatu-2024-zenos.pdf | %PDF-1.5 | — | 2026-06-29T16:31:35.165534444+00:00 |
| 65 | web | https://www.scribd.com/document/841947399/SONAR-Sentence-Level-Multimodal-and-Language-Agnostic-Representations | Skip to main content | — | 2026-06-29T16:31:36.445672366+00:00 |
| 66 | web | https://github.com/facebookresearch/SONAR | [Skip to content][1] | — | 2026-06-29T16:31:37.948561903+00:00 |
| 67 | web | https://www.youtube.com/watch?v=ZqBd0pClFXc | [][1][][2] | — | 2026-06-29T16:31:38.924594535+00:00 |
| 68 | web | https://www.semanticscholar.org/paper/SONAR%3A-Sentence-Level-Multimodal-and-Duquenne-Schwenk/dbcced1c0f3b01f66f1dc1b820f084d440b28d1e | https://www.semanticscholar.org/paper/SONAR%3A-Sentence-Level-Multimodal-and-Duquenne-Schwenk/dbcced1c0f3b01f66f1dc1b820f084d440b28d1e | — | 2026-06-29T16:31:38.980394950+00:00 |
| 69 | web | https://ai.meta.com/research/publications/sonar-sentence-level-multimodal-and-language-agnostic-representations | [[Meta]][1] | — | 2026-06-29T16:31:40.168162701+00:00 |
| 70 | web | https://www.kaggle.com/code/selcukcan/nlp-13-sonar-embeddings | https://www.kaggle.com/code/selcukcan/nlp-13-sonar-embeddings | — | 2026-06-29T16:31:41.222113886+00:00 |
| 71 | web | https://ar5iv.labs.arxiv.org/html/2308.11466 | # Sonar: Sentence-Level Multimodal | — | 2026-06-29T16:31:42.053787090+00:00 |
| 72 | web | https://www.isca-archive.org/interspeech_2022/yadav22b_interspeech.html | [ISCA][1] [Archive][2] [Interspeech 2022][3] | — | 2026-06-29T16:31:43.042019177+00:00 |
| 73 | web | https://cmsworkshops.com/ICASSP2026/papers/accepted_papers.php | * [Home][1] | — | 2026-06-29T16:31:55.800079628+00:00 |
| 74 | web | https://theses.hal.science/tel-04573934v1/file/144440_DUQUENNE_2024_archivage.pdf | %PDF-1.4 | — | 2026-06-29T16:32:09.584310380+00:00 |
| 75 | web | https://alexfraser.github.io/research.html | ** | — | 2026-06-29T16:32:10.362408959+00:00 |
| 76 | web | https://ceur-ws.org/Vol-4137/WOWS_2025_paper_1.pdf | %PDF-1.7 | — | 2026-06-29T16:32:16.141711700+00:00 |
| 77 | web | https://www.instagram.com/p/DYlRPtZmH9f | [ | — | 2026-06-29T16:32:17.195974289+00:00 |
| 78 | web | https://arxiv.org/html/2510.18480v3 | 1. [1 Introduction][1] | — | 2026-06-29T16:32:18.455123690+00:00 |
| 79 | web | https://x.com/TheValueist/article/2064844338467631454 | # JavaScript is not available. | — | 2026-06-29T16:32:21.489797722+00:00 |
| 80 | web | http://openaccess.thecvf.com/content/CVPR2025/papers/Ma_Scaling_Inference_Time_Compute_for_Diffusion_Models_CVPR_2025_paper.pdf | %PDF-1.3 | — | 2026-06-29T16:32:43.233674458+00:00 |
| 81 | web | https://www.themoonlight.io/en/review/visual-autoregressive-models-beat-diffusion-models-on-inference-time-scaling | [ | — | 2026-06-29T16:32:43.836447711+00:00 |
| 82 | web | https://www.together.ai/blog/consistency-diffusion-language-models | [ | — | 2026-06-29T16:32:44.608021890+00:00 |
| 83 | web | https://blog.ml.cmu.edu/2025/09/22/diffusion-beats-autoregressive-in-data-constrained-settings | # Machine Learning Blog \| ML@CMU \| Carnegie Mellon University | — | 2026-06-29T16:32:48.944721502+00:00 |
| 84 | web | https://mlsys.org/virtual/2025/session/3155 | [Skip to yearly menu bar][1] [Skip to main content][2] | — | 2026-06-29T16:32:51.072139069+00:00 |
| 85 | web | https://openreview.net/forum?id=j1tSLYKwg8 | [**OpenReview**.net][1] | — | 2026-06-29T16:32:51.962059139+00:00 |
