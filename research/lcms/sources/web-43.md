# Web source

- URL: https://www.lesswrong.com/posts/7Dtyhdkp5m6p4mquC/distillation-of-meta-s-large-concept-models-paper
- Title: x
- Captured (UTC): 2026-06-29T16:30:55.090754817+00:00

```text
x
This website requires javascript to properly function. Consider activating javascript to get access to all site
functionality.

## [LESSWRONG][1]

## [LW][2]

Login
Distillation of Meta's Large Concept Models Paper — LessWrong
[Academic Papers][3][AI Capabilities][4][Language Models (LLMs)][5][AI][6][
Frontpage
][7]

# 19

# [Distillation of Meta's Large Concept Models Paper][8]

by [NickyP][9]
4th Mar 2025
5 min read
[3][10]

# 19

Note: I had this as a draft for a while. I think it is accurate, but there may be errors. I am not in any way affiliated
with the authors of the paper. 

Below I briefly discuss the ["Large Concept Models" paper released by Meta][11], which tries to change some of the
paradigm of doing language modelling. It has some limitations that are not present for normal language models, but I
read spent the time to read the paper in relative depth so I am here to provide a brief summary of it.

# **"Large Concept Models" (LCM) Paper**

Large Concept Models aim to be a way to "improve language modelling" by "being more hierarchical". I think the easiest
way to explain is to compare to normal decoder-only language models.
* **Normal language model (LLM):**
  * Take text
  * Split the text into "tokens"
  * Embed the tokens into vectors
  * Pass token embed vectors into a model
  * The model outputs a probability distribution over potential new tokens
* **Large concept models (LCM):**
  * Take a text
  * Split the text into sentences or "concepts". (They add a maximum limit so sentences can be split up into concepts)
  * Use a semantic embedding model, in particular [SONAR][12], to embed the sentences into "concept vectors"
  * Pass the concept embed vectors into a model
  * The model somehow outputs a new sentence embed vector.
  * the new sentence embed vector can be decoded using SONAR again.

A normal LLM works by passing in and getting out single tokens. For the LCM, we instead pass and get out semantic
vectors. The key model that makes this possible is SONAR, which is a [text auto-encoder][13].

The main benefit I can see is that it likely is much better at long-contexts. Otherwise, it comes with some
disadvantages.

## ARCHITECTURES

The key difficulty: How to output new sentence embed vectors, since it a continuous space. They try a few approaches.
* **BASE LCM**: They try the base case of directly predicting the expected next semantic vector using MSE.
  * The Base LCM model does not work well.
* **Quant LCM:** In this approach^{[[1]][14]}, they try to make the sampling space of embedding vectors discrete by
  doing "Residual Vector Quantization". That is, they try to make it such that the semantic embed vector can be written
  as a decomposition of 64 components/"notebooks", each of which can be one of 8192 vectors/"units". These notebooks are
  also ordered by importance.
  * That is, a vector x would be written in terms of "units" from notebooks nb_1, ..., nb_64 like so:
  * x = nb_1[idx_1] + nb_2[idx_2] + ... nb_64[idx_64]
  * The Quant LCM variations they try work better than the Base LCM model, but not as well as Diffusion LCM.
* **Diffusion LCM**: Finally, they try another method based on the Diffusion. Here, they:
  * Embeds of all the previous text sections
  * Pass in a randomly generated embed
  * Pass the previous text embeds + randomly generated embed through the model
  * The model outputs a new text embed based on the randomly generated embed that is less noisy.
  * They do this "denoise" procedure multiple times to the embedding until we get a concrete prediction

They find diffusion model work significantly better. They had two variations that seemed to work equally well 
* ONE TOWER:
  * Decoder-only transformer, 32 layers, self-attention + MLPs, where one passes in good states X1,...XN, and 1 randomly
    initialised state state ^XTN+1
  * The model improves on the state to get a new state ^XT−1N+1
  * This is repeated until we get some clean ^X0N+1
  * The new clean sentence embed state ^X0N+1 is decoded, and the above is repeated to generate a new sentence.
* TWO TOWER
  * Encoder 5-layers (self-attention + MLP) + Decoder 13 layers (cross-attention + MLP)^{[[2]][15]}
  * Encoder converts semantic vectors X_1, ... X_N into some clean hidden states S1,...SN, using self-attention.
  * Decoder takes in a randomly initialised state ^XTN+1, and uses cross-attention to compare the N+1 state to all the
    previous 0 to N states.
  * The Decoder gets run multiple times until we get some clean next-sentence embedding state ^X0N+1, while the encoder
    only gets run once.
  * The new clean sentence embed state ^X0N+1 is decoded, and the above is repeated to generate a new sentence.

These two models performed very similarly, so they decided to focus on the "Two Towers" architecture as it's less
compute intensive.^{[[3]][16]}

In the rest of the paper, they try various methods for optimising hyperparameters and such, and they try scaling up the
model, and compare to other normal LLM models.

 

## BENCHMARKS

Disappointingly, they do not benchmark the model on any "normal benchmarks" like MMLU or anything similar. They state:
"As longform text generation is the main challenge for LCM, our benchmarking is mainly focused on generative tasks". I
will just provide two representitive benchmark results from the paper

First, they compare the different approaches for Large Concept Models that are instruction fine tuned. For the 1.6B size
models, we see that the two diffusion models significantly outperform the other methods for Large Concept Models.
However, we also see that the "SmaLLAMA" model of the same size performs better.

────────────────┬─────────────────────────────────────────────────┬──────────────────────────────────────────
**Metric**      │**Coherence ↑**                                  │**R-L ↑**                                 
────────────────┼─────────────────────────────────────────────────┼──────────────────────────────────────────
What it measures│How naturally text flows and connects (0-1 scale)│Overlap between generated & reference text
────────────────┼─────────────────────────────────────────────────┼──────────────────────────────────────────
**BASE-LCM**    │0.482                                            │23.69                                     
────────────────┼─────────────────────────────────────────────────┼──────────────────────────────────────────
**QUANT-LCM**   │0.847                                            │30.87                                     
────────────────┼─────────────────────────────────────────────────┼──────────────────────────────────────────
**ONE-TOWER**   │**0.968**                                        │33.40                                     
────────────────┼─────────────────────────────────────────────────┼──────────────────────────────────────────
**TWO-TOWER**   │0.938                                            │**33.64**                                 
────────────────┼─────────────────────────────────────────────────┼──────────────────────────────────────────
**SMALLAMA**    │**0.984**                                        │**34.88**                                 
────────────────┴─────────────────────────────────────────────────┴──────────────────────────────────────────

 

They scale up the model to 8B to compare against other models. 

Here is a representative benchmark that they used, which compares summary quality from LCM vs some similar sized models.

────────────────────────────────────────────────────────────────────────────────────────
LCFO 10% - Summarise text in 10% of original length                                     
──────────────────┬─────────┬─────────┬─────────┬─────────┬─────────┬─────────┬─────────
Model             │Word     │R-L(↑)   │OVL-3 (↑)│REP-4 (↓)│CoLA (↑) │SH-4 (↑) │SH-5 (↑) 
                  │         │         │         │         │         │         │         
                  │Ratio    │         │         │         │         │         │         
──────────────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────
GEMMA-7B-IT       │0.150    │29.25    │0.164    │6.427    │0.667    │0.377    │0.194    
──────────────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────
MISTRAL-7B-V0.3-IT│0.549    │25.00    │**0.537**│6.289    │0.848    │**0.660**│0.306    
──────────────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────
LLAMA-3.1-8B-IT   │0.128    │**42.85**│0.243    │3.804    │**0.907**│0.486    │**0.310**
──────────────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────┼─────────
TWO-TOWER-7B-IT   │0.089    │29.38    │0.202    │**3.00** │0.791    │0.623    │0.183    
──────────────────┴─────────┴─────────┴─────────┴─────────┴─────────┴─────────┴─────────

This table shows performance on the LCFO.10% task (long context summarization, where output should be 10% of input
length). I don't intuitively understand most of the metrics that well, but they are:
* WR: Word Ratio (ratio between generated text and source text, should be close to 0.10, and we see that mistral does
  quite poorly here)
* R-L: ROUGE-L score (measures similarity to reference summary)
* OVL-3: 3-gram overlap with source (how much is directly copied)
* REP-4: 4-gram repetition (lower is better - measures redundancy)
* CoLA: Grammatical acceptability score
* SH-4: SEAHORSE score, measuring if information is attributable to source
* SH-5: SEAHORSE score, measuring if main ideas are captured

The model seem to be OK but maybe not spectacular. 

 

## CONCLUSION

Overall, the LCM seems like an interesting model in some ways, and perhaps has the benefit of using context much more
slowly than in other models, but at the moment doesn't seem like much of an improvement to other models. It loses some
of the properties you get from tokenization that make training models easy. 

 
1. ^{**[^][17]**}
   
   Note that they actually have two methods for LCM-QUANT but as they don't decide to pursue either approach, I won't go
   into much detail here. You can see the original paper for details on that.
2. ^{**[^][18]**}
   
   Note that the diffusion decoder actually has: self-attention + cross-attention + MLP, but they do not let the token
   attend to any tokens other than itself, so it is pointless. They state:
   
   > The self-attention layers in the denoiser do only attend to the current position i.e., we do not attend to the
   > preceding noised context. The self-attention layers were kept for consistency with a standard Transformer block and
   > for the possible extension of denoising multiple vectors at once.
3. ^{**[^][19]**}
   
   It is less compute intensive since for one-tower, you need to pass through all 32 layers for all N diffusion steps,
   but for the two-tower you only need to pass through the encoder once and through the decoder for the N diffusion
   steps.

[Distillation of Meta's Large Concept Models Paper][20]
[1Martin Vlach][21]
[1NickyP][22]
[1Martin Vlach][23]
[3Comments][24]
3
[Academic Papers][25][AI Capabilities][26][Language Models (LLMs)][27][AI][28][
Frontpage
][29]

# 19

New Comment
Submit
3 comments, sorted by
top scoring
Click to highlight new comments since: Today at 4:30 PM
[-][Martin Vlach][30]1y1
0

Comparing to Gemma1, classic BigTech😅

 

And I seem to miss info on the effective context length..?

Reply
[-][NickyP][31]1y1
0

Yeah, the context length was 128 concepts for the small tests they did between architectures, and 2048 concepts for the
larger models.

How this exactly translates is kind of variable. They limit the concepts to be around 200 characters, but this could be
any number of tokens. They say they trained the large model on 2.7T tokens and 142B concepts, so on average 19 tokens
per concept.

The 128 would translate to 2.4k tokens, and the 2048 concepts would translate to approx 39k tokens.

Reply
[-][Martin Vlach][32]1y1
0

> read spent the time to read

typo?

Reply
[Moderation Log][33]
More from [NickyP][34]
[View more][35]
Curated and popular this week

[1]: /
[2]: /
[3]: /w/academic-papers
[4]: /w/ai-capabilities
[5]: /w/language-models-llms
[6]: /w/ai
[7]: /posts/5conQhfa4rgb4SaWx/site-guide-personal-blogposts-vs-frontpage-posts
[8]: /posts/7Dtyhdkp5m6p4mquC/distillation-of-meta-s-large-concept-models-paper
[9]: /users/nicky?from=post_header
[10]: #comments
[11]: https://arxiv.org/pdf/2412.08821
[12]: /posts/PjEtMQ65sux4mgrCT
[13]: http://link.nicky.pro/tae
[14]: #fnn61jw409xvr
[15]: #fnb3gw63wqcd5
[16]: #fn2zc5byzeasc
[17]: #fnrefn61jw409xvr
[18]: #fnrefb3gw63wqcd5
[19]: #fnref2zc5byzeasc
[20]: #
[21]: #LRYEq6jFrv3qnLcoj
[22]: #K6rGa3p3eb6brBL76
[23]: #mm5S3J9ExL2ajRLxN
[24]: #comments
[25]: /w/academic-papers
[26]: /w/ai-capabilities
[27]: /w/language-models-llms
[28]: /w/ai
[29]: /posts/5conQhfa4rgb4SaWx/site-guide-personal-blogposts-vs-frontpage-posts
[30]: /users/martin-vlach
[31]: /users/nicky
[32]: /users/martin-vlach
[33]: /moderation
[34]: /users/nicky
[35]: /users/nicky
```
