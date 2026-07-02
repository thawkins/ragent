# Web source

- URL: https://www.intoai.pub/p/metas-large-concept-models-lcms-are
- Title: [
- Captured (UTC): 2026-06-29T16:30:56.889007912+00:00

```text
[
[Into AI]
][1]

# [[Into AI]][2]

SubscribeSign in

# Meta’s Large Concept Models (LCMs) Are Here To Challenge And Redefine LLMs

### A deep dive into ‘Large Concept Model’, a novel language processing architecture and evaluating its performance
### against state-of-the-art LLMs

[
[Dr. Ashish Bamania's avatar]
][3]
[Dr. Ashish Bamania][4]
Jan 02, 2025
12
6
Share
[
][5]Image obtained from the [original research paper][6]

LLMs, or Large Language Models, are dominating all language-based tasks today.

Alongside language tasks, their [extension to images][7], video, and speech has led to state-of-the-art performance as
well.

Thanks for reading Into AI! Subscribe for free to receive new posts and support my work.

Subscribe

Most LLMs are based on the Decoder-only [Transformer][8] architecture.

This architecture is trained to predict the next token based on the context of preceding tokens (the **Next-word
prediction objective**).

*Consider the architecture of GPT as shown below.*

[
][9]GPT Visualised (Image from author’s book “[AI In 100 Images][10]”)

Despite their remarkable performance, this architecture is far from how human cognition works.

Instead of thinking in words, humans understand, reason about and generate ideas at multiple levels of abstraction.

Think about a teacher.

They do not plan every spoken word during their lecture.

Instead, in their mind, they outline a flow of high-level ideas they want to discuss.

These ideas can be distinct or depend on each other. They can also be in different languages or modalities (sound,
image, text, etc.).

In each case, the teacher never considers every single word associated with them.

Instead, these high-level abstract ideas are translated by adding details at the lower levels of abstraction using
different words each time they are discussed.

Such hierarchical high-level abstraction is **not explicitly present** in the current LLM architecture. (*Although they
might implicitly learn such representations during their training.*)

Building on this insight, Meta researchers [published a new architecture][11] called the **Large Concept Model **or**
LCM**.

Instead of reasoning and processing at the token level, LCMs do these in an abstract embedding space.

This embedding space is designed to be independent of language or modality.

*Compare this to the current LLMs that are English-centric and token-based.*

LCMs process, reason in and generate abstract atomic ideas called ‘**Concepts**’ that could correspond to a sentence in
a text document or an equivalent speech utterance in an audio clip. (*These could even be smaller or larger than a
sentence/ single utterance.*)

These generated Concepts can be decoded into any language and modality (that is supported by the embedding) in a purely
zero-shot fashion without re-training the model.

Interestingly, they outperform existing LLMs of similar size in this task and compare well with them in other
language-based tasks.

Here is a story where we deep dive into how Large Concept Models work and see how they stand next to state-of-the-art
LLMs.

Let’s begin!

Subscribe

### An Overview Of How An LCM Works

A Large Concept Model or LCM takes an input of ‘Concepts’.

For a text dataset, it is first segmented into sentences of 10 to 20 tokens in length. These sentences represent
‘Concepts’ for the model.

(*Note that a* ‘*Concept’ can even be bigger than a sentence.*)

These sentences are converted into embeddings using [SONAR][12]. (*We will come back to this soon.*)

The sequence of sentence embeddings is then fed to the LCM, which processes them to output a new sequence of embeddings.

These embeddings are again decoded back to sentences (Concepts) using SONAR.

The encoder and decoder used in the process are fixed and not trained with the model.

**It is interesting to note that the output of an LCM can be decoded into any other language or modality, irrespective
of the input language or modality, without performing any repeat processing.**

[
][13]Overview of how an LCM works (Image from the [original reserach paper][14])

Let’s understand this process step by step.

### How Is Text Broken Down Into Sentences?

The process starts with segmenting large corpora of text into sentences.

Reserachers use the [Segment any Text (SaT)][15] technique for this.

Segmentation is capped at 200 characters, as they found that this number produces the best results in their evaluations.

[
][16]Segmentation of different text examples using Segment any Text (SaT) ([Source][17])

### How Are The Concept Embeddings Generated?

Sentences are then converted into embeddings using SONAR or [Sentence-Level Multimodal and Language-Agnostic
Representations][18].

SONAR embeddings are trained as an [NLLB-1B][19] Encoder/ Decoder architecture using the [Mean Square Error (MSE)
loss][20].

After training the text embeddings, they are extended to the speech modality (initialised using the [W2v-Bert 2.0
Encoder][21]) using the teacher-student approach of [Knowledge Distillation][22], again using the MSE loss.

SONAR impressively supports 200 languages for text processing into and out of English and 76 languages for speech input
and output in English.

[
][23]Overview of training SONAR text and speech embeddings (Image from the [original research paper][24])

### The LCM Architecture

The Large Concept Model is essentially a decoder-only Transformer that is enhanced using two additional components that
handle Concept embeddings.

These are called **Pre-Net **and **Post-Net**.

The **PreNet** normalizes the SONAR embeddings and maps them into the model’s hidden dimension.

The **PostNet** reverses the normalization and maps the model’s outputs back into the SONAR embedding space.

[
][25]Equations for PreNet and PostNet operations where W(pre), W(post), b(pre) and b(post) are learned parameters
[
][26]Normalize and denormalize operations where x is an input SONAR embedding, μ and σ are the median and Interquartile
range (IQR) of the dataset’s SONAR embeddings, respectively.

The researchers termed this structure the** Base-LCM** (*as there are a few more LCM variants*).

The LCM operates with the next-concept (next-sentence embeddings) prediction objective.

In other words, it predicts the next Concept based on the preceding ones (in an autoregressive manner).

This is done by minimizing the **Mean Squared Error (MSE)** between the predicted and the ground truth embedding.

[
][27]Mean Squared Error (MSE) is calculated between the predicted embedding x^(n) and the ground truth embedding x(n)

For a data distribution of sequences of concepts denoted by `q`, the overall training loss is shown below:

[
][28]Overall training loss of the LCM model

An ‘End of Text’ (`eot`) suffix is added to the end of the sequence of concepts, and this suffix is encoded using SONAR
as well.

The model learns to predict this token to indicate the end of the sequence.

At inference time, text generation stops based on two conditions:
* The cosine similarity between the generated embedding and the `eot` embedding crosses a certain threshold (represented
  by `s(eot)`)
* The cosine similarity between consecutive embeddings crosses a certain threshold (represented by `s(prev)`)

Both `s(eot)` and `s(prev)` are set to `0.9` in the original model implementation.

[
][29]Architecture of the Base-LCM (Image from the [original research paper][30])

### Moving Forward To A Diffusion Based LCM

Training the Base-LCM by minimizing MSE loss results in the model producing deterministic responses and predicting an
average representation of all possible continuations.

This averaged embedding might not have any meaningful translation.

It might also not be creative enough for language tasks with multiple valid continuations.

To fix the poor results obtained by the Base LCM, researchers use [Diffusion][31] with the model so that it can generate
meaningful and diverse next-concept embeddings, **probabilistically**.

With Diffusion, an LCM can learn a conditional probability distribution over sentence embeddings and sample from it
during inference time.

Similar to how diffusion generates images, in a Diffusion-based LCM, a **forward noising process **adds noise to
sentence embeddings, and a **reverse denoising process** is used to predict the original noiseless embeddings.

Let’s learn this in more detail.

#### The Two Steps Of Diffusion

In the Forward noising process, sentence embeddings (`x(0)`) are gradually corrupted (noised) with Gaussian noise over a
series of timesteps (`t`) to produce noisy embeddings (`x(t)`).

A Noise schedule determines how the added noise increases over time.

This process is shown using the equation below:

[
][32]Forward noising process where α(t) and σ(t) are functions of a noise schedule that controls the corruption level
over time

In the Reverse denoising process, a Transformer-based model is trained to predict the original or noiseless embeddings
(`x(0)`) from the noisy embeddings (`x(t)`).

[
][33]Reverse denoising process where μ(θ) is the mean predicted by the model for denoising and Σ(θ) is the variance that
is usually fixed.

The objective for training a Diffusion model is to minimize the reconstruction error of the clean embeddings.

[
][34]Reconstruction loss where ϵ is the actual noise added during the forward process and ϵ(θ)(x(t), t) is the model’s
prediction of the added noise during the forward process.

During inference time, the model starts with pure Gaussian noise and iteratively denoises it to produce embeddings.

There’s another idea that is important to learn before we discuss Diffusion-based LCMs. This is **Classifier-Free
Guidance**.

#### Classifier-Free Guidance (CFG)

This is a technique that allows diffusion models to generate outputs based on a given conditioning input [without
requiring a separate classifier][35].

For example, generating an image based on a prompt.

(Before the invention of CFG, a separate classifier was trained with a diffusion model that guided the generation
process aligned to a conditioning input. This technique was called **Classifier-based guidance**.)

Classifier-free guidance works by training a diffusion model to generate both conditioned (output `p(x ∣ y)`, where `y`
is the conditioning variable) and unconditioned outputs (output `p(x)`).

At inference time, the model combines these to balance generation quality and diversity using the following equation:

[
][36]Equation to combine the unconditioned and conditioned outputs at inference time, where γ controls the strength of
conditioned generation or guidance

Influenced by the processes described above, two LCM variants are created:
* One-Tower Diffusion LCM
* Two-Tower Diffusion LCM

Let’s discuss them further.

#### One-Tower Diffusion LCM

This LCM uses a single Transformer for both noising and denoising processes.

Since just one component is involved in processing, the model is called ‘**One-Tower**’.

It predicts the clean next sentence embedding given a noisy input conditioned on previous embeddings.

First, each embedding is concatenated with the corresponding diffusion timestep embedding to give the model information
about the noise level.

Next, learned position embeddings are added to the input embeddings to encode the sequential structure.

This prepares the embeddings that are fed to the LCM.

The Transformer of the LCM uses **[Causal multi-head self-attention][37] **to process this sequence of embeddings.

During training, the input consists of interleaved noisy and clean embeddings, and the attention mask is specifically
designed to allow the noisy embeddings to attend only to the clean embeddings.

This teaches the model to condition its denoising process properly.

[
][38]Training of the One-Tower Diffusion LCM by interleaving clean (light blue) and noisy (blue) embeddings and sampling
different diffusion timesteps (Image from the [original research paper][39])

To enable classifier-free guidance at inference time, Self-attention is occasionally dropped based on a certain
probability during training.

This allows the model to learn both conditional and unconditional generation.

[
][40]Inference with One-Tower Diffusion LCM (Image from the [original research paper][41])

#### Two-Tower Diffusion LCM

This architecture consists of two components or ‘Towers’ that handle processing context and denoising separately.

The first Tower is called **Contextualizer.**

This is a decoder-only Transformer with Causal Self-attention which is tasked to encode the preceding context
embeddings.

The outputs of this tower are fed to the second tower called the **Denoiser**.

This tower consists of a stack of Transformer blocks with Cross-attention, which is tasked to iteratively denoise the
noisy next embedding to predict the clean embedding.

Each layer of the denoiser uses the **[Adaptive Layer Normalization (AdaLN)][42]** mechanism that helps modulate it to
handle varying levels of noise during the denoising process.

Also, the Self-attention layers in the denoiser only attend to the current position and not the preceding noised
context.

[
][43]Training of the Two-Tower Diffusion LCM (Image from the [original research paper][44])

During training, rows of the Cross-attention mask in the denoiser are randomly dropped with a certain probability. This
enables classifier-free guidance during inference.

At inference time, the model starts with a random noise vector.

The denoiser iteratively denoises it guided by the context from the contextualizer and the AdaLN-modulated
cross-attention.

[
][45]Inference with Two-Tower Diffusion LCM (Image from the [original research paper][46])

### There’s A Quantized LCM As Well

Lastly, another variant of LCM is trained where, instead of using the *continuous* sentence embeddings from the SONAR
space, these are quantized or converted into a *discrete* form.

A technique called [Residual Vector Quantization (RVQ)][47] is used for this process.

Two models, **Quant-LCM-d** and **Quant-LCM-c**, are then trained.

The former predicts discrete units, while the latter predicts continuous residuals to refine embeddings during the
generation process.

Subscribe

### Which LCM Variants Perform The Best?

All four LCM variants with 1.6 billion trainable parameters are first pre-trained on the [Fineweb-edu][48] dataset.

#### Performance on Pre-Training

When Pre-training** **is evaluated, Base-LCM shows the lowest L2 scores but the poorest performance on Contrastive
accuracy (CA) and Mutual information (MI) compared to Diffusion-based LCMs and Quant-LCM.

*The reason?*

Since many valid next-sentence continuations exist for a given context when Base-LCM generates an average next-sentence
continuation (by optimizing MSE loss), this may not correspond to any meaningful embedding in the SONAR space.

Also, if you’re new to these metrics —
* ***Contrastive Accuracy (CA)** measures whether the predicted embedding is closer to the true next-sentence embedding
  than to other unrelated embeddings in the batch.*
* ***Mutual Information (MI)** measures how well the predicted sentence aligns contextually with the preceding
  sentences.*

Both Diffusion-based models attain the highest mutual information (MI) with no consistent performance difference between
them.

This shows that the embeddings they produced effectively align with realistic text continuations.

[
][49]Pre-training evaluation results using L2 distance (l2), Round-trip L2 distance (l2r), Contrastive accuracy (CA),
Paraphrasing (PAR), and Mutual information (MI) on four datasets (Image from the [original research paper][50])

#### Performance on Instruction-tuning

All pre-trained models are next fine-tuned on the [Cosmopedia][51] dataset.

Upon evaluation, Diffusion-based models outperform both Quant-LCM and Base-LCM.

A small [Llama model][52] with 24 transformer layers and 1.4 billion parameters is pre-trained and fine-tuned in similar
ways and compared to the LCMs as well.

Notably, this model, called SmaLlama, still outperforms all LCM models on the given metrics, pointing towards better
fluency of generated text.

[
][53]Instruction-tuning evaluation results using [ROUGE-L (R-L)][54] and [Coherence][55] metrics (Arrows tell that
higher values are better) (Image from the [original research paper][56])

### How Do LCMs Compare With LLMs?

#### Inference Efficiency

LCMs work with Concepts rather than tokens, which leads to shorter embeddings.

Since the Attention mechanism used in Transformers has quadratic complexity for the sequence length, the computational
cost and interference time for LCMs are far lower than for LLMs.

It is only for extremely short sentences (<10 tokens) that an LLM is more computationally efficient than an LCM.

[
][57]Computational efficiency measured in FLOPs of different models across different context sizes. (Image from the
[original research paper][58])

#### Short-Context Summarization Task

For this task, the Two-Tower LCM scaled to 7 billion parameters has competitive generative performance to many
instruction fine-tuned LLMs (*higher [ROUGE-L][59] score*), has fewer repetitions in the generated text (*lower [REP-4
score][60]*) and generates more abstractive summaries (*lower OVL-3 score*).

On the other hand, it is less fluent than other LLMs (*lower [CoLA score][61]*).

[
][62]Performance on Short-context Summarization task on CNN DailyMail and XSum datasets. ‘IT’ stands for Intrusction
tuned. (Image from the [original research paper][63])

#### Long-Context Summarization Task

When evaluated for this type of task on the [LCFO dataset][64], the Two-Tower LCM outperforms Mistral-7B-v0.3-IT and
Gemma-7B-IT (*higher [ROUGE-L][65] score*) for compressed summary tasks (5% and 10% of source length).

LCM also has high semantic relevance for the summaries to the source for all conditions (*high [SH-5 score][66]*).

[
][67]Performance on Long-context Summarization task on the LCFO dataset (Image from the [original research paper][68])

#### Summary Expansion Task

On this task of generating longer text for a given summary, LLMs get higher ROUGE-L scores than LCMs.

LCMs generate more paraphrased content but score lower on fluency.

[
][69]Performance on Summary expansion task CNN DailyMail and XSum datasets. WR stands for Word count ratio, which is the
relative length of the generated expansion compared to the input summary. (Image from the [original research paper][70])

#### Zero-Shot Multi-Lingual Performance

SONAR embeddings support text in 200 languages and speech in 76 languages.

When an LCM, backed by SONAR, is evaluated on the [XL-Sum][71] dataset for multilingual abstractive summarization in 42
languages, its results are quite impressive.

As compared to [Llama-3.1–8B-IT][72] (that has been fine-tuned in different languages), Two-Tower LCM (that has never
seen training data language other than English) scores higher in [Multilingual ROUGE-L][73] score in English and other
languages supported by both models.

LCMs also generalize well to other low-resource languages (like Vietnamese, Hausa and Burmese) in a zero-shot setting
(languages it has never seen before).

[
][74][Multilingual ROUGE-L][75] score for Two-Tower-7B-LCM and [Llama-3.1–8B-IT][76] on the [XL-Sum][77] dataset (Image
from the [original research paper][78])

### LLMs Still Remain Undefeated. But For How Long?

LLMs stand strong in these evaluations.

This is because next-sentence (Concept) prediction is more challenging than next-token prediction.

This is because the number of possible sentences, given some context, is virtually unlimited. (*Compare this to token
vocabularies that are usually in the range of 100k.*)

LLMs produce logits for each token in the vocabulary and then use a Softmax output to convert these into probabilities.

The token with the highest probability is selected as the next token (if using [Greedy decoding][79]), or it can be
sampled based on a probability distribution (Temperature sampling).

LCMs, on the other hand, generate text in a **continuous embedding space** rather than choosing tokens from a
vocabulary.

Such a probability distribution can theoretically be learned using Diffusion (*and that’s why Diffusion-based LCMs
perform better than others), but they are still not at their best*.

LCMs still have a long way to go to reach the performance of current state-of-the-art LLMs, but they are really
promising contenders.

It would be exciting to see how their story unfolds when blindly scaling LLMs no longer works.

### Further Reading
* [Research paper titled ‘Large Concept Models: Language Modeling in a Sentence Representation Space’ published in
  ArXiv][80]
* [GitHub repository for the research on Large Concept Models][81]
* [Research paper titled ‘SONAR: Sentence-Level Multimodal and Language-Agnostic Representations’ published in
  ArXiv][82]
* [GitHub repository for SONAR][83]
* [Research paper titled ‘Denoising Diffusion Probabilistic Models’ published in ArXiv][84]

*[Subscribe to ‘Into AI’ — my weekly newsletter where I help you explore Artificial Intelligence from the ground up by
dissecting the original research papers.][85]*

[
][86]

Thanks for reading Into AI! Subscribe for free to receive new posts and support my work.

Subscribe
12
6
Share
PreviousNext

#### Discussion about this post

CommentsRestacks
[User's avatar]
Into AI reply rules
TopLatestDiscussions

No posts

### Ready for more?

Subscribe
© 2026 Dr. Ashish Bamania · [Privacy][87] ∙ [Terms][88] ∙ [Collection notice][89]
[ Start your Substack][90][Get the app][91]
[Substack][92] is the home for great culture
This site requires JavaScript to run correctly. Please [turn on JavaScript][93] or unblock scripts

[1]: /
[2]: /
[3]: https://substack.com/@drashishbamania
[4]: https://substack.com/@drashishbamania
[5]: https://substackcdn.com/image/fetch/$s_!sMpv!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-m
edia.s3.amazonaws.com%2Fpublic%2Fimages%2Fdbbe4e54-9fae-4e85-9fe3-6bb08c3b3568_1200x600.png
[6]: https://arxiv.org/pdf/2412.08821
[7]: https://intoai.pub/p/vision-transformers-straight-from
[8]: https://arxiv.org/abs/1706.03762
[9]: https://substackcdn.com/image/fetch/$s_!WZWH!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-m
edia.s3.amazonaws.com%2Fpublic%2Fimages%2Fefcd886b-3019-4499-a6c5-c39aac6e09d6_800x800.png
[10]: https://bamaniaashish.gumroad.com/l/visual_ai
[11]: https://arxiv.org/pdf/2412.08821
[12]: https://github.com/facebookresearch/SONAR
[13]: https://substackcdn.com/image/fetch/$s_!x1yp!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F0631e061-d8a3-4bd4-a0b0-c70da99c6e1f_695x552.png
[14]: https://arxiv.org/pdf/2412.08821
[15]: https://arxiv.org/abs/2406.16678
[16]: https://substackcdn.com/image/fetch/$s_!VgaZ!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F9cab365d-d068-433e-b6db-ba419c2c44b0_1200x326.png
[17]: https://arxiv.org/pdf/2406.16678
[18]: https://arxiv.org/abs/2308.11466
[19]: https://ai.meta.com/research/no-language-left-behind/
[20]: https://en.wikipedia.org/wiki/Mean_squared_error
[21]: https://arxiv.org/abs/2108.06209
[22]: https://en.wikipedia.org/wiki/Knowledge_distillation
[23]: https://substackcdn.com/image/fetch/$s_!AuQb!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F84c84e03-22bc-47f5-a6ad-9f9f6f5ca754_1200x572.png
[24]: https://arxiv.org/pdf/2412.08821
[25]: https://substackcdn.com/image/fetch/$s_!vt4L!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F78f570dc-4ba5-4e6e-9b2b-0448d77bc572_514x78.png
[26]: https://substackcdn.com/image/fetch/$s_!OmHe!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F9ea2f629-a6a5-4b4c-986b-0f669cb3aa83_604x72.png
[27]: https://substackcdn.com/image/fetch/$s_!pl-G!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F10ead859-62a2-4902-b093-947fb09facee_293x57.png
[28]: https://substackcdn.com/image/fetch/$s_!KC0z!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F380db080-9f11-45c8-8cba-249bfa807a08_474x114.png
[29]: https://substackcdn.com/image/fetch/$s_!CcZi!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F3beeb54c-d8bd-4198-ab8b-d4f4504a11dc_1200x498.png
[30]: https://arxiv.org/pdf/2412.08821
[31]: https://arxiv.org/abs/2006.11239
[32]: https://substackcdn.com/image/fetch/$s_!FQvz!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F313f2c40-351e-4700-805a-77797f95ff61_800x72.png
[33]: https://substackcdn.com/image/fetch/$s_!cwWZ!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F79ad5dda-a23b-4fe5-8dd2-2eacafff8a64_461x40.png
[34]: https://substackcdn.com/image/fetch/$s_!AYxn!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F6eabeb1f-41d7-4c26-a502-272d215205e9_359x42.png
[35]: https://arxiv.org/abs/2207.12598
[36]: https://substackcdn.com/image/fetch/$s_!GZDr!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F0377a24d-372d-4ffd-9872-26925d988ffc_626x54.png
[37]: https://arxiv.org/abs/1706.03762v7
[38]: https://substackcdn.com/image/fetch/$s_!pj2l!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Ff8b51c38-9e06-44d0-a3db-78912d209fc3_399x461.png
[39]: https://arxiv.org/pdf/2412.08821
[40]: https://substackcdn.com/image/fetch/$s_!8Bjf!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F286ca1f2-768e-4265-a491-af3d5b3ea445_437x403.png
[41]: https://arxiv.org/pdf/2412.08821
[42]: https://web.eecs.umich.edu/~stellayu/publication/doc/2022AdaLN.pdf
[43]: https://substackcdn.com/image/fetch/$s_!Caog!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F6164c767-0a3e-4618-b81b-8d23b2acf124_663x421.png
[44]: https://arxiv.org/pdf/2412.08821
[45]: https://substackcdn.com/image/fetch/$s_!fOhE!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F44bda9a0-c827-4140-8582-ec65526f0a41_621x367.png
[46]: https://arxiv.org/pdf/2412.08821
[47]: https://drscotthawley.github.io/blog/posts/2023-06-12-RVQ.html
[48]: https://huggingface.co/datasets/HuggingFaceFW/fineweb-edu
[49]: https://substackcdn.com/image/fetch/$s_!Pa8C!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F912f73e0-080c-489d-98a3-04554da1a28a_800x377.png
[50]: https://arxiv.org/pdf/2412.08821
[51]: https://huggingface.co/datasets/HuggingFaceTB/cosmopedia
[52]: https://arxiv.org/abs/2307.09288
[53]: https://substackcdn.com/image/fetch/$s_!VbWz!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F556ac918-65f5-4f31-8259-d4ca1e88377d_599x188.png
[54]: https://www.microsoft.com/en-us/research/wp-content/uploads/2016/07/was2004.pdf
[55]: https://arxiv.org/abs/2110.07198
[56]: https://arxiv.org/pdf/2412.08821
[57]: https://substackcdn.com/image/fetch/$s_!26gk!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F6518807a-736a-4675-a813-d77460dbce7d_1200x457.png
[58]: https://arxiv.org/pdf/2412.08821
[59]: https://www.microsoft.com/en-us/research/wp-content/uploads/2016/07/was2004.pdf
[60]: https://arxiv.org/abs/1908.04319
[61]: https://arxiv.org/abs/2010.05700
[62]: https://substackcdn.com/image/fetch/$s_!84w4!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F823197e9-bf8b-4ddd-9518-168f5ae85665_1200x656.png
[63]: https://arxiv.org/pdf/2412.08821
[64]: https://arxiv.org/abs/2412.08268
[65]: https://www.microsoft.com/en-us/research/wp-content/uploads/2016/07/was2004.pdf
[66]: https://arxiv.org/abs/2305.13194
[67]: https://substackcdn.com/image/fetch/$s_!TfEY!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F520bfb0b-6930-4641-8396-f957a7b30a90_1200x674.png
[68]: https://arxiv.org/pdf/2412.08821
[69]: https://substackcdn.com/image/fetch/$s_!qIWQ!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2Ffeb40b87-6ee0-4693-95b6-be1e4b6da449_800x435.png
[70]: https://arxiv.org/pdf/2412.08821
[71]: https://arxiv.org/abs/2106.13822
[72]: https://huggingface.co/meta-llama/Llama-3.1-8B
[73]: https://github.com/csebuetnlp/xl-sum/tree/master/multilingual_rouge_scoring
[74]: https://substackcdn.com/image/fetch/$s_!RI51!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F16c3e274-53f0-432d-a445-5128a93f7ecd_1200x486.png
[75]: https://github.com/csebuetnlp/xl-sum/tree/master/multilingual_rouge_scoring
[76]: https://huggingface.co/meta-llama/Llama-3.1-8B
[77]: https://arxiv.org/abs/2106.13822
[78]: https://arxiv.org/pdf/2412.08821
[79]: https://en.wikipedia.org/wiki/Greedy_algorithm
[80]: https://arxiv.org/abs/2412.08821
[81]: https://github.com/facebookresearch/large_concept_model
[82]: https://arxiv.org/abs/2308.11466
[83]: https://github.com/facebookresearch/SONAR
[84]: https://arxiv.org/abs/2006.11239
[85]: https://intoai.pub/
[86]: https://substackcdn.com/image/fetch/$s_!6tji!,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-
media.s3.amazonaws.com%2Fpublic%2Fimages%2F6e8d0bc9-904e-4459-8468-44b8f45025dc_700x335.png
[87]: https://substack.com/privacy
[88]: https://substack.com/tos
[89]: https://substack.com/ccpa#personal-data-collected
[90]: https://substack.com/signup?utm_source=substack&utm_medium=web&utm_content=footer
[91]: https://substack.com/app/app-store-redirect?utm_campaign=app-marketing&utm_content=web-footer-button
[92]: https://substack.com
[93]: https://enable-javascript.com/
```
