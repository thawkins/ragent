# Web source

- URL: https://huggingface.co/papers?q=Long-context+ASR
- Title: [[Hugging Face's logo] Hugging Face][1]
- Captured (UTC): 2026-06-29T16:31:22.135762458+00:00

```text
[[Hugging Face's logo] Hugging Face][1]
* [ Models ][2]
* [ Datasets ][3]
* [ Spaces ][4]
* [ Buckets new][5]
* [ Docs ][6]
* [ Enterprise ][7]
* [Pricing][8]
* * Website
    * [ Tasks][9]
    * [ HuggingChat][10]
    * [ Collections][11]
    * [ Languages][12]
    * [ Organizations][13]
  * Community
    * [ Blog][14]
    * [ Posts][15]
    * [ Daily Papers][16]
    * [ Learn][17]
    * [ Discord][18]
    * [ Forum][19]
    * [ GitHub][20]
  * Solutions
    * [ Team & Enterprise][21]
    * [ Hugging Face PRO][22]
    * [ Enterprise Support][23]
    * [ Inference Providers][24]
    * [ Inference Endpoints][25]
    * [ Storage Buckets][26]
* [Log In][27]
* [Sign Up][28]
new

Get trending papers in your email inbox once a day!

Get trending papers in your email inbox!

[Subscribe][29]

# Daily Papers

## by[AK][30] and the research community
* Daily
* Weekly
* Monthly

Jun 29
[
-
][31]

### [MURMUR: An Efficient Inference System for Long-Form ASR][32]

[

Long-form automatic speech recognition (ASR) requires both high accuracy and low latency, but existing systems force a
trade-off between the two. Chunk-based pipelines process audio in parallel windows for low latency, but lose cross-chunk
context and need brittle heuristics to align speakers and timestamps at boundaries. Long-context ASR models resolve
everything in a single pass for better accuracy, but are an order of magnitude slower. We propose Murmur, an inference
system that overcomes this trade-off by operating at two levels. At the inter-chunk level, we revisit the chunk-based
pipeline for modern long-context ASR, treating chunk size as a tunable hyperparameter, and show that intermediate chunk
sizes strike a good balance of accuracy and latency. At the intra-chunk level, we exploit attention sparsity through a
sliding window KV cache eviction policy applied to both output and speech tokens. On AMI-IHM, Murmur matches single-pass
accuracy while reducing latency by 4.2x, with further gains from token eviction at less than 1% relative tcpWER
degradation. The code of Murmur is available at https://github.com/uw-syfi/Murmur.

][33]
[
* 3 authors
][34]
·
May 30
[
65
][35]

### [Gemini 1.5: Unlocking multimodal understanding across millions of tokens of context][36]

[

In this report, we present the latest model of the Gemini family, Gemini 1.5 Pro, a highly compute-efficient multimodal
mixture-of-experts model capable of recalling and reasoning over fine-grained information from millions of tokens of
context, including multiple long documents and hours of video and audio. Gemini 1.5 Pro achieves near-perfect recall on
long-context retrieval tasks across modalities, improves the state-of-the-art in long-document QA, long-video QA and
long-context ASR, and matches or surpasses Gemini 1.0 Ultra's state-of-the-art performance across a broad set of
benchmarks. Studying the limits of Gemini 1.5 Pro's long-context ability, we find continued improvement in next-token
prediction and near-perfect retrieval (>99%) up to at least 10M tokens, a generational leap over existing models such as
Claude 2.1 (200k) and GPT-4 Turbo (128k). Finally, we highlight surprising new capabilities of large language models at
the frontier; when given a grammar manual for Kalamang, a language with fewer than 200 speakers worldwide, the model
learns to translate English to Kalamang at a similar level to a person who learned from the same content.

][37]
[
* 671 authors
][38]
·
Mar 8, 2024 [ 6][39]
[
1
][40]

### [Speech-Aware Long Context Pruning and Integration for Contextualized Automatic Speech Recognition][41]

[

Automatic speech recognition (ASR) systems have achieved remarkable performance in common conditions but often struggle
to leverage long-context information in contextualized scenarios that require domain-specific knowledge, such as
conference presentations. This challenge arises primarily due to constrained model context windows and the sparsity of
relevant information within extensive contextual noise. To solve this, we propose the SAP^{2} method, a novel framework
that dynamically prunes and integrates relevant contextual keywords in two stages. Specifically, each stage leverages
our proposed Speech-Driven Attention-based Pooling mechanism, enabling efficient compression of context embeddings while
preserving speech-salient information. Experimental results demonstrate state-of-the-art performance of SAP^{2} on the
SlideSpeech and LibriSpeech datasets, achieving word error rates (WER) of 7.71% and 1.12%, respectively. On SlideSpeech,
our method notably reduces biased keyword error rates (B-WER) by 41.1% compared to non-contextual baselines. SAP^{2}
also exhibits robust scalability, consistently maintaining performance under extensive contextual input conditions on
both datasets.

][42]
[
* 8 authors
][43]
·
Nov 14, 2025
[
-
][44]

### [MLMA: Towards Multilingual with Mamba Based Architectures][45]

[

Multilingual automatic speech recognition (ASR) remains a challenging task, especially when balancing performance across
high- and low-resource languages. Recent advances in sequence modeling suggest that architectures beyond Transformers
may offer better scalability and efficiency. In this work, we introduce MLMA (Multilingual Language Modeling with Mamba
for ASR), a new approach that leverages the Mamba architecture -- an efficient state-space model optimized for
long-context sequence processing -- for multilingual ASR. Using Mamba, MLMA implicitly incorporates language-aware
conditioning and shared representations to support robust recognition across diverse languages. Experiments on standard
multilingual benchmarks show that MLMA achieves competitive performance compared to Transformer-based architectures.
These results highlight Mamba's potential as a strong backbone for scalable, efficient, and accurate multilingual speech
recognition.

][46]
[
* 3 authors
][47]
·
Oct 21, 2025
[
-
][48]

### [Whispering Context: Distilling Syntax and Semantics for Long Speech Transcripts][49]

[

ASR systems often struggle with maintaining syntactic and semantic accuracy in long audio transcripts, impacting tasks
like Named Entity Recognition (NER), capitalization, and punctuation. We propose a novel approach that enhances ASR by
distilling contextual knowledge from LLaMA models into Whisper. Our method uses two strategies: (1) token level
distillation with optimal transport to align dimensions and sequence lengths, and (2) representation loss minimization
between sentence embeddings of Whisper and LLaMA, blending syntax and semantics. Evaluations on the Spoken Wikipedia
dataset, a benchmark with long audios and rich entities demonstrate significant improvements in Word Error Rate (WER),
NER, capitalization, and punctuation success. By introducing novel NER metrics and exploring semantics aware ASR, our
work highlights the value of integrating linguistic context into transcription, setting a foundation for robust,
context-aware ASR in longform speech.

][50]
[
* 1 authors
][51]
·
Aug 18, 2025
[
-
][52]

### [Mind the Gap: Entity-Preserved Context-Aware ASR Structured Transcriptions][53]

[

Automatic Speech Recognition (ASR) systems, such as Whisper, achieve high transcription accuracy but struggle with named
entities and numerical data, especially when proper formatting is required. These issues increase word error rate (WER)
and impair semantic understanding in critical domains like legal, financial, and medical applications. We propose a
novel training approach that extends the semantic context of ASR models by adding overlapping context windows during
training. By sliding 5-second overlaps on both sides of 30-second chunks, we create a 40-second "effective semantic
window," improving entity recognition and formatting while focusing predictions on the central 30 seconds. To address
entities spanning chunk boundaries, we reassign such entities entirely to the right-hand chunk, ensuring proper
formatting. Additionally, enriched training data with embedded entity labels enables the model to learn both recognition
and type-specific formatting. Evaluated on the Spoken Wikipedia dataset, our method improves performance across semantic
tasks, including named entity recognition (NER) and entity formatting. These results highlight the effectiveness of
context-aware training in addressing ASR limitations for long-form transcription and complex entity recognition tasks.

][54]
[
* 1 authors
][55]
·
Jun 28, 2025
[
1
][56]

### [Libriheavy: a 50,000 hours ASR corpus with punctuation casing and context][57]

[

In this paper, we introduce Libriheavy, a large-scale ASR corpus consisting of 50,000 hours of read English speech
derived from LibriVox. To the best of our knowledge, Libriheavy is the largest freely-available corpus of speech with
supervisions. Different from other open-sourced datasets that only provide normalized transcriptions, Libriheavy
contains richer information such as punctuation, casing and text context, which brings more flexibility for system
building. Specifically, we propose a general and efficient pipeline to locate, align and segment the audios in
previously published Librilight to its corresponding texts. The same as Librilight, Libriheavy also has three training
subsets small, medium, large of the sizes 500h, 5000h, 50000h respectively. We also extract the dev and test evaluation
sets from the aligned audios and guarantee there is no overlapping speakers and books in training sets. Baseline systems
are built on the popular CTC-Attention and transducer models. Additionally, we open-source our dataset creatation
pipeline which can also be used to other audio alignment tasks.

][58]
[
* 8 authors
][59]
·
Sep 14, 2023
[
-
][60]

### [PromptASR for contextualized ASR with controllable style][61]

[

Prompts are crucial to large language models as they provide context information such as topic or logical relationships.
Inspired by this, we propose PromptASR, a framework that integrates prompts in end-to-end automatic speech recognition
(E2E ASR) systems to achieve contextualized ASR with controllable style of transcriptions. Specifically, a dedicated
text encoder encodes the text prompts and the encodings are injected into the speech encoder by cross-attending the
features from two modalities. When using the ground truth text from preceding utterances as content prompt, the proposed
system achieves 21.9% and 6.8% relative word error rate reductions on a book reading dataset and an in-house dataset
compared to a baseline ASR system. The system can also take word-level biasing lists as prompt to improve recognition
accuracy on rare words. An additional style prompt can be given to the text encoder and guide the ASR system to output
different styles of transcriptions. The code is available at icefall.

][62]
[
* 8 authors
][63]
·
Sep 13, 2023
[
24
][64]

### [VIBEVOICE-ASR Technical Report][65]

[

This report presents VibeVoice-ASR, a general-purpose speech understanding framework built upon VibeVoice, designed to
address the persistent challenges of context fragmentation and multi-speaker complexity in long-form audio (e.g.,
meetings, podcasts) that remain despite recent advancements in short-form speech recognition. Unlike traditional
pipelined approaches that rely on audio chunking, VibeVoice-ASRsupports single-pass processing for up to 60 minutes of
audio. It unifies Automatic Speech Recognition, Speaker Diarization, and Timestamping into a single end-to-end
generation task. In addition, VibeVoice-ASR supports over 50 languages, requires no explicit language setting, and
natively handles code-switching within and across utterances. Furthermore, we introduce a prompt-based context injection
mechanism that allows users to supply customized conetxt, significantly improving accuracy on domain-specific
terminology and polyphonic character disambiguation.

][66]
[[microsoft] Microsoft][67]
·
Mar 13 [ 3][68]
[
1
][69]

### [ChunkFormer: Masked Chunking Conformer For Long-Form Speech Transcription][70]

[

Deploying ASR models at an industrial scale poses significant challenges in hardware resource management, especially for
long-form transcription tasks where audio may last for hours. Large Conformer models, despite their capabilities, are
limited to processing only 15 minutes of audio on an 80GB GPU. Furthermore, variable input lengths worsen
inefficiencies, as standard batching leads to excessive padding, increasing resource consumption and execution time. To
address this, we introduce ChunkFormer, an efficient ASR model that uses chunk-wise processing with relative right
context, enabling long audio transcriptions on low-memory GPUs. ChunkFormer handles up to 16 hours of audio on an 80GB
GPU, 1.5x longer than the current state-of-the-art FastConformer, while also boosting long-form transcription
performance with up to 7.7% absolute reduction on word error rate and maintaining accuracy on shorter tasks compared to
Conformer. By eliminating the need for padding in standard batching, ChunkFormer's masked batching technique reduces
execution time and memory usage by more than 3x in batch processing, substantially reducing costs for a wide range of
ASR systems, particularly regarding GPU resources for models serving in real-world applications.

][71]
[
* 4 authors
][72]
·
Feb 20, 2025
[
49
][73]

### [StepAudio 2.5 Technical Report][74]

[

Unified audio-language modeling has emerged as a prominent trend in modern speech systems, promising to bring the
reasoning capabilities of large language models to auditory tasks. However, existing unified foundations often struggle
to match the depth of specialized systems across automatic speech recognition (ASR), text-to-speech synthesis (TTS), and
realtime spoken interaction. Bridging this gap remains an open challenge. This report presents StepAudio 2.5, a unified
audio-language foundation model that matches or exceeds specialized systems across all three capabilities. Rather than
treating these tasks as architecturally distinct, we operate on the premise that once text and audio share a multimodal
representational space, task specialization becomes a matter of operational regimes: data construction, optimization
targets, and decoding constraints. Guided by this insight, we advance the post-training paradigm from standard
supervised learning to task-tailored Reinforcement Learning from Human Feedback (RLHF), using it as the primary
mechanism to define complex optimization targets. We leverage this RLHF-centric alignment, alongside specialized
decoding, to shape a shared backbone into three distinct operational modes. Concretely, the ASR branch advances
transcription efficiency via verifiable multi-token decoding; the TTS branch achieves controllable, expressive synthesis
through preference-based RLHF and context-rich supervision; and the Realtime branch realizes low-latency,
persona-consistent dialogue via generative reward modeling within an RLHF framework. On standard benchmarks, StepAudio
2.5 achieves state-of-the-art results across ASR, TTS, and Realtime, demonstrating that a singular audio-language
foundation can successfully internalize the distinct deployment objectives of speech understanding, generation, and live
interaction.

][75]
[
* 101 authors
][76]
·
May 21 [ 2][77]
[
1
][78]

### [Blending LLMs into Cascaded Speech Translation: KIT's Offline Speech Translation System for IWSLT 2024][79]

[

Large Language Models (LLMs) are currently under exploration for various tasks, including Automatic Speech Recognition
(ASR), Machine Translation (MT), and even End-to-End Speech Translation (ST). In this paper, we present KIT's offline
submission in the constrained + LLM track by incorporating recently proposed techniques that can be added to any
cascaded speech translation. Specifically, we integrate Mistral-7Bmistralai/Mistral-7B-Instruct-v0.1 into our system to
enhance it in two ways. Firstly, we refine the ASR outputs by utilizing the N-best lists generated by our system and
fine-tuning the LLM to predict the transcript accurately. Secondly, we refine the MT outputs at the document level by
fine-tuning the LLM, leveraging both ASR and MT predictions to improve translation quality. We find that integrating the
LLM into the ASR and MT systems results in an absolute improvement of 0.3% in Word Error Rate and 0.65% in COMET for
tst2019 test set. In challenging test sets with overlapping speakers and background noise, we find that integrating LLM
is not beneficial due to poor ASR performance. Here, we use ASR with chunked long-form decoding to improve context usage
that may be unavailable when transcribing with Voice Activity Detection segmentation alone.

][80]
[
* 7 authors
][81]
·
Jun 24, 2024
[
1
][82]

### [LongSpeech: A Scalable Benchmark for Transcription, Translation and Understanding in Long Speech][83]

[

Recent advances in audio-language models have demonstrated remarkable success on short, segment-level speech tasks.
However, real-world applications such as meeting transcription, spoken document understanding, and conversational
analysis require robust models capable of processing and reasoning over long-form audio. In this work, we present
LongSpeech, a large-scale and scalable benchmark specifically designed to evaluate and advance the capabilities of
speech models on long-duration audio. LongSpeech comprises over 100,000 speech segments, each approximately 10 minutes
long, with rich annotations for ASR, speech translation, summarization, language detection, speaker counting, content
separation, and question answering. We introduce a reproducible pipeline for constructing long-form speech benchmarks
from diverse sources, enabling future extensions. Our initial experiments with state-of-the-art models reveal
significant performance gaps, with models often specializing in one task at the expense of others and struggling with
higher-level reasoning. These findings underscore the challenging nature of our benchmark. Our benchmark will be made
publicly available to the research community.

][84]
[
* 10 authors
][85]
·
Jan 19
[
1
][86]

### [ContextASR-Bench: A Massive Contextual Speech Recognition Benchmark][87]

[

Automatic Speech Recognition (ASR) has been extensively investigated, yet prior evaluative efforts have largely been
restricted to contextless paradigms. This constraint stems from the limited proficiency of conventional ASR models in
context modeling and their deficiency in memory and reasoning based on world knowledge. Recent breakthroughs in the
development of Large Language Models (LLMs) and corresponding Large Audio Language Models (LALMs) have markedly enhanced
the visibility of general artificial intelligence capabilities. Consequently, there exists a compelling need for a
benchmark that can evaluate both the generality and intelligence of ASR systems. To address this gap, we propose
ContextASR-Bench: a comprehensive, large-scale benchmark designed to assess contextual speech recognition. This
benchmark encompasses up to 40,000 data entries across over 10 domains, enabling a thorough evaluation of model
performance in scenarios that omit or incorporate coarse-grained or fine-grained contextual information. Moreover,
diverging from conventional ASR evaluations, our benchmark includes an analysis of model efficacy in recognizing named
entities mentioned within the auditory input. Our extensive evaluation highlights that LALMs, with strong world
knowledge and context learning capabilities, outperform conventional ASR models by a large margin. The dataset and
evaluation code have been released at https://github.com/MrSupW/ContextASR-Bench.

][88]
[
* 7 authors
][89]
·
Jul 8, 2025
[
2
][90]

### [How to Train Long-Context Language Models (Effectively)][91]

[

We study continued training and supervised fine-tuning (SFT) of a language model (LM) to make effective use of
long-context information. We first establish a reliable evaluation protocol to guide model development -- Instead of
perplexity or simple needle-in-a-haystack (NIAH) tests, we use a broad set of long-context tasks, and we evaluate models
after SFT with instruction data as this better reveals long-context abilities. Supported by our robust evaluations, we
run thorough experiments to decide the data mix for continued pre-training, the instruction tuning dataset, and many
other design choices. We find that (1) code repositories and books are excellent sources of long data, but it is crucial
to combine them with high-quality short data; (2) training with a sequence length beyond the evaluation length boosts
long-context performance; (3) for SFT, using only short instruction datasets yields strong performance on long-context
tasks. Our final model, ProLong-8B, which is initialized from Llama-3 and trained on 40B tokens, demonstrates
state-of-the-art long-context performance among similarly sized models at a length of 128K. ProLong outperforms
Llama-3.18B-Instruct on the majority of long-context tasks despite having seen only 5% as many tokens during
long-context training. Additionally, ProLong can effectively process up to 512K tokens, one of the longest context
windows of publicly available LMs.

][92]
[
* 4 authors
][93]
·
Oct 3, 2024 [ 1][94]
[
2
][95]

### [LongSkywork: A Training Recipe for Efficiently Extending Context Length in Large Language Models][96]

[

We introduce LongSkywork, a long-context Large Language Model (LLM) capable of processing up to 200,000 tokens. We
provide a training recipe for efficiently extending context length of LLMs. We identify that the critical element in
enhancing long-context processing capability is to incorporate a long-context SFT stage following the standard SFT
stage. A mere 200 iterations can convert the standard SFT model into a long-context model. To reduce the effort in
collecting and annotating data for long-context language modeling, we develop two novel methods for creating synthetic
data. These methods are applied during the continual pretraining phase as well as the Supervised Fine-Tuning (SFT)
phase, greatly enhancing the training efficiency of our long-context LLMs. Our findings suggest that synthetic
long-context SFT data can surpass the performance of data curated by humans to some extent. LongSkywork achieves
outstanding performance on a variety of long-context benchmarks. In the Needle test, a benchmark for long-context
information retrieval, our models achieved perfect accuracy across multiple context spans. Moreover, in realistic
application scenarios, LongSkywork-13B demonstrates performance on par with Claude2.1, the leading long-context model,
underscoring the effectiveness of our proposed methods.

][97]
[
* 15 authors
][98]
·
Jun 1, 2024 [ 2][99]
[
73
][100]

### [Thus Spake Long-Context Large Language Model][101]

[

Long context is an important topic in Natural Language Processing (NLP), running through the development of NLP
architectures, and offers immense opportunities for Large Language Models (LLMs) giving LLMs the lifelong learning
potential akin to humans. Unfortunately, the pursuit of a long context is accompanied by numerous obstacles.
Nevertheless, long context remains a core competitive advantage for LLMs. In the past two years, the context length of
LLMs has achieved a breakthrough extension to millions of tokens. Moreover, the research on long-context LLMs has
expanded from length extrapolation to a comprehensive focus on architecture, infrastructure, training, and evaluation
technologies. Inspired by the symphonic poem, Thus Spake Zarathustra, we draw an analogy between the journey of
extending the context of LLM and the attempts of humans to transcend its mortality. In this survey, We will illustrate
how LLM struggles between the tremendous need for a longer context and its equal need to accept the fact that it is
ultimately finite. To achieve this, we give a global picture of the lifecycle of long-context LLMs from four
perspectives: architecture, infrastructure, training, and evaluation, showcasing the full spectrum of long-context
technologies. At the end of this survey, we will present 10 unanswered questions currently faced by long-context LLMs.
We hope this survey can serve as a systematic introduction to the research on long-context LLMs.

][102]
[
* 13 authors
][103]
·
Feb 24, 2025 [ 6][104]
[
-
][105]

### [KV Cache Compression, But What Must We Give in Return? A Comprehensive Benchmark of Long Context Capable
### Approaches][106]

[

Long context capability is a crucial competency for large language models (LLMs) as it mitigates the human struggle to
digest long-form texts. This capability enables complex task-solving scenarios such as book summarization, code
assistance, and many more tasks that are traditionally manpower-intensive. However, transformer-based LLMs face
significant challenges with long context input due to the growing size of the KV cache and the intrinsic complexity of
attending to extended inputs; where multiple schools of efficiency-driven approaches -- such as KV cache quantization,
token dropping, prompt compression, linear-time sequence models, and hybrid architectures -- have been proposed to
produce efficient yet long context-capable models. Despite these advancements, no existing work has comprehensively
benchmarked these methods in a reasonably aligned environment. In this work, we fill this gap by providing a taxonomy of
current methods and evaluating 10+ state-of-the-art approaches across seven categories of long context tasks. Our work
reveals numerous previously unknown phenomena and offers insights -- as well as a friendly workbench -- for the future
development of long context-capable LLMs. The source code will be available at
https://github.com/henryzhongsc/longctx_bench

][107]
[
* 13 authors
][108]
·
Jul 1, 2024
[
1
][109]

### [inftyBench: Extending Long Context Evaluation Beyond 100K Tokens][110]

[

Processing and reasoning over long contexts is crucial for many practical applications of Large Language Models (LLMs),
such as document comprehension and agent construction. Despite recent strides in making LLMs process contexts with more
than 100K tokens, there is currently a lack of a standardized benchmark to evaluate this long-context capability.
Existing public benchmarks typically focus on contexts around 10K tokens, limiting the assessment and comparison of LLMs
in processing longer contexts. In this paper, we propose inftyBench, the first LLM benchmark featuring an average data
length surpassing 100K tokens. inftyBench comprises synthetic and realistic tasks spanning diverse domains, presented in
both English and Chinese. The tasks in inftyBench are designed to require well understanding of long dependencies in
contexts, and make simply retrieving a limited number of passages from contexts not sufficient for these tasks. In our
experiments, based on inftyBench, we evaluate the state-of-the-art proprietary and open-source LLMs tailored for
processing long contexts. The results indicate that existing long context LLMs still require significant advancements to
effectively process 100K+ context. We further present three intriguing analyses regarding the behavior of LLMs
processing long context.

][111]
[
* 11 authors
][112]
·
Feb 21, 2024 [ 2][113]
[
-
][114]

### [What is Wrong with Perplexity for Long-context Language Modeling?][115]

[

Handling long-context inputs is crucial for large language models (LLMs) in tasks such as extended conversations,
document summarization, and many-shot in-context learning. While recent approaches have extended the context windows of
LLMs and employed perplexity (PPL) as a standard evaluation metric, PPL has proven unreliable for assessing long-context
capabilities. The underlying cause of this limitation has remained unclear. In this work, we provide a comprehensive
explanation for this issue. We find that PPL overlooks key tokens, which are essential for long-context understanding,
by averaging across all tokens and thereby obscuring the true performance of models in long-context scenarios. To
address this, we propose LongPPL, a novel metric that focuses on key tokens by employing a long-short context
contrastive method to identify them. Our experiments demonstrate that LongPPL strongly correlates with performance on
various long-context benchmarks (e.g., Pearson correlation of -0.96), significantly outperforming traditional PPL in
predictive accuracy. Additionally, we introduce LongCE (Long-context Cross-Entropy) loss, a re-weighting strategy for
fine-tuning that prioritizes key tokens, leading to consistent improvements across diverse benchmarks. In summary, these
contributions offer deeper insights into the limitations of PPL and present effective solutions for accurately
evaluating and enhancing the long-context capabilities of LLMs. Code is available at https://github.com/PKU-ML/LongPPL.

][116]
[
* 8 authors
][117]
·
Jul 26, 2025
[
5
][118]

### [L-Eval: Instituting Standardized Evaluation for Long Context Language Models][119]

[

Recently, there has been growing interest in extending the context length of instruction-following models in order to
effectively process single-turn long input (e.g. summarizing a paper) and conversations with more extensive histories.
While proprietary models such as GPT-4 and Claude have demonstrated considerable advancements in handling tens of
thousands of tokens of context, open-sourced models are still in the early stages of experimentation. It also remains
unclear whether developing these long context models can offer substantial gains on practical downstream tasks over
retrieval-based methods or models simply trained on chunked contexts. To address this challenge, we propose to institute
standardized evaluation for long context language models. Concretely, we develop L-Eval which contains 411 long
documents and over 2,000 query-response pairs manually annotated and checked by the authors encompassing areas such as
law, finance, school lectures, lengthy conversations, news, long-form novels, and meetings. L-Eval also adopts diverse
evaluation methods and instruction styles, enabling a more reliable assessment of Long Context Language Models (LCLMs).
Our findings indicate that while open-source models typically lag behind their commercial counterparts, they still
exhibit impressive performance. LLaMA2 achieves the best results (win 45\% vs turbo-16k) on open-ended tasks with only
4k context length and ChatGLM2 achieves the best results on closed-ended tasks with 8k input tokens. We release our new
evaluation suite, code, and all generation results including predictions from all open-sourced LCLMs, GPT4-32k,
Cluade-100k at {https://github.com/OpenLMLab/LEval}.

][120]
[
* 7 authors
][121]
·
Jul 20, 2023
[
89
][122]

### [Training Long-Context Vision-Language Models Effectively with Generalization Beyond 128K Context][123]

[

Long-context modeling is becoming a core capability of modern large vision-language models (LVLMs), enabling sustained
context management across long-document understanding, video analysis, and multi-turn tool use in agentic workflows. Yet
practical training recipes remain insufficiently explored, particularly for designing and balancing long-context data
mixtures. In this work, we present a systematic study of long-context continued pre-training for LVLMs, extending a 7B
model from 32K to 128K context with extensive ablations on long-document data. We first show that long-document VQA is
substantially more effective than OCR transcription. Building on this observation, our ablations further yield three key
findings: i) for sequence-length distribution, balanced data outperforms target-length-focused data (e.g., 128K),
suggesting that long-context ability requires generalizable key-information retrieval across various lengths and
positions; ii) retrieval remains the primary bottleneck, favoring retrieval-heavy mixtures with modest reasoning data
for task diversity; and iii) pure long-document VQA largely preserves short-context capabilities, suggesting that
instruction-formatted long data reduces the need for short-data mixing. Based on these findings, we introduce MMProLong,
obtained by long-context continued pre-training from Qwen2.5-VL-7B with only a 5B-token budget. MMProLong improves
long-document VQA scores by 7.1% and maintains strong performance at 256K and 512K contexts beyond its 128K training
window, without additional training. It further generalizes to webpage-based multimodal needle retrieval, long-context
vision-text compression, and long-video understanding without task-specific supervision. Overall, our study establishes
a practical LongPT recipe and an empirical foundation for advancing long-context vision-language models.

][124]
[[ByteDance-Seed] ByteDance Seed][125]
·
May 12 [ 3][126]
[
25
][127]

### [LongAlign: A Recipe for Long Context Alignment of Large Language Models][128]

[

Extending large language models to effectively handle long contexts requires instruction fine-tuning on input sequences
of similar length. To address this, we present LongAlign -- a recipe of the instruction data, training, and evaluation
for long context alignment. First, we construct a long instruction-following dataset using Self-Instruct. To ensure the
data diversity, it covers a broad range of tasks from various long context sources. Second, we adopt the packing and
sorted batching strategies to speed up supervised fine-tuning on data with varied length distributions. Additionally, we
develop a loss weighting method to balance the contribution to the loss across different sequences during packing
training. Third, we introduce the LongBench-Chat benchmark for evaluating instruction-following capabilities on queries
of 10k-100k in length. Experiments show that LongAlign outperforms existing recipes for LLMs in long context tasks by up
to 30\%, while also maintaining their proficiency in handling short, generic tasks. The code, data, and long-aligned
models are open-sourced at https://github.com/THUDM/LongAlign.

][129]
[
* 9 authors
][130]
·
Jan 31, 2024 [ 1][131]
[
2
][132]

### [Long Context is Not Long at All: A Prospector of Long-Dependency Data for Large Language Models][133]

[

Long-context modeling capabilities are important for large language models (LLMs) in various applications. However,
directly training LLMs with long context windows is insufficient to enhance this capability since some training samples
do not exhibit strong semantic dependencies across long contexts. In this study, we propose a data mining framework
ProLong that can assign each training sample with a long dependency score, which can be used to rank and filter samples
that are more advantageous for enhancing long-context modeling abilities in LLM training. Specifically, we first use
delta perplexity scores to measure the Dependency Strength between text segments in a given document. Then we refine
this metric based on the Dependency Distance of these segments to incorporate spatial relationships across
long-contexts. Final results are calibrated with a Dependency Specificity metric to prevent trivial dependencies
introduced by repetitive patterns. Moreover, a random sampling approach is proposed to optimize the computational
efficiency of ProLong. Comprehensive experiments on multiple benchmarks indicate that ProLong effectively identifies
documents that carry long dependencies and LLMs trained on these documents exhibit significantly enhanced long-context
modeling capabilities.

][134]
[
* 6 authors
][135]
·
May 28, 2024 [ 1][136]
[
49
][137]

### [A Comprehensive Survey on Long Context Language Modeling][138]

[

Efficient processing of long contexts has been a persistent pursuit in Natural Language Processing. With the growing
number of long documents, dialogues, and other textual data, it is important to develop Long Context Language Models
(LCLMs) that can process and analyze extensive inputs in an effective and efficient way. In this paper, we present a
comprehensive survey on recent advances in long-context modeling for large language models. Our survey is structured
around three key aspects: how to obtain effective and efficient LCLMs, how to train and deploy LCLMs efficiently, and
how to evaluate and analyze LCLMs comprehensively. For the first aspect, we discuss data strategies, architectural
designs, and workflow approaches oriented with long context processing. For the second aspect, we provide a detailed
examination of the infrastructure required for LCLM training and inference. For the third aspect, we present evaluation
paradigms for long-context comprehension and long-form generation, as well as behavioral analysis and mechanism
interpretability of LCLMs. Beyond these three key aspects, we thoroughly explore the diverse application scenarios where
existing LCLMs have been deployed and outline promising future development directions. This survey provides an
up-to-date review of the literature on long-context LLMs, which we wish to serve as a valuable resource for both
researchers and engineers. An associated GitHub repository collecting the latest papers and repos is available at:
https://github.com/LCLM-Horizon/A-Comprehensive-Survey-For-Long-Context-Language-Modeling{\color[RGB]{175,36,67}{LCLM-Ho
rizon}}.

][139]
[
* 37 authors
][140]
·
Mar 20, 2025 [ 2][141]
[
-
][142]

### [Index-ASR Technical Report][143]

[

Automatic speech recognition (ASR) has witnessed remarkable progress in recent years, largely driven by the emergence of
LLM-based ASR paradigm. Despite their strong performance on a variety of open-source benchmarks, existing LLM-based ASR
systems still suffer from two critical limitations. First, they are prone to hallucination errors, often generating
excessively long and repetitive outputs that are not well grounded in the acoustic input. Second, they provide limited
support for flexible and fine-grained contextual customization. To address these challenges, we propose Index-ASR, a
large-scale LLM-based ASR system designed to simultaneously enhance robustness and support customizable hotword
recognition. The core idea of Index-ASR lies in the integration of LLM and large-scale training data enriched with
background noise and contextual information. Experimental results show that our Index-ASR achieves strong performance on
both open-source benchmarks and in-house test sets, highlighting its robustness and practicality for real-world ASR
applications.

][144]
[
* 6 authors
][145]
·
Dec 31, 2025
[
7
][146]

### [Selecting Influential Samples for Long Context Alignment via Homologous Models' Guidance and Contextual Awareness
### Measurement][147]

[

The expansion of large language models to effectively handle instructions with extremely long contexts has yet to be
fully investigated. The primary obstacle lies in constructing a high-quality long instruction-following dataset devised
for long context alignment. Existing studies have attempted to scale up the available data volume by synthesizing long
instruction-following samples. However, indiscriminately increasing the quantity of data without a well-defined strategy
for ensuring data quality may introduce low-quality samples and restrict the final performance. To bridge this gap, we
aim to address the unique challenge of long-context alignment, i.e., modeling the long-range dependencies for handling
instructions and lengthy input contexts. We propose GATEAU, a novel framework designed to identify the influential and
high-quality samples enriched with long-range dependency relations by utilizing crafted Homologous Models' Guidance
(HMG) and Contextual Awareness Measurement (CAM). Specifically, HMG attempts to measure the difficulty of generating
corresponding responses due to the long-range dependencies, using the perplexity scores of the response from two
homologous models with different context windows. Also, the role of CAM is to measure the difficulty of understanding
the long input contexts due to long-range dependencies by evaluating whether the model's attention is focused on
important segments. Built upon both proposed methods, we select the most challenging samples as the influential data to
effectively frame the long-range dependencies, thereby achieving better performance of LLMs. Comprehensive experiments
indicate that GATEAU effectively identifies samples enriched with long-range dependency relations and the model trained
on these selected samples exhibits better instruction-following and long-context understanding capabilities.

][148]
[
* 10 authors
][149]
·
Oct 21, 2024 [ 3][150]
[
1
][151]

### [LongAttn: Selecting Long-context Training Data via Token-level Attention][152]

[

With the development of large language models (LLMs), there has been an increasing need for significant advancements in
handling long contexts. To enhance long-context capabilities, constructing high-quality training data with long-range
dependencies is crucial. Existing methods to select long-context data often rely on sentence-level analysis, which can
be greatly optimized in both performance and efficiency. In this paper, we propose a novel token-level framework,
LongAttn, which leverages the self-attention mechanism of LLMs to measure the long-range dependencies for the data. By
calculating token-level dependency strength and distribution uniformity of token scores, LongAttn effectively quantifies
long-range dependencies, enabling more accurate and efficient data selection. We filter LongABC-32K from open-source
long-context datasets (ArXiv, Book, and Code). Through our comprehensive experiments, LongAttn has demonstrated its
excellent effectiveness, scalability, and efficiency. To facilitate future research in long-context data, we released
our code and the high-quality long-context training data LongABC-32K.

][153]
[
* 8 authors
][154]
·
Feb 24, 2025 [ 1][155]
[
21
][156]

### [Revisiting Long-context Modeling from Context Denoising Perspective][157]

[

Long-context models (LCMs) have demonstrated great potential in processing long sequences, facilitating many real-world
applications. The success of LCMs can be attributed to their ability to locate implicit critical information within the
context for further prediction. However, recent research reveals that LCMs are often susceptible to contextual noise,
i.e., irrelevant tokens, that can mislead model attention. In this paper, we conduct a fine-grained analysis of the
context noise and propose an effective metric, the Integrated Gradient (IG) score, to detect and quantify the noise
information within the context. Our findings reveal that even simple mitigation of detected context noise can
substantially boost the model's attention on critical tokens and benefit subsequent predictions. Building on this
insight, we propose Context Denoising Training (CDT), a straightforward yet effective training strategy that improves
attention on critical tokens while reinforcing their influence on model predictions. Extensive experiments across four
tasks, under both context window scaling and long-context alignment settings, demonstrate the superiority of CDT.
Notably, when trained with CDT, an open-source 8B model can achieve performance (50.92) comparable to GPT-4o (51.00).

][158]
[[SUDA] Soochow University][159]
·
Oct 7, 2025 [ 3][160]
[
116
][161]

### [LongRoPE: Extending LLM Context Window Beyond 2 Million Tokens][162]

[

Large context window is a desirable feature in large language models (LLMs). However, due to high fine-tuning costs,
scarcity of long texts, and catastrophic values introduced by new token positions, current extended context windows are
limited to around 128k tokens. This paper introduces LongRoPE that, for the first time, extends the context window of
pre-trained LLMs to an impressive 2048k tokens, with up to only 1k fine-tuning steps at within 256k training lengths,
while maintaining performance at the original short context window. This is achieved by three key innovations: (i) we
identify and exploit two forms of non-uniformities in positional interpolation through an efficient search, providing a
better initialization for fine-tuning and enabling an 8x extension in non-fine-tuning scenarios; (ii) we introduce a
progressive extension strategy that first fine-tunes a 256k length LLM and then conducts a second positional
interpolation on the fine-tuned extended LLM to achieve a 2048k context window; (iii) we readjust LongRoPE on 8k length
to recover the short context window performance. Extensive experiments on LLaMA2 and Mistral across various tasks
demonstrate the effectiveness of our method. Models extended via LongRoPE retain the original architecture with minor
modifications to the positional embedding, and can reuse most pre-existing optimizations.

][163]
[
* 8 authors
][164]
·
Feb 21, 2024 [ 20][165]
[
37
][166]

### [Long-context LLMs Struggle with Long In-context Learning][167]

[

Large Language Models (LLMs) have made significant strides in handling long sequences exceeding 32K tokens. However,
their performance evaluation has largely been confined to metrics like perplexity and synthetic tasks, which may not
fully capture their abilities in more nuanced, real-world scenarios. This study introduces a specialized benchmark
(LIConBench) focusing on long in-context learning within the realm of extreme-label classification. We meticulously
selected six datasets with a label range spanning 28 to 174 classes covering different input (few-shot demonstration)
length from 2K to 50K. Our benchmark requires LLMs to comprehend the entire input to recognize the massive label spaces
to make correct prediction. We evaluate 13 long-context LLMs on our benchmarks. We find that the long-context LLMs
perform relatively well under the token length of 20K and the performance benefits from utilizing the long context
window. However, after the context window exceeds 20K, most LLMs except GPT-4 will dip dramatically. This suggests a
notable gap in current LLM capabilities for processing and understanding long, context-rich sequences. Further analysis
revealed a tendency among models to favor predictions for labels presented towards the end at the sequence. Their
ability to reason over multiple pieces in the long sequence is yet to be improved. Our study reveals that long context
understanding and reasoning is still a challenging task for the existing LLMs. We believe LIConBench could serve as a
more realistic evaluation for the future long context LLMs.

][168]
[
* 5 authors
][169]
·
Apr 2, 2024 [ 4][170]
[
22
][171]

### [Is It Really Long Context if All You Need Is Retrieval? Towards Genuinely Difficult Long Context NLP][172]

[

Improvements in language models' capabilities have pushed their applications towards longer contexts, making
long-context evaluation and development an active research area. However, many disparate use-cases are grouped together
under the umbrella term of "long-context", defined simply by the total length of the model's input, including - for
example - Needle-in-a-Haystack tasks, book summarization, and information aggregation. Given their varied difficulty, in
this position paper we argue that conflating different tasks by their context length is unproductive. As a community, we
require a more precise vocabulary to understand what makes long-context tasks similar or different. We propose to unpack
the taxonomy of long-context based on the properties that make them more difficult with longer contexts. We propose two
orthogonal axes of difficulty: (I) Diffusion: How hard is it to find the necessary information in the context? (II)
Scope: How much necessary information is there to find? We survey the literature on long-context, provide justification
for this taxonomy as an informative descriptor, and situate the literature with respect to it. We conclude that the most
difficult and interesting settings, whose necessary information is very long and highly diffused within the input, is
severely under-explored. By using a descriptive vocabulary and discussing the relevant properties of difficulty in
long-context, we can implement more informed research in this area. We call for a careful design of tasks and benchmarks
with distinctly long context, taking into account the characteristics that make it qualitatively different from shorter
context.

][173]
[
* 6 authors
][174]
·
Jun 29, 2024 [ 1][175]
[
2
][176]

### [Distilling Conversations: Abstract Compression of Conversational Audio Context for LLM-based ASR][177]

[

Standard LLM-based speech recognition systems typically process utterances in isolation, limiting their ability to
leverage conversational context. In this work, we study whether multimodal context from prior turns improves LLM-based
ASR and how to represent that context efficiently. We find that, after supervised multi-turn training, conversational
context mainly helps with the recognition of contextual entities. However, conditioning on raw context is expensive
because the prior-turn audio token sequence grows rapidly with conversation length. To address this, we propose Abstract
Compression, which replaces the audio portion of prior turns with a fixed number of learned latent tokens while
retaining corresponding transcripts explicitly. On both in-domain and out-of-domain test sets, the compressed model
recovers part of the gains of raw-context conditioning with a smaller prior-turn audio footprint. We also provide
targeted analyses of the compression setup and its trade-offs.

][178]
[[Idiap] Idiap Research Institute][179]
·
Mar 27 [ 2][180]
[
1
][181]

### [Reducing Distraction in Long-Context Language Models by Focused Learning][182]

[

Recent advancements in Large Language Models (LLMs) have significantly enhanced their capacity to process long contexts.
Ho

[Content truncated]
```
