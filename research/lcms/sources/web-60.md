# Web source

- URL: https://arxiv.org/html/2603.16606v3
- Title: ##### Report GitHub Issue
- Captured (UTC): 2026-06-29T16:31:25.984562696+00:00

```text
##### Report GitHub Issue

×
Title:

Content selection saved. Describe the issue below:

Description:
Submit without GitHub Submit in GitHub
[ [arXiv logo] Back to arXiv ][1]
[Why HTML?][2] [ Report Issue][3] [ Back to Abstract ][4] [ Download PDF][5]
1. [Abstract][6]
2. [1 Introduction][7]
3. [2 Related Work][8]
   1. [Cross-lingual Alignment.][9]
   2. [Contrastive Learning.][10]
   3. [Teacher-Student Distillation.][11]
   4. [Massively Multilingual Models.][12]
   5. [Code and Math.][13]
   6. [Speech Encoders and Embeddings.][14]
   7. [Multilingual and Multimodal Language Modeling.][15]
4. [3 Data][16]
   1. [3.1 Language Sets and Training Stages][17]
   2. [3.2 Natural Language Text Data][18]
      1. [Training Datasets.][19]
      2. [Evaluation Datasets.][20]
   3. [3.3 Code and Math data][21]
      1. [Training datasets.][22]
      2. [Evaluation datasets.][23]
   4. [3.4 Speech Data][24]
      1. [Training Datasets.][25]
      2. [Evaluation dataset.][26]
   5. [3.5 Data Filtering and Upsampling Strategies][27]
      1. [Filtering.][28]
      2. [Upsampling.][29]
   6. [3.6 Hard Negatives Generation][30]
5. [4 Model][31]
   1. [4.1 Tokenizers][32]
      1. [200-Language Tokenizer:][33]
      2. [Omnilingual Tokenizer:][34]
   2. [4.2 Architecture and Initialization][35]
      1. [Encoder.][36]
      2. [Decoder.][37]
      3. [Embedding Layer.][38]
   3. [4.3 Sequence-to-Sequence Pretraining][39]
   4. [4.4 Sentence Representation Learning][40]
      1. [4.4.1 Translation and Contrastive Finetuning][41]
      2. [4.4.2 Translation and Contrastive Continued Finetuning with Hard Negatives][42]
   5. [4.5 Omnilingual Extension][43]
      1. [4.5.1 Omnilingual Tokenizer Adaptation][44]
      2. [4.5.2 Omnilingual Extension Training][45]
         1. [Bidirectional Contrastive Loss.][46]
         2. [MSE Loss.][47]
   6. [4.6 Cross-Modal Speech Extension][48]
      1. [Architecture and Initialization.][49]
      2. [Distillation Objective.][50]
      3. [Unified Multilingual Capacity.][51]
   7. [4.7 Decoder Finetuning][52]
   8. [4.8 Smaller Models Distillation][53]
      1. [Model Pruning Strategy.][54]
      2. [Knowledge Distillation.][55]
6. [5 Experimental Configuration][56]
   1. [Architecture.][57]
   2. [Sequence-to-Sequence.][58]
   3. [Translation and Contrastive Finetuning.][59]
   4. [Translation and Contrastive Finetuning with Hard Negatives.][60]
   5. [Omnilingual Extension.][61]
   6. [Decoder Finetuning.][62]
   7. [Small Encoders Distillation.][63]
   8. [Cross-modal Speech Extension.][64]
7. [6 Results][65]
   1. [6.1 Cross-lingual Similarity Search][66]
   2. [6.2 Downstream Tasks][67]
      1. [Classification.][68]
      2. [Pair Classification.][69]
      3. [Semantic Textual Similarity.][70]
   3. [6.3 Decoding Capabilities][71]
   4. [6.4 Smaller Encoders Performance][72]
8. [7 Ablations][73]
   1. [7.1 Training Objectives][74]
   2. [7.2 Contrastive Signals][75]
   3. [7.3 Model Initialization][76]
   4. [7.4 Omnilingual Extension Ablations and Analysis][77]
      1. [Effectively learning thousands of new languages.][78]
      2. [Avoiding catastrophic forgetting.][79]
      3. [Omnilingual tokenization warm-up.][80]
         1. [Scaling without sacrificing foundational performance.][81]
            1. [How important is vocabulary size for omnilinguality?][82]
               1. [What makes a language easy to learn?][83]
                  1. [8 Cross-linguality Analysis][84]
                     1. [8.1 Downstream Cross-lingual Transfer][85]
                     2. [8.2 Is Omnilinguality a Curse or a Blessing? Zero-shot Generalization on Unseen Languages][86]
                     3. [8.3 Zero-shot Generalization on Unseen Languages for the Speech Modality][87]
                        1. [9 Spectrum: Zero-shot Omnilingual Speech/Text Language Modeling with \sonar][88]
                           1. [9.1 Architecture][89]
                           2. [Training datasets][90]
                           3. [Evaluation datasets][91]
                           4. [Preprocessing][92]
                           5. [Decoder-Only Architecture (Llama 3B/8B)][93]
                           6. [Encoder-Decoder Architecture (Spectrum)][94]
                           7. [FLOPs Comparison Methodology][95]
                           8. [9.2 Results][96]
                           9. [9.3 Takeaways][97]
                              1. [10 Beyond Sentence-Level Fixed-Size Representations][98]
                                 1. [10.1 OmniSONAR-Token: Better Cross-lingual Token Representations][99]
                                    1. [Evaluation][100]
                                    2. [Training dynamics][101]
                                 2. [10.2 Extending the Context Length of \sonar][102]
                                    1. [Belebele Hard Negatives.][103]
                                    2. [IWSLT2017 Hard Negatives.][104]
                                    3. [Sentence-Level Performance Preservation.][105]
                                    4. [Belebele Paragraph Mining.][106]
                                    5. [IWSLT2017 Document Mining.][107]
                                       1. [11 Conclusion][108]
                                          1. [12 Contribution Statement][109]
                                             1. [12.1 \sonar][110]
                                             2. [12.2 Spectrum][111]
                                             3. [12.3 OmniSONAR-Token][112]
                                             4. [12.4 Acknowledgement][113]
                                                1. [References][114]
                                                2. [13 Data Processing][115]
                                                   1. [13.1 Custom Segment-Any-Text model][116]
                                                      1. [Stage 1: English Segmentation.][117]
                                                      2. [Stage 2: Multilingual Translation.][118]
                                                      3. [Round-trip Translation Quality.][119]
                                                      4. [Cross-lingual Segmentation Consistency.][120]
                                                   2. [13.2 Translation Data Sources][121]
                                                   3. [13.3 Code and Math Translation data generation][122]
                                                   4. [13.4 Hard negatives generation][123]
                                                      1. [Natural Language][124]
                                                      2. [Code and math][125]
                                                   5. [13.5 Language code correspondence][126]
                                                   6. [13.6 Languages breakdown][127]
                                                   7. [13.7 Data Statistics][128]
                                                   8. [13.8 Bible][129]
                                                   9. [13.9 Details on omnilingual language groups][130]
                                                      1. [14 Experimental Configuration][131]
                                                         1. [15 Other Ablations and Analysis][132]
                                                            1. [15.1 Pooling][133]
                                                            2. [15.2 Model Representation Collapse][134]
                                                            3. [15.3 Embedding Dimension Informativeness][135]
                                                            4. [15.4 Analyzing examples to understand where models
                                                               fail][136]
                                                               1. [16 Prompts][137]
                                                                  1. [17 Full Results][138]
                                                                     1. [18 Embedding visualization][139]
                                                                        1. [19 Omninilingual Extension Training
                                                                           Algorithm][140]
[ License: CC BY-SA 4.0 ][141]
arXiv:2603.16606v3 [cs.CL] 18 Jun 2026

]FAIR at Meta \contribution[†]OmniSONAR core contributors \contribution[‡]Spectrum core contributors
\contribution[§]OmniSONAR-Token core contributor

# Omnilingual SONAR:
# Cross-Lingual and Cross-Modal Sentence Embeddings Bridging Massively Multilingual Text and Speech

Omnilingual SONAR Team João Maria Janeiro Pere-Lluís Huguet Cabot Ioannis Tsiamas Yen Meng Vivek Iyer Guillem Ramírez
Loic Barrault Belen Alastruey Xiang “Tony” Cao Yu-An Chung Marta R. Costa-Jussa David Dale Kevin Heffernan Jaehyeong Jo
Artyom Kozhevnikov Alexandre Mourachko Christophe Ropers Holger Schwenk Paul-Ambroise Duquenne [ [padqn@meta.com][142]
(June 18, 2026)

###### Abstract

Cross-lingual sentence encoders have traditionally been limited to a few hundred languages, and have sacrificed
downstream performance to achieve better alignment across languages, limiting their adoption. In this work, we introduce
\sonar a novel family of omnilingual, cross-lingual and cross-modal sentence embedding models that breaks this barrier.
We establish a unified semantic space, natively encompassing text, speech, code and mathematical expressions, while
achieving state-of-the-art downstream performance for an unprecedented scale of thousands of languages, from
high-resource languages to extremely low-resource varieties.

To achieve this scale without representation collapse and while maintaining top-tier performance in the high-resource
languages, we employ a progressive training strategy. We first build a state-of-the-art foundational embedding space for
200 languages using an LLM-initialized Encoder-Decoder, combining token-level decoding with a novel split-softmax
contrastive loss and synthetic hard negatives. Leveraging this strong foundational space, we expand to several thousands
of language varieties via a specialized two-stage teacher-student encoder distillation framework. Further modeling
extensions derived from \sonar address long context inputs and token-centric representations. Finally, we demonstrate
the cross-modal extensibility of this space by seamlessly mapping 177 spoken languages into it.

\sonar

redefines the state of the art for multilingual representation learning. It halves the cross-lingual similarity search
error rate of the previous best models on the 200 languages of FLORES, while also achieving a staggering 15-fold error
rate reduction across 1,560 languages in the BIBLE benchmark. Furthermore, our embedding model enables unprecedented
translation capabilities, outperforming NLLB-3B on several multilingual benchmarks, and surpassing all previous models,
including multi-billion-parameter LLMs, by 15 chrF++ points in 1,560→\rightarrowEnglish translation in the BIBLE
benchmark. Beyond alignment and translation, \sonar demonstrates strong general-purpose capabilities across downstream
embedding tasks on MTEB and programming languages on XLCoST. For the speech modality, our massively multilingual
extension exhibits a 43% lower error rate in cross-lingual and cross-modal similarity search, while achieving 97% of
SeamlessM4T performance in speech-to-text translation, despite being a zero-shot translation model trained only with ASR
data. Finally, by training an encoder-decoder language model, Spectrum, exclusively on English text that processes
\sonar sequences, we unlock immediate high-performance transfer to thousands of languages and the speech modality for
complex downstream tasks. These outstanding results position \sonar as a robust, language- and modality-agnostic
foundation for any downstream usage.

###### keywords:

Multilingual, Cross-lingual, Sentence Embeddings, Sentence Encoder, Large Concept Model.
\correspondence

Paul-Ambroise Duquenne at

###### Contents
1. [1 Introduction][143]
2. [2 Related Work][144]
3. [3 Data][145]
   1. [3.1 Language Sets and Training Stages][146]
   2. [3.2 Natural Language Text Data][147]
   3. [3.3 Code and Math data][148]
   4. [3.4 Speech Data][149]
   5. [3.5 Data Filtering and Upsampling Strategies][150]
   6. [3.6 Hard Negatives Generation][151]
4. [4 Model][152]
   1. [4.1 Tokenizers][153]
   2. [4.2 Architecture and Initialization][154]
   3. [4.3 Sequence-to-Sequence Pretraining][155]
   4. [4.4 Sentence Representation Learning][156]
      1. [4.4.1 Translation and Contrastive Finetuning][157]
      2. [4.4.2 Translation and Contrastive Continued Finetuning with Hard Negatives][158]
   5. [4.5 Omnilingual Extension][159]
      1. [4.5.1 Omnilingual Tokenizer Adaptation][160]
      2. [4.5.2 Omnilingual Extension Training][161]
   6. [4.6 Cross-Modal Speech Extension][162]
   7. [4.7 Decoder Finetuning][163]
   8. [4.8 Smaller Models Distillation][164]
5. [5 Experimental Configuration][165]
6. [6 Results][166]
   1. [6.1 Cross-lingual Similarity Search][167]
   2. [6.2 Downstream Tasks][168]
   3. [6.3 Decoding Capabilities][169]
   4. [6.4 Smaller Encoders Performance][170]
7. [7 Ablations][171]
   1. [7.1 Training Objectives][172]
   2. [7.2 Contrastive Signals][173]
   3. [7.3 Model Initialization][174]
   4. [7.4 Omnilingual Extension Ablations and Analysis][175]
      1. [8 Cross-linguality Analysis][176]
         1. [8.1 Downstream Cross-lingual Transfer][177]
         2. [8.2 Is Omnilinguality a Curse or a Blessing? Zero-shot Generalization on Unseen Languages][178]
         3. [8.3 Zero-shot Generalization on Unseen Languages for the Speech Modality][179]
            1. [9 Spectrum: Zero-shot Omnilingual Speech/Text Language Modeling with \sonar][180]
               1. [9.1 Architecture][181]
               2. [9.2 Results][182]
               3. [9.3 Takeaways][183]
                  1. [10 Beyond Sentence-Level Fixed-Size Representations][184]
                     1. [10.1 OmniSONAR-Token: Better Cross-lingual Token Representations][185]
                     2. [10.2 Extending the Context Length of \sonar][186]
                        1. [11 Conclusion][187]
                           1. [12 Contribution Statement][188]
                              1. [12.1 \sonar][189]
                              2. [12.2 Spectrum][190]
                              3. [12.3 OmniSONAR-Token][191]
                              4. [12.4 Acknowledgement][192]
                                 1. [References][193]
                                 2. [13 Data Processing][194]
                                    1. [13.1 Custom Segment-Any-Text model][195]
                                    2. [13.2 Translation Data Sources][196]
                                    3. [13.3 Code and Math Translation data generation][197]
                                    4. [13.4 Hard negatives generation][198]
                                    5. [13.5 Language code correspondence][199]
                                    6. [13.6 Languages breakdown][200]
                                    7. [13.7 Data Statistics][201]
                                    8. [13.8 Bible][202]
                                    9. [13.9 Details on omnilingual language groups][203]
                                       1. [14 Experimental Configuration][204]
                                          1. [15 Other Ablations and Analysis][205]
                                             1. [15.1 Pooling][206]
                                             2. [15.2 Model Representation Collapse][207]
                                             3. [15.3 Embedding Dimension Informativeness][208]
                                             4. [15.4 Analyzing examples to understand where models fail][209]
                                                1. [16 Prompts][210]
                                                   1. [17 Full Results][211]
                                                      1. [18 Embedding visualization][212]
                                                         1. [19 Omninilingual Extension Training Algorithm][213]

[Refer to caption] Figure 1: The [Refer to caption] training stages. In Stage 1, we train our LLM-initialized
encoder-decoder on translation data with a decoding loss. In Stage 2, we introduce an encoder bottleneck via pooling and
train with a combination of contrastive and decoding objectives. In Stage 3, we introduce hard negatives and continue
training with a split-softmax contrastive objective and the decoding loss. In Stage 4, we extend the space to
omnilingual-level language coverage by training with teacher-student distillation on 4,200 language varieties with a
combination of MSE and Contrastive objectives, while first warming-up the omnilingual tokenization with MSE-based
distillation. Lastly, in Stage 5, we extend the omnilingual space to the speech modality with teacher-student
distillation using ASR data.

## 1 Introduction

Multilingual representation learning has long been a central focus in Natural Language Processing, spanning from
traditional Machine Translation (nllb; kocmi-etal-2025-findings) to the recent surge in multilingual large language
models (workshop2022bloom; aya; gemma3). Furthermore, there has been growing interest in the speech modality, with
advances in both representation learning (w2v_bert; chen2022wavlm) and language modeling (zhang2023speechgpt; moshi;
roy2026personaplex). However, a persistent challenge remains: the extreme scarcity of training data for the vast
majority of the world’s languages for both text and speech. This scarcity has motivated the development of cross-lingual
(sonar; mexma; labse) and cross-modal (duquenne2021multimodal; khurana2022samu; clip) sentence encoders, models that
establish a shared semantic space where sentences with similar meanings are embedded closely together regardless of
their language or modality. These aligned embeddings act as the vital engine for critical applications, including
large-scale parallel data mining for text and speech (schwenk-etal-2021-ccmatrix; duquenne2023speechmatrix), zero-shot
classification (costa-jussa-etal-2024-mutox), translation quality estimation for text and speech
(chen-etal-2023-blaser), and expanding multilingual and multimodal coverage of language modeling, even while training on
monolingual data, as shown in the Large Concept Model (barrault2024large). In general, since their representations are
aligned across languages (and potentially modalities), they unlock multilingual zero-shot downstream performance for
tasks without the need of data in all languages.

Despite their utility, existing encoders face two critical limitations that restrict their widespread adoption. First, a
fundamental performance trade-off exists: achieving good cross-lingual alignment often degrades individual
representation quality, leaving these models trailing behind general-purpose embeddings (wang2024multilingual;
qwen3embedding; embedding_gemma_2025) that do not exhibit language-agnostic alignment, but perform well in downstream
evaluations. Second, coverage is typically restricted to roughly 100 to 200 languages because the field has lacked a
methodology that can effectively scale coverage in data-scarce regimes. Scaling beyond this barrier is often
additionally hindered by the well-documented ‘*curse of multilinguality*’ (massively_multilingual_nmt;
lifting_the_curse_of_multilinguality; alastruey2025interferencematrixquantifyingcrosslingual), where adding more
languages to a fixed-capacity model degrades performance due to parameter competition.

In this work, we introduce \sonar, a novel family of omnilingual, cross-lingual, and cross-modal sentence embedding
models designed to break these barriers. \sonar establishes a unified semantic space spanning an unprecedented 4,200
language varieties, supporting speech, code, and mathematical expressions. To achieve this scale without sacrificing
representation quality, we employ a three-stage progressive training strategy ([Figure˜1][214]):
* •
  
  Step 1: Establishing a state-of-the-art foundation. We first build a foundational embedding space for 200 languages
  using an LLM-initialized Encoder-Decoder architecture. By combining token-level decoding (mexma; sonar) with a novel
  split-softmax contrastive loss and synthetic hard negatives, we capture deep semantic nuances often lost in standard
  alignment techniques.
* •
  
  Step 2: Omnilingual expansion. Leveraging this strong foundation, we expand to thousands of language varieties through
  a teacher-student distillation framework. We project new languages into the space using a hybrid Mean Squared Error
  (MSE) and contrastive loss objective.
* •
  
  Step 3: Speech expansion. This space is then expanded into speech through distillation, aligning spoken sentences and
  their transcriptions through an MSE objective.

\sonar

redefines the state of the art for multilingual and cross-lingual representation learning. \sonar halves the
cross-lingual similarity search error rate of previous best models on the 200 languages of FLORES while achieving a
staggering 15-fold error rate reduction across 1,560 languages in the BIBLE benchmark. \sonar-speech also achieves a 43%
error rate reduction compared to the previous state of the art. Furthermore, these representations are powerful enough
to enable unprecedented translation capabilities, surpassing multi-billion-parameter LLMs by 15 chrF++ points in
1,560→\rightarrowEnglish translation.

The ultimate validation of \sonar’s representational strength is showcased through Spectrum, our encoder-decoder
language model that operates on \sonar’s embeddings. By training Spectrum exclusively on English text to process \sonar
sequences, we unlock high-performance, zero-shot transfer to thousands of languages and the speech modality for complex
reasoning tasks. Spectrum achieves a 16% improvement over LLaMA3.2 3B in XBelebele, due its better multilingual
representations, powered by \sonar and seamlessly transfer this high performance to Speech-XBelebele. These results
demonstrate that \sonar is more than a retrieval tool, it is a robust, language- and modality-agnostic foundation for a
wide range of multilingual speech/text tasks.

Our main contributions are as follows:
* •
  
  A Novel LLM-based Embedding Framework: We introduce an Encoder-Decoder architecture initialized from an
  English-centric pretrained LLM that establishes a state-of-the-art foundational space for 200 languages, natively
  encompassing code and math. In this framework, we introduce a sequence-to-sequence pre-training stage to provide the
  multilingual and translation capabilities the base LLM lacks. Then, we couple a translation reconstruction objective
  with a novel split-softmax contrastive loss, forcing the model to capture nuanced semantic information. This space
  double the performance of the current state of the art in multilingual alignment in FLORES, and closes the gap in
  downstream performance to general purpose models.
* •
  
  A Lossless Omnilingual Extension Framework: We provide a novel method for language expansion combining contrastive and
  MSE objectives. This method enables new languages to be natively integrated into the representation space, while also
  ensuring that the performance of existing languages is preserved. With this expansion we boost the coverage to 4,200+
  language varieties. It achieves a 15-fold error rate reduction across 1,560 evaluated languages in the BIBLE
  framework.
* •
  
  Massively Multilingual Speech Integration: We map the speech modality into this shared space, creating a unified
  speech encoder covering 177 languages that achieves a 43% reduction in cross-lingual cross-modal similarity search
  error rates.
* •
  
  The First Omnilingual Space: We present the most massive sentence embedding space to date, natively encompassing code,
  math, speech and trained on 4,200+ language varieties. Models were trained at various scales, ranging from 1.5B to 39M
  parameters, in order to accommodate a wide range of compute budget constraints.
* •
  
  Unprecedented Omnilingual Decoding: We demonstrate that our omnilingual representations preserve enough fine-grained
  semantic information to drastically outperform multi-billion-parameter LLMs in translation benchmarks when evaluated
  with the paired model decoder.
* •
  
  General-Purpose Capabilities & Omnilingual Analysis: Beyond alignment, \sonar demonstrates strong general-purpose
  performance on MTEB and XLCoST. We provide analysis showing how our methodology transforms the multilinguality curse
  into a blessing for zero-shot generalization, and several ablations for model components.
* •
  
  Zero-shot Omnilingual Speech & Text Language Modeling with \sonar: We show how training an encoder-decoder language
  model (Spectrum) on \sonar representations of English text alone, can unlock zero-shot massively multilingual and
  speech understanding. Achieving 61% on XBelebele and 89% on SpeechSIB zero-shot, Spectrum outperforms bespoke
  fine-tuned models, demonstrating how \sonar can pave the way for radically simple multilingual and multimodal transfer
  in LLMs.

## 2 Related Work

The field of multilingual sentence embeddings has grown rapidly, driven by benchmarks like MTEB
(muennighoff-etal-2023-mteb), xsim/xsim++ (laser; chen-etal-2023-xsim), and MIRACL (zhang-etal-2023-miracl). In our
work, we differentiate between multilingual and cross-lingual sentence embeddings. The former provides multilingual
coverage to general-purpose embeddings, where alignment across languages is only one sub-task among many others. On the
other hand, cross-lingual sentence embeddings build semantic representations by focusing explicitly on cross-lingual
alignment between translations.

##### Cross-lingual Alignment.

Cross-lingual embedding models map vector representations across languages into a shared space. Training on translation
data typically enables semantic alignment via contrastive objectives using encoders only (yang2019improving;
feng-etal-2022-language; miao-etal-2024-enhancing) or non-contrastive objectives with decoder signals (sonar;
janeiro-etal-2025-mexma). In \sonar, we combine both decoder and contrastive losses to build a foundational embedding
space for 200 languages.

##### Contrastive Learning.

While contrastive learning dominates sentence embedding training (gao-etal-2021-simcse), hard negatives remain
underexplored in cross-lingual alignment, with LaBSE (feng-etal-2022-language) reporting negative results.
General-purpose models (wang2024multilingual; sturua2024jinaembeddingsv3multilingualembeddingstask) have successfully
used mined and synthetic negatives. In \sonar, we unlock contrastive objectives with synthetic hard negatives for better
cross-lingual alignment.

##### Teacher-Student Distillation.

Teacher-student distillation is commonly used to extend existing embedding spaces to new languages or new modalities
like speech. This was introduced by reimers-gurevych-2020-making for text and extended to more languages with LASER3
(heffernan2022bitext). duquenne2021multimodal introduced teacher-student training to extend text-only embedding spaces
to the speech modality, extracting a fixed-size semantic representation from speech utterances. khurana2022samu and
sonar followed a similar approach for the LaBSE and SONAR embedding spaces, respectively. charsonar employed
teacher-student distillation to adapt the SONAR encoder to a character-level tokenization, addressing tokenization
bottlenecks in unseen scripts. Although Mean Squared Error (MSE) is the gold standard for distilling representations,
mult_representation_distill demonstrated that contrastive learning objectives can yield sharper decision boundaries and
superior retrieval performance. Our approach for the omnilingual extension synthesizes these insights: we employ a
student-teacher framework similar to reimers-gurevych-2020-making, but scale it to thousands of languages by combining
the stability of MSE with the discriminative power of contrastive losses (mult_representation_distill), while explicitly
adapting the vocabulary to handle the immense linguistic diversity of the 4,200 language varieties we use for training.

##### Massively Multilingual Models.

XLM-R (xlmr) was one of the earliest highly multilingual MLM encoders, while more recently Glot500 (glot500) scaled the
coverage to 500 languages. Several works have proposed massively multilingual encoder-decoders for translation-oriented
tasks, with NLLB (nllb) and SeamlessM4T (seamless) covering 100 languages for speech and 200 for text, respectively,
while Madlad (madlad) offers support for 400 languages. Recent efforts in speech models have scaled coverage for ASR to
an omnilingual level with MMS (mms) and Omni-ASR (omni_asr).

##### Code and Math.

Recent general-purpose models (wang2024multilingual; nussbaum2025trainingsparsemixtureexperts) and code-specific
embeddings (zhang2024code; liu2025codexembed; sureshcornstack) incorporate code and math data. Most code embedding
systems use docstring-implementation pairs (Husain2019CodeSearchNetCE; zhang2024code; sureshcornstack), focusing on
function-level rather than sentence-level representations.

##### Speech Encoders and Embeddings.

Extending text-centric semantic spaces to the speech modality enables powerful cross-modal applications, such as
zero-shot speech translation (duquenne2022t; duquenne2023modular; duquenne:hal-04629427; zeroswot) and cross-lingual
speech mining (duquenne2021multimodal; barrault2023seamlessm4t). Significant gains were observed when training
speech-to-text and speech-to-speech translation models with such mined data (lee2022textless; chen2023speech). SONAR
(sonar) utilized the self-supervised representations of w2v-BERT encoders (w2v_bert) to map the speech modality to the
embedding space, while charSONAR (charsonar) utilized the highly multilingual CTC-based MMS encoder (mms). Here we build
upon these foundations by initializing our student speech encoder with the massively multilingual wav2vec 2.0 model from
Omni-ASR (omni_asr) and projecting speech into the \sonar space.

##### Multilingual and Multimodal Language Modeling.

A plethora of multilingual decoder-only LLMs have been proposed recently, including Llama (llama3), Qwen
(yang2025qwen3), Gemma (gemma3) and Aya (salamanca2025tinyaya). Despite strong progress, recent works on multilingual
(bandarkar-etal-2024-belebele; singh2024globalmmluunderstandingaddressing) and cross-lingual
(marchisio-etal-2024-understanding; iyer-etal-2025-xl) benchmarking have shown that even frontier LLMs continue to
underperform for low-resource languages, underscoring the need for representations that better bridge the multilingual
gap. A similar challenge emerges in the context of multimodal transfer. Speech/Text Language Models (mitsui2024pslm;
moshi) are actively working to bridge the gap between modalities with respect to downstream performance. Various works
have introduced novel methods to improve cross-modal transfer, such as interleaving techniques (nguyen2025spirit) and
chain-of-modality (zhang2023speechgpt) approaches. Finally, several projects have addressed cross-modal transfer by
leveraging shared cross-modal embedding spaces (agostinelli2023musiclm; wang2025mats).

## 3 Data

In this section, we introduce the datasets used to train the \sonar text and speech models, encompassing monolingual
text, parallel translation pairs, code and mathematical expressions, and ASR audio data. We define the specific data
regimes employed across our training stages, outline our sources, and detail our filtering, synthetic generation, and
upsampling strategies. Lastly, we discuss the evaluation datasets used to measure \sonar’s performance.

### 3.1 Language Sets and Training Stages

Throughout our training pipeline, we distinguish between a *foundational* set of base languages and an extended
*omnilingual* set of thousands of language varieties. The foundational set includes 200 languages nllb; sonar, which
overall benefit from extensive data sources and well-established evaluation benchmarks (i.e., FLORES nllb). The base set
additionally includes code and math data. We structure our data usage across the training stages as follows:
* •
  
  Stage 1 (Sequence-to-Sequence Pre-training): We utilize parallel translation data spanning 200 ↔\leftrightarrow 200
  directions among the foundational set of languages.
* •
  
  Stages 2 & 3 (Contrastive Fine-tuning): To effectively leverage contrastive signals and hard negatives, we restrict
  the parallel data to 200 →\rightarrow English translation pairs.
* •
  
  Stage 4 (Omnilingual Expansion): We use parallel translation data covering all 4,200+ language varieties. The strict
  requirement for this stage is that at least one language in the translation pair must be part of the 200+ foundational
  languages, enabling the frozen teacher model to encode it, while the student model learns the new language
  representation.
* •
  
  Stage 5 (Cross-Modal Speech Extension): We use audio-transcription pairs (ASR data) for 177 languages.

### 3.2 Natural Language Text Data

##### Training Datasets.

Translation data aligned at the sentence level has become the standard source of supervised data for learning
multilingual sentence embeddings (sonar; wang2024multilingual; janeiro-etal-2025-mexma). Prior massive multilingual
efforts, such as NLLB (nllb), relied on three primary data streams: human-annotated translations, mined parallel data,
and back-translated segments.

We adopt a similar, but modernized, protocol to construct our training corpus. First, to establish our foundational data
for the 200 foundational languages, we utilize a mixture of human-translated and mined datasets roughly reproducing the
original data composition used to train the NLLB models. Then we generate massive amounts of synthetic translation data
sourced from recent, large-scale monolingual document-based web corpora covering 200 languages and segment these raw
documents into sentences using a custom SaT model (minixhofer-etal-2023-wheres) that has been fine-tuned for extensive
language coverage (see [Section˜13.1][215] for further details). Leveraging the NLLB-3.3B
model¹¹1[https://huggingface.co/facebook/nllb-200-3.3B][216], we translate English sentences from these document sources
into the 200 NLLB-supported languages and non-English sentences into English. Such synthetic data can either be used as
back-translated data (source text is synthetic) or forward translated data (target text is synthetic).

To successfully scale our coverage to an omnilingual level, we aggregate a diverse set of high-quality, massively
multilingual human-annotated translation datasets, including Bible texts, PanLex (panlex) and Tatoeba (tatoeba).
Extensive details for the massively multilingual datasets used in our omnilingual training pipeline are provided in
Appendix [13.2][217].

##### Evaluation Datasets.

We evaluate our models on a series of highly multilingual translation benchmarks:
* •
  
  FLORES (nllb): An n-way parallel benchmark for 202 languages, utilizing English hard negatives for challenging
  similarity search evaluations (xsimplusplus). This covers our foundational set of languages.
* •
  
  FLORES+ (wmt_oldi_24; wmt_oldi_25): An extension of FLORES with 212 test languages, to measure performance on new
  languages within the FLORES domain.
* •
  
  BOUQuET (bouquet): A multi-centric, multi-domain benchmark. We use the X→\rightarrowEnglish directions of version
  v2025.11.13 (omtbigpaper), covering 177 languages, ∼\sim40% of which are outside our foundational language set.
* •
  
  AfroLingu-MT (afrolingumt): A benchmark dedicated to 38 low-resource African languages.
* •
  
  BIBLE: Our primary omnilingual benchmark, covering 1,560 languages (1,420 added during Stage 5). We use John’s Gospel
  (chapters 1-10 for dev, 11-22 for test).

Additionally, we evaluate general-purpose downstream capabilities using the sentence-level MTEB benchmark suite, which
includes the following tasks and benchmarks:
* •
  
  Classification: MassiveIntent & MassiveScenario (fitzgerald2022massive), MTOPDomain & MTOPIntent (li-etal-2021-mtop),
  AmazonCounterfactual (oneill-etal-2021-wish) and SIB200 (adelanietal2024sib).
* •
  
  Pair Classification: XNLI (conneau2018xnli) and its extension, XNLIV2 (upadhyay2023xnli).
* •
  
  Semantic Textual Similarity (STS): STS17 (cer-etal-2017-semeval).

### 3.3 Code and Math data

##### Training datasets.

Although our primary focus is on sentence-level, modality-agnostic representations, we treat code and mathematical
expressions as semantic units that can be mapped into this shared embedding space. In this framework, programming
languages like JavaScript or Go are considered alongside natural languages such as Catalan or Portuguese. To create
translation data that encompasses both programming and natural languages, we have developed a comprehensive pipeline
that overcomes the limitations of traditional docstring-based methods. We focus on sentence-level code snippets and
mathematical expressions whose semantics can be described in a single natural language sentence. Our approach involves
the following steps:
1. (1)
   
   syntax-aware segmentation of code from 7 programming languages using Abstract Syntax Trees
2. (2)
   
   extraction of LaTeX mathematical expressions from scientific corpora
3. (3)
   
   generation of natural language descriptions using LLaMA3.3 70B Instruct,
4. (4)
   
   creation of multilingual versions through back-translation. Quality is ensured through consistency filtering of the
   synthetic data.

Some examples of code and math data are presented in [Table˜1][218].

────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Example 1: Python                                                                                                       
────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Sour│if input_event["enable_points"] or event_info.get("bonus"):                                                        
ce  │                                                                                                                   
────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Targ│The script determines if the "enable_points" in the input_event dictionary is set to True, or if the event_info    
et  │dictionary includes a key called "bonus" with any associated value.                                                
────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Example 2: JavaScript                                                                                                   
────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Sour│const DataHandler = require(’./lib/dataHandler.js’); let windowRef; const dataHandler = new DataHandler({});       
ce  │                                                                                                                   
────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Targ│A constant named DataHandler is initialized by importing a module from a file called dataHandler.js located in a   
et  │folder named lib, then a variable windowRef is declared, and a constant dataHandler is created as a new instance of
    │DataHandler, passing an empty object to its constructor.                                                           
────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Example 3: Math                                                                                                         
────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Sour│Ψ∈L2(W)\Psi\in\mathop{\rm L}\nolimits^{2}(W)                                                                       
ce  │                                                                                                                   
────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Targ│The function Ψ\Psi is a square-integrable function defined on the set W, meaning its square has a finite integral  
et  │over W.                                                                                                            
────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Table 1: Examples of paired data for code and mathematical expressions.

For complete technical details, implementation procedures, and filtering methods, please refer to [Section˜13.3][219].

##### Evaluation datasets.

To further assess \sonar performance on domains including mathematical expressions and programming languages, we
evaluate on GMMLU (singh2024globalmmluunderstandingaddressing), MMLU translated to 41 languages, by pairing questions in
any language to their English equivalent and XLCoST (xlcost), to our knowledge the only snippet-level Code2Code
benchmark. It was built for C++, Java, Python, C#, Javascript, PHP, C and natural language. It contains parallel
programs in all 6 programming languages that were split into parallel code snippets and natural text comments paired to
them. Here we focus solely on the Code2Code snippet retrieval benchmark in a zero-shot fashion, as we never train \sonar
on Code2Code pairs.

### 3.4 Speech Data

##### Training Datasets.

For training speech encoders, we use a portion of the Omnilingual ASR Corpus (omni_asr). The total volume of the data
portion we use is approximately 121k hours covering a total of 177 languages. The selection of these 177 languages is
based on the overlap between the 200 NLLB languages and those covered by the entire Omnilingual ASR Corpus. The data is
composed of publicly available data and internal data. The publicly available data include ALFFA (abate2005alffa;
gelas2012alffa; gauthier2016alffa), LibriSpeech ASR (panayotov2015librispeech), the South African language data of
vanniekerk2017rapid, ASR and TTS data by kjartansson2018crowd, kjartansson2018tts and he2020open, CSS10 (park2019css10),
FOSD (fosd), Zeroth Korean dataset,²²2[https://github.com/goodatlas/zeroth][220] Burmese Speech Corpus (oo2020burmese),
Common Voice v22 (ardila2020common), VoxPopuli (wang2021voxpopuli), VoxLingua-107 (valk2021slt),
RuLS,³³3[https://www.openslr.org/96/][221] the Kokoro Speech
Dataset,⁴⁴4[https://github.com/kaiidams/Kokoro-Speech-Dataset][222] MLS (pratap2020mls), Samrómur
(mollberg2020samromur), the Kazakh Speech Corpus (khassanov2021crowdsourced), iMaSC (gopinath2022imascic),
ParlaSpeech-HR (ljubesic2022parlaspeech), NPSC (solberg2022norwegian), FLEURS (conneau2023fleurs) and NaijaVoices
(emezue2025naijavoices).

##### Evaluation dataset.

We evaluated \sonar speech encoders on the massively multilingual FLEURS test set (conneau2023fleurs), which extends
FLORES-101 (goyal2022flores) to the speech modality. It can be used as a Speech Translation evaluation set, as it
provides speech recordings in 101 languages paired with their English transcriptions.

### 3.5 Data Filtering and Upsampling Strategies

##### Filtering.

Given the vast amount of data available, and that the data regimes required across our experimental setup varies, we
will use a different set of filtering strategies across our work:
* •
  
  We estimate direction-specific thresholds by applying BLASER2 (blaser2) to the high-quality data of FLORES (nllb) dev
  set. We then take the mean, μ(scoresxy)\mu(\text{scores}_{xy}), and standard deviation, σ(scoresx
  y)\sigma(\text{scores}_{xy}), where scoresxy\text{scores}_{xy} is the BLASER2 scores for the pair of languages x-y for
  the 997 examples in the set. We score our paired translation data for the languages covered by BLASER2 to be used
  later as filtering criterion. The filtering criteria applied is μ(scoresxy)−k⋅σ(scoresx
  y)\mu(\text{scores}_{xy})-k\cdot\sigma(\text{scores}_{xy}), where k depends on the training stage.
* •
  
  The vast majority of the languages covered in our data are not supported by BLASER2. To filter this data, we use an
  early version of our omnilingual encoder. Similar to our approach with Blaser-based filtering, we calibrate the
  language-specific similarity thresholds in BIBLE dev. For languages that are not included in the BIBLE development
  set, we apply a relaxed similarity threshold of 0.25. This helps us filter out pairs that are clearly noisy or
  incorrect. We also remove pairs that have extreme source-to-target length ratios, after accounting for the expected
  length of each language.
* •
  
  Given the origin of our data, with some sources being n-way parallel, there are numerous duplicates in either source
  or target sides. To address this, we will apply exact deduplication to both sides of the translation data.
* •
  
  The provenance of our data is diverse, with many of our sources originating from synthetic generation. As a result, we
  will differentiate between ‘primary’ translation data and ‘synthetic’ data.

Data statistics are reported in [Table˜34][223]. Finally, we did not apply any specific data filtering on ASR training
data.

##### Upsampling.

For text modality, in stages 1-3 of training, we sample according to the natural frequencies of the data in our data
mix. For the omnilingual extension, we apply temperature-based sampling with a temperature of 0.6. For ASR data, we
follow an upsampling strategy to balance training data across domains and languages. To this end, we employ a two-step
sampling procedure. First, for each data source, we sample the data for the LL different languages from a distribution

─────────────────────────────────────────────────────────────┬───
pl∼(nlN)βL,p_{l}\sim\left(\frac{n_{l}}{N}\right)^{\beta_{L}},│(1)
─────────────────────────────────────────────────────────────┴───

where l=1,…,Ll=1,...,L, nln_{l} is the amount of unlabeled audio for each language in the current data source, NN is the
total amount of unlabeled audio in the current data source, and βL\beta_{L} is the upsampling factor which controls the
trade-off between high- and low-resource languages during pre-training. Second, we balanced the different data sources
by treating each source as a language and applying the same sampling scheme with a sampling parameter βD\beta_{D}. In
practice, we set both βL\beta_{L} and βD\beta_{D} to 0.5.

### 

[Content truncated]
```
