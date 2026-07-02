# Web source

- URL: https://gonzoml.substack.com/p/lcm-large-concept-model
- Title: [
- Captured (UTC): 2026-06-29T16:29:30.069782011+00:00

```text
[
[Gonzo ML]
][1]

# [Gonzo ML][2]

SubscribeSign in

# LCM: Large Concept Model

### Language Modeling in a Sentence Representation Space

[
[Grigory Sapunov's avatar]
][3]
[Grigory Sapunov][4]
Jan 04, 2025
3
Share

**Large Concept Models: Language Modeling in a Sentence Representation Space**
**Authors:** *LCM team, Loïc Barrault, Paul-Ambroise Duquenne, Maha Elbayad, Artyom Kozhevnikov, Belen Alastruey, Pierre
Andrews, Mariano Coria, Guillaume Couairon, Marta R. Costa-jussà, David Dale, Hady Elsahar, Kevin Heffernan, João Maria
Janeiro, Tuan Tran, Christophe Ropers, Eduardo Sánchez, Robin San Roman, Alexandre Mourachko, Safiyyah Saleem, Holger
Schwenk*
**Paper:** [https://arxiv.org/abs/2412.08821 ][5]
**Code:** [https://github.com/facebookresearch/large_concept_model][6]

Another impressive work from Meta (BLT was also their project).

Thanks for reading Gonzo ML! Subscribe for free to receive new posts and support my work.

Subscribe
[

## BLT: Byte Latent Transformer

][7]
[Grigory Sapunov][8]
·
December 23, 2024
[[BLT: Byte Latent Transformer ]][9]

Title: Byte Latent Transformer: Patches Scale Better Than Tokens

[
Read full story
][10]

## Working in a Sentence Representation Space

We want to work at different levels of abstraction. The brain, obviously, can do this, and our thinking doesn't operate
solely at the word level. We have some kind of top-down process for solving complex problems. For example, when creating
a long document, we (usually) first plan its high-level structure, and then start adding details at lower levels of
abstraction. Current LLMs work rather differently, with the token level being their everything. Maybe there are some
implicit hierarchical representations inside them, but having them explicitly would be more useful. Having reasoning and
planning at this level would also be valuable. It would be even better to have this level independent of specific
language and modality — the same thought can be expressed in different languages and modalities, such as text or voice.

We want to (again) move away from tokens. In [BLT, we moved to latent tokenization][11] not visible from the outside,
and **here, we're moving to a higher-level embedding space for concepts.** We want to model the reasoning process at the
semantic level rather than tokens and have an **LCM (Large Concept Model)** instead of an LLM.

[
][12]

To test this idea, we limit ourselves to two levels: 1) subword tokens and 2) concepts. A concept is understood as an
abstract indivisible idea, often corresponding to a sentence in a document or an utterance in spoken language. Unlike
single words, this is a suitable element for achieving language independence.

For this approach, we need a sentence embedding space with an accessible encoder and decoder. They chose FAIR's
[SONAR][13], which supports 200 languages (all languages from the [No Language Left Behind][14] project) for text
inputs/outputs, 76 languages for speech input, and English for speech output. SONAR outperforms LASER3 and LabSE in
quality and is available in the [repo][15]. The embedding size is 1024 float numbers (which means a typical sentence in
embeddings will take several times more space; in the work, 1TB of text required approximately 15-20TB of embeddings).

[
][16]

With all this, we can get a sequence of concepts (sentence embeddings) from the input text through the SONAR encoder.
Then process this sequence with LCM, generating a new sequence of concepts at the output. And then decode it with SONAR
into a sequence of tokens. SONAR's encoder and decoder are taken as-is and aren't trained, only LCM is trained.

What's beautiful is that the same sequence of concepts from LCM can be decoded into different languages and modalities
without needing to rerun the entire reasoning process. LCM doesn't know anything about the languages or modalities from
which its input data came. This creates elegant modularity — train an encoder/decoder for a new language, and the
already trained LCM automatically works with it. The paper includes a table about the number of supported languages in
different modalities; LCM, with its 200 languages for text, beats everyone here, but it's not entirely clear how the
numbers for GPT/Gemini/Claude were obtained, as I haven't seen a declared list of supported languages for these models.
Also, it would be interesting to look at concepts that don't decode equally well into different languages.

[
][17]

A separate benefit of this approach for processing long documents is that the sequence of concepts is at least an order
of magnitude shorter than the sequence of tokens, making it easier to process with a transformer with a fixed context
window (or more can fit in).

To some extent, LCM resembles LeCun's [JEPA][18], which also predicts representations of the next observation in
embedding space. But JEPA focused on learning such a space in a self-supervised mode, while LCM focuses on accurate
prediction in an existing embedding space (though merging these two approaches probably makes sense).

So, working in embedding space, to train LCM we need to prepare a text dataset and convert it through SONAR into
embeddings, one for each sentence. In practice, this isn't so simple; exact segmentation isn't always easy due to
dataset errors or specific formatting. Additionally, long sentences can be too complex for encoding/decoding through
SONAR, and the quality will suffer. Eventually, they chose [Segment any Text (SaT)][19] for sentence splitting with an
additional segment length limit — anything longer than 250 characters (we'll see this number again soon) gets split;
this method is called SaT Capped.

LCM must conditionally generate continuous embeddings based on context. This differs from LLM work, where you need to
output a probability distribution over discrete tokens in the vocabulary. A straightforward approach would be to train a
transformer to generate embeddings with an objective of minimizing MSE loss. This would be called **Base-LCM**. This
isn't so simple because a given context can have many suitable but semantically different continuations, as seen in
image generation with diffusion models, where one prompt produces quite different images. And in general, that area has
many developments in learning conditional probability distributions for continuous data, so another logical variant to
try is a diffusion model, **Diffusion-based LCM**. Finally, another option is quantization and return to the task of
generating discrete elements, **Quantized LCM**.

## LCM Architectures

Let's go through the LCM variants in detail.

**Base-LCM** serves as a baseline; it's a standard transformer decoder that converts a sequence of preceding concepts
(sentence embeddings) into a sequence of future ones. The transformer is surrounded by two simple networks on the input
and output sides, *PreNet* and *PostNet*, handling normalization/denormalization and projection of SONAR embeddings into
and out of the model's dimension. It's trained on a semi-supervised task of predicting the next concept, minimizing MSE
loss relative to ground truth. Training documents are appended with an "End of text" suffix, enabling learning to
generate variable-length documents. During inference, one stop criterion checks the proximity of the generated embedding
to this suffix's embedding and stops generation if the proximity exceeds a given threshold; another stop criterion looks
at the cosine similarity between current and previous embeddings and stops if it's above the threshold (both thresholds
are set to 0.9).

[
][20]

**Diffusion-based LCM** also autoregressively generates concepts, one at a time, performing a specified number of
denoising steps for each generated concept. It uses classifier-free diffusion guidance. There are One-Tower and
Two-Tower model versions. In the first case, it's one transformer tower doing everything. In the second, a separate
tower (*contextualizer*) handles encoding the preceding context, while the second (*denoiser*) generates new concept
embeddings and uses cross-attention to look at the context from the first tower.

[
][21]

**Quantized LCM** uses Residual Vector Quantization and then works similarly to regular LLMs predicting discrete units.
Here, you can use temperature and top-p/top-k parameters. They try to build the architecture as similar as possible to
Diffusion-based LCM for easier comparison.

All models are made with approximately **1.6B** trainable parameters. Base-LCM has 32 layers and 2048 hidden dimension,
One-Tower is similar. Two-Tower has 5 layers in the contextualizer and 13 in the denoiser. Quant-LCM is similar to
One-Tower but with different output dimension.

## Evaluations

They pre-trained on [FineWeb-Edu][22] (apparently English-only), evaluated pre-training results on four datasets
(ROC-stories, C4, Wikipedia, Gutenberg) using next sentence prediction metrics.

[
][23]

Overall, diffusion LCMs showed better results. They did instruction-tuning on Cosmopedia, with similar results. Along
the way, they showed the importance of hyperparameters for diffusion.

[
][24]
[
][25]

They showed that LCM scales well with context length, requiring fewer FLOPS for the same context length in tokens. I
understand this is purely because a concept corresponds to a sentence of multiple tokens, so there are fewer concepts,
quadratic attention requires fewer resources (and this heavily depends on how paragraphs are split into sentences). It's
also important to remember that each LCM inference includes three steps: 1) SONAR encoding, 2) transformer-LCM, 3) SONAR
decoding. On very short sentences (less than 10 tokens), LLM is better than LCM in FLOPS.

[
][26]

They investigated the *fragility* of SONAR's embedding space. Fragile embeddings are those where small perturbations in
the space can lead to substantial information loss during decoding. This can be evaluated, for example, by BLEU between
the original and post-perturbation text (called *Auto-Encoding BLEU*). They fine-tuned a decoder that is more resistant
to noise, which performs better by this metric.

[
][27]

You can also evaluate by cosine similarity through an encoder independent of SONAR. They drew curves showing how metrics
deteriorate with increasing text length and noise level. It gets really bad at lengths over 250 characters (the maximum
length we chose to split sentences). Meanwhile, metrics behave somewhat differently, and SONAR fine-tuning helps quite a
bit. In short, these embeddings aren't simple, and there's room for investigation.

[
][28]

After experiments, they scaled up the Two-Tower diffusion variant to **7B**. This version has 5 layers in the
contextualizer, 14 in the denoiser, and a hidden dimension of 4096. They pre-trained on 2.3B documents with 2.7T tokens
and 142.4B concepts/sentences. The context was expanded to 2048 concepts. This resulted in the **Two-Tower-7B** model.
They fine-tuned it on open instruction tuning datasets, creating **Two-Tower-7B-IT**.

They tested summarization on CNN DailyMail and XSum. They looked at Rouge-L, input trigram overlap ratio (OVL-3), output
four-gram repetition ratio (REP-4), metrics from Q4, Q5 from [SEAHORSE][29], and another metric from a classifier
trained on CoLA about whether sentences are linguistically acceptable.

[
][30]

Baselines for comparison were T5-3B, Gemma-7B, Llama-3.1-8B, Mistral-7B-v0.3. T5 is much smaller but, unlike others, was
fine-tuned on the given datasets.

[
][31]

LCM outperformed instruct-finetuned LLM in Rouge. OVL-3 shows summaries are more abstractive than extractive. REP-4
shows fewer repetitions, CoLA classifier shows less fluent summaries. But human ground truth also scores lower on this
metric than LLMs.

[
][32]

Long-context summarization is generally better than Mistral and Gemma but worse than Llama (they suspect contamination
or poor performance of other models on long context).

## LCM extensions

The paper then proposes several LCM extensions.

Summary Expansion involves writing long text from short summaries, essentially the reverse of summarization, though the
task isn't to recreate the original document but rather generate coherent text. Based on available metrics, it generally
performs worse than LLMs.

[
][33]

In Zero-shot generalization, they test the model on other languages available in XLSum. LCM saw nothing but English in
training, while Llama was fine-tuned on eight languages from the list and saw many others in pre-training. Overall, LCM
generalizes very well to other languages, often beating Llama, especially on low-resource languages. What numbers would
we see if LCM trained on a proper multilingual corpus?

[
][34]

For the Explicit planning task, another *planning model (LPM)* generates a high-level plan of what should be done next,
and LCM generates a sequence of concepts + break concept (which can indicate paragraph end) based on this plan. The
final setting is called *LPCM*. They evaluated coherence in LLM-as-a-judge mode (Llama-3.1-8B-IT). On [Cosmopedia][35],
LPCM seemed better than just LCM, but does 2.82 ± 0.62 versus 2.74 ± 0.70 mean anything with such large and intersecting
confidence intervals? Not sure, it's a peculiar setting — the dataset is generated by LLM, evaluated by LLM, there are
many questionable factors here.

[
][36]

Well, okay, this is a proof of concept work, and as proof it's good. The fact that they haven't set a new state of the
art right now doesn't matter. We probably won't see a new ConceptLlama tomorrow, but this is an interesting approach,
and I like it. I also don't believe that predicting the next token is what we globally need, and it's good to be able to
work at a level higher than usually happens in LLM. I also really like the modularity. It will be interesting to see how
this develops further.

Thanks for reading Gonzo ML! Subscribe for free to receive new posts and support my work.

Subscribe
3
Share
PreviousNext

#### Discussion about this post

CommentsRestacks
[User's avatar]
TopLatestDiscussions

No posts

### Ready for more?

Subscribe
© 2026 Grisha · [Privacy][37] ∙ [Terms][38] ∙ [Collection notice][39]
[ Start your Substack][40][Get the app][41]
[Substack][42] is the home for great culture
This site requires JavaScript to run correctly. Please [turn on JavaScript][43] or unblock scripts

[1]: /
[2]: /
[3]: https://substack.com/@gonzoml
[4]: https://substack.com/@gonzoml
[5]: https://arxiv.org/abs/2412.08821
[6]: https://github.com/facebookresearch/large_concept_model
[7]: https://gonzoml.substack.com/p/blt-byte-latent-transformer
[8]: https://substack.com/profile/1253653-grigory-sapunov
[9]: https://gonzoml.substack.com/p/blt-byte-latent-transformer
[10]: https://gonzoml.substack.com/p/blt-byte-latent-transformer
[11]: https://gonzoml.substack.com/p/blt-byte-latent-transformer
[12]: https://substackcdn.com/image/fetch/$s_!tIe3!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Fdcab57af-01d5-4f3b-abb1-66fe09ab95c3_1229x617.png
[13]: https://arxiv.org/abs/2308.11466
[14]: https://arxiv.org/abs/2207.04672
[15]: https://github.com/facebookresearch/SONAR
[16]: https://substackcdn.com/image/fetch/$s_!ndUX!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F1341b0f8-52e1-475b-afc0-d17cec5c2d7e_1188x552.png
[17]: https://substackcdn.com/image/fetch/$s_!1LQo!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Fd3d5ea4b-3245-4327-8ed5-050db4b3c506_1219x455.png
[18]: https://openreview.net/pdf?id=BZ5a1r-kVsf
[19]: https://github.com/segment-any-text/wtpsplit
[20]: https://substackcdn.com/image/fetch/$s_!IBVT!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Ff59a889a-a74d-41e2-9931-3cb7e2db71aa_1215x504.png
[21]: https://substackcdn.com/image/fetch/$s_!Tzyu!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F2b269bca-a2a1-4318-a1da-6ef14766c740_1212x489.png
[22]: https://huggingface.co/datasets/HuggingFaceFW/fineweb-edu
[23]: https://substackcdn.com/image/fetch/$s_!_fk1!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F1c85f98f-4844-45af-acf1-6d6458f4d972_1213x508.png
[24]: https://substackcdn.com/image/fetch/$s_!6RPq!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F5833d0e1-73f8-47d7-b78a-f3dc4ce79de1_1210x698.png
[25]: https://substackcdn.com/image/fetch/$s_!o38F!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F9a3be2ae-14df-43e1-ad28-53d6dfdd41e0_1137x377.png
[26]: https://substackcdn.com/image/fetch/$s_!7A1L!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F7c1009c1-ff65-4ef5-96f5-76ad6a1f4628_1196x590.png
[27]: https://substackcdn.com/image/fetch/$s_!YH-s!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F98e81857-9fce-4711-bd67-d96eba93ffeb_1226x650.png
[28]: https://substackcdn.com/image/fetch/$s_!Ou3B!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Fe0c2f972-5892-4924-938c-f760dba5386d_1024x484.png
[29]: https://arxiv.org/abs/2305.13194
[30]: https://substackcdn.com/image/fetch/$s_!mhQu!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F9e08bc09-4aef-48d6-a71e-b268d867bb4d_1213x354.png
[31]: https://substackcdn.com/image/fetch/$s_!dtp7!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Fe617754b-b1b3-42a9-8c52-c0bfac207166_1214x722.png
[32]: https://substackcdn.com/image/fetch/$s_!ECNT!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Ff9068cb4-8316-4a4c-ba27-0b648a3cd650_1218x765.png
[33]: https://substackcdn.com/image/fetch/$s_!8w8H!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F8fac64a8-2665-43d2-99f2-fa4477df279e_1204x579.png
[34]: https://substackcdn.com/image/fetch/$s_!u9Pz!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F56031406-e756-4942-87dc-a2eea3ec694f_1192x560.png
[35]: https://huggingface.co/datasets/HuggingFaceTB/cosmopedia
[36]: https://substackcdn.com/image/fetch/$s_!ntdP!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F99c22a0c-8169-4f68-b462-1b74786901d6_1124x265.png
[37]: https://substack.com/privacy
[38]: https://substack.com/tos
[39]: https://substack.com/ccpa#personal-data-collected
[40]: https://substack.com/signup?utm_source=substack&utm_medium=web&utm_content=footer
[41]: https://substack.com/app/app-store-redirect?utm_campaign=app-marketing&utm_content=web-footer-button
[42]: https://substack.com
[43]: https://enable-javascript.com/
```
