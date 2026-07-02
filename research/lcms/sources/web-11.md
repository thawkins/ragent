# Web source

- URL: https://arxiv.org/html/2412.08821v2
- Title: 1. Large Concept Models: Language Modeling in a Sentence Representation Space
- Captured (UTC): 2026-06-29T16:29:42.854314175+00:00

```text
1. Large Concept Models: Language Modeling in a Sentence Representation Space
   1.  [1 Introduction][1]
   2.  [2 Main Design Principles][2]
       1. [2.1 The SONAR embedding space][3]
       2. [2.2 Data preparation][4]
          1. [Sentence segmentation analysis][5]
       3. [2.3 Large Concept Model variants][6]
          1. [2.3.1 Base-LCM][7]
          2. [2.3.2 Diffusion-based LCM][8]
             1. [Forward process and noise schedule][9]
             2. [Reverse process and objective function][10]
             3. [Classifier-free diffusion guidance for the LCM][11]
             4. [Inference][12]
          3. [2.3.3 One-Tower Diffusion LCM][13]
          4. [2.3.4 Two-Tower Diffusion LCM][14]
             1. [Two-Tower training.][15]
          5. [2.3.5 Quantized LCM][16]
             1. [Quantization of SONAR space.][17]
             2. [Finetuning the SONAR decoder on quantized representations.][18]
             3. [Quant-LCM architecture.][19]
             4. [Discrete targets.][20]
             5. [Continuous targets.][21]
       4. [2.4 Ablations][22]
          1. [2.4.1 Experimental setup][23]
             1. [Models architectures.][24]
             2. [Pre-training evaluation.][25]
             3. [Pre-training evaluation data.][26]
             4. [Instruction-tuning evaluation.][27]
          2. [2.4.2 Importance of the diffusion inference hyper-parameters][28]
          3. [2.4.3 Studying the noise schedules][29]
          4. [2.4.4 Studying the loss weighting strategies][30]
             1. [Fragility as a sample weighing strategy][31]
       5. [2.5 Analysis][32]
          1. [2.5.1 Inference efficiency of LCMs][33]
          2. [2.5.2 Fragility of SONAR space][34]
             1. [Finetuned robust decoder.][35]
             2. [Fragility study.][36]
   3.  [3 Scaling the model to 7B][37]
       1. [3.1 Evaluation Tasks and Data][38]
          1. [3.1.1 Metrics][39]
          2. [3.1.2 Summarization][40]
             1. [Task and datasets.][41]
             2. [Baselines.][42]
             3. [Summarization results.][43]
             4. [Long-context summarization results.][44]
   4.  [4 Large Concept Model Extensions][45]
       1. [4.1 Summary Expansion][46]
          1. [Task and datasets.][47]
          2. [Results.][48]
       2. [4.2 Zero-shot generalization performance][49]
       3. [4.3 Exploring explicit planning][50]
          1. [Data preprocessing.][51]
          2. [Metrics.][52]
   5.  [5 Related work][53]
       1. [5.1 Sentence representations][54]
          1. [Multilingual sentence representations][55]
          2. [Joint speech/text sentence representations][56]
          3. [LLM based sentence representations][57]
       2. [5.2 Multilingual LLMs][58]
       3. [5.3 Alternative LLM architectures][59]
          1. [Sentence embeddings for language modeling.][60]
          2. [Language modeling with diffusion.][61]
   6.  [6 Limitations][62]
       1. [Choice of the embedding space.][63]
       2. [Concept granularity][64]
       3. [Continuous versus discrete][65]
   7.  [7 Acknowledgments][66]
   8.  [8 Conclusion and Future Work][67]
   9.  [A Data][68]
   10. [B Open Sourced Code][69]
   11. [C System prompt: Generation of Topic Descriptions][70]
   12. [D User prompt: LLM As a Judge - Coherence][71]

]FAIR at Meta \contribution[*]Core contributors, alphabetical order \contribution[†]Contributors to data preparation,
LCM extensions and evaluation, alphabetical order \contribution[‡]Research and project management, alphabetical order
\contribution[+]Initial work while at FAIR at Meta, new affiliation: INRIA, France \correspondenceHolger Schwenk at

# Large Concept Models:
# Language Modeling in a Sentence Representation Space

LCM team Loïc Barrault Paul-Ambroise Duquenne Maha Elbayad Artyom Kozhevnikov Belen Alastruey Pierre Andrews Mariano
Coria Guillaume Couairon Marta R. Costa-jussà David Dale Hady Elsahar Kevin Heffernan João Maria Janeiro Tuan Tran
Christophe Ropers Eduardo Sánchez Robin San Roman Alexandre Mourachko Safiyyah Saleem Holger Schwenk [
[schwenk@meta.com][72]
(December 12, 2024)

###### Abstract

LLMs have revolutionized the field of artificial intelligence and have emerged as the de-facto tool for many tasks. The
current established technology of LLMs is to process input and generate output at the token level. This is in sharp
contrast to humans who operate at multiple levels of abstraction, well beyond single words, to analyze information and
to generate creative content. In this paper, we present an attempt at an architecture which operates on an explicit
higher-level semantic representation, which we name a “concept”. Concepts are language- and modality-agnostic and
represent a higher level idea or action in a flow. Hence, we build a “Large Concept Model”. In this study, as proof of
feasibility, we assume that a concept corresponds to a sentence, and use an existing sentence embedding space, SONAR,
which supports up to 200 languages in both text and speech modalities.

The Large Concept Model is trained to perform autoregressive sentence prediction in an embedding space. We explore
multiple approaches, namely MSE regression, variants of diffusion-based generation, and models operating in a quantized
SONAR space. These explorations are performed using 1.6B parameter models and training data in the order of 1.3T tokens.
We then scale one architecture to a model size of 7B parameters and training data of about 2.7T tokens. We perform an
experimental evaluation on several generative tasks, namely summarization and a new task of summary expansion. Finally,
we show that our model exhibits impressive zero-shot generalization performance to many languages, outperforming
existing LLMs of the same size. The training code of our models is freely
available.¹¹1[https://github.com/facebookresearch/large_concept_model][73]

## 1 Introduction

Large Language models (LLMs) are dominating current research in natural language processing, and with their recent
extension to more modalities, namely images, video and speech, they seem to be considered as the de-facto technique to
follow to approach human intelligence. LLMs achieve indeed impressive performance on a large variety of tasks, such as
providing detailed answers for general knowledge questions, helping in performing long document analysis, or drafting
different types of messages, and writing or debugging code. Building an LLM from scratch requires access to enormous
computational resources to process ever larger amounts of data and train models, the size of which now exceeds four
hundred billion parameters. Knowledge acquisition in LLMs is heavily data-driven and extending them to more languages or
modalities usually requires injecting additional (synthetic) data to cover them.

The landscape of available LLMs can be structured into open models such as Llama (The Llama3 team, [2024][74]), Mistral
(Jiang et al., [2024][75]), Bloom (BigScience Workshop, [2023][76]) or Falcon (Almazrouei et al., [2023][77]), on the
one hand, and closed models such as Gemini (Gemini Team Google, [2024][78]), GPT (OpenAI, [2024][79]) or Claude
(Anthropic, [2024][80]), on the other. It is striking that all these models are based on the same underlying
architecture: a transformer-based, decoder-only language model, which is pretrained to predict the next token, given a
long context of preceding tokens. Despite the undeniable success of LLMs and continued progress, all current LLMs miss a
crucial characteristic of human intelligence: explicit reasoning and planning at multiple levels of abstraction. The
human brain does not operate at the word level only. We usually have a top-down process to solve a complex task or
compose a long document: we first plan at a higher level the overall structure, and then step-by-step, add details at
lower levels of abstraction. One may argue that LLMs are implicitly learning a hierarchical representation, but we
stipulate that models with an explicit hierarchical architecture are better suited to create coherent long-form output.

Imagine a researcher giving a fifteen-minute talk. In such a situation, researchers do not usually prepare detailed
speeches by writing out every single word they will pronounce. Instead, they outline a flow of higher-level ideas they
want to communicate. Should they give the same talk multiple times, the actual words being spoken may differ, the talk
could even be given in different languages, but the flow of higher-level abstract ideas will remain the same. Similarly,
when writing a research paper or essay on a specific topic, humans usually start by preparing an outline that structures
the whole document into sections, which they then refine iteratively. Humans also detect and remember dependencies
between the different parts of a longer document at an abstract level. If we expand on our previous research writing
example, keeping track of dependencies means that we need to provide results for each of the experiment mentioned in the
introduction. Finally, when processing and analyzing information, humans rarely consider every single word in a large
document. Instead, we use a hierarchical approach: we remember which part of a long document we should search to find a
specific piece of information.

To the best of our knowledge, this explicit hierarchical structure of information processing and generation, at an
abstract level, independent of any instantiation in a particular language or modality, cannot be found in any of the
current LLMs.

──────────────────
[Refer to caption]
──────────────────

──────────────────
[Refer to caption]
──────────────────
Figure 1: Left: visualization of reasoning in an embedding space of concepts (task of summarization).
Right: fundamental architecture of an Large Concept Model (LCM).
⋆⋆\star⋆: concept encoder and decoder are frozen.

In this work, we present a new approach which moves away from processing at the token level and closer to (hierarchical)
reasoning in an abstract embedding space. This abstract embedding space is designed to be independent of the language or
modality in which the content is expressed; in other words, we aim to model the underlying reasoning process at a purely
semantic level, not its instantiation in a specific language. In order to verify our approach, we limit our study to two
levels of abstraction: subword tokens and concepts. We define a concept as an abstract atomic idea. In practice, a
concept would often correspond to a sentence in a text document, or an equivalent speech utterance. We posit that a
sentence is an appropriate unit to achieve language independence, in opposition to single words. This is in sharp
contrast to current LLMs techniques which are heavily English centric and token based.

Our fundamental idea could be based on any fixed-size sentence embedding space for which an encoder and decoder are
available. In particular, we could aim to train a new embedding space specifically optimized to our reasoning
architecture. In this work, we chose an existing and freely available sentence embedding, named SONAR (Duquenne et al.,
[2023b][81]). SONAR supports text input and output in 200 languages, speech input in 76 languages, and speech output in
English. We discuss the constraints and impact of this choice in [Section 2.1][82], and share some ideas on alternative
embedding spaces in [Section 6][83].

[Figure 1][84]-left visualizes reasoning in an embedding space with the example of a summarization task, which is
materialized by a function on the embedding space, mapping five concept representations into two. [Figure 1][85]-right
summarizes the overall architecture and processing flow. The input is first segmented into sentences, and each one is
encoded with SONAR to achieve a sequence of concepts, i.e., sentence embeddings. This sequence of concepts is then
processed by a Large Concept Model (LCM) to generate at the output a new sequence of concepts. Finally, the generated
concepts are decoded by SONAR into a sequence of subwords. The encoder and decoder are fixed and are not trained. It is
important to highlight that the unchanged sequence of concepts at the output of the LCM can be decoded into other
languages or modalities without performing again the whole reasoning process. In the same spirit, a particular reasoning
operation such as summarization can be performed in a zero-shot setting on input in any language or modality, since it
solely operates on concepts. To summarize, the LCM neither has information on the input language or modality nor
generates output in a particular language or modality. We explore multiple architectures to train the LCM, in particular
several variants of diffusion. Finally, we envision an additional level of abstraction beyond concepts which could
correspond to a short description of a paragraph or small section. In [Section 4.3][86] we report initial ideas on how
conditioning and predicting such higher-level representations can improve consistency of output generated by an LCM.

To some extent, the LCM architecture resembles the Jepa approach (LeCun, [2022][87]) that also aims to predict the
representation of the next observation in an embedding space. However, unlike Jepa that places more emphasis on learning
a representation space in a self-supervised way, the LCM focuses on accurate prediction in the existing embedding space.

The mains characteristics of our generic Large Concept Model approach are as follows:
* •
  
  Reasoning at an abstract language- and modality-agnostic level beyond tokens:
  * –
    
    We model the underlying reasoning process, not its instantiation in a particular language.
  * –
    
    The LCM can be trained, i.e. acquire knowledge, on all languages and modalities at once, promising scalability in an
    unbiased way.
* •
  
  Explicit hierarchical structure:
  * –
    
    Better readability of long-form output by a human.
  * –
    
    Facilitates local interactive edits by a user.
* •
  
  Handling of long context and long-form output:
  * –
    
    The complexity of a vanilla transformer model increases quadratically with the sequence length. This makes handling
    of large context windows challenging and several techniques have been developed to alleviate this problem, e.g.,
    sparse attention (Child et al., [2019][88]) or LSH attention (Kitaev et al., [2020][89]). Our LCM operates on
    sequences which are at least an order of magnitude shorter.²²2We assume an average sentence length of 10–20 tokens.
* •
  
  Unparalleled zero-shot generalization:
  * –
    
    Independently of the language or modality the LCM is pre-trained and fine-tuned on, it can be applied to any
    language and modality supported by the SONAR encoders, without the need of additional data or fine-tuning. We report
    results for multiple languages in the text modality.
* •
  
  Modularity and extensibility:
  * –
    
    Unlike multimodal LLMs that can suffer from modality competition (Aghajanyan et al., [2023][90]; Chameleon team,
    [2024][91]), concept encoders and decoders can be independently developed and optimized without any competition or
    interference.
  * –
    
    New languages or modalities can be easily added for an existing system.

The goal of this paper is to provide a proof of concept of this high-level vision of an alternative architecture to
current best practice in language modeling. In the next section we present the main design principles of our models and
discuss several variants to build and train a Large Concept Model. We discuss several designs to implement diffusion
approaches with concept embeddings and carefully study noise scheduling. This section is completed by a compute
complexity comparison with token-based LLMs. [Section 3][92] is dedicated to the analysis of a larger 7B parameter
model. We discuss challenges when instruction fine-tuning this model on multiple generative tasks, and provide a
comparison with existing LLMs of comparable size. The paper concludes with a discussion of related work, the current
limitations and perspectives of our approach.

To foster research in this area, we make our LCM training
code³³3[https://github.com/facebookresearch/large_concept_model][93] as well as SONAR encoders and
decoders⁴⁴4[https://github.com/facebookresearch/SONAR][94] for up to 200 languages and multiple modalities freely
available.

## 2 Main Design Principles

In this section, we outline the main design principles of the LCM. We first describe the SONAR embedding space with its
encoders and decoders. Then, we discuss details of data preparation, namely sentence segmentation i.e., how we split
long documents into sentences. And finally, we describe in details the different versions of LCMs introduced in this
work.

### 2.1 The SONAR embedding space

The motivation of this work is to perform reasoning at a higher conceptual level than tokens. This requires an embedding
space which is highly semantic. We chose SONAR (Duquenne et al., [2023b][95]) since it achieves best performance on
several semantic similarity metrics like xsim or xsim++ (Chen et al., [2023b][96]), and it was successfully used in
large-scale bitext mining for translation (Seamless Communication et al., [2023b][97]).

[Refer to caption] Figure 2: Encoder/decoder bottleneck architecture to train the SONAR text embeddings (right part of
figure). Teacher-student approach to extend SONAR to the speech modality (left part).

The SONAR text embedding space was trained as an encoder/decoder architecture, with a fixed-size bottleneck instead of
cross-attention (see [Figure 2][98]). The criterion combines a machine translation objective for 200 languages into and
out of English, denoising auto-encoding and an explicit MSE loss at the embedding bottleneck layer. Once the text
embedding space was trained, a teacher-student approach was applied to extend the SONAR space to the speech modality.
More details on the architecture and training procedure can be found in Duquenne et al. ([2023b][99]), and detailed
speech recognition and translation results in the appendix of Seamless Communication et al. ([2023a][100]).

────────────┬────────────┬────────────┬────────────┬────────────
            │Text        │Speech      │Image       │Video       
────────────┼─────┬──────┼─────┬──────┼─────┬──────┼─────┬──────
Model       │Input│Output│Input│Output│Input│Output│Input│Output
────────────┼─────┼──────┼─────┼──────┼─────┼──────┼─────┼──────
Gemini      │47   │47    │62   │✓     │✓    │✓     │✓    │✗     
────────────┼─────┼──────┼─────┼──────┼─────┼──────┼─────┼──────
GPT         │85   │85    │✓    │✓     │✓    │✓     │?    │✗     
────────────┼─────┼──────┼─────┼──────┼─────┼──────┼─────┼──────
Claude      │37   │37    │✓    │✓     │✓    │✓     │✗    │✗     
────────────┼─────┼──────┼─────┼──────┼─────┼──────┼─────┼──────
Bloom       │46   │46    │✗    │✗     │✓    │✓     │✗    │✗     
────────────┼─────┼──────┼─────┼──────┼─────┼──────┼─────┼──────
Llama 3-400B│8    │8     │34   │✗     │✓    │✓     │✗    │✗     
────────────┼─────┼──────┼─────┼──────┼─────┼──────┼─────┼──────
LCM-SONAR   │200  │200   │76   │1     │✗    │✗     │(ASL)│✗     
────────────┴─────┴──────┴─────┴──────┴─────┴──────┴─────┴──────
Table 1: Comparison of language and modality coverage for several LLMs and our LCM operating on the SONAR embedding
space. SONAR has an experimental support for American Sign Language (ASL) which is not used in this paper.

Our LCM operates directly on SONAR concepts embeddings, hence, it can perform reasoning on all supported languages and
modalities. [Table 1][101] compares the language coverage of several other LLMs. The LCM supports substantially more
languages than other models, in particular many low-resource languages. In addition to the text modality, SONAR supports
76 languages for speech input and speech output in English. We have also developed an experimental encoder for American
Sign language (ASL). All these encoders and decoders are freely
available.⁵⁵5[https://github.com/facebookresearch/SONAR][102] Exact listings of the supported languages can be found in
the SONAR GitHub repository.

### 2.2 Data preparation

To train and evaluate the LCM, we need to convert raw text datasets into a sequence of SONAR embeddings, each one
corresponding to a sentence. Dealing with large text corpora presents several practical limitations. First, the precise
segmentation of a text into sentences can be challenging due to the presence of errors, specific formatting issues or
any other sources of noise. This requires us to apply robust automatic text segmentation techniques. Second, some
sentences (even well formed) can be very long and complex, which might negatively impact the quality of the encoded
SONAR embeddings. This is particularly prevalent for texts in the scientific domain. In the following, we discuss
strategies for sentence segmentation and how they affect the SONAR encoding.

##### Sentence segmentation analysis

We have identified two potential sentence segmentation techniques; as we are exploring multilingual data, we focus on
sentence segmenters with a large language coverage:
1. 1.
   
   SpaCy segmenter (SpaCy) (Honnibal et al., [2020][103]) is a well established multilingual NLP toolkit that provides a
   rule-based approach to sentence segmentation. SpaCy is thoroughly tested for high-resource languages.
2. 2.
   
   Segment any Text (SaT) (Minixhofer et al., [2023][104]; Frohmann et al., [2024][105]) offers a suite of models and
   adapters that predict sentence boundaries at the token level. SaT is designed to be resilient to perturbations,
   particularly avoiding the over-reliance on punctuation and capitalization. This is valuable in domains where these
   conventional markers are often missing. The quality of SaT’s segmentation is however dependent on the choice of an
   “appropriate” split probability threshold.

We additionally customize both methods by incorporating a maximum sentence length cap in characters. We refer to these
extensions by SpaCy Capped and SaT Capped. Long sentences are broken down into smaller, logically coherent fragments
using a rule-based approach based on punctuation marks for SpaCy. For SaT, we leverage the provided splitting
probability estimates to identify the next best potential split.

To measure the efficacy of a given segmenter, we evaluate the quality of the reconstructed sentences with AutoBLEU. It
is defined as a BLEU score (Papineni et al., [2002][106]) comparing the decoded text from a SONAR vector after encoding
a segment, to the the reference segment. A good segmentation will yield segments that can be encoded and then decoded
without loss of signal, and thus score a higher AutoBLEU.

For this analysis, we sample 10k documents from our pretraining datasets, representing approximately 500k sentences. The
documents are processed with each segmenter, the sentences are encoded then decoded and the AutoBLEU score is
calculated. We stratified the results based on the lengths of the original sentences.

[Refer to caption] Figure 3: Segmenters quality. Average Auto-BLEU scores for different sentence segmentation methods
depending on sentence length, for both out of the box (left) and capped implementations (right).

As illustrated in [Figure 3][107] and with a capping at 200 characters, the SaT Capped method demonstrates a slight but
consistent advantage over SpaCy Capped. Both out-of-the-box segmenters, however, exhibit significant under-performance
across all sentence lengths. This lower performance is especially pronounced for sentences exceeding 250 characters,
underscoring the limitations of using the segmenters without capping.

Accordingly, we prepare the LCM training data with SaT Capped. We discuss in [Appendix A][108] technical and engineering
challenges faced when handling large amounts of SONAR embeddings.

### 2.3 Large Concept Model variants

The design of the LCM is driven by the need to conditionally generate a continuous sentence embedding. This obviously
contrasts with how current LLMs work, i.e., estimating a probability distribution over a vocabulary of discrete tokens.
A straightforward way of solving the task is to train a transformer model to generate an embedding with the objective of
minimizing the MSE loss (see [Section 2.3.1][109]). However, a given context may have many plausible, yet semantically
different, continuations. The model should thus be able to learn a conditional probability distribution over the
continuous embedding of the next sentence.

There is a large body of work in computer vision aiming to learn such conditional probability distributions over
continuous data (Dhariwal and Nichol, [2021][110]; Rombach et al., [2021][111]). Models like Dall-E 3 (Betker et al.,
[2023][112]) or Imagen Video (Ho et al., [2022][113]) use a diffusion process to generate an image or video from a text
prompt. Many different real images may satisfy the same input prompt, hence the model has to learn a probability
distribution over continuous pixel data. This motivates the exploration of diffusion models for sentence embedding
generation. Two variants are presented in [Sections 2.3.3][114] and [2.3.4][115]. Another prevalent take on continuous
data generation consists of quantizing said data to ultimately model with discrete units; we explore LCM modeling with
quantization in [Section 2.3.5][116].

#### 2.3.1 Base-LCM

[Refer to caption] Figure 4: TheBase-LCM. Illustration of the Base-LCM. At its core is a standard decoder-only
Transformer surrounded with a PreNetPreNet\operatorname{PreNet}roman_PreNet and a
PostNetPostNet\operatorname{PostNet}roman_PostNet.

Our baseline architecture for next-concept prediction is a standard decoder-only Transformer that transduces a sequence
of preceding concepts (read sentence embeddings) into a sequence of future ones. As illustrated in [Figure 4][117], the
Base-LCM is equipped with a “PostNetPostNet\operatorname{PostNet}roman_PostNet” and a
“PreNetPreNet\operatorname{PreNet}roman_PreNet”. The PreNetPreNet\operatorname{PreNet}roman_PreNet normalizes the input
SONAR embeddings and maps them to the model’s hidden dimension dmodelsubscriptdmodel{\textnormal{d}}_{\text{model}}d
start_POSTSUBSCRIPT model end_POSTSUBSCRIPT.

──────────────────────┬───────────────────────────────────────────────────────────────────────────────────────────┬───
PreNet⁡(𝐱)PreNet𝐱\displ│=normalize⁡(𝐱)⁢𝐖pret+𝐛pre,absentnormalize𝐱superscriptsubscript𝐖pre𝑡subscript𝐛pre\displaystyle│(1)
aystyle\operatorname{P│=\operatorname{normalize}({\mathbf{x}}){\mathbf{W}}_{\text{pre}}^%                         │   
reNet}({\mathbf{x}})ro│{t}+{\mathbf{b}}_{\text{pre}},= roman_normalize ( bold_x ) bold_W start_POSTSUBSCRIPT pre  │   
man_PreNet ( bold_x ) │end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_t end_POSTSUPERSCRIPT + bold_b              │   
                      │start_POSTSUBSCRIPT pre end_POSTSUBSCRIPT ,                                                │   
──────────────────────┼───────────────────────────────────────────────────────────────────────────────────────────┼───
PostNet⁡(𝐱)PostNet𝐱\dis│=denormalize⁡(𝐱𝐖postt+𝐛post),absentdenormalizesuperscriptsubscript𝐱𝐖post𝑡subscript𝐛post\disp│(2)
playstyle\operatorname│laystyle=\operatorname{denormalize}\left({\mathbf{x}}{\mathbf{W}}_{\text{%                 │   
{PostNet}({\mathbf{x}}│post}}^{t}+{\mathbf{b}}_{\text{post}}\right),= roman_denormalize ( bold_xW                 │   
)roman_PostNet (      │start_POSTSUBSCRIPT post end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_t                  │   
bold_x )              │end_POSTSUPERSCRIPT + bold_b start_POSTSUBSCRIPT post end_POSTSUBSCRIPT ) ,                │   
──────────────────────┴───────────────────────────────────────────────────────────────────────────────────────────┴───

where
𝐖post∈ℝdSONAR×dmodelsubscript𝐖postsuperscriptℝsubscriptdSONARsubscriptdmodel{\mathbf{W}}_{\text{post}}\in\mathbb{R}^{{\t
extnormal{d}}_{\text{{SONAR}}}% \times{\textnormal{d}}_{\text{model}}}bold_W start_POSTSUBSCRIPT post end_POSTSUBSCRIPT
∈ blackboard_R start_POSTSUPERSCRIPT d start_POSTSUBSCRIPT SONAR end_POSTSUBSCRIPT × d start_POSTSUBSCRIPT model
end_POSTSUBSCRIPT end_POSTSUPERSCRIPT,
𝐛post∈ℝdSONARsubscript𝐛postsuperscriptℝsubscriptdSONAR{\mathbf{b}}_{\text{post}}\in\mathbb{R}^{{\textnormal{d}}_{\text{{
SONAR}}}}bold_b start_POSTSUBSCRIPT post end_POSTSUBSCRIPT ∈ blackboard_R start_POSTSUPERSCRIPT d start_POSTSUBSCRIPT
SONAR end_POSTSUBSCRIPT end_POSTSUPERSCRIPT,
𝐖pre∈ℝdmodel×dSONARsubscript𝐖presuperscriptℝsubscriptdmodelsubscriptdSONAR{\mathbf{W}}_{\text{pre}}\in\mathbb{R}^{{\text
normal{d}}_{\text{model}}\times{% \textnormal{d}}_{\text{{SONAR}}}}bold_W start_POSTSUBSCRIPT pre end_POSTSUBSCRIPT ∈
blackboard_R start_POSTSUPERSCRIPT d start_POSTSUBSCRIPT model end_POSTSUBSCRIPT × d start_POSTSUBSCRIPT SONAR
end_POSTSUBSCRIPT end_POSTSUPERSCRIPT and
𝐛pre∈ℝdmodelsubscript𝐛presuperscriptℝsubscriptdmodel{\mathbf{b}}_{\text{pre}}\in\mathbb{R}^{{\textnormal{d}}_{\text{mode
l}}}bold_b start_POSTSUBSCRIPT pre end_POSTSUBSCRIPT ∈ blackboard_R start_POSTSUPERSCRIPT d start_POSTSUBSCRIPT model
end_POSTSUBSCRIPT end_POSTSUPERSCRIPT.

In order to learn the maps “normalizenormalize\operatorname{normalize}roman_normalize” and its inverse
“denormalizedenormalize\operatorname{denormalize}roman_denormalize” we fit a robust scaler to a set of randomly sampled
SONAR vectors from different corpora and domains of text data. This scaler removes the median statistics and scales the
data according to the interquartile range (IQR).

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
normalize⁡(𝐱)=𝐱−𝝁𝝈,denormalize⁡(𝐱)=𝝁+𝝈⁢𝐱.formulae-sequencenormalize𝐱𝐱𝝁𝝈denormalize𝐱𝝁𝝈𝐱\displaystyle\operatorname{norm│(4)
alize}({\mathbf{x}})=\frac{{\mathbf{x}}-{\bm{%                                                                    │   
\mu}}}{{\bm{\sigma}}},\quad\operatorname{denormalize}({\mathbf{x}})={\bm{\mu}}%                                   │   
+{\bm{\sigma}}{\mathbf{x}}.roman_normalize ( bold_x ) = divide start_ARG bold_x - bold_italic_μ end_ARG start_ARG │   
bold_italic_σ end_ARG , roman_denormalize ( bold_x ) = bold_italic_μ + bold_italic_σ bold_x .                     │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

The Base-LCM is trained on the semi-supervised task of next concept prediction, that is, the model predicts the next
concept 𝐱^nsubscript^𝐱𝑛\hat{\mathbf{x}}_{n}over^ start_ARG bold_x end_ARG start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT
and its parameters 𝜽𝜽{\bm{\theta}}bold_italic_θ are optimized to regress the ground truth next concept
(𝐱nsubscript𝐱𝑛{\mathbf{x}}_{n}bold_x start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT).

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
𝐱^n=f⁢(𝐱<n;𝜽),MSE⁡(𝐱^n,𝐱n)=‖𝐱^n−𝐱n‖2.formulae-sequencesubscript^𝐱𝑛𝑓subscript𝐱absent𝑛𝜽MSEsubscript^𝐱𝑛subscript𝐱𝑛super│(5)
scriptnormsubscript^𝐱𝑛subscript𝐱𝑛2\displaystyle\hat{\mathbf{x}}_{n}=f({\mathbf{x}}_{<n};{\bm{\theta}}),\quad%     │   
\operatorname{MSE}(\hat{\mathbf{x}}_{n},{\mathbf{x}}_{n})=\|\hat{\mathbf{x}}_{% n}-{\mathbf{x}}_{n}\|^{2}.over^   │   
start_ARG bold_x end_ARG start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT = italic_f ( bold_x start_POSTSUBSCRIPT < │   
italic_n end_POSTSUBSCRIPT ; bold_italic_θ ) , roman_MSE ( over^ start_ARG bold_x end_ARG start_POSTSUBSCRIPT     │   
italic_n end_POSTSUBSCRIPT , bold_x start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT ) = ∥ over^ start_ARG bold_x   │   
end_ARG start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT - bold_x start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT ∥  │   
start_POSTSUPERSCRIPT 2 end_POSTSUPERSCRIPT .                                                                     │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

Given a data distribution q of documents (sequences of concepts), the training loss is evaluated as:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
ℒBase-LCM⁢(𝜽)=𝔼𝐱∼q⁢[∑n=1|𝐱|MSE⁡(f⁢(𝐱<n;𝜽),𝐱n)].subscriptℒBase-LCM𝜽subscript𝔼similar-to𝐱𝑞delimited-[]superscriptsubscri│(6)
pt𝑛1𝐱MSE𝑓subscript𝐱absent𝑛𝜽subscript𝐱𝑛\displaystyle\mathcal{L}_{\textsc{Base-LCM}}({\bm{\theta}})=\mathbb{E}_{{%  │   
\mathbf{x}}\sim q}\Big{[}\sum_{n=1}^{|{\mathbf{x}}|}\operatorname{MSE}\left(f(%                                   │   
{\mathbf{x}}_{<n};{\bm{\theta}}),{\mathbf{x}}_{n}\right)\Big{]}.caligraphic_L start_POSTSUBSCRIPT Base-LCM        │   
end_POSTSUBSCRIPT ( bold_italic_θ ) = blackboard_E start_POSTSUBSCRIPT bold_x ∼ italic_q end_POSTSUBSCRIPT [ ∑    │   
start_POSTSUBSCRIPT italic_n = 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT | bold_x | end_POSTSUPERSCRIPT roman_MSE │   
( italic_f ( bold_x start_POSTSUBSCRIPT < italic_n end_POSTSUBSCRIPT ; bold_italic_θ ) , bold_x                   │   
start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT ) ] .                                                              │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

In order to enable the generation of variable length documents at inference time, we suffix training documents with the
sentence “End of text.”. Similar to any sentence in the document, this special suffix will be encoded with SONAR. This
means that 𝐱|𝐱|=eot→≔encode⁡("End of text.")subscript𝐱𝐱→eot≔encode"End of
text."{\mathbf{x}}_{|{\mathbf{x}}|}=\overrightarrow{\text{eot}}\coloneqq% \operatorname{encode}(\text{"End of
text."})bold_x start_POSTSUBSCRIPT | bold_x | end_POSTSUBSCRIPT = over→ start_ARG eot end_ARG ≔ roman_encode ( "End of
text." ). During inference, we implement two main early stopping mechanisms: the first one measures the similarity of
the generated embedding 𝐱^nsubscript^𝐱𝑛\hat{\mathbf{x}}_{n}over^ start_ARG bold_x end_ARG start_POSTSUBSCRIPT italic_n
end_POSTSUBSCRIPT to eot→→eot\overrightarrow{\text{eot}}over→ start_ARG eot end_ARG and stops if the cosine similarity
exceeds a threshold seotsubscript𝑠eots_{\text{eot}}italic_s start_POSTSUBSCRIPT eot end_POSTSUBSCRIPT. The second
mechanism compares the newly generated embedding 𝐱^nsubscript^𝐱𝑛\hat{\mathbf{x}}_{n}over^ start_ARG bold_x end_ARG
start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT to the previous generation 𝐱^n−1subscript^𝐱𝑛1\hat{\mathbf{x}}_{n-1}over^
start_ARG bold_x end_ARG start_POSTSUBSCRIPT italic_n - 1 end_POSTSUBSCRIPT and stops if their cosine similarity is
higher than a threshold sprevsubscript𝑠prevs_{\text{prev}}italic_s start_POSTSUBSCRIPT prev end_POSTSUBSCRIPT. We set
both seotsubscript𝑠eots_{\text{eot}}italic_s start_POSTSUBSCRIPT eot end_POSTSUBSCRIPT and
sprevsubscript𝑠prevs_{\text{prev}}italic_s start_POSTSUBSCRIPT prev end_POSTSUBSCRIPT to 0.9.

#### 2.3.2 Diffusion-based LCM

Diffusion-based LCMs are generative latent variable models that learn a model distribution
p𝜽subscriptp𝜽{\textnormal{p}}_{\bm{\theta}}p start_POSTSUBSCRIPT bold_italic_θ end_POSTSUBSCRIPT approximating a data
distribution q. Similar to the Base-LCM, we model the diffusion LCMs as auto-regressive models that generate concepts in
a document, one at a time. The model distribution is thus expressed at each position n𝑛nitalic_n of the sequence as
p𝜽⁢(𝐱n|𝐱<n)subscriptp𝜽conditionalsubscript𝐱𝑛subscript𝐱absent𝑛{\textnormal{p}}_{\bm{\theta}}({\mathbf{x}}_{n}|{\mathbf{x}}
_{<n})p start_POSTSUBSCRIPT bold_italic_θ end_POSTSUBSCRIPT ( bold_x start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT |
bold_x start_POSTSUBSCRIPT < italic_n end_POSTSUBSCRIPT ) i.e., the generation of the next concept is conditioned on the
preceding context.

In what follows we use a superscript for the denoising/diffusion step (t∈[0,1]𝑡01t\in[0,1]italic_t ∈ [ 0 , 1 ]) and a
subscript (n𝑛nitalic_n) for indexing the sequence of concepts. We simplify for a given n𝑛nitalic_n the conditional model
distribution
p𝜽⁢(𝐱n0|𝐱<n0)subscriptp𝜽conditionalsubscriptsuperscript𝐱0𝑛subscriptsuperscript𝐱0absent𝑛{\textnormal{p}}_{\bm{\theta}}({\m
athbf{x}}^{0}_{n}|{\mathbf{x}}^{0}_{<n})p start_POSTSUBSCRIPT bold_italic_θ end_POSTSUBSCRIPT ( bold_x
start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT | bold_x
start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT start_POSTSUBSCRIPT < italic_n end_POSTSUBSCRIPT ) as
p𝜽⁢(𝐱0)subscriptp𝜽superscript𝐱0{\textnormal{p}}_{\bm{\theta}}({\mathbf{x}}^{0})p start_POSTSUBSCRIPT bold_italic_θ
end_POSTSUBSCRIPT ( bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT ), and the conditional data distribution
q⁢(𝐱n0|𝐱<n0)qconditionalsubscriptsuperscript𝐱0𝑛subscriptsuperscript𝐱0absent𝑛{\textnormal{q}}({\mathbf{x}}^{0}_{n}|{\mathb
f{x}}^{0}_{<n})q ( bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT |
bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT start_POSTSUBSCRIPT < italic_n end_POSTSUBSCRIPT ) as
q⁢(𝐱0)qsuperscript𝐱0{\textnormal{q}}({\mathbf{x}}^{0})q ( bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT ).

Diffusion models involve two processes: a *forward* noising process and a *reverse* denoising process (Ho et al.,
[2020][118]; Song et al., [2020][119]):

##### Forward process and noise schedule

The forward process is a Gaussian diffusion process characterized by the marginal distribution
q⁢(𝐱t|𝐱0)qconditionalsuperscript𝐱𝑡superscript𝐱0{\textnormal{q}}({\mathbf{x}}^{t}|{\mathbf{x}}^{0})q ( bold_x
start_POSTSUPERSCRIPT italic_t end_POSTSUPERSCRIPT | bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT ), given for
every timestep t∈[0,1]𝑡01t\in[0,1]italic_t ∈ [ 0 , 1 ] as:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
q⁢(𝐱t|𝐱0)≔𝒩⁢(αt⁢𝐱0,σt2⁢𝐈).≔qconditionalsuperscript𝐱𝑡superscript𝐱0𝒩subscript𝛼𝑡superscript𝐱0superscriptsubscript𝜎𝑡2𝐈\dis│(7)
playstyle{\textnormal{q}}({\mathbf{x}}^{t}|{\mathbf{x}}^{0})\coloneqq%                                            │   
\mathcal{N}(\alpha_{t}{\mathbf{x}}^{0},\sigma_{t}^{2}{\mathbf{I}}).q ( bold_x start_POSTSUPERSCRIPT italic_t      │   
end_POSTSUPERSCRIPT | bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT ) ≔ caligraphic_N ( italic_α             │   
start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT , italic_σ      │   
start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 2 end_POSTSUPERSCRIPT bold_I ) .             │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

With the reparameterization trick, we can sample from this marginal distribution via:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
𝐱t=αt⁢𝐱0+σt⁢ϵwhere ⁢ϵ∼𝒩⁢(𝟎,𝐈)formulae-sequencesuperscript𝐱𝑡subscript𝛼𝑡superscript𝐱0subscript𝜎𝑡bold-italic-ϵsimilar-tow│(8)
here bold-italic-ϵ𝒩0𝐈\displaystyle{\mathbf{x}}^{t}=\alpha_{t}{\mathbf{x}}^{0}+\sigma_{t}{\bm{%                    │   
\epsilon}}\quad\text{where }{\bm{\epsilon}}\sim\mathcal{N}({\mathbf{0}},{% \mathbf{I}})bold_x                     │   
start_POSTSUPERSCRIPT italic_t end_POSTSUPERSCRIPT = italic_α start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT      │   
bold_x start_POSTSUPERSCRIPT 0 end_POSTSUPERSCRIPT + italic_σ start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT      │   
bold_italic_ϵ where bold_italic_ϵ ∼ caligraphic_N ( bold_0 , bold_I )                                             │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

We use a variance-preserving forward process (Karras et al., [2022][120]) for which we have:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
αt2=sigmoid⁡(λt),σt2=sigmoid⁡(−λt)=1−sigmoid⁡(λt),λt=log⁡(αt2/σt2),formulae-sequenceformulae-sequencesuperscriptsubscr│(9)
ipt𝛼𝑡2sigmoidsubscript𝜆𝑡superscriptsubscript𝜎𝑡2sigmoidsubscript𝜆𝑡1sigmoidsubscript𝜆𝑡subscript𝜆𝑡superscriptsubscrip│   
t𝛼𝑡2superscriptsubscript𝜎𝑡2\displaystyle\alpha_{t}^{2}=\operatorname{sigmoid}(\lambda_{t}),\quad\quad%            │   
\sigma_{t}^{2}=\operatorname{sigmoid}(-\lambda_{t})=1-\operatorname{sigmoid}(%                                    │   
\lambda_{t}),\quad\quad\lambda_{t}=\log\left({\alpha_{t}^{2}}/{\sigma_{t}^{2}}% \right),italic_α                  │   
start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 2 end_POSTSUPERSCRIPT = roman_sigmoid (      │   
italic_λ start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ) , italic_σ start_POSTSUBSCRIPT italic_t                 │   
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 2 end_POSTSUPERSCRIPT = roman_sigmoid ( - italic_λ start_POSTSUBSCRIPT    │   
italic_t end_POSTSUBSCRIPT ) = 1 - roman_sigmoid ( italic_λ start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ) ,    │   
italic_λ start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT = roman_log ( italic_α start_POSTSUBSCRIPT italic_t       │   
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 2 end_POSTSUPERSCRIPT / italic_σ start_POSTSUBSCRIPT italic_t             │   
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 2 end_POSTSUPERSCRIPT ) ,                                                 │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

where λtsubscript𝜆𝑡\lambda_{t}italic_λ start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT is the log signal-to-noise ratio
(log-SNR) for timestep t𝑡titalic_t.

The noise schedule is a strictly monotonically decreasing function fλsubscript𝑓𝜆f_{\lambda}italic_f start_POSTSUBSCRIPT
italic_λ end_POSTSUBSCRIPT that maps from the timestep t∈[0,1]𝑡01t\in[0,1]italic_t ∈ [ 0 , 1 ] to a log-SNR level:
λt=fλ⁢(t)subscript𝜆𝑡subscript𝑓𝜆𝑡\lambda_{t}=f_{\lambda}(t)italic_λ start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT =
italic_f start_POSTSUBSCRIPT italic_λ end_POSTSUBSCRIPT ( italic_t ).

It is common in previous work to also define the noise schedule based on a discrete variance schedule
(β0,…,βT)subscript𝛽0…subscript𝛽𝑇(\beta_{0},\ldots,\beta_{T})( italic_β start_POSTSUBSCRIPT 0 end_POSTSUBSCRIPT , … ,
italic_β start_POSTSUBSCRIPT italic_T end_POSTSUBSCRIPT ). This stems from the formulation of the forward process as a
discrete-time Markov chain that gradually adds Gaussian noise to the data according to said variance schedule:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
q⁢(𝐱1⁢…⁢T|𝐱0)≔∏t=1Tq⁢(𝐱t|𝐱t−1),q⁢(𝐱t|𝐱t−1)≔𝒩⁢(𝐱t;1−βt⁢𝐱t−1,βt⁢𝐈),formulae-sequence≔qconditionalsuperscript𝐱1…Tsuperscript𝐱│(10
0superscriptsubscriptproduct𝑡1Tqconditionalsuperscript𝐱𝑡superscript𝐱𝑡1≔qconditionalsuperscript𝐱𝑡superscript𝐱𝑡1𝒩sup│)  
erscript𝐱𝑡1subscript𝛽𝑡superscript𝐱𝑡1subscript𝛽𝑡𝐈\displaystyle{\textnormal{q}}({\mathbf{x}}^{1\ldots{\textnormal{T}│   
}}|{\mathbf{% x}}^{0})\coloneqq\prod_{t=1}^{\textnormal{T}}{\textnormal{q}}({\mathbf{x}}^{t}%                     │   
|{\mathbf{x}}^{t-1}),\quad{\textnormal{q}}({\mathbf{x}}^{t}|{\mathbf{x}}^{t-1}%                                   │   
)\coloneqq\mathcal{N}({\mathbf{x}}^{t};\sqrt{1-\bet

[Content truncated]
```
