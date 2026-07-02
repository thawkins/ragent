# Web source

- URL: https://www.themoonlight.io/en/review/visual-autoregressive-models-beat-diffusion-models-on-inference-time-scaling
- Title: [
- Captured (UTC): 2026-06-29T16:32:43.836447711+00:00

```text
[
[Moonlight Logo]
][1]
* * open navigation menu
  * [
    [Moonlight Logo]
    ][2]
* [Features][3]
* [Pricing][4]
* [FAQ][5]
* [Blog][6]
* [Explore Literature][7]
* [2nd Anniversary][8]
* open navigation menu
* [
  [Moonlight Logo]
  ][9]

[Features][10]

[Pricing][11]

[FAQ][12]

[Blog][13]

[Explore Literature][14]

[2nd Anniversary][15]
* EN
* Upload Paper

This page provides the most accurate and concise summary worldwide for the paper titled Visual Autoregressive Models
Beat Diffusion Models on Inference Time Scaling. With Moonlight, your AI research colleague, you can effortlessly and
quickly grasp all the papers you read. Install the Chrome extension from https://www.themoonlight.io/ or directly upload
files on the web. Moonlight offers features tailored to your needs: - Text Explanation: AI simplifies complex concepts
and paragraphs. - Image Explanation: One-click explanations of images, tables, and formulas. - AI Chat: Engage with AI
to dive deeper into paper discussions. - Smart Citations: Instantly view information (title, author, summary) of cited
papers without scrolling to the References. - Translation: Quickly translate unfamiliar words, sentences, or the entire
page. - Auto Highlight: AI highlights key points automatically, helping you identify originality, methods, and results
swiftly. - External Link Explanation: AI analyzes external sources and explains their relevance to your document. -
Markup: Highlight important sentences and annotate to create personalized research notes. - Save and Share: Store
documents in your library and easily share them. - Scholar Deep Search: Receive recommendations of relevant papers based
on your stored documents.

# [Literature Review] Visual Autoregressive Models Beat Diffusion Models on Inference Time Scaling

This paper investigates the effectiveness of inference-time search strategies for visual generation, positing that model
architecture, rather than just scale, is critical for optimization. While large language models (LLMs) have seen
revolutionary gains from search and deliberation during inference, similar approaches have shown limited benefits for
continuous diffusion models. The authors demonstrate that the discrete, sequential nature of visual autoregressive
models (VARs) is fundamentally more compatible with search algorithms, enabling substantial improvements in
text-to-image generation.

The core methodology revolves around applying tree search algorithms to autoregressive image generation. The generative
model used is **Infinity (Han et al., 2024)**, a state-of-the-art VAR model. Unlike traditional raster-scan
autoregressive models, Infinity generates $1024 \times 1024$ images through 13 progressive scales using "next-scale
prediction," where $p(R) = \prod_{k=1}^{K} p(r_k|r_1, \ldots, r_{k-1})$. Each $r_k$ represents all tokens at scale $k$,
generated simultaneously in a single forward pass. This hierarchical, scale-wise generation creates only 13 discrete
decision points, allowing for efficient computational reuse: once tokens at scales $r_1, \ldots, r_k$ are computed,
their transformer key-value representations can be cached and reused across all search branches sharing that prefix.

The paper employs a **verification framework** to assess image quality during search. Primary verifiers include
**ImageReward (Xu et al., 2023)** for human preference, **CLIPScore (Hessel et al., 2021)** for semantic alignment, and
**Aesthetic Score (Schuhmann et al., 2022)** for visual quality. An ensemble verifier combines these via ranking-based
aggregation. For more complex reasoning tasks, **LLaVA-OneVision (Li et al., 2024)**, a 7B vision-language model, is
used for prompt alignment. These verifiers have significant computational overhead differences, with lightweight models
like CLIPScore processing images in 14ms (1.6GB GPU memory) compared to LLaVA-OneVision requiring 500ms (15.3GB GPU
memory), a $36 \times$ speed difference.

Three **search strategies** are compared:
1. **Random Search**: Generates $n$ complete images independently using different random seeds and selects the one with
   the highest score: $R^* = \arg \max_{i \in [n]} S(R^{(i)})$. This offers maximal diversity but no computational
   reuse.
2. **Greedy Token Optimization (GTO)**: At each scale $k$, it generates $c$ complete continuations from the current
   prefix, selecting the token $r_k^*$ that produces the highest-scoring final image: $r^**k = \arg \max*{j \in [c]}
   S(r_1, \ldots, r_{k-1}, r^{(j)}*k, r^{(j)}*{k+1}, \ldots, r^{(j)}_K)$. This creates a single optimized path,
   leveraging prefix reuse, but risks local optima.
3. **Beam Search**: Maintains $w$ parallel hypotheses (beams), expanding each with $c$ candidates at every scale. After
   scoring all $w \times c$ complete images, it retains the top $w$ prefixes for continued expansion:
   $\text{Beams}_{k+1} = \text{top-}w{S(R) : R \in \text{Candidates}_k}$. This balances exploration breadth with
   tractability through aggressive pruning. The computational advantage of GTO and beam search is that shared
   computation reduces complexity from $O(n \cdot K)$ for independent generation to approximately $O(n \cdot K/w)$ for
   beam search with width $w$, where $n$ is candidate images and $K$ is the number of scales.

Computational cost is measured by "Number of Images" (total complete images evaluated) and "Number of Function
Evaluations (NFEs)" (total transformer forward passes, one NFE per Infinity scale).

Experimental results on DrawBench revealed a **logarithmic scaling relationship** between budget size ($k$) and expected
maximum verification score: $E[\max_{i \leq k} s_i] \approx \alpha \log(k) + \beta$, indicating diminishing returns for
random sampling. Beam search consistently outperformed random search and GTO. A 2B parameter autoregressive model with
beam search (1365 NFEs for 195 images) surpassed the baseline Infinity-2B and even a larger Infinity-8B model, achieving
higher scores in visual quality, prompt adherence, and aesthetic appeal. This efficiency gain comes from prefix caching
and guided exploration, allowing beam search to achieve superior performance with 46% fewer NFEs than equivalent random
search. Verifier analysis showed a trade-off: ImageReward was cost-effective for attribute binding, while
LLaVA-OneVision was crucial for reasoning-heavy tasks like spatial reasoning, despite its high computational cost.

Crucially, the paper presents a **comparison with continuous diffusion models**, specifically FLUX.1-dev (Ma et al.,
2025), a 12B parameter diffusion model. The authors demonstrate that their 2B autoregressive model with beam search
achieves superior absolute performance and larger relative improvements across DrawBench and compositional benchmarks
(T2I-CompBench++ and GenEval), despite being $6 \times$ smaller in parameter count and using fewer NFEs. For instance,
on T2I-CompBench++, the 2B autoregressive model with beam search achieved an average absolute improvement of 11.3%
across categories, compared to 5.7% for the 12B diffusion model with search. This includes significant gains in
structured tasks like shape (+17.38% vs. +7.72%) and spatial reasoning (+10.45% vs. +6.14%). The discrete token space of
autoregressive models enables efficient pruning and computational reuse, yielding a step-change in performance scaling
not observed in continuous latent spaces.

The paper concludes that autoregressive image models possess a fundamental architectural advantage for inference-time
search due to their discrete token space, enabling efficient pruning and computational reuse. This allows a smaller 2B
model with beam search to outperform a 12B diffusion model using fewer function evaluations, challenging the assumption
that quality scales primarily with model size and highlighting the importance of co-designing models and inference
algorithms for efficient and capable visual generation.

[
[Moonlight Logo]
][16]
[Terms of Use][17][Privacy Policy][18][Medium][19][GitHub][20][LinkedIn][21][Email][22]

Corca, Inc. / CEO Younghyun Chung / Business Registration Number 271-86-02206

6F, 11-8 Teheran-ro 77-gil, Gangnam-gu, Seoul, Republic of Korea, 06159

Contact 02-6925-6978 E-mail: moonlight@corca.ai

© 2026 Corca, Inc. All rights reserved.

[1]: /
[2]: /
[3]: /features
[4]: /pricing
[5]: https://docs.themoonlight.io/help
[6]: /blog
[7]: /explore
[8]: /events/anniversary
[9]: /
[10]: /features
[11]: /pricing
[12]: https://docs.themoonlight.io/help
[13]: /blog
[14]: /explore
[15]: /events/anniversary
[16]: /
[17]: /terms-of-use
[18]: /policy
[19]: https://medium.com/corca
[20]: https://github.com/corca-ai
[21]: https://www.linkedin.com/company/corca-ai
[22]: mailto:moonlight@corca.ai
```
