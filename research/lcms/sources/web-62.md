# Web source

- URL: https://ai.meta.com/research/publications/omnilingual-sonar-cross-lingual-and-cross-modal-sentence-embeddings-bridging-massively-multilingual-text-and-speech
- Title: [[Meta]][1]
- Captured (UTC): 2026-06-29T16:31:30.856819155+00:00

```text
[[Meta]][1]

[Products][2]

[AI Research][3]

[Resources][4]

[About][5]

[Get Llama][6]

[Try Meta AI][7]


#### RESEARCH

#### SPEECH & AUDIO

# Omnilingual SONAR: Cross-Lingual and Cross-Modal Sentence Embeddings Bridging Massively Multilingual Text and Speech

March 17, 2026

## Abstract

Cross-lingual sentence encoders have traditionally been limited to a few hundred languages, and have sacrificed
downstream performance to achieve better alignment across languages, limiting their adoption. In this work, we introduce
OmniSONAR, a novel family of omnilingual, cross-lingual and cross-modal sentence embedding models that breaks this
barrier. We establish a unified semantic space, natively encompassing text, speech, code and mathematical expressions,
while achieving state-of-the-art downstream performance for an unprecedented scale of thousands of languages, from
high-resource languages to extremely low-resource varieties. To achieve this scale without representation collapse and
while maintaining top-tier performance in the high-resource languages, we employ a progressive training strategy. We
first build a state-of-the-art foundational embedding space for 200 languages using an LLM-initialized Encoder-Decoder,
combining token-level decoding with a novel split-softmax contrastive loss and synthetic hard negatives. Leveraging this
strong foundational space, we expand to several thousands of language varieties via a specialized two-stage
teacher-student encoder distillation framework. Further modeling extensions derived from OmniSONAR address long context
inputs and token-centric representations. Finally, we demonstrate the cross-modal extensibility of this space by
seamlessly mapping 177 spoken languages into it. OmniSONAR redefines the state of the art for multilingual
representation learning. It halves the cross-lingual similarity search error rate of the previous best models on the 200
languages of FLORES, while also achieving a staggering 15-fold error rate reduction across 1,560 languages in the BIBLE
benchmark. Furthermore, our embedding model enables unprecedented translation capabilities, outperforming NLLB-3B on
several multilingual benchmarks, and surpassing all previous models, including multi-billion-parameter LLMs, by 15
chrF++ points in 1,560→English translation in the BIBLE benchmark. Beyond alignment and translation, OmniSONAR
demonstrates strong general-purpose capabilities across downstream embedding tasks on MTEB and programming languages on
XLCoST. For the speech modality, our massively multilingual extension exhibits a 43% lower error rate in cross-lingual
and cross-modal similarity search, while achieving 97% of SeamlessM4T performance in speech-to-text translation, despite
being a zero-shot translation model trained only with ASR data. Finally, by training an encoder-decoder language model,
Spectrum, exclusively on English text that processes OmniSONAR sequences, we unlock immediate high-performance transfer
to thousands of languages and the speech modality for complex downstream tasks. These outstanding results position
OmniSONAR as a robust, language- and modality-agnostic foundation for any downstream usage.

[
Download the Paper
][8]

#### AUTHORS

เขียนโดย

Omnilingual SONAR Team

João Maria Janeiro

Pere Lluís Huguet Cabot

Ioannis Tsiamas

Yen Meng

Vivek Iyer

Guillem Ramirez

[Loic Barrault][9]

Belen Alastruey

Yu-An Chung

Marta R. Costa-jussa

David Dale

Kevin Heffernan

Jaehyeong Jo

Artyom Kozhevnikov

Alexandre Mourachko

Christophe Ropers

[Holger Schwenk][10]

Paul-Ambroise Duquenne

Publisher

arXiv

Research Topics

[Natural Language Processing (NLP)][11]

### Related Publications

June 05, 2026

#### CONVERSATIONAL AI

#### RANKING AND RECOMMENDATIONS

#### Superintelligent Retrieval Agent: The Next Frontier of Agentic Retrieval

Zeyu Yang, Qi Ma, Jason Chen, Anshumali Shrivastava

June 05, 2026

[Read the Paper][12]

May 26, 2026

#### HUMAN & MACHINE INTELLIGENCE

#### THEORY

#### Misalignment Between Backpropagation and the Hierarchy of Brain Responses to Images

Josephine Raugel, Max Seitzer, Marc Szafraniec, Huy V. Vo, Jérémy Rapin, Patrick Labatut, Piotr Bojanowski, Valentin
Wyart, Jean Remi King

May 26, 2026

[Read the Paper][13]

May 20, 2026

#### HUMAN & MACHINE INTELLIGENCE

#### RESEARCH

#### EgoBabyVLM: Benchmarking Cross-Modal Learning from Naturalistic Egocentric Video Data

[Dongyan Lin][14], Phillip Rust, Angel Villar Corrales, Alvin W. M. Tan, Mahi Luthra, Charles-Eric Saint-James, Rashel
Moritz, Sheila Krogh-Jespersen, Vanessa Stark, Surya Parimi, Jiayi Shen, Youssef Benchekroun, Yosuke Higuchi, Martin
Gleize, Tom Fizycki, Nicolas Hamilakis, Manel Khentout, Sho Tsuji, Balázs Kégl, [Juan Pino][15], Michael C. Frank,
Emmanuel Dupoux

May 20, 2026

[Read the Paper][16]

May 18, 2026

#### CONVERSATIONAL AI

#### RESEARCH

#### GIM: Evaluating models via tasks that integrate multiple cognitive domains

Rohit Patel, Alexandre Rezende, Steven McClain

May 18, 2026

[Read the Paper][17]
[
See All Papers
][18]

## Help Us Pioneer The Future of AI

##### We share our open source frameworks, tools, libraries, and models for everything from research exploration to
##### large-scale production deployment.

[
Join our Team
][19]
[Our approach][20]
[About AI at Meta][21]
[People][22]
[Careers][23]
[Research][24]
[Infrastructure][25]
[Resources][26]
[Demos][27]
[Meta AI][28]
[Explore Meta AI][29]
[Get Meta AI][30]
[AI Studio][31]
[Latest news][32]
[Blog][33]
[Newsletter][34]

Foundational models

[Llama][35]
[
][36]
[
][37]
[
][38]
[
][39]

Our approach

[Our approach][40][About AI at Meta][41][People][42][Careers][43]

Research

[Research][44][Infrastructure][45][Resources][46][Demos][47]

Meta AI

[Meta AI][48][Explore Meta AI][49][Get Meta AI][50][AI Studio][51]

Latest news

[Latest news][52][Blog][53][Newsletter][54]

Foundational models

[Llama][55]
[
][56]
[
][57]
[
][58]
[
][59]
[Privacy Policy][60]
[Terms][61]
[Cookies][62]

Meta © 2026

[
][63]
[
][64]
[
][65]
[
][66]

[1]: #
[2]: #
[3]: #
[4]: #
[5]: #
[6]: https://www.llama.com/?utm_source=ai_meta_site&utm_medium=web&utm_content=AI_nav&utm_campaign=09252025_moment
[7]: https://applink.meta.ai/?utm_source=ai_meta_site&utm_medium=web&utm_content=AI_nav&utm_campaign=04082026_moment
[8]: https://scontent.fbkk12-1.fna.fbcdn.net/v/t39.2365-6/653708461_776946831802909_5860622253438220368_n.pdf?_nc_cat=10
1&ccb=1-7&_nc_sid=3c67a6&_nc_ohc=KZ6lSq9Uxn8Q7kNvwGTWbRS&_nc_oc=Adof2u-BXZUtsQrcD0fLoY6SgdGvz2z8Z96mMCwOl9VokLXdKvZZVsUa
2GIjwc4-jH5QgJQLbf8AkswRBp9ujxw2&_nc_zt=14&_nc_ht=scontent.fbkk12-1.fna&_nc_gid=7P6zuOUvIPyETbcVJ_HNQg&_nc_ss=7f20f&oh=0
0_Af81MWIBAqMnYaHKnQwYdM9kyTHgAZkHPrZeyw87FUikbQ&oe=6A485C70
[9]: /people/1679073599776654/loic-barrault/
[10]: /people/271799079300984/holger-schwenk/
[11]: /research/nlp/
[12]: /research/publications/superintelligent-retrieval-agent-the-next-frontier-of-agentic-retrieval/
[13]: /research/publications/misalignment-between-backpropagation-and-the-hierarchy-of-brain-responses-to-images/
[14]: /people/2023351861723323/dongyan-lin/
[15]: /people/776668760684735/juan-pino/
[16]: /research/publications/egobabyvlm-benchmarking-cross-modal-learning-from-naturalistic-egocentric-video-data/
[17]: /research/publications/gim-evaluating-models-via-tasks-that-integrate-multiple-cognitive-domains/
[18]: /global_search/?content_types%5B0%5D=publication&page=1
[19]: /join-us/
[20]: /about
[21]: /about
[22]: /results/?content_types%5B0%5D=person&sort_by=random
[23]: https://www.metacareers.com/jobs/?is_leadership=0&sub_teams[0]=Artificial%20Intelligence&is_in_page=0
[24]: /research
[25]: /infrastructure
[26]: /resources
[27]: https://aidemos.meta.com/
[28]: /meta-ai/
[29]: /meta-ai/
[30]: /get-meta-ai/
[31]: /ai-studio/
[32]: /blog
[33]: /blog
[34]: /subscribe
[35]: https://www.llama.com/
[36]: https://www.facebook.com/aiatmeta/
[37]: https://twitter.com/aiatmeta/
[38]: https://www.linkedin.com/showcase/aiatmeta
[39]: https://www.youtube.com/@aiatmeta
[40]: /about
[41]: /about
[42]: /results/?content_types%5B0%5D=person&sort_by=random
[43]: https://www.metacareers.com/jobs/?is_leadership=0&sub_teams[0]=Artificial%20Intelligence&is_in_page=0
[44]: /research
[45]: /infrastructure
[46]: /resources
[47]: https://aidemos.meta.com/
[48]: /meta-ai/
[49]: /meta-ai/
[50]: /get-meta-ai/
[51]: /ai-studio/
[52]: /blog
[53]: /blog
[54]: /subscribe
[55]: https://www.llama.com/
[56]: https://www.facebook.com/aiatmeta/
[57]: https://twitter.com/aiatmeta/
[58]: https://www.linkedin.com/showcase/aiatmeta
[59]: https://www.youtube.com/@aiatmeta
[60]: https://www.facebook.com/about/privacy/
[61]: https://www.facebook.com/policies/
[62]: https://www.facebook.com/policies/cookies/
[63]: https://www.facebook.com/aiatmeta/
[64]: https://twitter.com/aiatmeta/
[65]: https://www.linkedin.com/showcase/aiatmeta
[66]: https://www.youtube.com/@aiatmeta
```
