# Web source

- URL: https://arxiv.org/html/2510.18480v3
- Title: 1. [1 Introduction][1]
- Captured (UTC): 2026-06-29T16:32:18.455123690+00:00

```text
1. [1 Introduction][1]
2. [2 Background and Related Work][2]
   1. [2.1 Inference Paradigms of AR, Diffusion, and Block Diffusion Language Models][3]
      1. [2.1.1 Autoregressive Language Models][4]
      2. [2.1.2 Diffusion Language Models][5]
      3. [2.1.3 Block Diffusion Language Models][6]
   2. [2.2 Strategies for Efficient DLM Inference][7]
      1. [2.2.1 Reducing Per-step Computational Overhead][8]
      2. [2.2.2 Reducing Diffusion Sampling Steps][9]
3. [3 Efficiency Evaluation Issues][10]
4. [4 Experimental Design and Setup][11]
   1. [4.1 Overview and Research Questions][12]
   2. [4.2 Experimental Setup][13]
      1. [4.2.1 Inference Framework and Hardware][14]
      2. [4.2.2 Benchmark and Scenarios][15]
      3. [4.2.3 Models and Acceleration Methods][16]
      4. [4.2.4 Evaluation Metrics][17]
5. [5 Evaluation and Analysis][18]
   1. [5.1 RQ1: How do DLMs compare with AR Models in efficiency?][19]
      1. [5.1.1 Effect of Sequence Length][20]
      2. [5.1.2 Effect of Batch Size][21]
   2. [5.2 RQ2: How can we theoretically explain the variations in inference throughput?][22]
      1. [5.2.1 Theoretical Analysis of Inference Throughput][23]
      2. [5.2.2 Analysis of AR Models][24]
      3. [5.2.3 Analysis of Diffusion Models][25]
      4. [5.2.4 Analysis of Block Diffusion Models][26]
   3. [5.3 RQ3: How do acceleration strategies benefit DLMs in varied conditions?][27]
      1. [5.3.1 Effect of sequence length][28]
      2. [5.3.2 Effect of batch size][29]
6. [6 Future Directions][30]
7. [7 Conclusion][31]
8. [A Arithmetic Intensity Estimation][32]

# How Efficient Are Diffusion Language Models?
# A Critical Examination of Efficiency Evaluation Practices

Han Peng^{1}, Peiyu Liu^{2}, Zican Dong^{1}, Daixuan Cheng^{1}, Junyi Li^{4}, Yiru Tang^{1},
 Shuo Wang^{3}, Wayne Xin Zhao^{1}
^{1} Gaoling School of Artificial Intelligence, Renmin University of China
² University of International Business and Economics   ³ Tsinghua University
⁴ Department of Data Science, City University of Hong Kong
panospeng@ruc.edu.cn, batmanfly@gmail.com
Corresponding author.

###### Abstract

Diffusion language models (DLMs) have emerged as a promising alternative to the long-dominant autoregressive (AR)
paradigm, offering a parallelable decoding process that could yield greater efficiency. Yet, in practice, current
open-source DLMs often underperform their AR counterparts in speed, limiting their real-world utility. This work
presents a systematic study of DLM efficiency, identifying key issues in prior evaluation methods. Through empirical
benchmarking and a theoretical analysis, we demonstrate that AR models generally achieve higher throughput, while DLMs
consistently lag. We also investigate acceleration strategies, finding that techniques like dual cache and parallel
decoding mainly offer gains at small batch sizes, with their benefits diminishing upon scaling. Our findings underscore
the necessity of robust evaluation methods and improved acceleration strategies to advance research on DLMs.

## 1 Introduction

Diffusion language models (DLMs) have recently emerged as a competitive alternative to the long-dominant autoregressive
(AR) approach [llmsurvey-arxiv-2023-wayne] for natural language generation. Despite the relatively low profile of
non-autoregressive research, its persistent advancement has now yielded performant models like
LLaDA [dllm-arxiv-2025-shen] that are competitive with their AR counterparts. This represents a significant departure
from the sequential generation paradigm that has defined the field for years.

In contrast to AR models, which generate text one token at a time, DLMs produce multiple tokens in parallel via an
iterative denoising process. This approach theoretically offers a path to overcome the efficiency bottleneck of
sequential generation in AR models. In practice, however, a “paradox” has become increasingly evident: current
open-source DLMs often demonstrate slower inference speeds than their AR counterparts of similar scale. For example,
LLaMA3-Instruct-8B [llama3-arxiv-2024-abhimanyu] exhibits an inference throughput 13.7×\times greater than that of
LLaDA-Instruct-8B on evaluation benchmarks [4-arxiv-2025-xu]. This “theoretically fast but practically slow” predicament
greatly limits the deployment of DLMs in real-world applications, as their potential speed advantages are negated by
implementation inefficiencies.

To improve the inference efficiency of DLMs, recent research has explored various acceleration strategies, which can
broadly fall into two categories: (1) reducing per-step computational overhead (*e.g.,* using lighter diffusion steps or
more efficient sampling methods) and (2) reducing the number of diffusion sampling steps required (*e.g.,* through
faster convergence or distillation).

Despite this progress, our extensive review of the literature identifies a fundamental problem: a widespread lack of
rigorous evaluation for efficiency improvements in DLMs. Specifically, we find three major issues in existing research
on acceleration and evaluation of DLMs:
* •
  
  Evaluation Scope: Prior evaluations often compare DLMs and AR models in limited conditions, such as single instance
  inference or fixed generation length. These constrained conditions fail to capture performance¹¹1In this paper,
  performance refers to system-level efficiency metrics rather than task-level or downstream accuracy. across diverse
  settings and poorly represent real-world use.
* •
  
  Infrastructure Implementation: Some impressive studies rely on hybrid or proprietary infrastructure implementations,
  mixing algorithmic innovations with kernel- or system-level optimizations, which blurs the boundary between
  algorithmic and engineering gains.
* •
  
  Efficiency Metrics: Reported metrics are inconsistent across studies, ranging from latency per token to throughput on
  specific hardware, making it difficult to compare results or account for true computational cost.

To better promote the development of DLMs, this work aims to systematically evaluate their efficiency and investigate
their performance relative to AR models across a wider range of conditions. Our investigation is guided by three key
research questions:
* •
  
  RQ1: How do DLMs compare with AR Models in efficiency?
* •
  
  RQ2: How can we theoretically analyze the variations in inference throughput across architectures?
* •
  
  RQ3: How do acceleration strategies benefit DLMs in varied conditions?

Guided by these questions, we undertake a further investigation, integrating empirical analysis with theoretical
insights into DLM efficiency and acceleration. The principal findings and contributions of this work are as follows:
* •
  
  Systematic efficiency evaluation: We conduct a comprehensive comparison of three model types—DLM, AR and block
  diffusion—under varying sequence lengths and batch sizes. Our findings show that AR models consistently achieve the
  highest throughput, followed by block diffusion, with DLMs being the slowest across most evaluated settings, including
  different prompt lengths, generation lengths and batch sizes.
* •
  
  Throughput modeling and theoretical analysis: We conduct a theoretical analysis aiming to provide a fine-grained
  understanding of inference throughput. Specifically, we model throughput as a function of hardware-side
  performance (FLOPs/s) and model-side efficiency (FLOPs/token), and analytically characterize how autoregressive,
  diffusion, and block diffusion models behave under varying sequence lengths and batch sizes.
* •
  
  Empirical insights into acceleration strategies: We analyze two major types of acceleration methods for DLMs—reducing
  per-step cost (*e.g.,* dual cache) and reducing step count (*e.g.,* parallel decoding). We find that these
  acceleration strategies yield significant gains at a batch size of 1, sometimes outperforming AR models, but their
  advantage diminishes as batch size grows, eventually falling behind AR.

The remainder of this technical report is organized as follows: Section [2][33] reviews relevant background and related
work. Section [3][34] discusses key issues in current efficiency evaluation practices. Section [4][35] introduces our
three core research questions and details the experimental setup. Section [5][36] presents our main empirical results
and theoretical analysis. Finally, Section [6][37] outlines future directions and open challenges for advancing
efficient diffusion-based language modeling.

## 2 Background and Related Work

In this section, we review the background and related work about different paradigms of language models.

### 2.1 Inference Paradigms of AR, Diffusion, and Block Diffusion Language Models

#### 2.1.1 Autoregressive Language Models

Autoregressive language models represent the dominant paradigm in current large language models, such as GPT, LLaMA, and
Qwen series. They are trained to predict the next token by attending only to previous ones, which yields high generation
quality but enforces strictly sequential decoding. To speed up this process, the KV Cache stores key and value tensors
from previous tokens, eliminating redundant computation and enabling faster inference. However, this comes at the cost
of significantly increased memory usage and bandwidth pressure.

#### 2.1.2 Diffusion Language Models

Diffusion language models generate text through an iterative denoising process: starting from random noise, they
progressively refine the sequence into a coherent sample from the data distribution. Unlike AR models, DLMs update all
token positions in parallel during each refinement step, allowing for simultaneous generation. Architecturally, they
typically use bidirectional (non-causal) attention, allowing every token to attend to the full context of the entire
sequence. However, vanilla bidirectional attention is not compatible with KV caching, leading to higher latency as
sequence length grows. A notable example of DLMs is LLaDA, a masked diffusion model trained from scratch. It employs a
forward data masking process and a reverse process parameterized by Transformer to predict masked tokens, demonstrating
strong scalability and competitive performance on various benchmarks.

#### 2.1.3 Block Diffusion Language Models

Block diffusion models combines autoregressive dependencies across blocks with parallel diffusion refinement within each
block, maintaining long-range contextual coherence while enabling efficient parallel generation. This architecture also
naturally supports KV caching, enhancing inference throughput. It introduces a complementary attention mask that enables
bidirectional attention within blocks while preserving autoregressive dependencies across them. During inference, this
design allows the model to perform autoregressive generation across blocks and parallel diffusion refinement within each
block, leading to a significant reduction in training cost and improved inference efficiency. For example,
BD3-LM [5-iclr-2025-marianne], trained from scratch, demonstrates that block-level diffusion can substantially improve
efficiency without sacrificing generation quality. Building on this direction, D2F and Fast-dLLM
v2 [6-arxiv-2025-chengyue] show that block diffusion-style generation can also be achieved by training from existing
models—D2F from diffusion backbones and Fast-dLLM v2 from autoregressive ones.

### 2.2 Strategies for Efficient DLM Inference

Existing acceleration methods for DLMs can be broadly categorized into two approaches: reducing per-step computational
overhead and reducing the number of diffusion sampling steps.

#### 2.2.1 Reducing Per-step Computational Overhead

Representative approaches for reducing per-step cost can be classified into two lines:

Sequence-level KV Cache. One line of works store and reuse key/value states across decoding steps: The conventional KV
cache is designed for strictly autoregressive decoding and does not directly apply to DLMs. To address this, recent work
shows that redesigning the decoding schedule allows DLMs to recover much of the efficiency benefit of KV caching. For
example, Fast-dLLM [7-arxiv-2025-chengyue] introduces a DualCache (prefix + suffix) to improve reuse while bounding
quality loss. DPad [8-arxiv-2025-xinhua] proposes a suffix-window and distance-decay dropout to restrict attention and
thereby limit suffix token caching and computation. And Sparse‑dLLM [9-arxiv-2025-yuerong] develops a training-free
dynamic cache eviction framework that retains only salient token states in cache and evicts less relevant prefix/suffix
entries to improve throughput.

Step-level Feature Reuse. Another line of works reuse stable intermediate representations inside each denoising
iteration. dKV-Cache [10-arxiv-2025-xinyin] introduces a delayed caching scheme, where a token’s key and value states
are cached one denoising step after it is decoded. dLLM-Cache [21-arxiv-2025-zhiyuan] uses feature similarity to
identify stable tokens and reuse their cached features., and FreeCache [11-arxiv-2025-zhanqiu] reuses stable KV states
of early clean tokens while updating only actively changing ones. These methods are complementary to sequence-level
caching.

#### 2.2.2 Reducing Diffusion Sampling Steps

Another major direction for improving DLM efficiency is reducing the number of denoising steps required during
generation. Existing methods typically achieve this through parallel decoding or progressive distillation:

Parallel Decoding. The main challenge is that token predictions interfere with each other because they are dependent.
Fast-dLLM counters this by unmasking tokens whose predicted probabilities exceed a confidence threshold. Tokens below
the threshold remain masked until later rounds.

Step Reduction via Distillation. A key direction for accelerating DLMs is reducing the number of denoising steps through
distillation. Early works on diffusion models, such as Progressive Distillation [12-iclr-2022-tim], adopt
teacher–student schemes that iteratively halve the required sampling steps. Di4C [13-arxiv-2024-satoshi] extends this
approach to discrete diffusion by explicitly distilling inter-token correlations. Recently,
DLM-One [14-arxiv-2025-tianqi] apply these distillation ideas to continuous DLMs, using score-based distillation to
train a one-step generator that produces the full sequence in a single forward pass, achieving substantial speedups
while maintaining near-teacher quality.

## 3 Efficiency Evaluation Issues

While acceleration techniques for DLMs are advancing rapidly, the field currently lacks standardized evaluation
protocols. The wide divergence in experimental configurations across studies often renders their efficiency claims
incomparable—or even an inaccurate reflection of genuine performance gains. By reviewing existing evaluation
methodologies, we identify several issues that may affect the comprehensiveness of efficiency evaluation. Our objective
is to highlight these challenges in efficiency evaluation, thereby fostering more rigorous and comparable research
practices within the field.

Evaluation Scope. Current evaluations of these techniques primarily focus on simplified and constrained settings—most
notably using a batch size of one and fixed output lengths. Such limited scope fails to capture the full range of
real-world deployment scenarios: evaluations cannot fully reflect how these methods perform across varying batch sizes,
output length distributions, or diverse generation tasks. The lack of comprehensive benchmarking across different
operational conditions limits our understanding of when and where each acceleration technique provides genuine
advantages.

Infrastructure Implementation. Another challenge lies in the heterogeneous infrastructure configurations adopted across
studies. Fair efficiency comparisons require controlled inference environments, yet many works integrate kernel- or
system-level optimizations into their core modeling contributions. Some representative DLMs that report impressive
decoding speeds remain closed-source, making it difficult to verify whether the gains arise from architectural
innovations or implementation optimizations. Such inconsistencies hinder reproducibility and obscure the true sources of
efficiency improvements across the field.

Efficiency Metrics. Efficiency reporting are inconsistent across studies, making it difficult to compare models fairly.
Some studies report latency per token [11-arxiv-2025-zhanqiu], while others focus on throughput (tokens per second) on
specific hardware setups [9-arxiv-2025-yuerong, 10-arxiv-2025-xinyin]. These metrics, while useful, often fail to
capture the complete picture. For example, a model might show high throughput, but if each step involves heavy
computation, the overall cost could still be high. This is especially relevant for DLMs, which often require more
operations per token than AR models. To support clearer and more practical comparisons, it would be helpful to include
standardized metrics that consider both computational cost and decoding performance. Examples might include FLOPs per
token or throughput under fixed resource constraints. These indicators can offer a more balanced view of model
efficiency across different architectures.

## 4 Experimental Design and Setup

### 4.1 Overview and Research Questions

Motivated by the limitations of existing works discussed in Section 3, this paper presents an empirical study of
inference efficiency for current architectures. We conduct a comparative analysis across three foundational
architectures AR, DLM and block diffusion—along with their respective acceleration methods. Specifically, we organize
this study around three key research questions. For each question, we perform targeted experiments and analyses to help
readers develop a clearer understanding of the efficiency differences. The three research questions under investigation
are listed as follows:

RQ1: How do DLMs compare with AR Models in efficiency? We conduct a controlled comparison of these architectures,
evaluating their throughput under varying conditions. The analysis specifically investigates the impact of critical
factors such as prompt length, generation length, and batch size.

RQ2: How can we theoretically explain variations in inference efficiency? We aim to provide a fine-grained theoretical
analysis that interprets throughput in terms of hardware-side performance (FLOPs/s) and model-side
efficiency (FLOPs/token). This analysis helps explain the empirical results in RQ1, reveal key computational
bottlenecks, and suggest potential directions for future acceleration.

RQ3: How do DLMs benefit from acceleration strategies under varied conditions? In RQ3, we investigate existing
acceleration strategies by classifying them into two main categories: reducing the computational cost per denoising step
and decreasing the total number of steps. We evaluate their performance across varied inference scenarios to reveal
conditions under which these strategies yield meaningful efficiency gains and guide the design of more efficient DLMs.

### 4.2 Experimental Setup

To ensure a fair and consistent comparison of inference efficiency across different models and methods, we standardized
our experimental setup as follows:

#### 4.2.1 Inference Framework and Hardware

All evaluations are conducted using the Hugging Face Transformers library. To maintain consistency in core computations,
the attention mechanism for all models leverages PyTorch’s official torch.nn.functional.scaled_dot_product_attention.
All experiments are conducted on a single NVIDIA A800 GPU (80 GB) using FP16 precision. The experiments are run within a
unified Conda virtual environment to ensure that all core dependencies, including PyTorch and CUDA, are identical across
all tests.

#### 4.2.2 Benchmark and Scenarios

We use the GSM8K dataset for all performance benchmarks. The prompt length is controlled by adjusting the number of
examples (*i.e.,* 0-shot and 5-shot) in the prompt. For generation length, DLMs allow direct parameter control. For AR
and block diffusion models, we enforce generation to continue until the target length is met (*i.e.,* ranging from 64 to
2048), even after an “<eos>” token is produced, to ensure a fair comparison of throughput.

#### 4.2.3 Models and Acceleration Methods

We evaluate three representative models and their default decoding strategies:
* •
  
  LLaDA-8B-Instruct (DLM)²²2https://huggingface.co/GSAI-ML/LLaDA-8B-Instruct: it achieves optimal performance when the
  number of sampling steps matches the output length, making each decoding step effectively generate about one token,
  without KV-cache support.
* •
  
  LLaMA-3.1-8B-Instruct (AR)³³3https://huggingface.co/meta-llama/Llama-3.1-8B-Instruct: it follows a left-to-right
  autoregressive decoding process, generating one token per decoding step with KV-cache.
* •
  
  Fast-dLLM v2-7B (block diffusion)⁴⁴4https://huggingface.co/Efficient-Large-Model/Fast_dLLM_v2_7B: it generates text
  block by block—each block decoded sequentially and predicted in parallel internally—but, similar to LLaDA, each
  decoding step effectively produces about one token, while supporting KV-cache.

Specifically for the LLaDA model, we evaluate two acceleration strategies introduced in the Fast-dLLM series: the dual
cache strategy (to reduce per-step computation) and the confidence-aware parallel decoding strategy (to reduce the
number of steps). For the block diffusion, which inherently supports KV cache utilization due to its block-wise
autoregressive design, we analyze the confidence-aware parallel decoding strategy. All hyperparameters for these
acceleration methods are configured according to their original papers’ settings for GSM8K, and we further verify that
the accuracy remains aligned with the original models.

#### 4.2.4 Evaluation Metrics

We mainly consider two commonly used evaluation metrics:

Throughput. To quantify time efficiency, we report the end-to-end decoding throughput in tokens per second. This metric
is computed by dividing the number of generated tokens by the total wall-clock time, measured from the start of the
generation process to the completion of the final token.

Arithmetic Intensity. In model inference, the overall latency is determined by both computational workload and memory
access. The computational workload is typically measured in floating-point operations (FLOPs), while memory activity is
quantified in bytes of read and write operations (MOPs). Furthermore, the arithmetic intensity (Luebke *et
al.* [gpgpu-siggraph-2004-david]) is defined as the ratio of floating-point operations to memory operations, measuring
the balance between computation and data movement:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
Arithmetic Intensity=Volume of Computation (FLOPs)Volume of Memory Access (MOPs)(FLOPs/Byte)\text{Arithmetic      │(1)
Intensity}=\frac{\text{Volume\ of\ Computation (FLOPs)}}{\text{Volume\ of\ Memory\ Access                         │   
(MOPs)}}\quad(\text{FLOPs/Byte})                                                                                  │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

Arithmetic intensity characterizes how many floating-point operations are executed for each byte of data moved. A low
arithmetic intensity indicates that the workload requires frequent memory access relative to computation, meaning the
attainable performance is limited by memory bandwidth—a scenario referred to as memory-bound. Conversely, a high
arithmetic intensity implies that the operation performs many computations per byte transferred, and the performance
bottleneck shifts to the GPU’s compute capacity, known as a compute-bound regime.

To understand the relationship between arithmetic intensity and performance, we adopt the roofline model
[15-arxiv-2009-samue] for our analysis, following the approach of Kim *et al.* [16-arxiv-2025-minseo]. This model
relates a system’s peak compute performance to its memory bandwidth, identifying whether a workload is compute-bound or
memory-bound. In this framework, the arithmetic intensity ridge point (Arithmetic Intensityridge\text{Arithmetic
Intensity}_{\text{ridge}}) defines the boundary between the two regimes (compute-bound and memory-bound) and is
calculated as follows:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
Arithmetic Intensityridge=Peak FLOP PerformancePeak Memory Bandwidth(FLOPs/Byte).\text{Arithmetic                 │(2)
Intensity}_{\text{ridge}}=\frac{\text{Peak FLOP Performance}}{\text{Peak Memory Bandwidth}}\;(\text{FLOPs/Byte}). │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

According to the roofline model, workloads with arithmetic intensity below this ridge point are memory-bound regime,
while those above it are compute-bound regime. In the memory-bound regime, the attainable performance PP scales linearly
with arithmetic intensity, with a slope equal to the peak sustainable memory bandwidth BmemB_{\text{mem}}:

──────────────────────────────────────────────────────────────────────────────
P=Bmem×Arithmetic Intensity.P=B_{\text{mem}}\times\text{Arithmetic Intensity}.
──────────────────────────────────────────────────────────────────────────────

In contrast, in the compute-bound regime, the performance reaches the flat ceiling defined by the peak floating-point
performance PmaxP_{\text{max}}.

## 5 Evaluation and Analysis

### 5.1 RQ1: How do DLMs compare with AR Models in efficiency?

For this question, we compare the efficiency of three types of models—DLM, AR, and block diffusion. We further
investigate the factors influencing their efficiency and discuss how these models’ efficiency varies across different
scenarios. Here, we consider two main factors, sequence length and batch size, which are critical for all these models.

[Refer to caption] (a) Throughput (0-shot).
[Refer to caption] (b) Throughput (5-shot).
[Refer to caption] (c) Throughput (long generation).
Figure 1: (a) and (b) show the throughput comparison across different generation lengths under the 0-shot and 5-shot
settings, respectively, while (c) presents the throughput comparison between AR and block diffusion models under longer
generation lengths.

#### 5.1.1 Effect of Sequence Length

Experimental Settings. We select LLaDA-8B-Instruct, LLaMA3.1-8B-Instruct, and Fast-dLLM v2-7B as the representative
models for DLM, AR and block diffusion. Subsequently, we fix the batch size as 1 and evaluate the effect of sequence
lengths on model efficiency. Specifically, we compare the model performance from two aspects: prompt length and
generation length. For convenience, we sample 100 examples from the GSM8K dataset for evaluation. We conduct both 0-shot
and 5-shot experiments, corresponding to short and long prompt lengths, respectively. For each experiment setting, we
evaluate different generation lengths, considering values of 64, 128, 256, 512, 1024, and 2048 tokens. Since AR cannot
precisely control their effective generation length, we enforce them to generate an “<eos>” token until the generation
exceeds the target length for fair comparison. We adopt a similar approach for block diffusion.

Main Results. We present the results of short and long prompt length in Figure [1][38].

First, across different generation lengths and prompt settings, the AR is faster than block diffusion, which in turn is
faster than DLM. We can observe that regardless of the input and output lengths, the throughput of the DLM is
significantly lower than that of AR and block diffusion with comparable size. We infer that DLM requires encoding the
entire sequence when modeling each token, which greatly increases computational cost. Similar to AR, block diffusion is
unidirectional, but its throughput decreases due to the diffusion-based modeling within each block.

Second, as the generation length increases, the throughput of DLM drops rapidly, while AR and block diffusion remain
relatively stable within the 2K tokens. Under different prompt lengths, we can consistently observe that the throughput
of the DLM drops rapidly as the generation length increases. In contrast, the throughput of the other two models remains
roughly constant around a sequence length of 2K. As we extend the generation length further, their throughput decreases
gradually, though at a much slower rate than the DLM.

Finally, increasing the prompt length leads to a decrease in throughput for all three models, with the DLM being the
most affected. We compared the throughput variations of the three models under short and long prompt lengths. We can
observe that the throughput of all three models decreases as the input length increases. Among them, AR and block
diffusion show only a slight decline (around 10%), and maintain relatively stable throughput across different input
lengths. In contrast, the DLM exhibits a much larger drop, especially on shorter generation length. Specifically, the
DLM’s throughput drops by about 75% at a generation length of 64 and by around 50% at 1024. We hypothesize that this
behavior results from the quadratic computational complexity of DLM with respect to sequence length.

[Refer to caption] (a) Throughput (0-shot).
[Refer to caption] (b) Throughput (5-shot).
Figure 2: The throughput comparison across different batch sizes under the 0-shot (a) and 5-shot (b) settings,
respectively.

#### 5.1.2 Effect of Batch Size

Experimental Settings. We choose the same model and data as the evaluation of sequence length. We fix the generation
length as 256 tokens, and prompt length as 40 (0-shot) and 920 (5-shot) tokens. Subsequently, we evaluate the changes of
throughput under different batch sizes, *i.e.,* 1, 2, 4, 8, 16, 20, 24, 32, and 64.

Main Results. We present the results of effect of batch size under the settings of short and long prompt lengths in
Figure [2][39].

First, consistent with the evaluation on sequence lengths in Section [5.1.1][40], DLM remains slower than block
diffusion, which in turn is slower than AR. Similar to the observations in analysis of sequence lengths, the DLM is also
significantly slower than the other models.

Second, the throughput of DLM remains consistent across different batch sizes, but it eventually hits GPU memory limits
at larger batch sizes. As the batch size increases, the throughput of the DLM is consistent, which may be owing to the
compute-bound decoding. Additionally, the memory cost increases sharply, becoming out-of-memory with the batch sizes of
16 and 64 under the long and short prompt lengths.

Finally, as the batch size increases, the throughput of both AR and block diffusion grows steadily until it eventually
stabilizes. Unlike DLM, the throughput of AR and block diffusion rises with larger batch sizes. However, after a certain
point, the throughput of block diffusion plateaus, resembling the behavior of the DLM, whereas the AR model keeps
increasing.

### 5.2 RQ2: How can we theoretically explain the variations in inference throughput?

To better understand the throughput variations observed in RQ1, we conduct a theoretical analysis of how inference
throughput varies with critical factors (*i.e.,* sequence length and batch size) across different model architectures.
In a prior study, Kim *et al.* [16-arxiv-2025-minseo] adopt the roofline model [15-arxiv-2009-samue] to derive
asymptotic formulations of FLOPs, MOPs, and arithmetic intensity for AR and DLMs, thereby revealing whether each model
operates in a compute- or memory-bound regime. This analysis provides valuable intuition about hardware utilization.
Inspired by their work, we focus on the overall inference throughput and develop an extended analytical framework that
explicitly connects model-side computational efficiency (FLOPs per token) with hardware-side computational performance
(FLOPs per second). This formulation allows us to interpret throughput variations through both hardware and
model-architecture perspectives, revealing how architectural design, sequence length, and batch size affect inference
efficiency.

#### 5.2.1 Theoretical Analysis of Inference Throughput

To formalize how inference throughput is influenced by hardware performance and model design, we decompose throughput
into two components: (1) hardware-side computational performance, representing the amount of computation effectively
executed per unit time (FLOPs/s) and determined by the workload’s arithmetic intensity and the roofline model; and (2)
model-side computational efficiency, defined as the average computational cost required to generate a single token
(FLOPs/token). Formally, the inference throughput can be expressed as the ratio between these two components, as shown
below:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
Throughput=Generated TokensGeneration Time=FLOPs/Generation TimeFLOPs/Generated Tokens=FLOPs per secondFLOPs      │(3)
 per token.\text{Throughput}=\frac{\text{Generated Tokens}}{\text{Generation Time}}\\                             │   
=\frac{\text{FLOPs}/\text{Generation Time}}{\text{FLOPs}/\text{Generated                                          │   
Tokens}}=\frac{\text{FLOPs~per~second}}{\text{FLOPs ~per~token}}.                                                 │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───
[Refer to caption] (a) ArInt (0-shot).
[Refer to caption] (b) ArInt (5-shot).
[Refer to caption] (c) ArInt (long generation).
Figure 3: (a) and (b) show the arithmetic intensity comparison across different generation lengths under the 0-shot and
5-shot settings, respectively, while (c) presents the arithmetic intensity comparison between AR and block diffusion
models under longer generation lengths.
[Refer to caption] (a) ArInt (0-shot).
[Refer to caption] (b) ArInt (5-shot).
Figure 4: The arithmetic intensity comparison across different batch sizes under the 0-shot (a) and 5-shot (b) settings,
respectively.

Hardware-Side Performance. To estimate the hardware-side attainable performance, we first compute the arithmetic
intensity of each model and then apply the roofline model to obtain its attainable FLOPs per second. Specifically, based
on the formulations proposed by Kim *et al.* [16-arxiv-2025-minseo], we derive simplified asymptotic expressions for
FLOPs per second under both memory- and compute-bound conditions, as summarized in Table [1][41] (rows of FLOPs per
second). Building on the formulation in Table [1][42], we further perform detailed approximations of arithmetic
intensity under the practical experimental settings used in RQ1, with the corresponding formulas provided in
Appendix [A][43]. Figure [3][44] and Figure [4][45] present these estimated values together with the hardware-specific
arithmetic intensity ridge, which separates the memory-bound and compute-bound regimes. Configurations below the ridge
are approximately memory-bound, whereas those above are compute-bound. This analysis allows us to infer the attainable
computational performance for each setting, providing the basis for the subsequent throughput interpretation.

Table 1: Analysis of FLOPs per second and FLOPs per token for AR, block diffusion, and DLMs. The gray rows are derived
based on the asymptotic analysis proposed by Kim *et al.* [16-arxiv-2025-minseo], following their approximate
formulations and notations. Here, BmemB_{\text{mem}} denotes the peak sustainable memory bandwidth, PmaxP_{\text{max}}
is the peak floating-point performance, L=Lp+LgL=L_{p}+L_{g} (the total length of prompt and generated response), BB is
batch size, dd is hidden dimension, KK is the number of diffusion steps, and GG is the block size.

──────┬─────────────────────────────────────┬─────────────────────────────────────┬─────────────────────────────────────
      │AR                                   │Block Diffusion                      │DLM                                  
──────┼─────────────────────────────────────┼─────────────────────────────────────┼─────────────────────────────────────
FLOPs │\cellcolor[HTML]F7F7F7{𝒪(BmemB),L≪d𝒪 │\cellcolor[HTML]F7F7F7{𝒪(BmemBG),L≪d𝒪│\cellcolor[HTML]F7F7F7{𝒪(BmemBL),L≪d𝒪
per   │(Bmem),L≫d\displaystyle\par\cellcolor│(Bmem                                │(Bmem                                
second│[HTML]{F7F7F7}\begin{cases}\mathcal{O│G),L≫d\displaystyle\cellcolor[HTML]{F│L),L≫d\displaystyle\cellcolor[HTML]{F
(memor│}(B_{\text{mem}}B),\ L\ll d\\        │7F7F7}\begin{cases}\mathcal{O}(B_{\te│7F7F7}\begin{cases}\mathcal{O}(B_{\te
y-boun│\mathcal{O}(B_{\text{mem}}),\ L\gg   │xt{mem}}BG),\ L\ll d\\               │xt{mem}}BL),\ L\ll d\\               
d)    │d\end{cases}                         │\mathcal{O}(B_{\text{mem}}G),\ L\gg  │\mathcal{O}(B_{\text{mem}}L),\ L\gg  
      │                                     │d\end{cases}                         │d\end{cases}                         
──────┼─────────────────────────────────────┼─────────────────────────────────────┼─────────────────────────────────────
FLOPs │PmaxP_{\text{max}}                   │PmaxP_{\text{max}}                   │PmaxP_{\text{max}}                   
per   │                                     │                                     │                                     
second│                                     │                                     │                                     
(compu│                                     │                                     │                                     
te-bou│                                     │                                     │                                     
nd)   │                                     │                                     │                                     
──────┼─────────────────────────────────────┼─────────────────────────────────────┼─────────────────────────────────────
Genera│BLgBL_{g}                            │BLg≈BKBL_{g}\approx BK               │BLg≈BKBL_{g}\approx BK               
ted   │                                     │                                     │                                     
Tokens│                                     │                                     │                                     
──────┼─────────────────────────────────────┼─────────────────────────────────────┼─────────────────────────────────────
FLOPs │\cellcolor[HTML]F7F7F7 𝒪(d2)+𝒪(L     │\cellcolor[HTML]F7F7F7 𝒪(Gd2)+𝒪(GL   │\cellcolor[HTML]F7F7F7 𝒪(Ld2)+𝒪(L2   
per   │d)\mathcal{O}(d^{2})+\mathcal{O}(Ld) │d)\mathcal{O}(Gd^{2})+\mathcal{O}(GLd│d)\mathcal{O}(Ld^{2})+\mathcal{O}(L^{
token │                                     │)                                    │2}d)                                 
──────┴─────────────────────────────────────┴─────────────────────────────────────┴─────────────────────────────────────

Model-Side Efficiency. As shown in Equation [4][46], the average FLOPs per token is a statistical measure of the model’s
computational efficiency. It is obtained by dividing the model’s theoretical asymptotic FLOPs—representing the total
floating-point operations required for the entire generation process—by the total number of generated tokens. The
results of the three architectural model types are shown in Table [1][47].

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
FLOPs/token=Total FLOPs for decodingGenerated Tokens.\text{FLOPs/token}=\frac{\text{Total FLOPs for               │(4)
decoding}}{\text{Generated Tokens}}.                                                                              │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

#### 5.2.2 Analysis of AR Models

In the decoding phase of AR models, the model generates only one token at each step, incurring per-token FLOPs of 𝒪(B
d2)+𝒪(BLd)\mathcal{O}(Bd^{2})+\mathcal{O}(BLd) and MOPs of 𝒪(d2)+𝒪(BLd)\mathcal{O}(d^{2})+\mathcal{O}(BLd) due to
Key-Value (KV) cache reads. According to Figure [3][48], at the batch size of one, the arithmetic intensity of the AR
decoding process is the lowest, making it a memory-bound workload. Consequently, its achievable throughput is mainly
limited by memory bandwidth and can be asymptotically expressed as:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
Throughput≈{𝒪(BmemB)𝒪(d2)+𝒪(Ld)=𝒪(B)𝒪(d2)+𝒪(Ld),L≪d,𝒪(Bmem)𝒪(d2)+𝒪(Ld)=𝒪(1)𝒪(d2)+𝒪(L                              │(5)
d),L≫d.\text{Throughput}\approx\begin{cases}\dfrac{\mathcal{O}(B_{\text{mem}}B)}{\mathcal{O}(d^{2})+\mathcal{O}(Ld│   
)}=\dfrac{\mathcal{O}(B)}{\mathcal{O}(d^{2})+\mathcal{O}(Ld)},&L\ll d,\\[10.0pt]                                  │   
\dfrac{\mathcal{O}(B_{\text{mem}})}{\mathcal{O}(d^{2})+\mathcal{O}(Ld)}=\dfrac{\mathcal{O}(1)}{\mathcal{O}(d^{2})+│   
\mathcal{O}(Ld)},&L\gg d.\end{cases}                                                                              │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

This formulation is applied in the analysis of generation length (with the batch size fixed at 11) and batch size (under
the condition L≪dL\ll d).

Effect of Generation Length. The throughput formula can be derived from Equation [5][49]:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
Throughput≈{𝒪(1)𝒪(d2),L≪d,𝒪(1)𝒪(L                                                                                 │(6)
d),L≫d.\text{Throughput}\approx\begin{cases}\dfrac{\mathcal{O}(1)}{\mathcal{O}(d^{2})},&L\ll d,\\[10.0pt]         │   
\dfrac{\mathcal{O}(1)}{\mathcal{O}(Ld)},&L\gg d.\end{cases}                                                       │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

When the generation length is short, both the numerator and denominator in the throughput expression remain nearly
constant (batch size fixed at 1). The denominator is dominated by the 𝒪(d2)\mathcal{O}(d^{2}) term, while the 𝒪(L
d)\mathcal{O}(Ld) component is negligible. Consequently, the throughput remains stable with respect to sequence length.
When the generation length is long, however, the 𝒪(Ld)\mathcal{O}(Ld) term becomes dominant, so as LL increases, overall
throughput decreases.

Effect of Batch Size. In our evaluation setting (with generation length fixed at 256 tokens and relatively short
prompts), the condition L≪dL\ll d holds. In this regime, the denominator remains constant while the numerator scales
linearly with batch size BB (because of the arithmetic intensity is 𝒪(B)\mathcal{O}(B) ), leading to nearly linear
throughput growth until the system reaches the compute roof. Once the arithmetic intensity reaches the compute roof,
throughput remains constant as the workload becomes compute-bound.

#### 5.2.3 Analysis of Diffusion Models

In decoding, DLMs perform denoising steps on the entire sequence of length L=Lp+LgL=L_{p}+L_{g}. The per-step
computational cost scales as FLOPs 𝒪(BLd2)+𝒪(BL2d)\mathcal{O}(BLd^{2})+\mathcal{O}(BL^{2}d), with MOPs scaling as 𝒪
(d2)+𝒪(BLd)\mathcal{O}(d^{2})+\mathcal{O}(BLd) due to reading/writing activations.

Therefore, the arithmetic intensity satisfies:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
ArInt≈{𝒪(BL),L≪d,𝒪(L),L≫d.\text{ArInt}\approx\begin{cases}\mathcal{O}(BL),&L\ll d,\\[6.0pt] \mathcal{O}(L),&L\gg  │(7)
d.\end{cases}                                                                                                     │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

Effect of Generation Length. For ver

[Content truncated]
```
