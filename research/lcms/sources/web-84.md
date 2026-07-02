# Web source

- URL: https://mlsys.org/virtual/2025/session/3155
- Title: [Skip to yearly menu bar][1] [Skip to main content][2]
- Captured (UTC): 2026-06-29T16:32:51.072139069+00:00

```text
[Skip to yearly menu bar][1] [Skip to main content][2]

## Main Navigation

[ [conference_logo]][3]
* [ MLSys ][4]
  * [ Code of Conduct ][5]
  * [ Create Profile ][6]
  * [ Privacy Policy ][7]
  * [ Contact MLSys ][8]
  * [ Help/FAQ ][9]
* [ My Stuff ][10]
[ Login][11]
* [ Select Year: (2025) ][12]
  * [2026 ][13]
  * [2025 ][14]
  * [2023 ][15]
  * [2024 ][16]
  * [2022 ][17]
  * [2021 ][18]
  * [2020 ][19]
  * [2019 ][20]
  * [2018 ][21]
* [ Getting Started ][22]
* [ Schedule ][23]
* [ Invited Talks ][24]
* [ Papers ][25]
* [ Sponsors ][26]
* [ ][27]
* [ Help ][28]
  * [ Bookmarking/Agenda ][29]
  * [ Code Of Conduct ][30]
* [ Awards ][31]
* [ Recordings ][32]



### Session

## Session 8: LLM and Diffusion Model Serving

Moderator: Pradeep Dubey

Abstract:
Chat is not available.

#36
Outstanding Paper Honorable Mention

##### **[Seesaw: High-throughput LLM Inference via Model Re-sharding][33]**

Qidong Su ⋅ Wei Zhao ⋅ Xin Li ⋅ Muralidhar Andoorveedu ⋅ Chenhao Jiang ⋅ Zhanda Zhu ⋅ Kevin Song ⋅ Christina Giannoula ⋅
Gennady Pekhimenko

To improve the efficiency of distributed large language model (LLM) inference, various parallelization strategies, such
as tensor and pipeline parallelism, have been proposed. However, the distinct computational characteristics inherent in
the two stages of LLM inference—prefilling and decoding—render a single static parallelization strategy insufficient for
the effective optimization of both stages.In this work, we present Seesaw, an LLM inference engine optimized for
throughput-oriented tasks. The key idea behind Seesaw is dynamic model re-sharding, a technique that facilitates the
dynamic reconfiguration of parallelization strategies across stages, thereby maximizing throughput at both phases.To
mitigate re-sharing overhead and optimize computational efficiency, we employ tiered KV cache buffering and
transition-minimizing scheduling. These approaches work synergistically to reduce the overhead caused by frequent stage
transitions while ensuring maximum batching efficiency.Our evaluation demonstrates that Seesaw achieves a throughput
increase of up to 1.78$\times$ (1.36$\times$ on average) compared to vLLM, the most widely used state-of-the-art LLM
inference engine.

#37

##### **[ScaleFusion: Scalable Inference of Spatial-Temporal Diffusion Transformers for High-Resolution Long Video
##### Generation][34]**

Jiacheng Yang ⋅ Jun Wu ⋅ Zhen Zhang ⋅ Xinwei Fu ⋅ Zhiying Xu ⋅ Zhen Jia ⋅ Yida Wang ⋅ Gennady Pekhimenko

Recent advancements in training diffusion models have made generating high-quality videos possible. Particularly, the
spatial-temporal diffusion transformers (ST-DiTs) emerge as a promising diffusion model architecture for generating
videos of high-resolution (1080p) and long duration (20 seconds). However, the quadratic scaling of compute cost with
respect to resolution and duration, primarily due to spatial-temporal attention layers processing longer sequences,
results in high inference latency of ST-DiTs. This hinders their applicability in time-sensitive scenarios. Existing
sequence parallelism techniques, such as DeepSpeed-Ulysses and RingAttention, are not optimally scalable for ST-DiT
inference across multiple GPU machines due to cross-machine communication overheads. To address this challenge, we
introduce ScaleFusion, a scalable inference engine designed to optimize ST-DiT inference for high-resolution, long video
generation. By leveraging the inherent structure of spatial-temporal attention layers, ScaleFusion effectively hides
cross-machine communication overhead through novel intra-layer and inter-layer communication scheduling algorithms. This
enables strong scaling of 3.60$\times$ on 4 Amazon EC2 p4d.24xlarge machines (32 A100 GPUs) against 1 machine (8 A100
GPUs). Our experiments demonstrate that ScaleFusion surpasses state-of-the-art techniques, achieving an average speedup
of 1.36$\times$ (up to 1.58$\times$).

#39

##### **[TurboAttention: Efficient attention approximation for high throughputs llm][35]**

Hao Kang ⋅ Srikant Bharadwaj ⋅ James Hensman ⋅ Tushar Krishna ⋅ Victor Ruehle ⋅ Saravan Rajmohan

Large language model (LLM) inference demands significant amount of computation and memory, especially in the key
attention mechanisms. While techniques, such as quantization, and acceleration algorithms, like FlashAttention, have
improved efficiency of the overall inference, they address different aspects of the problem: quantization focuses on
weight-activation operations, while FlashAttention improves execution but requires high-precision formats. Recent
Key-value (KV) cache quantization reduces memory bandwidth but still needs floating-point dequantization for attention
operations.We present TurboAttention, a comprehensive approach to enable quantized execution of attention that
simultaneously addresses both memory and computational efficiency. Our solution introduces two key innovations: FlashQ,
a headwise attention quantization technique that enables both compression of KV cache and quantized execution of
activation-activation multiplication, and Sparsity-based Softmax Approximation (SAS), which eliminates the need for
dequantization to FP32 during exponentiation operation in attention. Experimental results demonstrate that
TurboAttention achieves 1.2-1.8x speedup in attention, reduces the KV cache size by over 4.4x, and enables up to 2.37x
maximum throughput over the FP16 baseline while outperforming state-of-the-art quantization and compression techniques
across various datasets and models.

#55

##### **[FlexInfer: Flexible LLM Inference with CPU Computations][36]**

Seonjin Na ⋅ Geonhwa Jeong ⋅ Byung Hoon Ahn ⋅ Aaron Jezghani ⋅ Jeffrey Young ⋅ Christopher Hughes ⋅ Tushar Krishna ⋅
Hyesoon Kim

LLMs have demonstrated remarkable performance across various fields, prompting data centers to use high computation cost
accelerators like GPUs and NPUs for model training and inference. However, the immense size of these models and
key-value (KV) caches poses substantial memory capacity challenges. While offloading-based approaches utilize CPU memory
to store model weights and KV caches—enabling deployment of models exceeding GPU memory capacity—they often suffer from
performance degradation due to PCIe transfer bottlenecks. To address the performance limitations of existing
offloading-based LLM inference in CPU and memory-limited single GPU systems, this paper proposes FlexInfer. FlexInfer
uses a performance estimator to dynamically select the most appropriate execution policy for each phase—prefill and
decode—based on hardware configurations and runtime parameters such as sequence length and batch size. Our evaluation
results show that by selecting optimal policies for these phases, FlexInfer can significantly reduce end-to-end latency
by 75% and 76% on average across two different server configurations, when compared to FlexGen, a state-of-the-art
offload-based LLM inference technique.

#58

##### **[SOLA: Optimizing SLO Attainment for Large Language Model Serving with State-Aware Scheduling][37]**

Ke Hong ⋅ Xiuhong Li ⋅ Lufang Chen ⋅ Qiuli Mao ⋅ Guohao Dai ⋅ Xuefei Ning ⋅ Shengen Yan ⋅ Yun Liang ⋅ Yu Wang

Serving large language models (LLMs) efficiently requires elaborate request scheduling to satisfy service-level
objectives (SLOs).In the context of LLM serving, SLOs include the constraints on Time-to-First-Token (TTFT) and
Time-per-Output-Token (TPOT).Existing serving systems apply a coarse-grained request scheduling that follows a fixed
principle at different iterations during the serving procedure, leading to (1) a significant distribution bias between
TTFT and TPOT and (2) a significant distribution variance among different requests as shown in Fig. 1(a), and hence
causes disappointing SLO attainment.We identify that fine-grained scheduling based on a formal description of the design
space addresses the issues mentioned above.To this end, we first formulate a scheduling design space with flexible
control of the request execution order and the workload at each iteration. Based on that, we introduce a state-aware
scheduling strategy, which enables the awareness of two kinds of states: the states from the single request perspective
and the states from the systemic perspective, and further balances between TTFT and TPOT and balances among different
requests to improve the SLO attainment, as shown in Fig. 2. We implement SOLA with the above insights. The evaluation
shows that SOLA enhances the SLO attainment from 45.5\% to 99.4\%, thus serving more requests. Given SLO constraints,
SOLA serves 1.04-1.27$\times$ more requests than the state-of-the-art systems on average.

Successful Page Load

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬─────
MLSys uses cookies for essential functions only. We do not sell your personal information. [Our Privacy Policy    │Accep
» ][38]                                                                                                           │t    
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴─────


###### [MLSys logo]

The MLSys Logo above may be used on presentations. Right-click and choose download. It is a vector graphic and may be
used at any scale.

###### Useful links
* [Sponsor Information][39]

###### Contact

82799 Kingsboro Lane, Indio CA 92201

[Email][40]

Phone: +1 ‭(858) 761-6256‬

[MLSys Proceedings][41]

[1]: #child-menu
[2]: #main
[3]: /
[4]: #
[5]: /public/CodeOfConduct
[6]: /Profile/create
[7]: /public/PrivacyPolicy
[8]: /Help/Contact
[9]: /FAQ
[10]: /MyStuff
[11]: /accounts/login?nextp=/Conferences/2025/ProgramCommittee 
[12]: #
[13]: /Conferences/2026
[14]: /Conferences/2025
[15]: /Conferences/2023
[16]: /Conferences/2024
[17]: /Conferences/2022
[18]: /Conferences/2021
[19]: /Conferences/2020
[20]: /Conferences/2019/
[21]: /Conferences/2018
[22]: /virtual/2025/index.html
[23]: /virtual/2025/calendar
[24]: /virtual/2025/eventlistwithbios/Invited%20Talk
[25]: /virtual/2025/papers.html?filter=titles
[26]: /virtual/2025/sponsor_list
[27]: /virtual/2025/search
[28]: #
[29]: 
[30]: /public/CodeOfConduct
[31]: /virtual/2025/awards_detail
[32]: /virtual/2025/recorded-events
[33]: /virtual/2025/poster/3253
[34]: /virtual/2025/poster/3252
[35]: /virtual/2025/poster/3250
[36]: /virtual/2025/poster/3234
[37]: /virtual/2025/poster/3231
[38]: /public/PrivacyPolicy
[39]: /sponsors/prospectus
[40]: /Help/Contact
[41]: https://proceedings.mlsys.org
```
