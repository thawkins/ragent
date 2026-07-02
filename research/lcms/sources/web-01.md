# Web source

- URL: https://ernesenorelus.medium.com/large-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T16:29:27.437133707+00:00

```text
[Sitemap][1]
[Open in app][2]

Sign up

[Sign in][3]

Get app
[
Write
][4]
[
Search
][5]

Sign up

[Sign in][6]

[Unknown user]

# Large Concept Models (LCMs) — Rethinking the Future of AI

[
[Ernese Norelus]
][7]
[Ernese Norelus][8]
12 min read
·
Mar 25, 2025
[
][9]

--

[
][10]
[][11]
[

Listen

][12]

Share

In recent years, large language models (**LLMs**) such as **ChatGPT**, **Claude**, and **Gemini** have transformed
natural language processing (**NLP**) by enabling machines to generate and interpret human-like text. These models
function as advanced autocomplete engines, generating language by predicting the next token — typically a word or
sub-word — based on the previous token/word. While this approach has enabled powerful applications in text generation,
summarisation, and dialogue, it remains fundamentally limited by its token-level perspective.

Imagine trying to understand a movie by watching individual frames out of sequence — you might capture surface-level
details but miss the plot, emotions, and deeper meaning. That’s how LLMs interpret language sequentially without truly
grasping the text’s overarching intent or conceptual coherence.

Press enter or click to view image in full size
**Source**: Created by [https://www.linkedin.com/in/leadgenmanthan/][13]

A new paradigm has emerged to address these limitations: Large Concept Models (**LCMs**). Pioneered by [Meta][14] in
their research on “[**Large Concept Models: Language Modelling in a Sentence Representation Space**][15]*,*” LCMs shift
the focus from token-based prediction to concept-based reasoning. Instead of predicting the next token, LCMs predict the
next high-level semantic unit — a complete sentence or idea — treating language as a series of abstract, structured
concepts.

LCMs aim to emulate human cognition more closely by operating in a sentence representation space, where each sentence or
paragraph is represented as a high-dimensional vector encapsulating its whole meaning. This allows for
language-agnostic, multimodal, and semantically rich representations, enabling the model to generalise across languages
and media formats more effectively.

At the core of this shift lies the “C” in LCM — **Concept**, which represents an abstract, high-level semantic construct
independent of a specific linguistic structure or modality. LCMs offer an architecture that is less reliant on the
traditional Transformer stack, using lightweight components to enable explicit reasoning, longer-term coherence, and
better performance in multilingual and cross-modal scenarios.

The emergence of LCMs is a technical evolution and a conceptual rethinking of how machines understand language. While
LLMs are powerful in generating fluent text, they struggle with tasks that require deep semantic understanding,
long-term logic, or domain-specific reasoning. LCMs, in contrast, are designed to incorporate structured knowledge —
such as knowledge graphs and ontologies — to reason over complex abstractions like causality, ethics, and legal logic.

Meta’s development of LCMs was partly driven by the challenge of supporting over **200 languages across its platforms**.
Traditional LLMs proved costly and inefficient for tasks like translation and content moderation. By abstracting away
language and operating directly at the conceptual level, LCMs offer a more scalable, efficient, and semantically rich
alternative for global artificial intelligence (AI) applications.

In this article, we’ll explore how LCMs differ from LLMs in architecture, methodology, and capability — highlighting the
key innovations that position LCMs as a potential leap forward in artificial intelligence.

## **Understanding Large Language Models (LLMs) and Large Concept Models (LCMs)**

The power of language has long fuelled artificial intelligence. Large Language Models (LLMs) — like OpenAI’s ChatGPT or
Google’s Gemini — have transformed how machines understand and generate human-like text. Built on deep learning
architectures such as Transformers, LLMs learn by predicting the next word in a sequence, processing language token by
token. This approach has enabled coherent conversations, text generation, and code completion at scale. However, LLMs
are still constrained by their reliance on surface-level linguistic patterns and statistical prediction.

**Source**: Created by [LCMs vs LLMs][16]

Enter the next evolution: **large concept models (LCMs)**. Introduced by Meta, LCMs mark a **paradigm shift** in AI,
moving away from token-based language modelling to **concept-based reasoning**. Instead of focusing on individual words
or phrases, LCMs encode and operate on **abstract, language-independent units of meaning** called ***concepts***. These
concepts are derived from entire sentences, actions, or ideas and are represented as high-dimensional embeddings that
reflect their semantic content.

While LLMs ask, *“What’s the next word?”* LCMs ask, *“What’s the next idea?”*

This change unlocks new capabilities:
* **Deeper comprehension**: LCMs grasp the intent behind complete sentences or paragraphs.
* **Cross-lingual fluency**: Concepts are **language-agnostic**, enabling seamless multilingual translation — even for
  low-resource languages.
* **Contextual clarity**: By working at the level of complete thoughts, LCMs excel at summarisation, long-form content
  generation, and reasoning over extended text.
* **Multimodal understanding**: LCMs can process not just text but also speech, images, and actions, bridging
  modalities.
* **Structured reasoning**: Tasks like scientific discovery, legal analysis, and ethical inference benefit from LCMs’
  ability to handle hierarchical, cause-effect, or goal-driven reasoning.

Meta’s ambitious goal with LCMs was to **decouple meaning from language**, enabling AI to interact with information the
way humans do — by understanding concepts and then expressing them as needed in any format or language.

In essence, while LLMs give machines the ability to speak, LCMs may also provide them with the ability to think.

## **The Growing Pains of a New Paradigm: Challenges in Large Concept Models (LCMs)**

While **Large Concept Models (LCMs)** represent a compelling evolution in artificial intelligence — shifting from
token-level prediction to concept-level abstraction — they also introduce a range of novel challenges that underscore
their early-stage maturity.

### **1. Concept Definition and Granularity**

Unlike LLMs, which rely on clearly defined tokens, LCMs grapple with the ambiguity of what constitutes a “concept.”
Current models often use complete sentences as proxies for concepts, but questions remain: Should concepts span phrases,
clauses, or even paragraphs? Long-form sentences with multiple ideas can be complicated to segment, while short or
fragmented inputs may fail to capture the whole semantic unit.

### **2. Training Complexity and Data Scarcity**

Training LCMs to reason over abstract, structured meanings demands semantically rich datasets — far beyond simple token
alignments. Curating such concept-aligned corpora/corpus is resource-intensive and domain-dependent, requiring
ontologies, knowledge graphs, and expert annotation. Moreover, rare concepts are underrepresented in datasets, limiting
model generalisability and accuracy in niche domains.

### **3. Computational Load and Scalability**

Operating in high-dimensional, sentence-embedding spaces requires significantly more computing power than token-based
architectures. For example, Meta’s diffusion-based LCM architecture involves iterative inference steps that increase
latency and strain real-time applications. This presents a trade-off between semantic depth and operational efficiency.

### **4. Interpretability and Debugging**

By their nature, concepts are abstract and less observable than discrete tokens. As a result, tracing how an LCM arrives
at a specific output — or identifying where it fails — is substantially more complex. Using non-Euclidean (hyperbolic)
geometry in some models further complicates visualisation, alignment, and control.

### **5. Evaluation and Benchmarking**

Traditional natural language processing (NLP) benchmarks — rooted in token prediction — are ill-suited for evaluating
concept-level reasoning. The AI community has yet to establish robust metrics to assess how well an LCM understands,
generalises, or reasons about abstract ideas across languages and modalities.

### **6. Modality and Language Generalisation**

Although LCMs aim to be language-agnostic and multimodal, performance still degrades in underrepresented domains — such
as low-resource languages, scientific texts, or programming languages. SONAR (**S**emantically **O**rganized **N**eural
and **A**bstract **R**epresentations) embeddings, for example, are more reliable on short, social-media-style sentences
than on technical documents or long-form reasoning.

### **7. Architectural and Ecosystem Shifts**

LCMs require rethinking many foundational elements of NLP, from tokenisers and decoders to training pipelines and
knowledge ingestion. Integration with current tooling — largely built for LLMs — remains limited, posing barriers to
adoption. Currently, pre-trained LCMs are not widely accessible, further slowing ecosystem development.

### **8. Diffusion and Semantic Noise**

In diffusion-based LCMs, transforming noisy concept vectors into coherent meanings introduces unique instability. Since
discrete concepts are embedded in continuous vector spaces, the denoising process must preserve fine-grained semantic
details — a delicate balance not yet fully optimised.

## **SONAR: The Semantic Backbone of Large Concept Models**

As LCMs rise as a new paradigm in AI that operates not on words or tokens but on high-level concepts, they require a
foundational mechanism to encode meaning beyond language, modality, or syntax. That foundation is **SONAR**.

### **What Is SONAR?**

**SONAR** (**S**emantically **O**rganized **N**eural and **A**bstract **R**epresentations) is the **embedding system**
that enables LCMs to function in a language-agnostic, modality-agnostic space. It’s a **Transformer-based
encoder-decoder architecture** designed to map entire sentences — or concepts — into **high-dimensional vectors**,
representing the underlying meaning rather than the surface structure of the language.

Where traditional LLMs work by processing text tokens by token, SONAR allows LCMs to treat an entire sentence as a
single, unified semantic unit. This abstraction enables **conceptual generalisation** rather than simple pattern
recognition.

### **Key Features of SONAR**
* **Language-Agnostic Reasoning**: SONAR supports **200+ languages** for text and **76 for speech**, including
  experimental support for American Sign Language (ASL). It can embed and decode meaning across languages, allowing LCMs
  to think in concepts, not words.
* **Multimodal Compatibility**: It works with written text and speech, enabling seamless **cross-lingual and
  cross-modal** reasoning.
* **Encoder & Decoder in One**: SONAR acts as the **input encoder** (sentence → concept embedding) and the **output
  decoder** (concept → sentence or speech), standardising input/output handling in LCM pipelines.
* **Zero-Shot Generalisation**: By abstracting meaning, SONAR enables **zero-shot translation and reasoning**, even in
  languages or modalities the model wasn’t explicitly trained on.
* **Efficient Representation**: It bypasses token-level attention and uses a **vector bottleneck** to compress an entire
  sentence into a single embedding — ideal for conceptual alignment and faster inference.

### **Why SONAR Matters to LCMs**

In the LCM architecture, **SONAR is to concepts what tokenisers are to words in LLMs**. It’s the critical enabler that
allows LCMs to:
* Generalise across linguistic and cultural boundaries.
* Work efficiently with short-form inputs like social media posts or commands.
* Perform semantic operations such as analogy-making, causal inference, and multilingual retrieval in a shared
  conceptual space.

Example:
* “Tim wasn’t very athletic. He tried out for several teams.”
gets compressed and semantically linked to “He decided to train on his own.”*

This abstraction shows how SONAR condenses multi-sentence reasoning into core, language-independent concepts, which LCMs
can reason over, refine, and regenerate.

### **Limitations and Outlook**

Despite its innovation, SONAR has some limitations. It was primarily trained on **parallel translation data**, which
tends to favour **short, declarative sentences**. This means it may struggle with:
* Long, complex sentences with multiple embedded ideas.
* Precision and structure are crucial in scientific writing, code, or legal text.
* Tasks requiring fine-grained syntactic control.

### **LCM Architecture: Thinking in Concepts**

LCMs represent a new generation of AI architecture that moves away from word-by-word prediction and begins to think in
full ideas. Unlike LLMs, which rely on token-based generation and sequential text prediction, LCMs embrace an
abstraction-first approach that processes meaning at the sentence or concept level.

At the heart of LCMs is a novel three-part architecture that transforms raw language into semantically rich embeddings,
performs reasoning over abstract concepts, and then reconstructs language — sometimes in entirely new forms.

### **1. Concept Encoder (Input Layer)**

LCMs begin by converting an input sentence — or any other modality — into a concept embedding, a high-dimensional vector
that captures the semantic essence of the entire input. This process is powered by **SONAR**, the multilingual and
multimodal embedding engine that enables sentence-level understanding across **200+ languages** and **76 spoken
dialects**.

Example:
* “Tim wasn’t athletic.” → Vector representing “personal limitation”*

### **2. Core LCM (Reasoning Layer)**

This is where traditional LLMs and LCMs diverge.

Instead of processing tokens sequentially, LCMs use:
* **Transformer-based decoders**,
* **Diffusion-based inference**, or
* **Quantised vector manipulation**

to reason **between concepts**, not tokens. This means the model understands and predicts entire thoughts, often across
multiple sentences. It operates in a **hyperbolic probabilistic space**, allowing for nuanced, non-linear semantic
relationships — similar to how human cognition handles ambiguity, planning, and abstraction.

It’s like putting [Schrödinger’s cat][17] in a semantic box — where the model reasons over what the concept might mean
before opening the box to generate language.

### **3. Concept Decoder (Output Layer)**

Once the LCM predicts the next concept in the sequence (e.g., the next sentence), it translates that concept back into
natural language — again via SONAR. Because SONAR is language-agnostic, the same idea can be rendered in multiple
languages or formats, enabling multilingual and multimodal outputs.

## **Real-World Use Cases for Large Concept Models (LCMs)**

LCMs unlock a new frontier in AI by reasoning at the **conceptual level** rather than just processing sequences of
tokens. By operating on high-dimensional representations of meaning — rather than language-specific syntax — LCMs
provide broader flexibility, stronger generalisation, and deeper semantic understanding. This enables a wide array of
**cross-lingual, cross-modal, and high-level cognitive applications** that are difficult or inefficient for LLMs to
handle.

### **Healthcare & Life Sciences**
* **Symptom-to-diagnosis mapping** with concept-level reasoning improves diagnostic accuracy.
* **Treatment pathway optimisation** based on drug interaction networks and patient history.
* Transparent AI systems that explain clinical recommendations using structured medical ontologies.
* Accelerating research (e.g., protein folding) by abstracting knowledge across scientific disciplines, as seen in
  [AlphaFold][18].

### **Finance & Economics**
* **Regulatory compliance auditing** using explainable, traceable logic structures.
* **Fraud detection** via semantic pattern recognition across structured transaction data.
* Real-time **economic trend analysis** connects macroeconomic indicators, news events, and internal data into unified
  concepts.

### **Legal Systems**
* **Case reasoning** across jurisdictions using precedent-case-law mapping.
* **Contract analysis** is powered by concept-grounded AI that understands legal logic, obligations, and clause
  relationships.
* **Cross-lingual legal comparison** of statutes and rulings with shared semantic interpretation.

### **Multilingual AI & Conversational Systems**
* **Cross-lingual dialogue agents** that reason in one language and respond in another.
* **Multilingual summarisation**: one abstract summary rendered into multiple target languages.
* **Language-agnostic virtual assistants** can switch between languages mid-conversation while maintaining coherence and
  context.
* **Edge AI**: reasoning is done once at the concept level, with low-latency decoding into different languages or
  formats locally.

### **Search, Retrieval & Semantic Understanding**
* **Semantic search** retrieves relevant information based on meaning — not keyword matches.
* **Intent and sentiment detection** in complex, nuanced statements.
* **Knowledge aggregation** across languages and formats into unified concept clusters.

### **Education & Tutoring Systems**
* **Concept-based explanation** engines that tailor responses based on semantic understanding of student input.
* **Multilingual education**: abstract once, teach in many languages.
* **Personalised tutoring** that understands long-form questions and adapts feedback to learning intent.

### **Enterprise, Content, and Creativity**
* **Enterprise document generation**: turn bullet points into coherent reports.
* **Content summarisation and transformation**: reduce complex documents to digestible summaries and regenerate them in
  different formats.
* **Creative tools**: assist in writing fiction, music, or screenplays with thematic consistency.
* **Short-form creative writing**: Summarising long prompts into concept-driven taglines or descriptions.

### **Cross-Modal & Scientific Reasoning**
* **Multimodal assistants**: combine voice, image, and text inputs to derive shared conceptual outputs.
* **Scientific discovery tools**: reason across disciplines (e.g., genomics, ecology, and climate models) to propose
  hypotheses.
* **Engineering systems**: understand interdependencies in complex domains like power grids or aerospace.

### **Social, Cultural, and Accessibility Applications**
* **Global customer support**: consistent understanding across 200+ languages using shared concept embeddings.
* **Social media moderation**: faster and fairer classification based on conceptual meaning rather than surface
  phrasing.
* **Cross-cultural AI**: bridge idioms, metaphors, and tone to ensure culturally aware interactions.

## **Conclusion**

The emergence of LCMs signals more than just the next iteration of AI architecture — it represents a paradigm shift in
how machines understand and generate language. Where LLMs revolutionised natural language processing by generating
human-like text at the token level, LCMs go further — thinking in ideas instead of words, planning before predicting and
operating in a shared semantic space that mirrors human cognition.

By leveraging sentence-level embeddings, explicit reasoning mechanisms like Local Prior Matching** (**LPM), and
language-agnostic encoders like SONAR, LCMs are designed to generate fluent responses and reason, contextualise, and
generalise across languages, tasks, and modalities. Whether it’s multilingual summarisation, scientific discovery, or
legal reasoning, LCMs promise to bring more profound understanding and richer interaction to the AI systems of the
future.

Challenges remain — from defining and disambiguating abstract concepts to handling the computational demands of
concept-space training. Interpretability, model access, and new evaluation metrics will also require continued
innovation. But the trajectory is clear: LCMs mark the beginning of concept-driven AI, where understanding precedes
expression.

This is no longer about predicting the next token. It’s about modelling** **human thought — capturing the nuance,
hierarchy, and intent that traditional models often miss. As the AI field evolves, LCMs may not just complement LLMs;
they could redefine the very foundation of machine intelligence.

In short:
**LLMs speak the language. LCMs understand the meaning.**
Together, they may shape a future where machines don’t just communicate — but **reason**.

## Co-author
* [**Morgan Lee** — Data / Software Engineer | Former Lawyer][19]

## References

You want to learn more about Large Concept Models (LCMs) and their challenges and benefits. Please refer to the links
below for more content on mastering the core concepts. We spent a great deal of time trying to make sense of the complex
topics; the best we could do was to distil the concepts to the layman:
* [Large Concept Models: Language Modelling in a Sentence Representation Space][20]
* [Meta Large Concept Models (LCM): End of LLMs?][21]
* [LLM vs LCM: The AI Revolution You Didn’t See Coming][22]
* [LCM vs LLM][23]
* [AI Is Breaking Free Of Token-Based LLMs By Upping The Ante To Large Concept Models That Devour Sentences And Adore
  Concepts][24]
* [LCM vs. LLM][25]
* [Large Concept Model (LCM): will it replace traditional LLM?][26]
* [LLMs vs. SLMs: The Differences in Large & Small Language Models][27]
* [LCM: Large Concept Model][28]
* [Meta Launches ‘Large Concept Models’ (LCMs)! Breaking Through LLM Limitations and Leading a New Direction in AI
  Language Understanding][29]
* [LCMs vs LLMs][30]
* [Meta’s new LLM architecture: Large Concept Models][31]
* [Large Language Models: A Survey][32]
* [A Survey of Large Language Models][33]

[
AI
][34]
[
Technology
][35]
[
ChatGPT
][36]
[
Machine Learning
][37]
[
Data Science
][38]
[
][39]

--

[
][40]

--

[
][41]
[][42]
[
[Ernese Norelus]
][43]
[
[Ernese Norelus]
][44]
[

## Written by Ernese Norelus

][45]
[948 followers][46]
·[58 following][47]

Ernese is responsible for providing technical oversight to Cloud client projects!

[

Help

][48]
[

Status

][49]
[

About

][50]
[

Careers

][51]
[

Press

][52]
[

Blog

][53]
[

Store

][54]
[

Privacy

][55]
[

Rules

][56]
[

Terms

][57]
[

Text to speech

][58]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-
lcms-rethinking-the-future-of-ai-8bb5b268c78d&source=post_page---top_nav_layout_nav-----------------------global_nav----
--------------
[4]: https://medium.com/m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layo
ut_nav-----------------------new_post_topnav------------------
[5]: https://medium.com/search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-
lcms-rethinking-the-future-of-ai-8bb5b268c78d&source=post_page---top_nav_layout_nav-----------------------global_nav----
--------------
[7]: /?source=post_page---byline--8bb5b268c78d---------------------------------------
[8]: /?source=post_page---byline--8bb5b268c78d---------------------------------------
[9]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F8bb5b268c78d&operation=register&red
irect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d&user=E
rnese+Norelus&userId=fb1419663342&source=---header_actions--8bb5b268c78d---------------------clap_footer----------------
--
[10]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F8bb5b268c78d&operation=register&
redirect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d&use
r=Ernese+Norelus&userId=fb1419663342&source=---header_actions--8bb5b268c78d---------------------repost_header-----------
-------
[11]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F8bb5b268c78d&operation=registe
r&redirect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d&s
ource=---header_actions--8bb5b268c78d---------------------bookmark_footer------------------
[12]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D
8bb5b268c78d&operation=register&redirect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-t
he-future-of-ai-8bb5b268c78d&source=---header_actions--8bb5b268c78d---------------------post_audio_button---------------
---
[13]: https://www.linkedin.com/in/leadgenmanthan/
[14]: https://www.meta.com/
[15]: https://arxiv.org/pdf/2412.08821
[16]: https://www.linkedin.com/posts/rakeshgohel01_large-concept-models-can-be-the-next-powerful-activity-72805914390743
40864-cMyW/?utm_source=share&utm_medium=member_desktop&rcm=ACoAAAGro68BcnuJgf0RKVqEdZT7tEObiFPbbv4
[17]: https://en.wikipedia.org/wiki/Schr%C3%B6dinger%27s_cat
[18]: https://deepmind.google/technologies/alphafold/
[19]: https://www.linkedin.com/in/morganjlee1/
[20]: https://arxiv.org/pdf/2412.08821
[21]: https://medium.com/data-science-in-your-pocket/meta-large-concept-models-lcm-end-of-llms-68cb0c5cd5cf
[22]: https://pub.towardsai.net/llm-vs-lcm-the-ai-revolution-you-didnt-see-coming-384cc80ba382
[23]: https://prezi.com/p/wramtwa_7s4a/lcm-vs-llm/
[24]: https://www.forbes.com/sites/lanceeliot/2025/01/06/ai-is-breaking-free-of-token-based-llms-by-upping-the-ante-to-l
arge-concept-models-that-devour-sentences-and-adore-concepts/
[25]: https://dev.to/mehmetakar/lcm-vs-llm-20kk
[26]: https://ai.plainenglish.io/large-concept-model-lcm-will-it-replace-traditional-llm-3e7b90dc5d15
[27]: https://www.splunk.com/en_us/blog/learn/language-models-slm-vs-llm.html
[28]: https://gonzoml.substack.com/p/lcm-large-concept-model
[29]: https://www.aibase.com/news/13985
[30]: https://www.amandeep.org/blog/lcms
[31]: https://wandb.ai/byyoung3/ml-news/reports/Meta-s-new-LLM-architecture-Large-Concept-Models---VmlldzoxMDc4Mzk4Mw
[32]: https://arxiv.org/pdf/2402.06196
[33]: https://arxiv.org/pdf/2303.18223
[34]: https://medium.com/tag/ai?source=post_page-----8bb5b268c78d---------------------------------------
[35]: https://medium.com/tag/technology?source=post_page-----8bb5b268c78d---------------------------------------
[36]: https://medium.com/tag/chatgpt?source=post_page-----8bb5b268c78d---------------------------------------
[37]: https://medium.com/tag/machine-learning?source=post_page-----8bb5b268c78d---------------------------------------
[38]: https://medium.com/tag/data-science?source=post_page-----8bb5b268c78d---------------------------------------
[39]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F8bb5b268c78d&operation=register&re
direct=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d&user=
Ernese+Norelus&userId=fb1419663342&source=---footer_actions--8bb5b268c78d---------------------clap_footer---------------
---
[40]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F8bb5b268c78d&operation=register&re
direct=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d&user=
Ernese+Norelus&userId=fb1419663342&source=---footer_actions--8bb5b268c78d---------------------clap_footer---------------
---
[41]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F8bb5b268c78d&operation=register&
redirect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d&use
r=Ernese+Norelus&userId=fb1419663342&source=---footer_actions--8bb5b268c78d---------------------repost_footer-----------
-------
[42]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F8bb5b268c78d&operation=registe
r&redirect=https%3A%2F%2Fernesenorelus.medium.com%2Flarge-concept-models-lcms-rethinking-the-future-of-ai-8bb5b268c78d&s
ource=---footer_actions--8bb5b268c78d---------------------bookmark_footer------------------
[43]: /?source=post_page---post_author_info--8bb5b268c78d---------------------------------------
[44]: /?source=post_page---post_author_info--8bb5b268c78d---------------------------------------
[45]: /?source=post_page---post_author_info--8bb5b268c78d---------------------------------------
[46]: /followers?source=post_page---post_author_info--8bb5b268c78d---------------------------------------
[47]: /following?source=post_page---post_author_info--8bb5b268c78d---------------------------------------
[48]: https://help.medium.com/hc/en-us?source=post_page-----8bb5b268c78d---------------------------------------
[49]: https://status.medium.com/?source=post_page-----8bb5b268c78d---------------------------------------
[50]: https://medium.com/about?autoplay=1&source=post_page-----8bb5b268c78d---------------------------------------
[51]: https://medium.com/jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----8bb5b268c78d-------------------
--------------------
[52]: mailto:pressinquiries@medium.com
[53]: https://blog.medium.com/?source=post_page-----8bb5b268c78d---------------------------------------
[54]: https://medium.com/store
[55]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----8bb5b268c78d--------------------
-------------------
[56]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----8bb5b268c78d-----------------------------
----------
[57]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----8bb5b268c78d------------------
---------------------
[58]: https://speechify.com/medium?source=post_page-----8bb5b268c78d---------------------------------------
```
