# Web source

- URL: https://medium.com/@roseserene/sonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T16:31:32.031973961+00:00

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

# SONAR: AI with Multimodal Language-Agnostic Representations

[
[Tech_with_KJ]
][7]
[Tech_with_KJ][8]
7 min read
·
Mar 25, 2025
[
][9]

--

1

[
][10]
[][11]
[

Listen

][12]

Share

Let’s talk about Meta’s SONAR Embeddings — short for Sentence-Level Multimodal and Language-Agnostic Representations.
Unlike traditional translation models such as BERT or RoBERTa, SONAR embeddings prioritize understanding and preserving
meaning rather than mere word-to-word translations.

What are SONAR Embedding?

SONAR Embeddings Space fundamentally changes the way machines perceive language and meaning. It is multimodal fixed-size
sentence embedding space. Its single text encoder, covering 200 languages, substantially outperforms existing sentence
embeddings. The aim is to capture the essence or concept behind sentences, regardless of the language or modality. Thus,
SONAR facilitates translation tasks, including Text-to-Text and Speech-to-Text, significantly improving machine
translation robustness.

Philosophy Behind SONAR Embeddings

The core philosophy behind this is the preservation of underlying thoughts and concepts across languages, regardless of
the exact wordings used. Imagine someone is presenting this topic using same Power Point Presentation or topic in
different sessions through German, or same language but by different Speakers or even the same speaker at different
times. Although the specific words or phrases might vary across each session, the core meaning and concepts remain
consistent. Regardless of Languages, all humans process the meaning of a sentence in a similar way. SONAR precisely aims
to maintain this conceptual consistency across different languages and modalities.

**Practical Applications**

**Concept Prediction**

Traditional Large Language Models (LLMs) predict the next word or sentence (NextSentence Prediction, NSP). In contrast,
Large Concept Models (LCMs), underpinned by SONAR embeddings, predict the next concept. This represents a significant,
allowing AI to focus on underlying meanings rather than surface-level word predictions.

**Universal Search Engine for Meaning**

Imagine searching a proverb in English and effortlessly retrieving equivalent expressions across languages, even when
phrased entirely differently. For instance, the English proverb “A bad workman blames his tools” translates meaningfully
to the Hindi expression “नाच न आवे आंगन टेढ़ा” preserving semantic essence rather than literal translations.

**Zero-Shot Translation**

SONAR embeddings enable zero-shot translation, translating languages unseen by the model without requiring parallel
data. This capability significantly impacts languages with limited available training data, enabling effective
translation and transcription even in low-resource contexts.

**Sentence Embeddings and Semantic Similarity**

Let’s touch base upon basics first. Sentence embeddings represent sentences numerically through vectors, capturing
semantic information. Mathematically, this can be represented by cosine similarity:

Press enter or click to view image in full size

This ensures sentences with similar meanings cluster closely in vector spaces, enhancing semantic understanding i.e. we
measure the closeness of vectors intern measuring the similarity of sentences in terms of respective meanings.

Press enter or click to view image in full size

In the same we also can measure similarity of multilingual sentences and cluster them together like shown below:

Press enter or click to view image in full size

**Standard vs. SONAR Machine Translations**

Conventional translation models use token-based cross-self-attention mechanisms within an encoder-decoder architecture:

To understand self-attention more intuitively, consider the sentence examples:

· “I crossed the river to reach the **bank**.”

· “I carried my cheque to the **bank**.”

The word “bank” appears in both, but its meaning changes entirely. In the first sentence, the self-attention mechanism
enables the model to give more “attention” to the word “river” when interpreting the word “bank,” understanding it as a
riverbank. In the second sentence, the model “notices” more to the word “cheque,” associating the context of “bank” as a
financial institution. This is the crux of self-attention: within a sentence, each word dynamically weighs its
relationship with surrounding words to extract context-specific meanings.

Self-attention is termed such because the mechanism involves a word or token attending to others within the same
sentence — “paying attention” to itself, so to speak, to resolve meaning ambiguity based on context. In encoder-decoder
models, the encoder generates a contextual representation (context vector), which the decoder uses to generate
translated output. This self-awareness in attention pinpoints the semantic interpretation in transformer models.

SONAR builds upon this foundation but diverges from traditional reliance on token-based models. Instead, it employs a
denoising autoencoder alongside the encoder-decoder architecture. This introduces noise — such as spelling errors or
paraphrasing — to train the model robustly in reconstructing meaning rather than literal text.

**Creation and Training Phases of SONAR**

Press enter or click to view image in full size

**Phase 1: Text-Based Training**

**Phase 1: Encoder-Decoder with Denoising Autoencoder**

Let’s go a bit deeper into what an autoencoder is, why it’s important, and how denoising works in this context.

Noise is deliberately added to the input to help the model learn **robust representations**. It’s the job of the
autoencoder to remove this noise and reconstruct a cleaner, more meaningful output.

The noise can be introduced in various ways — by removing characters, tokens, or words, or by adding unwanted elements
such as random characters, spelling errors, or paraphrased fragments. This noisy version is then fed into the encoder,
as shown in the top-right diagram.

