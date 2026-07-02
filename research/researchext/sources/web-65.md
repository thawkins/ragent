# Web source

- URL: https://arxiv.org/html/2606.23221v1
- Title: ##### Report GitHub Issue
- Captured (UTC): 2026-06-30T09:41:20.266595047+00:00

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
   1. [2.1 Image Generation and Editing][9]
   2. [2.2 Unified Multimodal Understanding and Generation Models][10]
   3. [2.3 Visual Agentic Systems][11]
4. [3 RS-Gen][12]
   1. [3.1 Image Router Agent][13]
   2. [3.2 Intent Analysis Agent][14]
   3. [3.3 Reasoning & Search Agent][15]
   4. [3.4 Image Generation Agent][16]
5. [4 Experiments][17]
   1. [4.1 Benchmarks and Evaluation Protocols][18]
   2. [4.2 Experiment Settings][19]
   3. [4.3 Main Results][20]
   4. [4.4 Ablation Study][21]
   5. [4.5 Limitations & Future Work][22]
6. [5 Conclusion][23]
7. [6 Acknowledgments][24]
8. [References][25]
9. [A Additional Experimental Results][26]
[ License: arXiv.org perpetual non-exclusive license ][27]
arXiv:2606.23221v1 [cs.CV] 22 Jun 2026

# RS-Gen: A Multi-Stage Agentic Framework for Reasoning and Search-Augmented Image Generation

Feifei Bian, Zhimin Zheng, Wei Deng, Daiguo Zhou and Jian Luan
MiLM Plus, Xiaomi Inc.
{bianfeifei,zhengzhimin,dengwei1,zhoudaiguo,luanjian}@xiaomi.com

###### Abstract

Recent years have witnessed remarkable progress in image generation and editing, particularly regarding instruction
following and visual fidelity. However, when handling ambiguous intentions, logical reasoning, and Out-of-Distribution
(OOD) knowledge, existing image models often yield sub-optimal results due to a lack of deep reasoning capabilities and
real-time external information. Although emerging unified understanding-and-generation models attempt to bridge this
gap, they remain constrained by their intrinsic parameter scales and static knowledge gaps. Inspired by agentic
paradigms, we propose RS-Gen: a plug-and-play, training-free, multi-stage image agentic framework. RS-Gen innovatively
introduces a "Questioning-and-Solving" closed-loop mechanism to accurately identify logical issues and knowledge gaps,
autonomously planning actions to bridge information deficits and execute deep logical reasoning. Extensive experiments
demonstrate that RS-Gen significantly expands the capability boundaries of foundational image generation and editing
models. Specifically, on the WISE_Verified and RISEBench benchmarks, RS-Gen yields substantial absolute performance
gains of 0.313 for Qwen-Image and 19.70 for Qwen-Image-Edit-2511, respectively, successfully elevating both to the
state-of-the-art (SOTA) level among open-source models.

[[Uncaptioned image]]
Figure 1: Representative generation results by our proposed RS-Gen. By integrating external knowledge retrieval and
logical reasoning mechanisms, RS-Gen achieves superior accuracy and fidelity in challenging tasks, including specific
entity generation, logical puzzle-solving, visual reasoning, and physical evolution.

## 1 Introduction

In recent years, visual generation technologies, represented by diffusion models, have made breakthrough progress in the
fields of image generation and editing. A series of cutting-edge models have emerged, such as FLUX.1-dev [[1][28]],
Qwen-Image [[2][29]], Z-Image [[3][30]], and LongCat-Image [[4][31]], bringing the generated results to unprecedented
heights in terms of high fidelity and realism. However, despite their ability to synthesize visually striking images,
the trajectory toward truly "intelligent generation" remains impeded by three core bottlenecks. First, existing models
exhibit an insufficient capability in parsing implicit intentions. They are heavily reliant on precise and explicit user
prompts, lacking a deep semantic understanding of instructions that contain co-references or ambiguous expressions,
which causes them to struggle with grounding and manipulating target objects during complex multi-turn interactive
editing. Second, they lack visual logical reasoning capabilities. The current generation process is largely restricted
to a superficial "text-to-pixel" statistical mapping. When confronted with complex visual tasks such as logical
puzzle-solving or physical state evolution, models fail to comprehend the underlying causal logic and objective
constraints, frequently yielding outputs that violate physical laws and common sense. Finally, these models suffer from
inherent knowledge lag and the "hallucination" dilemma. Constrained by the static nature of training data and knowledge
cut-offs, they are incapable of perceiving novel concepts or long-tail entities; when processing Out-of-Distribution
(OOD) concepts, they are highly susceptible to factual errors and visual hallucinations, resulting in content that is
severely decoupled from the real world.

To overcome these limitations, the academic community has engaged in active exploration. For instance, unified
understanding-and-generation architectures, represented by Transfusion [[5][32]], JanusFlow [[6][33]], and
Bagel [[7][34]], have made notable progress in intent comprehension and logical reasoning. However, they still exhibit
significant bottlenecks when tackling multi-step reasoning or handling Out-of-Distribution (OOD) knowledge. Such
limitations suggest that traditional monolithic architectural designs may struggle to systematically address these
complex, end-to-end intelligent generation tasks.

Inspired by agentic technologies, researchers have begun to construct image agentic systems. Commercial products such as
Seedream [[8][35]], Nano Banana Pro [[9][36]], and FLUX-2 Max [[10][37]] have already demonstrated the immense potential
of synergizing search and reasoning in visual generation tasks. Concurrently, the open-source community is actively
advancing in this direction; research works like Mind-Brush [[11][38]] and Unify-Agent [[12][39]] leverage the deep
reasoning capabilities of Multimodal Large Language Models (MLLMs) alongside external search tools to augment
foundational models. Nevertheless, existing open-source solutions either fall short in their capacity to handle complex
problems or require prohibitively expensive data and training costs, failing to universally empower a broad spectrum of
open-source image models. Consequently, a substantial gap persists between the open-source community and commercial
products within the realm of "intelligent generation."

Inspired by the recent paradigm of OpenClaw [[13][40]], we propose RS-Gen, a multi-stage image agentic framework
augmented by reasoning and search. RS-Gen diverges from treating image generation and editing as a simplistic black-box
mapping process; instead, it reconstructs it into a "questioning-and-solving" closed-loop task driven collaboratively by
multi-stage and multi-agent systems. As a plug-and-play, training-free universal solution, RS-Gen can be seamlessly
integrated with existing open-source image models, significantly enhancing their capabilities in implicit intent
parsing, complex logical reasoning, and real-time information perception. As illustrated in Figure [1][41], RS-Gen
demonstrates exceptional performance in tasks that necessitate real-time knowledge retrieval and complex logical
reasoning. For instance, when confronted with highly challenging prompts such as "rendering the latest concept sports
car of a certain brand," "inferring and generating the shape represented by the question mark," and "moving a single
matchstick to correct the equation," the framework consistently produces accurate and logically coherent visual content.

Specifically, the core advantages of RS-Gen include:
1. 1.
   
   Implicit Intent Parsing in Multi-Turn Interactions: Leveraging the agent’s memory mechanism and the reasoning
   capabilities of multimodal models, the system accurately captures the user’s deep intentions across multi-turn
   dialogues. By resolving coreferences and explicitly identifying the source images and target objects for editing, it
   reconstructs ambiguous and implicit user intentions into clear, actionable image generation and editing instructions.
2. 2.
   
   Knowledge Gap and Logical Issue Detection: Utilizing Multimodal Large Language Models (MLLMs), the system
   pre-evaluates the complexity of user instructions to precisely identify latent information deficits and logical
   reasoning barriers. For complex tasks, the system proactively formulates questions centered around these knowledge
   gaps and logical issues, thereby guiding the subsequent search and reasoning processes.
3. 3.
   
   Retrieval-Augmented Generation: By integrating powerful external information retrieval tools, RS-Gen thoroughly
   breaks the static knowledge constraints imposed by the models’ inherent training data, providing accurate factual
   grounding and a robust logical basis for the subsequent image generation and editing.
4. 4.
   
   Autonomous Planning and Self-Correction Mechanism Driven by the ReAct Paradigm: Drawing upon the ReAct [[14][42]]
   pattern, the system follows an "Observation-Thought-Action" paradigm prior to executing specific image generation. It
   autonomously conducts step-by-step planning and tool invocation, dynamically adjusting strategies based on tool
   feedback to ensure that knowledge gaps and logical reasoning issues are resolved before image generation begins.
   Concurrently, during the image generation phase, the system introduces a "Generate-Review-Correct" self-correction
   closed loop, which significantly enhances the reliability and robustness of the final output.

## 2 Related Work

### 2.1 Image Generation and Editing

In recent years, Latent Diffusion Models (LDMs), prominently represented by Stable Diffusion (SD) [[15][43]], have
achieved milestone advancements in the field of image generation. By performing iterative denoising within a
low-dimensional latent space, SD enables the efficient synthesis of high-resolution images. Subsequently, pioneering
works such as SD3 [[16][44]] and FLUX.1 [[1][45]] introduced Flow Matching [[17][46]] techniques. Coupled with
large-scale Transformer [[18][47]] architectures, these models have further elevated the fidelity and photorealism of
visual generation to unprecedented heights. However, despite these massive leaps in visual synthesis quality, such
models continue to confront a significant "semantic gap" when tackling complex tasks. Their underlying architectures
rely heavily on static pre-trained text encoders, such as CLIP [[19][48]] or T5 [[20][49]]. Constrained by limited
semantic representation spaces, these models frequently exhibit a severe degradation in instruction understanding and
instruction following capabilities when processing complex prompts that involve ambiguous intentions or coreferences.
Inherently, the generation process of these models remains confined to a "text-to-pixel" statistical mapping, lacking
the capacity for deep comprehension and logical reasoning regarding the high-level semantics underlying complex
instructions.