The **encoder compresses** the input into a fixed-size representation. Once compressed, it’s passed to the decoder,
which then **decompresses** or reconstructs the original sentence.

One key distinction here is how SONAR differs from standard transformer-based models. Traditional machine translation
models rely on **sequence-to-sequence** architectures that generate outputs token-by-token in an autoregressive manner.
However, SONAR uses a **fixed-size sentence embedding** — specifically a 1024-dimensional vector — generated after
encoding. This is a defining feature of SONAR.

The decoder doesn’t process the sequence token-by-token. Instead, it reconstructs or translates the input directly from
the compressed embedding.

To visualize this, think of corrupted images being restored. In the image below , we see heavily distorted digits going
through an encoder-decoder pipeline, resulting in clear, recognizable outputs. You can think of the same thing happening
with sentences — corrupted or noisy text gets transformed back into its original, clean form.

Press enter or click to view image in full size

SONAR training begins by initializing from Meta’s NLLB-1B model, emphasizing denoising, translation, and auto-encoding
objectives. This results in high-quality embeddings capturing essential sentence meanings. Below is the quick reference
sequence for mind mapping.

**SONAR Text-to-Text (Quick Order):**

Press enter or click to view image in full size
*Text Input (with noise) → Multilingual Text Encoder (initialized NLLB-1B encoder) → SONAR Sentence Embedding →
Multilingual Text Decoder (initialized NLLB-1B decoder) → Output (Reconstructed or Translated Text)*

**Phase 2: Teacher-Student Learning**

Phase two introduces a teacher-student learning architecture, where the Student model learns from a teacher model. It is
here where we are focused on teaching the student “how” to learn and translate, rather than just memorizing “what” to
translate.

We use, Bitext training data (parallel sentences) aligns new language embeddings with pre-existing ones. Again, here
too, noise is added to the sentences. Mathematically, this alignment is achieved using:

Press enter or click to view image in full size

This approach reduces the dependency on extensive fine-tuning, improving efficiency and scalability.

**Speech-to-Text (Teacher-Student)**

Press enter or click to view image in full size
*Speech Input → Speech Encoder (Student, W2v-BERT init.) → SONAR Sentence Embedding → Multilingual Text Decoder
(Teacher, NLLB 1B init.) → Output Text (Transcription or Translation)*

**SONAR in Large Concept Models (LCMs)**

SONAR embeddings underpin LCMs, enabling these models to predict concepts across diverse languages and modalities
seamlessly. This facilitates cross-modal search and translation, unified in a single embedding space, allowing LCMs to
reason at sentence level meaning.

**Evaluation and Benchmarks**

SONAR embeddings are evaluated through robust metrics:

**Text-to-Text Evaluation**

**xsim**: Evaluates SONAR embeddings’ semantic alignment across languages, where lower error rates indicate better
performance. SONAR shows superior performance with an xsim error rate of just 0.1%, significantly better than other
models like LASER3 (1.1%) and LaBSE (1.5%).

**xsim++**: Tests robustness against challenging scenarios (entity changes, causalityshifts). SONAR demonstrates strong
robustness with an error rate of only 9.3%,markedly outperforming models like LASER3 (27.5%) and LaBSE (15.4%).

Press enter or click to view image in full size

**Speech-to-Text Evaluation**

**SpBLEU**: Measures similarity between generated transcriptions and reference texts (higher scores are better). SONAR
achieves competitive scores (64.7 English,54.3 French) that significantly improve with fine-tuning (69.7 English, 64.1
French).While slightly behind Whisper AI, SONAR shows promising potential, especially post fine-tuning.

**BERT-score**: Evaluates semantic similarity, with scores closer to 1.0 being ideal. SONAR embeddings achieve excellent
semantic coherence (0.948 English), slightly improved upon fine-tuning (0.951 English), closely following Whisper
models.

Press enter or click to view image in full size

**Challenges and Considerations**

Like any model, SONAR Embeddings does not come without challenges. It currently may not match translation accuracy for
languages with extensive data (e.g., German, Mandarin) compared to highly trained models.

Additionally, SONAR emphasizes capturing meaning over exact literal translations, which can sometimes result in
paraphrasing which can be occasionally taken as disadvantageous for exact translation tasks.

**Conclusion**

SONAR embeddings mark a transformative step forward, offering significant improvements in machine translation by
prioritizing meaning over literal translations. Its multimodal and language-agnostic nature promises broad, impactful
applications, from universal meaning search engines to enhanced multilingual communication.