As user demands evolve from basic "generation from scratch" to the fine-grained modification of existing visual content,
image editing has emerged as a highly challenging frontier research topic. Unlike pure image generation tasks, image
editing is confronted with far more stringent multi-dimensional constraints: the model must not only accurately parse
and execute editing instructions but also strictly preserve the semantic coherence and visual textural consistency of
non-editing regions during the editing process. In this context, InstructPix2Pix [[21][50]] pioneered a data-driven
"instruction-to-edit" mapping paradigm. Subsequently, a series of representative works, such as MagicBrush [[22][51]],
HQ-Edit [[23][52]], and Emu Edit [[24][53]], have successively emerged. These approaches primarily fine-tune
text-to-image models by constructing large-scale triplet datasets—comprising the source image, editing instruction, and
target image—which has significantly enhanced the models’ editing precision and generalization capabilities across
fine-grained tasks, including local modifications, style transfers, and attribute adjustments.

To further mitigate the challenges of complex instruction comprehension, researchers have begun exploring the
integration of Vision-Language Models (VLMs) into image editing architectures. This approach aims to leverage the
powerful multimodal understanding and logical reasoning capabilities of VLMs, thereby enhancing the semantic parsing,
context awareness, and fine-grained control capabilities of image editing models. For instance, works such as
OmniGen2 [[25][54]], Qwen-Image-Edit [[2][55]], and Step1X-Edit [[26][56]] innovatively employ VLMs to replace
traditional text encoders like CLIP [[19][57]] or T5 [[20][58]], utilizing the deep semantic representations output by
VLMs as control conditions for the generation process. The evolution of such architectural paradigms has significantly
elevated the models’ comprehension capabilities when tackling long texts and complex instructions.

However, although the introduction of VLMs has endowed image generation and editing models with enhanced semantic
comprehension capabilities, such architectures still confront the following three fundamental bottlenecks when tackling
real-world and complex user tasks:
1. 1.
   
   Coreference Ambiguity in Multi-Turn Interactions: Real-world application scenarios typically involve continuous
   multi-turn interactions, where user instructions are often highly colloquial and contain implicit referential
   pronouns (e.g., "add a cat next to him," "change it to a dog instead"). Existing image models lack contextual
   understanding and memory mechanisms, making it difficult to accurately parse user intentions and pinpoint the target
   objects for editing across multiple dialogue turns. Such intention resolution failures easily lead to unsuccessful
   image editing, rendering them inadequate to support real and complex user demands.
2. 2.
   
   Static Knowledge Cutoff and Hallucination: For the vast majority of models, their parameterized knowledge boundaries
   are solidified upon the completion of training. When confronted with out-of-distribution (OOD) novel concepts,
   real-time information, or long-tail entities (e.g., "generate the newly released conceptual supercar by brand X"),
   models frequently suffer from severe visual and factual hallucinations due to the lack of prior knowledge regarding
   the target’s accurate appearance and detailed features. This results in generated content that is severely decoupled
   from the objective real world.
3. 3.
   
   Lack of Explicit Logical Reasoning Mechanisms: Inherently, the generation process of existing image models remains a
   "text-to-pixel" statistical mapping, typically requiring users to provide direct and explicit instructions. When
   faced with instructions that possess ambiguous intentions or implicitly embed complex physical laws or visual logic
   puzzles (e.g., "draw the scene two hours later," "deduce and draw the shape represented by the question mark based on
   the visual patterns in the image"), models are incapable of performing explicit deep reasoning. Lacking the ability
   for in-depth deconstruction of the underlying rules behind the instructions and visual content, the generated results
   often violate physical common sense and basic logic, thereby failing to fulfill the users’ advanced demands.

### 2.2 Unified Multimodal Understanding and Generation Models

To fundamentally break the barriers between understanding and generation tasks and achieve their deep synergy,
researchers have proposed Unified Multimodal Understanding and Generation Models. Unlike the traditional separated
paradigm of "text encoder + image generator," unified architectures aim to map visual understanding and visual
generation into the same representation space for joint modeling. Works such as Chameleon [[27][59]], Show-o [[28][60]],
Janus [[29][61]], and Bagel [[7][62]] have fully demonstrated the immense potential of the native unification of
understanding and generation in enhancing generative performance. By leveraging their native and powerful multimodal
perception and parsing capabilities, such models can more profoundly comprehend physical common sense and logical rules,
thereby significantly alleviating the issue of "visual hallucinations" that violate objective common sense in the
generated results.

However, although unified models have achieved significant progress in multimodal understanding and synergistic
generation, they still confront severe challenges when tackling high-complexity tasks: (1) Static Knowledge and Factual
Hallucinations: The capability boundaries of unified models are constrained by the static data distribution during the
training phase. Once model training is completed, it inherently faces the "knowledge cutoff" issue. When confronted with
continuously emerging novel concepts and new knowledge in the real world, such models inevitably produce factual
hallucinations, making it difficult to guarantee the accuracy of the generated content. (2) Prohibitive Training Costs:
Unified models rely on massive amounts of high-quality interleaved image-text data and large-scale computing power. This
renders their training costs prohibitively high, making it difficult for the open-source community to bear their
adaptation and iteration costs. (3) Lack of Explicit Multi-Step Planning and Deep Reasoning Mechanisms: Despite
outperforming traditional separated architectures at the multimodal perception level, unified models are unable to
provide highly reliable and logically rigorous generated results when dealing with advanced tasks requiring multi-step
planning, self-correction, and deep reasoning, due to their inherently limited implicit reasoning capabilities.

### 2.3 Visual Agentic Systems

To break through the performance bottlenecks of monolithic architectures in complex multimodal understanding and
generation tasks, the field of artificial intelligence has recently been undergoing a paradigm shift from traditional
"instruction-following" models to "AI Agents" equipped with autonomous decision-making capabilities. Modern agents are
no longer confined to unimodal pure-text interactions or basic question-answering; rather, they have evolved into
composite architectures integrating advanced capabilities such as multimodal perception, long- and short-term memory,
autonomous multi-step planning, and external tool invocation.

Within the agentic architecture, the ReAct [[14][63]] paradigm endows models with the capability to acquire
environmental feedback and adjust strategies in real-time through an explicit "Thought-Action-Observation" closed-loop
mechanism, thereby significantly enhancing the logical rigor of agent systems when handling complex tasks. Cutting-edge
agent applications, exemplified by OpenClaw [[13][64]], further demonstrate the powerful potential of high-level synergy
among foundation models, memory modules, external tools, and specialized skills, successfully transforming ambiguous
user demands into executable automated workflows.

Inspired by these advancements, researchers have begun exploring Image Agents specifically tailored for visual tasks,
attempting to reconstruct traditional image generation and editing tasks into agent-driven automated workflows. Works
such as Mind-Brush [[11][65]], Unify-Agent [[12][66]], and Gen-Seacher [[30][67]] attempt to deploy Multimodal Large
Language Models (MLLMs) as the core decision-making hub. By invoking external retrieval tools to bridge inherent
knowledge gaps, these approaches effectively suppress the generation of factual hallucinations. These efforts mark the
evolution of generative AI from pure "text-to-pixel" shallow mapping to agentic architectures equipped with autonomous
planning and closed-loop processing capabilities for complex visual tasks.

However, existing image agent frameworks remain in an exploratory stage and confront the following significant
challenges when addressing exceptionally complex tasks:
1. 1.
   
   System Coupling and Training Cost: Existing image agent architectures are frequently deeply bound to specific
   underlying models or require prohibitively expensive training. For instance, Unify-Agent reframes image generation as
   an agentic pipeline but requires curating 143K highly tailored multimodal trajectories to explicitly fine-tune the
   model. This high-coupling and computationally expensive training paradigm severely restricts its generalization
   capabilities within the open-source ecosystem. Lacking plug-and-play flexibility, it struggles to broadly empower
   diverse foundation models.
2. 2.
   
   Workflow Rigidity and Absence of Fault Tolerance: Current frameworks typically adopt a linear execution pipeline. For
   example, Mind-Brush executes a one-way "think-research-create" paradigm. If intermediate retrieval steps fail or the
   generated image contains flaws, the system lacks an effective self-reflection and dynamic error-correction mechanism.
   This rigid workflow makes the system highly susceptible to error cascading, ultimately causing the generated results
   to deviate from factual accuracy or violate logical principles.

To address the aforementioned challenges, we propose the RS-Gen framework. Deeply inspired by the ReAct [[14][68]]
paradigm in the agentic domain, we innovatively construct a multi-stage agentic architecture enhanced by reasoning and
search. As a highly flexible, training-free solution, RS-Gen aims to comprehensively break through the performance
bottlenecks of existing open-source models when dealing with ambiguous intentions, complex logical reasoning, and
knowledge cutoffs through explicit logical deduction chains, factual retrieval closed-loops, and dynamic
error-correction mechanisms, thereby significantly elevating the reliability and accuracy of the generated results.

## 3 RS-Gen

[[Uncaptioned image]] Figure 2: The overall framework of RS-Gen. The overall architecture of the RS-Gen framework.
First, the user input is processed by the Image Routing sub-agent to accurately identify target images and disambiguate
vague references in the instructions. Then, the Intent Analysis sub-agent evaluates task complexity to plan the optimal
execution path, dynamically extracting key sub-problems and refining user instructions as needed. Subsequently, the
Reasoning and Search sub-agent autonomously invokes external tools to accomplish factual information retrieval and
logical reasoning. Finally, the Image Generation sub-agent executes specific image generation and editing tasks,
achieving iterative self-correction through a built-in review mechanism.

We propose RS-Gen, a multi-stage reasoning and search-augmented agentic framework designed to decouple and reconstruct
image generation and editing tasks from a monolithic "end-to-end" black-box mapping into a multi-stage collaborative
agentic workflow. As illustrated in Figure [2][69], RS-Gen operates through the synergy of four sub-agents with
specifically defined responsibilities. The core design philosophy of this framework is to introduce explicit reasoning
chains and closed-loop search feedback mechanisms. Prior to executing specific image generation actions, it
pre-emptively resolves ambiguous semantics in user instructions, bridges knowledge gaps, and completes complex logical
reasoning at both the logical and knowledge levels. Consequently, it transforms the original inputs into highly precise
and informationally complete image generation and editing instructions, as well as reference images.

The execution workflow of RS-Gen strictly adheres to the progressive paradigm of "Perception - Analysis - Retrieval &
Reasoning - Generation". The functional definitions of each core module are as follows:
1. 1.
   
   Image Router Agent: Serving as the entry module of the system, this agent perceives contextual information through a
   memory mechanism to accurately comprehend the user’s ambiguous task intentions. Its core responsibilities include
   localizing the target image and specific regions to be edited, and disambiguating vague references in colloquial user
   instructions, thereby preliminarily transforming original unstructured inputs into structured initial image operation
   instructions with clear intentions.
2. 2.
   
   Intent Analysis Agent: This agent is responsible for conducting in-depth semantic parsing of the initial
   instructions. It mines potential requirements for logical reasoning and knowledge gaps, abstracting and distilling
   them into specific structured queries, which provide clear goal guidance for the downstream retrieval and reasoning
   stage.
3. 3.
   
   Reasoning & Search Agent: Acting as the "cognitive hub" of the system, this module draws inspiration from the
   ReAct [[14][70]] pattern, possessing the capability to autonomously invoke external search engines, Visual Question
   Answering (VQA) tools, and logical reasoning tools. Through multi-step deep reasoning and external information
   retrieval, this agent effectively bridges knowledge blind spots, supplements factual evidence, and accomplishes
   complex logical deconstruction. Ultimately, it integrates multi-source information to further reconstruct the user’s
   intent into precise instructions and reference images that are informationally complete, logically self-consistent,
   factually grounded, and compliant with physical laws and objective common sense.
4. 4.
   
   Image Generation Agent: Functioning as the "execution and feedback terminal" of the system, this agent adopts the
   ReAct [[14][71]] execution mode and incorporates core tools such as "Generation", "Editing", and "Critique". It
   proactively executes image generation and editing tasks, and rigorously evaluates the alignment between the generated
   results and the user’s intentions, as well as the overall visual quality. Consequently, it autonomously decides
   whether to trigger a new round of local modification, iterative polishing, or complete regeneration. Through this
   "generation-critique-modification" closed-loop mechanism, the quality and reliability of the final generated results
   are significantly enhanced.

### 3.1 Image Router Agent

Serving as the entry module of the RS-Gen framework, the Image Router Agent aims to address the challenges of intent
ambiguity and coreference resolution in multi-turn multimodal dialogue scenarios. Formally, given the user’s original
instruction PtP_{t} and the input image set ℐin,t\mathcal{I}_{in,t} at the current time step tt, alongside the
multimodal historical context Ht−1={(Pi,ℐin,i,ℐou
t,i)}i=1t−1H_{t-1}=\{(P_{i},\mathcal{I}_{in,i},\mathcal{I}_{out,i})\}_{i=1}^{t-1} (where PiP_{i}, ℐi
n,i\mathcal{I}_{in,i}, and ℐout,i\mathcal{I}_{out,i} denote the user input instruction, the input image set, and the
system output image set at time step ii, respectively), this agent outputs structured task information. Specifically,
this comprises the image operation instruction Pt(R)P^{(R)}_{t}, which has undergone coreference resolution and
preliminary optimization, as well as the designated target or reference image set ℐin,t(R)\mathcal{I}^{(R)}_{in,t}.

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
(Pt(R),ℐin,t(R))=F(R)(Pt,ℐi                                                                                       │(1)
n,t,Ht−1)(P^{(R)}_{t},\mathcal{I}^{(R)}_{in,t})=\text{F}^{(R)}(P_{t},\mathcal{I}_{in,t},H_{t-1})                  │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

Multimodal Coreference Resolution and Image Anchoring: In complex multi-turn generation or editing tasks, user
instructions frequently contain implicit intents or ambiguous colloquial references (e.g., "draw the result represented
by the question mark" or "change its color to red"). Leveraging a memory mechanism, the agent deeply parses the
multimodal historical context Ht−1H_{t-1} to execute precise coreference resolution. For the anchoring of target or
reference images, this module designs a hybrid strategy based on time decay and intent matching. Specifically: if the
intent is new image generation without specifying or uploading a reference image, the target image set is empty, i.e.,
ℐin,t(R)=∅\mathcal{I}^{(R)}_{in,t}=\emptyset; if the intent is new image generation and a reference image is specified
or uploaded, the system accurately identifies and extracts this image set as ℐin,t(R)\mathcal{I}^{(R)}_{in,t}; if the
intent is image editing, the system prioritizes the results of coreference resolution to anchor the target image;
otherwise, it defaults to a backtracking mechanism, extracting the most temporally adjacent historical or uploaded image
as ℐin,t(R)\mathcal{I}^{(R)}_{in,t}.

Principle of Delayed Resolution: To prevent the model from hallucinating in the absence of external factual support and
deep reasoning bases, we propose a "Delayed Resolution" mechanism to strictly demarcate the functional boundaries of
this module. During the initial instruction reconstruction phase, the mapping function F(R)F^{(R)} is solely responsible
for image routing and coreference resolution. When encountering the following scenarios, the model is strictly
prohibited from utilizing its internal prior knowledge for ungrounded inference or heuristic completion: (1) External
Knowledge: Involving explicit objective entities, proper nouns, specific spatiotemporal coordinates, as well as
ambiguous entities, events, knowledge, or concepts. (2) State Evolution: Involving spatiotemporal progression or object
state changes induced by external forces. (3) Logical Deduction: Involving mathematical calculations, physical laws,
domain-specific knowledge, or complex causal logical inferences. Instead, the system treats these high-level semantic
constraints as unresolved variables, preserving them intact within Pt(R)P^{(R)}_{t}. This design effectively defers and
transfers all complex decisions requiring deep reasoning and external retrieval to the downstream "Reasoning & Search
Agent," thereby ensuring that the final generated results possess solid factual grounding and logical self-consistency.

Robust Instruction Translation Protocol: Considering the generally weak instruction-following capability of downstream
image generation and editing models (e.g., diffusion models) regarding negative semantics, this module incorporates a
semantic equivalent translation mechanism when generating the final instruction Pt(R)P^{(R)}_{t}. Specifically, this
mechanism translates negative constraints within user instructions into equivalent positive visual descriptions. For
instance, it reconstructs a negative state constraint such as "do not keep the stickers" into a positive attribute
description like "a solid and clean surface." This transformation significantly mitigates the feature contamination in
downstream diffusion models caused by the failure of negative semantic guidance during the generation process.

### 3.2 Intent Analysis Agent

Serving as the task routing and complexity evaluator of the RS-Gen framework, the Intent Analysis Agent receives the
instruction Pt(R)P^{(R)}_{t} and the image set ℐin,t(R)\mathcal{I}^{(R)}_{in,t} from the Image Router module. Its
primary function is to evaluate the complexity of the current task to determine whether to adopt the conventional direct
generation path or to trigger the downstream Reasoning and Search Agent. When it is determined that the task requires
external knowledge or logical support, this agent identifies potential knowledge blind spots and logical issues,
accurately extracting the key unresolved questions. Formally, the mapping function F(A)F^{(A)} of this agent outputs a
structured tuple (Ct,𝒬t,Pt(A))(C_{t},\mathcal{Q}_{t},P^{(A)}_{t}), where CtC_{t} denotes the routing decision flag,
𝒬t\mathcal{Q}_{t} represents the extracted structured query set, and Pt(A)P^{(A)}_{t} is the reconstructed instruction.

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
(Ct,𝒬t,Pt(A))=F(A)(Pt(R),ℐi                                                                                       │(2)
n,t(R))(C_{t},\mathcal{Q}_{t},P^{(A)}_{t})=F^{(A)}(P^{(R)}_{t},\mathcal{I}^{(R)}_{in,t})                          │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

Task Complexity Assessment and Adaptive Routing: To ensure an optimal balance between execution efficiency and
generation accuracy, this agent introduces a task complexity assessment mechanism aimed at identifying potential
challenges of the task in knowledge dimensions (e.g., time-sensitive information, domain-specific knowledge) and logical
dimensions (e.g., physical state evolution, visual puzzle-solving). Based on the evaluation results, the system executes
the following adaptive routing strategies:
* •
  
  Direct Generation (Ct=direct_generationC_{t}=\text{direct\_generation}): When Pt(R)P^{(R)}_{t} possesses a clear
  intent and explicit entities, without knowledge gaps, complex physical law deductions, or deep logical reasoning
  requirements, the system determines that the internal prior knowledge of the downstream image generation model is
  sufficient to cover the current task requirements, thereby routing the task directly to the image generation module.
* •
  
  Reasoning and Search (Ct=search_reason_generationC_{t}=\text{search\_reason\_generation}): When Pt(R)P^{(R)}_{t}
  involves time-sensitive constraints, abstract concepts, specific entities lacking visual references, implicit
  scientific common sense, physical state evolution, or features requiring logical puzzle-solving, the system triggers a
  deep parsing pipeline and dynamically routes the task to the downstream "Reasoning and Search Agent".

Mining of Implicit Constraints and Structured Questioning: When the routing strategy is determined as
Ct=search_reason_generationC_{t}=\text{search\_reason\_generation}, the Intent Analysis Agent deeply parses ambiguous
instructions, accurately mining the implicit constraints hidden behind natural language and translating them into an
explicit, structured question set 𝒬t={q1,q2,…,qk}\mathcal{Q}_{t}=\{q_{1},q_{2},\dots,q_{k}\} directed at downstream
modules. To ensure the completeness of question coverage, this module designs multi-dimensional constraint mining and
questioning strategies:
* •
  
  Defensive Questioning: For specific entities, proprietary concepts, or named events involved in the instructions, this
  module establishes a "fact-evidence first" defensive questioning mechanism. Except for unambiguous basic universal
  entities (e.g., common flora and fauna, daily objects), even if the downstream image diffusion model contains prior
  knowledge of the concept, this strategy strictly prohibits the model from relying on its internal prior knowledge for
  unconstrained heuristic generation. The system forces these non-universal or long-tail entities to be translated into
  explicit questions, specifically interrogating their exact objective visual features. This mechanism essentially acts
  as a "cognitive firewall" within the system, fundamentally mitigating the knowledge hallucination issues common in
  image generation models.
* •
  
  Process Decoupling and Terminal Visual Anchoring: When faced with instructions implying complex dynamic reasoning
  (e.g., physical laws, biochemical reactions, visual puzzles), this strategy mandates the agent to discard descriptions
  of intermediate procedural details. By introducing a "process decoupling" mechanism, the structured questions
  generated by the agent must bypass the convoluted intermediate deduction phases and directly target the final visual
  features. Specifically, this strategy strictly constrains the agent to focus solely on and specifically interrogate
  the terminal visual features—such as the final physical morphology and spatial layout of the target entities—that are
  directly renderable by the diffusion model.

Modality Fusion and Prompt Refinement: When a task is routed as direct generation
(Ct=direct_generationC_{t}=\text{direct\_generation}), the Intent Analysis Agent bypasses the structured questioning
phase (i.e., 𝒬t=∅\mathcal{Q}_{t}=\emptyset) and directly refines the initial instruction Pt(R)P^{(R)}_{t} to generate an
operational prompt Pt(A)P^{(A)}_{t} that is easily interpretable and renderable by the underlying models. Considering
the fundamental mechanistic differences in instruction-following between image generation and editing models, the agent
executes an adaptive prompt optimization strategy based on the prior conditions of the input modalities (i.e., the
presence or absence of reference images) and the specific task type, ensuring optimal semantic alignment between the
final output prompt Pt(A)P^{(A)}_{t} and the underlying diffusion models. Specifically, this strategy comprises the
following three execution paths:
* •
  
  High-density Semantic Expansion (for text-only generation tasks): For pure text-driven generation tasks without
  reference images, the agent performs high-density semantic expansion, automatically supplementing fine-grained visual
  descriptions such as environmental background, perspective composition, lighting, materials, and artistic styles to
  compensate for the information sparsity of the initial text.
* •
  
  Terminal Visual Anchoring (for generation tasks with reference images): For generation tasks accompanied by reference
  images, the agent strictly adheres to a "terminal-visual-state-oriented" principle. It actively filters out all
  procedural action descriptions (e.g., operational verbs or intermediate deduction logic) and outputs only absolute
  objective visual descriptions of the "final desired scene," thereby minimizing the risk of feature confusion in the
  underlying diffusion models.
* •
  
  Structured Action Decomposition (for image editing tasks): For image editing tasks, the agent translates ambiguous
  natural language into a strict, structured instruction formatted as "Editing Action + Target Object + Resulting
  State," guiding the underlying image editing model to execute precise spatial and semantic modifications.

### 3.3 Reasoning & Search Agent

The Reasoning and Search Agent assumes the core functions of complex logical reasoning and external knowledge
acquisition within the RS-Gen framework. This module is activated when the intent analysis agent outputs the routing
decision Ct=search_reason_generationC_{t}=\text{search\_reason\_generation}. Given the instruction Pt(R)P_{t}^{(R)}, the
input image set ℐin,t(R)\mathcal{I}^{(R)}_{in,t}, the structured question set 𝒬t\mathcal{Q}_{t}, and the tool library
𝒯\mathcal{T}, the agent adopts the ReAct (Reason-Act-Observe) paradigm to execute multiple rounds of external
information retrieval and logical reasoning. Ultimately, the agent outputs an information-complete, logically
self-consistent, and intent-precise image manipulation instruction Pt(S)P_{t}^{(S)}, alongside a high-quality visual
reference set ℐref,t(S)\mathcal{I}^{(S)}_{ref,t}:

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
(Pt(S),ℐref,t(S))=F(S)(Pt(R),ℐi                                                                                   │(3)
n,t(R),𝒬t,𝒯)\left(P_{t}^{(S)},\mathcal{I}^{(S)}_{ref,t}\right)=F^{(S)}\left(P_{t}^{(R)},\mathcal{I}^{(R)}_{in,t},\│   
mathcal{Q}_{t},\mathcal{T}\right)                                                                                 │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

Reasoning and Search Loop: For each sub-question q∈𝒬tq\in\mathcal{Q}_{t} within the structured question set
𝒬t\mathcal{Q}_{t}, this module draws inspiration from the ReAct paradigm in the agent domain to construct a rigorous
"Thought-Action-Observation" reasoning and search loop. The system deploys a diversified expert tool library 𝒯={τgeo,τre
ason,τvqa,τweb,τimg}\mathcal{T}=\{\tau_{geo},\tau_{reason},\tau_{vqa},\tau_{web},\tau_{img}\}, encompassing geographic
information querying, deep logical reasoning, visual understanding and recognition, web search engines, and image
retrieval engines. Under the ReAct paradigm, the agent breaks through traditional static preset pipelines. Relying on
the powerful cognitive capabilities of multimodal large models, it can adaptively invoke external tools to verify
hypotheses, supplement critical information, or conduct in-depth reasoning, thereby gradually eliminating information
uncertainty. This loop will continue to operate until all implicit logical and knowledge gaps are fully bridged.

Cross-modal Cascade Retrieval and Adaptive Fallback: To address the inherent uncertainties during external tool
invocation (e.g., API failures, noisy retrieval results, or information absence), this module designs a robust
cross-modal cascade retrieval and adaptive fallback mechanism to ensure system stability:
* •
  
  Text-then-Image Cascade Anchoring: When a target entity involves ambiguous references or broad semantic constraints
  (e.g., "a certain brand’s latest concept car"), the system strictly prohibits directly invoking the image search tool
  τimg\tau_{img}. The agent prioritizes invoking the web search tool τweb\tau_{web} to acquire relevant textual
  information. After precisely locking onto the entity’s objective identifier (e.g., a specific model or professional
  name), it then utilizes this exact expression as the query to execute the image retrieval. This cascade strategy of
  "text preceding image" effectively eliminates semantic ambiguity and fundamentally ensures the accuracy of the visual
  reference images.
* •
  
  Multi-round Reformulation and Visual Substitution: When target image retrieval fails or the returned image quality is
  substandard (e.g., blurry images, excessive watermarks, mismatch between text and image), the agent automatically
  triggers the fallback mechanism and sequentially executes the following actions based on priority: switching to
  alternative search tools, relaxing the semantic constraint boundaries of the search, or seeking surrogate entities
  with highly similar visual features. This mechanism maximizes the guarantee that, even in extreme scenarios of
  information absence, the final image generation remains grounded in reliable factual evidence and visual references.

### 3.4 Image Generation Agent

The Image Generation Agent is responsible for executing the image operational instructions passed from upstream modules
to synthesize the final high-quality images. Unlike the traditional single-step generation paradigm, this module
discards the static single-step generation pipeline. Instead, it encapsulates the underlying foundational models for
image generation and editing into independent expert tools, upon which it constructs an iterative self-verifying loop
based on the ReAct paradigm. Given the generation instruction Pt(G)P^{(G)}_{t} and the reference image set ℐre
f(G)\mathcal{I}^{(G)}_{ref}, the agent dynamically invokes the tool library to ultimately output a target image set ℐou
t\mathcal{I}_{out} that is highly aligned with the user’s intent. This process can be formally defined as:

─────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
ℐout=F(G)(Pt(G),ℐref(G),𝒯gen)\mathcal{I}_{out}=F^{(G)}(P^{(G)}_{t},\mathcal{I}^{(G)}_{ref},\mathcal{T}_{gen})│(4)
─────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

where 𝒯gen\mathcal{T}_{gen} denotes the expert toolset, encompassing tools for image generation, image editing, and
image verification; the instruction Pt(G)P^{(G)}_{t} dynamically maps to either Pt(A)P^{(A)}_{t} from direct generation
or Pt(S)P^{(S)}_{t} from the reasoning and search output, depending on the routing decision of the intent analysis;
similarly, the reference image set ℐref(G)\mathcal{I}^{(G)}_{ref} corresponds to either the initial input ℐi
n,t(R)\mathcal{I}^{(R)}_{in,t} or the high-quality retrieved reference set ℐref,t(S)\mathcal{I}^{(S)}_{ref,t}.

Dynamic Tool Invocation and Self-correcting Loop: The core innovation of this module lies in fundamentally breaking the
traditional open-loop generation paradigm by transforming image generation into a multi-round "Generate-Verify-Correct"
iterative process. In the kk-th iteration, the agent executes the following workflow:
* •
  
  Image Generation and Editing: The agent parses the generation strategy pkp_{k} for the current round (with the initial
  state p0=Pt(G)p_{0}=P^{(G)}_{t}), autonomously and dynamically schedules the required generation or editing models
  from the expert tool library 𝒯gen\mathcal{T}_{gen}, and synthesizes the candidate image IkI_{k} for the current round.
* •
  
  Multimodal Alignment Verification: Once generated, the candidate image is not output directly. Instead, a visual
  verification tool based on a Multimodal Large Language Model (MLLM) is invoked to conduct a rigorous review of the
  image quality and visual-semantic consistency of IkI_{k}. This step evaluates whether visual elements—such as entity
  attributes, spatial topological relations, and stylistic features in IkI_{k}—strictly align with the instruction
  Pt(G)P^{(G)}_{t}, while simultaneously checking for artifacts or structural flaws, ultimately generating a structured
  verification report vkv_{k}.
* •
  
  Strategy Adjustment and Image Correction: If the verification report vkv_{k} indicates semantic deviations or visual
  defects in the current candidate image (e.g., violations of common sense or objective laws), the agent triggers an
  internal reasoning mechanism. Based on the diagnostic feedback from vkv_{k}, the agent autonomously deduces targeted
  correction strategies Δpk\Delta p_{k} and updates the execution parameters for the next round as pk+1=Update(pk,Δ
  pk,Pt(G))p_{k+1}=\text{Update}(p_{k},\Delta p_{k},P^{(G)}_{t}).

The aforementioned "Generate-Verify-Correct" loop continuously and iteratively optimizes the candidate image until the
visual verification tool determines that the image achieves complete pixel-level and semantic-level alignment with the
user’s intent. At this point, the loop terminates and outputs the final result IoutI_{out}. This self-correcting
mechanism effectively compensates for the instruction-following and visual quality issues prone to occur in traditional
open-loop image generation models when processing complex instructions, significantly enhancing the reliability and
robustness of the system’s output.

## 4 Experiments

### 4.1 Benchmarks and Evaluation Protocols

To comprehensively evaluate the overall capabilities of the RS-Gen framework in complex instruction understanding,
knowledge retrieval augmentation, and logical reasoning, we conducted extensive experiments on two core visual tasks:
image generation and image editing. For these two tasks, we selected two highly challenging, cutting-edge evaluation
benchmarks for in-depth validation:

Table 1: Performance of different models on the WISE_Verified [[31][72]] benchmark. The table is categorized into four
parts: Commercial Models, Generation-Only Models, Unified Models, and our proposed RS-Gen. The best results within each
group are highlighted in bold. A

[Content truncated]
```