**References**
* SONAR Research Paper:
* [https://arxiv.org/pdf/2308.11466][13]
* [Medium Blogs:][14]
* [*https://medium.com/@bteo/sonar-explained-a9c99f1376e8*][15]
* [*https://medium.com/@chs.li.work/sonar-sentence-level-multimodal-and-language-agnostic-representations-73a81d3f5913*]
  [16]
* [YouTube tutorials:][17]
* [https://www.youtube.com/watch?v=A8HEPBdKVMA][18]
* Others:
* [https://blog.keras.io/building-autoencoders-in-keras.html][19]
* [https://huggingface.co/blog/encoder-decoder][20]
* [https://www.youtube.com/watch?v=El1xxnn7074][21]

[
AI
][22]
[
Language
][23]
[
][24]

--

[
][25]

--

1

[
][26]
[][27]
[
[Tech_with_KJ]
][28]
[
[Tech_with_KJ]
][29]
[

## Written by Tech_with_KJ

][30]
[43 followers][31]
·[69 following][32]
[

Help

][33]
[

Status

][34]
[

About

][35]
[

Careers

][36]
[

Press

][37]
[

Blog

][38]
[

Store

][39]
[

Privacy

][40]
[

Rules

][41]
[

Terms

][42]
[

Text to speech

][43]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agn
ostic-representations-df4d7d2f7d31&source=post_page---top_nav_layout_nav-----------------------global_nav---------------
---
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agn
ostic-representations-df4d7d2f7d31&source=post_page---top_nav_layout_nav-----------------------global_nav---------------
---
[7]: /@roseserene?source=post_page---byline--df4d7d2f7d31---------------------------------------
[8]: /@roseserene?source=post_page---byline--df4d7d2f7d31---------------------------------------
[9]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2Fdf4d7d2f7d31&operation=register&redirect=https%3A%2F%
2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31&user=Tech_with_KJ
&userId=f380d6dc5b51&source=---header_actions--df4d7d2f7d31---------------------clap_footer------------------
[10]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Fdf4d7d2f7d31&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31&user=Tech_with
_KJ&userId=f380d6dc5b51&source=---header_actions--df4d7d2f7d31---------------------repost_header------------------
[11]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Fdf4d7d2f7d31&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31&source=---he
ader_actions--df4d7d2f7d31---------------------bookmark_footer------------------
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3Ddf4d7d2f7d31&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representat
ions-df4d7d2f7d31&source=---header_actions--df4d7d2f7d31---------------------post_audio_button------------------
[13]: https://arxiv.org/pdf/2308.11466
[14]: /@bteo/sonar-explained-a9c99f1376e8
[15]: /@bteo/sonar-explained-a9c99f1376e8
[16]: /@chs.li.work/sonar-sentence-level-multimodal-and-language-agnostic-representations-73a81d3f5913
[17]: https://www.youtube.com/watch?v=A8HEPBdKVMA
[18]: https://www.youtube.com/watch?v=A8HEPBdKVMA
[19]: https://blog.keras.io/building-autoencoders-in-keras.html
[20]: https://huggingface.co/blog/encoder-decoder
[21]: https://www.youtube.com/watch?v=El1xxnn7074
[22]: /tag/ai?source=post_page-----df4d7d2f7d31---------------------------------------
[23]: /tag/language?source=post_page-----df4d7d2f7d31---------------------------------------
[24]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2Fdf4d7d2f7d31&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31&user=Tech_with_K
J&userId=f380d6dc5b51&source=---footer_actions--df4d7d2f7d31---------------------clap_footer------------------
[25]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2Fdf4d7d2f7d31&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31&user=Tech_with_K
J&userId=f380d6dc5b51&source=---footer_actions--df4d7d2f7d31---------------------clap_footer------------------
[26]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Fdf4d7d2f7d31&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31&user=Tech_with
_KJ&userId=f380d6dc5b51&source=---footer_actions--df4d7d2f7d31---------------------repost_footer------------------
[27]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Fdf4d7d2f7d31&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40roseserene%2Fsonar-ai-with-multimodal-language-agnostic-representations-df4d7d2f7d31&source=---fo
oter_actions--df4d7d2f7d31---------------------bookmark_footer------------------
[28]: /@roseserene?source=post_page---post_author_info--df4d7d2f7d31---------------------------------------
[29]: /@roseserene?source=post_page---post_author_info--df4d7d2f7d31---------------------------------------
[30]: /@roseserene?source=post_page---post_author_info--df4d7d2f7d31---------------------------------------
[31]: /@roseserene/followers?source=post_page---post_author_info--df4d7d2f7d31---------------------------------------
[32]: /@roseserene/following?source=post_page---post_author_info--df4d7d2f7d31---------------------------------------
[33]: https://help.medium.com/hc/en-us?source=post_page-----df4d7d2f7d31---------------------------------------
[34]: https://status.medium.com/?source=post_page-----df4d7d2f7d31---------------------------------------
[35]: /about?autoplay=1&source=post_page-----df4d7d2f7d31---------------------------------------
[36]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----df4d7d2f7d31-------------------------------------
--
[37]: mailto:pressinquiries@medium.com
[38]: https://blog.medium.com/?source=post_page-----df4d7d2f7d31---------------------------------------
[39]: https://medium.com/store
[40]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----df4d7d2f7d31--------------------
-------------------
[41]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----df4d7d2f7d31-----------------------------
----------
[42]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----df4d7d2f7d31------------------
---------------------
[43]: https://speechify.com/medium?source=post_page-----df4d7d2f7d31---------------------------------------
```
