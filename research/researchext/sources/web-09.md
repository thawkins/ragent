# Web source

- URL: https://arxiv.org/html/2504.21561v2
- Title: 1. [1 Introduction][1]
- Captured (UTC): 2026-06-30T09:39:11.611215343+00:00

```text
1. [1 Introduction][1]
2. [2 Related Work][2]
   1. [2.1 Agent Tuning][3]
   2. [2.2 Step-wise Preference Tuning][4]
   3. [2.3 Learning from AI Feedback][5]
3. [3 Method][6]
   1. [3.1 Formulation][7]
   2. [3.2 Task Synthesis][8]
   3. [3.3 Data construction][9]
   4. [3.4 Preference Tuning][10]
4. [4 Experiments][11]
   1. [4.1 Setting][12]
   2. [4.2 GTA Results][13]
   3. [4.3 GAIA Results][14]
   4. [4.4 Ablation][15]
      1. [4.4.1 Effectiveness of iteration step size d𝑑ditalic_d.][16]
      2. [4.4.2 Applying DPO to MAT Preference Data][17]
   5. [4.5 Statistic][18]
   6. [4.6 Data Quality][19]
   7. [4.7 Visualization][20]
5. [5 Conclusion][21]
6. [A Comparison with Existing Sampling Frameworks][22]
7. [B Prompt of the Step Verifier][23]
8. [C User Study Interface][24]
   1. [C.1 Preference Alignment Study][25]
   2. [C.2 Data quality][26]

# Iterative Tool Usage Exploration for Multimodal Agents
# via Step-wise Preference Tuning

Pengxiang Li Zhi Gao Bofei Zhang Yapeng Mi Xiaojian Ma Chenrui Shi Tao Yuan Yuwei Wu Yunde Jia Song-Chun Zhu Qing Li

###### Abstract

Multimodal agents, which integrate a controller (*e.g.*, a large language model) with external tools, have demonstrated
remarkable capabilities in tackling complex tasks. However, existing agents need to collect a large number of expert
data for fine-tuning to adapt to new environments. In this paper, we propose an online self-exploration method for
multimodal agents, namely SPORT, via step-wise preference optimization to refine the trajectories of agents, which
automatically generates tasks and learns from solving the generated tasks, without any expert annotation. SPORT operates
through four iterative components: task synthesis, step sampling, step verification, and preference tuning. First, we
synthesize multimodal tasks using language models. Then, we introduce a novel search scheme, where step sampling and
step verification are executed alternately to solve each generated task. We employ a verifier to provide AI feedback to
construct step-wise preference data. The data is subsequently used to update the controller’s policy through preference
tuning, producing a SPORT Agent. By interacting with real environments, the SPORT Agent evolves into a more refined and
capable system. Evaluation in the GTA and GAIA benchmarks shows that the SPORT Agent achieves 6.41% and 3.64%
improvements, underscoring the generalization and effectiveness introduced by our method. The project page is
[https://SPORT-Agents.github.io][27].

Machine Learning, ICML


## 1 Introduction

Leveraging large language models (LLMs) or vision-language models (VLMs) as controllers to call external tools has
become a promising direction in building multimodal agents (Surís et al., [2023][28]; Gupta & Kembhavi, [2023][29];
Zhong et al., [2023][30]; Wang et al., [2024a][31]), achieving impressive performance across multiple complex downstream
tasks, such as GUI control (Cheng et al., [2024][32]), multimodal reasoning (Gao et al., [2024a][33]), and embodied
AI (Fan et al., [2024][34]). To enhance the planning and reasoning abilities of agents in new environments, existing
studies focus on collecting training data to tune the controller of an agent, where commonly used techniques include
supervised fine-tuning (SFT) (Hu et al., [2024][35]; Gao et al., [2024b][36]) and reinforcement learning (RL) (Deng
et al., [2024c][37]; Xiong et al., [2024b][38]) for alignments with desired behaviors.

The key to agent tuning is collecting sufficient expert data (*e.g.*, tasks, labels, and trajectories to solve tasks).
SFT methods collect expert data via human annotation or distillation from closed-source API to fine-tune the
controllers. RL methods use the expert data to tune reward models, update policy models (Qi et al., [2024][39]; He
et al., [2024][40]), or derive performance data for optimization (Zhang et al., [2024][41]; Putta et al., [2024][42]).
However, collecting high-quality expert data may be difficult in new environments. It is labor-extensive and high-cost,
and such pre-collected data may lead to biased distributions inconsistent with the target environments, causing inferior
results.

[Refer to caption]

Figure 1: Pipeline of the proposed SPORT method, including four iterative components: task generation, step sampling,
step verification, and preference tuning.

In this paper, we focus on the multimodal reasoning tasks, where the agents are required to call diverse tools (*e.g.*,
web search, visual reasoning, file understanding, and object localization) to answer given questions. We explore whether
multimodal agents can improve their performance via online self-exploration without any expert data. We are inspired by
existing research in LLMs and VLMs, which has shown impressive performance in self-instruction (Wang et al., [2023][43];
Liu et al., [2023][44]), self-verification (Madaan et al., [2024][45]; Yu et al., [2024][46]), and self-learning (Deng
et al., [2024b][47]; Kumar et al., [2024][48]). Based on the above observation, we expect that the agent automatically
generates tasks, searches for possible trajectories to call tools in solving synthetic tasks, evaluates these
trajectories by itself, and updates controllers using this data. In this case, agents will improve their generalization
capability by interacting with environments.

To achieve this goal, we must address three key challenges in such an online self-exploration framework for multimodal
reasoning tasks. (1) Lack of off-the-shelf tasks and expert trajectories. There is no off-the-shelf dataset with ground
truth annotation for multimodal reasoning tasks. It is non-trivial to identify whether a task is correctly solved, thus
making it challenging to select correct trajectories. (2) Difficulty in reward modeling. Unlike other settings (*e.g.*,
GUI setting), where sufficient data or pre-defined rules make reward modeling feasible, predicted answers or
trajectories in multimodal reasoning cannot be easily and reliably verified. (3) Low sampling efficiency and high cost.
Sampling trajectories often involves expensive tools (*e.g.*, LLM APIs, web search), which results in both high monetary
and computational costs, making it challenging to scale up.

To solve the above challenges, we propose SPORT, an iterative self-exploration method via step-wise preference
optimization to refine trajectories of multimodal agents, as shown in [Figure 1][49]. SPORT operates through four
iterative components: task synthesis, step sampling, step verification, and preference tuning. First, we generate
queries and multimodal files for task synthesis based on provided task seeds. Second, we introduce a new search scheme
that samples step-level candidate solutions to call tools. Third, we employ a multimodal verifier that, given the task
context, intermediate reasoning states, and step candidates, provides AI-generated feedback to estimate step-level
preferences. Finally, we perform step-wise preference tuning to refine the controller’s policy and obtain the SPORT
Agent, which is then used to guide trajectory sampling in the next iteration.

SPORT enables agents to autonomously generate tasks and explore tool-use trajectories, removing reliance on expert data
or pre-collected datasets. For pre-trained LLMs, providing step-level preference signals is easier than accurate
rewards, circumventing the difficulties of reward modeling in complex multimodal tasks. Furthermore, SPORT improves the
utilization of sampled trajectories—even for some failed trajectories, we can still discover useful step-level
preference data, allowing us to acquire more effective data utilization with the same number of sampled trajectories.
These capabilities collectively support stable, scalable self-exploration for complex multimodal tasks.

We conduct experiments on the two multimodal reasoning benchmarks: GTA and GAIA, and results show that our SPORT Agent
outperforms the SFT Agent by 6.41% and 3.64%, respectively. This indicates that our SPORT Agent method leads to a more
powerful reasoning and planning capability for tool usage by interacting with the environment.

In summary, our contributions are three-fold.

(1) We propose an online self-exploration framework for multimodal agents, through which the agents can adapt to new
environments without any expert annotation.

(2) We propose SPORT, an online self-exploration method that utilizes step-wise optimization and AI feedback, providing
a possible way to self-learning in complex multimodal environments.

(3) The obtained SPORT Agent achieves significant performance improvements compared with SFT based agents on two popular
benchmarks: GTA and GAIA.

## 2 Related Work

### 2.1 Agent Tuning

Due to the disparity between the LLMs and the requirements of agents, agent tuning is necessary to adapt to practical
tasks in new environments. Research for agent tuning could be divided into two categories: supervised fine-tuning (SFT)
and reinforcement learning (RL). SFT methods collect expert trajectory data via distillation from closed-source API
(*e.g.*, GPT-4o) (Gao et al., [2024b][50]; Liu et al., [2024d][51]; Zeng et al., [2023][52]) or human annotation (Liu
et al., [2024c][53]; Deng et al., [2024a][54]). Then they use these collected data to tune the controller via SFT.
However, the SFT methods suffer from huge costs and inferior generalization (Shi et al., [2024][55]). To solve this
issue, researchers have paid attention to RL agent tuning methods that allow agents to interact with the environment and
learn from the feedback. Some methods utilize the policy gradient technique (Zhou et al., [2024a][56]) to update the
controller with a reward model that is designed as a fine-tuned model (Qi et al., [2024][57]; Zhai et al., [2024b][58]),
environment feedback (Bai et al., [2024][59]; Zhai et al., [2024a][60]), human-designed rules (He et al., [2024][61];
Zhou et al., [2024b][62]), or tree search results (Deng et al., [2024c][63]). To simplify this procedure, the direct
preference optimization (DPO) method is applied to agent tuning (Xiong et al., [2024b][64]), where
trajectory-level (Zhang et al., [2024][65]; Song et al., [2024][66]) or step-level preference data (Putta et al.,
[2024][67]; Chen et al., [2024b][68]) is constructed to update the controller. The above DPO methods construct step-wise
preference data based on whole correct trajectories. Nevertheless, such expert or correct trajectories are difficult to
obtain in complex multimodal tasks, and the predefined correct trajectories may lead to biased distributions. In
contrast, our online self-exploration framework does not rely on any correct trajectories. In addition, it uses an
online manner to avoid biased distributions.

### 2.2 Step-wise Preference Tuning

Preference tuning methods rely on paired data, which is not readily available for complex tasks with multi-step
reasoning, making it non-trivial to determine which trajectory is better. Furthermore, for long trajectories, only an
overall preference verification can not capture the relationships among steps and ignores the fine-grained preference
between different steps. To overcome this problem, step-wise preference has been studied. STEP-DPO (Lai et al.,
[2024][69]) and SCDPO (Lu et al., [2024][70]) collect step-wise preference data by localizing error steps or disturbing
the correct path. OREO (Chen et al., [2024a][71]) and SVPO (Wang et al., [2024b][72]) train value models for step-wize
verification and inference guidance. SDPO (Kong et al., [2025][73]) combines step-level, turn-level, and session-level
preference data for full-grained optimization. The above methods mainly focus on code generation and math reasoning
tasks that are easy to obtain correct trajectories to construct step-wise preference data. In contrast, this method
focuses on multimodal agents, where obtaining correct trajectories is challenging. Thus, our agent explores the
environment by itself via an online manner, which uses designed AI feedback to construct preference data without any
annotation.

### 2.3 Learning from AI Feedback

Using models to generate AI feedback (natural languages, scores, or preference) for performance improvement has emerged
as a critical paradigm (Bai et al., [2022][74]). Existing methods can be broadly divided into three categories based on
how to use the obtained AI feedback. Methods of the first category add the AI feedback into prompts for in-context
learning (Madaan et al., [2024][75]). LLaVA-Cirtic (Xiong et al., [2024a][76]) collects sufficient data to train a model
to provide rich multimodal AI feedback. VLM-F (Liao et al., [2024][77]), VolCaNo (Lee et al., [2024a][78]), and
Clarify (Lee et al., [2024b][79]) add AI feedback to VLM prompts to address visual hallucinations. CLOVA (Gao et al.,
[2024a][80]) and CompAgent (Wang et al., [2024f][81]) are representative agents that use AI feedback to refine prompts
for better task-solving. Methods of the second category use AI feedback to filter data for supervised fine-tuning.
M-STAR (Liu et al., [2024b][82]) shows its effectiveness in visual mathematical reasoning. APIGen (Liu et al.,
[2024d][83]), MAT (Gao et al., [2024b][84]), and visualagentbench (Liu et al., [2024c][85]) design verifiers to collect
trajectories for agents. Methods of the third category use AI feedback for reinforcement learning. They use LLMs or VLMs
to produce rewards for policy gradient optimization ([Lee et al., ][86]; [Fu et al., ][87]; Rocamonde et al.,
[2023][88]), and several of them produce preference data for the DPO algorithm (Yu et al., [2024][89]). Different from
them, our AI feedback is well-designed for evaluating each step in complex scenarios of multimodal agents.

[Refer to caption]

Figure 2: Demonstrations of the search scheme used in the SPORT method. Given a task, the agent samples potential
solutions for each step and verifies their qualities in an online manner. Then, we construct the step-wise preference
data based on such self-exploration.

## 3 Method

### 3.1 Formulation

We opt for the framework of the ReAct agent (Yao et al., [2023][90]) that performs step-by-step reasoning for tool usage
for task solving. In each step, based on the input xisubscript𝑥𝑖x_{i}italic_x start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT, the agent outputs a solution sisubscript𝑠𝑖s_{i}italic_s start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT for tool calling.

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
si⋆=arg⁡max⁡πθ⁢(si|xi,T),superscriptsubscript𝑠𝑖⋆subscript𝜋𝜃conditionalsubscript𝑠𝑖subscript𝑥𝑖𝑇\displaystyle           │(1)
s_{i}^{\star}=\arg\max\pi_{\theta}(s_{i}|x_{i},T),italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT         │   
start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT = roman_arg roman_max italic_π start_POSTSUBSCRIPT italic_θ           │   
end_POSTSUBSCRIPT ( italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT | italic_x start_POSTSUBSCRIPT        │   
italic_i end_POSTSUBSCRIPT , italic_T ) ,                                                                         │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

where πθsubscript𝜋𝜃\pi_{\theta}italic_π start_POSTSUBSCRIPT italic_θ end_POSTSUBSCRIPT is the controller (an VLM in our
method) of agents with θ𝜃\thetaitalic_θ being the parameters, xisubscript𝑥𝑖x_{i}italic_x start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT is composed of the task (including a query Q𝑄Qitalic_Q in natural language and multimodal files
F𝐹Fitalic_F) and the history hisubscriptℎ𝑖h_{i}italic_h start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT of previous
steps, *i.e.*, xi={Q,F,hi}subscript𝑥𝑖𝑄𝐹subscriptℎ𝑖x_{i}=\{Q,F,h_{i}\}italic_x start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT = { italic_Q , italic_F , italic_h start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT }. The solution
sisubscript𝑠𝑖s_{i}italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT consists of the thought
tisubscript𝑡𝑖t_{i}italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT and code cisubscript𝑐𝑖c_{i}italic_c
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT,
si=⁢{ti,ci}superscriptsubscript𝑠𝑖subscript𝑡𝑖subscript𝑐𝑖s_{i}^{=}\{t_{i},c_{i}\}italic_s start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT = end_POSTSUPERSCRIPT { italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
, italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT }. T𝑇Titalic_T denotes available tools, and we follow the
work (Gao et al., [2024b][91]) using the same toolkit. In this case, we further rewrite Eq. ([1][92]) as

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
ti⋆,ci⋆=arg⁡max⁡πθ⁢(ti,ci|Q,F,hi,T),superscriptsubscript𝑡𝑖⋆superscriptsubscript𝑐𝑖⋆subscript𝜋𝜃subscript𝑡𝑖conditionalsu│(2)
bscript𝑐𝑖𝑄𝐹subscriptℎ𝑖𝑇\displaystyle t_{i}^{\star},c_{i}^{\star}=\arg\max\pi_{\theta}(t_{i},c_{i}|Q,F%            │   
,h_{i},T),italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT ,   │   
italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT = roman_arg   │   
roman_max italic_π start_POSTSUBSCRIPT italic_θ end_POSTSUBSCRIPT ( italic_t start_POSTSUBSCRIPT italic_i         │   
end_POSTSUBSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT | italic_Q , italic_F , italic_h      │   
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT , italic_T ) ,                                                     │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

where ti⋆superscriptsubscript𝑡𝑖⋆t_{i}^{\star}italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT and ci⋆superscriptsubscript𝑐𝑖⋆c_{i}^{\star}italic_c start_POSTSUBSCRIPT
italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT are thought and code for the i𝑖iitalic_i-th step,
and the history
hi={t1,c1,o1,⋯,ti−1,ci−1,oi−1}subscriptℎ𝑖subscript𝑡1subscript𝑐1subscript𝑜1⋯subscript𝑡𝑖1subscript𝑐𝑖1subscript𝑜𝑖1h_{i}=\{t
_{1},c_{1},o_{1},\cdots,t_{i-1},c_{i-1},o_{i-1}\}italic_h start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT = { italic_t
start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT , italic_c start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT , italic_o
start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT , ⋯ , italic_t start_POSTSUBSCRIPT italic_i - 1 end_POSTSUBSCRIPT , italic_c
start_POSTSUBSCRIPT italic_i - 1 end_POSTSUBSCRIPT , italic_o start_POSTSUBSCRIPT italic_i - 1 end_POSTSUBSCRIPT } is
composed of thought, code, and observation of previous steps.

Agent tuning aims to update θ𝜃\thetaitalic_θ to increase the planning and reasoning capabilities of agents in new
environments. This paper proposes an online self-exploration method, SPORT, to update θ𝜃\thetaitalic_θ via step-wise
preference optimization in refining trajectories, as shown in [Figure 1][93]. Concretely, SPORT has iterative
components: task synthesis, step sampling, step verification, and preference tuning. In one iteration, SPORT first
generates some multimodal tasks. For each generated task, SPORT performs step sampling and step verification alternately
to construct step-wise preference data. Finally, SPORT uses the step-wise preference data to tune the controller.

### 3.2 Task Synthesis

Since a multimodal task is composed of a language query and multimodal files, the task synthesis component is divided
into query generation and multimodal file generation. We first generate queries and then generate files, rather than the
reverse order, since the multimodal files are diverse (such as DOCX, PPTX, XLSX, and PDF are commonly encountered), and
it is challenging to construct a diverse file dataset in advance. In addition, tasks are usually based on multiple files
instead of only one. First obtaining files and then generating queries may cause weak relevance of files and produce
non-natural queries.

To produce diverse and practical queries, we collect seed from the existing method MAT (Gao et al., [2024b][94]) and
employ an LLM (GPT-4o-mini in practice) to generate queries. We feed randomly sampled seed queries, used tools, and a
designed prompt to the LLM that generates multiple queries once. Adding tool descriptions to the prompt makes GPT-4o
mini better understand what queries can be solved, improving their qualities. For each generated query, we prompt GPT-4o
mini to output the needed files. If images are needed, we search for source images from off-the-shelf datasets based on
similarities. For other files, we prompt GPT-4o mini to generate Python code to produce files.

### 3.3 Data construction

The two components: step sampling and step verification are performed to construct high-quality preference data, playing
key roles in our method. To avoid potential bias issues in constructed data, we introduce an online search scheme, where
the step sampling and step verification are executed alternately in each generated task, as shown in [Figure 2][95].

The step-wise preference data is formulated as a triplet
(xi,sip⁢r⁢e,sid⁢i⁢s)subscript𝑥𝑖superscriptsubscript𝑠𝑖𝑝𝑟𝑒superscriptsubscript𝑠𝑖𝑑𝑖𝑠(x_{i},s_{i}^{pre},s_{i}^{dis})( italic_x
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT , italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT italic_p italic_r italic_e end_POSTSUPERSCRIPT , italic_s start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_d italic_i italic_s end_POSTSUPERSCRIPT ), where
xi={Q,F,hi}subscript𝑥𝑖𝑄𝐹subscriptℎ𝑖x_{i}=\{Q,F,h_{i}\}italic_x start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT = {
italic_Q , italic_F , italic_h start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT } denotes the input including the query
Q𝑄Qitalic_Q, files F𝐹Fitalic_F, and the history hisubscriptℎ𝑖h_{i}italic_h start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT of previous steps, sip⁢r⁢esuperscriptsubscript𝑠𝑖𝑝𝑟𝑒s_{i}^{pre}italic_s start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_p italic_r italic_e end_POSTSUPERSCRIPT is the preferred solution in the
current step including, and sid⁢i⁢ssuperscriptsubscript𝑠𝑖𝑑𝑖𝑠s_{i}^{dis}italic_s start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_d italic_i italic_s end_POSTSUPERSCRIPT is the dispreferred solution.

Given a task with the query Q𝑄Qitalic_Q and files F𝐹Fitalic_F, the agent expands the search space for the first step by
sampling n𝑛nitalic_n solutions
{s11,s12,⋯,s1n}superscriptsubscript𝑠11superscriptsubscript𝑠12⋯superscriptsubscript𝑠1𝑛\{s_{1}^{1},s_{1}^{2},\cdots,s_{1}^
{n}\}{ italic_s start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , italic_s
start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 2 end_POSTSUPERSCRIPT , ⋯ , italic_s start_POSTSUBSCRIPT 1
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT }, including the thought and code
{t11,c11,⋯,t1n,c1n}superscriptsubscript𝑡11superscriptsubscript𝑐11⋯superscriptsubscript𝑡1𝑛superscriptsubscript𝑐1𝑛\{t_{1}^
{1},c_{1}^{1},\cdots,t_{1}^{n},c_{1}^{n}\}{ italic_t start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1
end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , ⋯ ,
italic_t start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT , italic_c
start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT } from the controller, and
execute them to obtain n𝑛nitalic_n observations
{o11,⋯,o1n}superscriptsubscript𝑜11⋯superscriptsubscript𝑜1𝑛\{o_{1}^{1},\cdots,o_{1}^{n}\}{ italic_o start_POSTSUBSCRIPT 1
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , ⋯ , italic_o start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT }. Then, we feed the query Q𝑄Qitalic_Q, n𝑛nitalic_n solutions, and
n𝑛nitalic_n observations to an LLM, and ask the LLM to select the best solution
{t1∗,c1∗}superscriptsubscript𝑡1superscriptsubscript𝑐1\{t_{1}^{*},c_{1}^{*}\}{ italic_t start_POSTSUBSCRIPT 1
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT } with its corresponding observation
o1∗superscriptsubscript𝑜1o_{1}^{*}italic_o start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗
end_POSTSUPERSCRIPT.

Along {t1∗,c1∗}superscriptsubscript𝑡1superscriptsubscript𝑐1\{t_{1}^{*},c_{1}^{*}\}{ italic_t start_POSTSUBSCRIPT 1
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT } and o1∗superscriptsubscript𝑜1o_{1}^{*}italic_o start_POSTSUBSCRIPT 1
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT, we expand the search space for the second step. Regarding
{t1∗,c1∗,o1∗}superscriptsubscript𝑡1superscriptsubscript𝑐1superscriptsubscript𝑜1\{t_{1}^{*},c_{1}^{*},o_{1}^{*}\}{
italic_t start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT , italic_c
start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT , italic_o start_POSTSUBSCRIPT 1
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT } as the history h2subscriptℎ2h_{2}italic_h
start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT, the controller samples n𝑛nitalic_n solutions
{t21,c21,⋯,t2n,c2n}superscriptsubscript𝑡21superscriptsubscript𝑐21⋯superscriptsubscript𝑡2𝑛superscriptsubscript𝑐2𝑛\{t_{2}^
{1},c_{2}^{1},\cdots,t_{2}^{n},c_{2}^{n}\}{ italic_t start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1
end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , ⋯ ,
italic_t start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT , italic_c
start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT } from the controller, and
executes them to obtain n𝑛nitalic_n observations
{o21,⋯,o2n}superscriptsubscript𝑜21⋯superscriptsubscript𝑜2𝑛\{o_{2}^{1},\cdots,o_{2}^{n}\}{ italic_o start_POSTSUBSCRIPT 2
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , ⋯ , italic_o start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT }. We feed the query Q𝑄Qitalic_Q, history h2subscriptℎ2h_{2}italic_h
start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT, n𝑛nitalic_n solutions in this step, and n𝑛nitalic_n corresponding observations
to an LLM, and ask the LLM to select the best solution
{t2∗,c2∗}superscriptsubscript𝑡2superscriptsubscript𝑐2\{t_{2}^{*},c_{2}^{*}\}{ italic_t start_POSTSUBSCRIPT 2
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT } with its corresponding observation
o2∗superscriptsubscript𝑜2o_{2}^{*}italic_o start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗
end_POSTSUPERSCRIPT. In this case, the agent gradually expands the search space and selects the best solution for each
step, until the agent believes that the task is over.

Assume there are m𝑚mitalic_m steps in solving one task. In this case, we could collect m⁢(n−1)𝑚𝑛1m(n-1)italic_m (
italic_n - 1 ) preference data pairs. In the i𝑖iitalic_i-th step, the selected best solution
{ti∗,ci∗}superscriptsubscript𝑡𝑖superscriptsubscript𝑐𝑖\{t_{i}^{*},c_{i}^{*}\}{ italic_t start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT } is the preferred output, and the rest n−1𝑛1n-1italic_n - 1 solutions are
the dispreferred outputs, denoted as 𝒟id⁢i⁢ssuperscriptsubscript𝒟𝑖𝑑𝑖𝑠\mathcal{D}_{i}^{dis}caligraphic_D
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_d italic_i italic_s end_POSTSUPERSCRIPT,
|𝒟id⁢i⁢s|=n−1superscriptsubscript𝒟𝑖𝑑𝑖𝑠𝑛1|\mathcal{D}_{i}^{dis}|=n-1| caligraphic_D start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_d italic_i italic_s end_POSTSUPERSCRIPT | = italic_n - 1. The preference
data in one task is denoted as
{(x,{ti∗,ci∗},{tij,cij})}𝑥superscriptsubscript𝑡𝑖superscriptsubscript𝑐𝑖superscriptsubscript𝑡𝑖𝑗superscriptsubscript𝑐𝑖𝑗\{(x
,\{t_{i}^{*},c_{i}^{*}\},\{t_{i}^{j},c_{i}^{j}\})\}{ ( italic_x , { italic_t start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT } , { italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT italic_j end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT italic_j end_POSTSUPERSCRIPT } ) }, where
{tij,cij}∈𝒟id⁢i⁢ssuperscriptsubscript𝑡𝑖𝑗superscriptsubscript𝑐𝑖𝑗superscriptsubscript𝒟𝑖𝑑𝑖𝑠\{t_{i}^{j},c_{i}^{j}\}\in\mathcal
{D}_{i}^{dis}{ italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_j
end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_j
end_POSTSUPERSCRIPT } ∈ caligraphic_D start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_d
italic_i italic_s end_POSTSUPERSCRIPT and i∈[1,m]𝑖1𝑚i\in[1,m]italic_i ∈ [ 1 , italic_m ].

### 3.4 Preference Tuning

Objective. In one iteration, we may generate multiple tasks and construct preference data for them. After that, we
denote the obtained preference data set as
𝒟={(xi,sip⁢r⁢e,sid⁢i⁢s)}𝒟subscript𝑥𝑖superscriptsubscript𝑠𝑖𝑝𝑟𝑒superscriptsubscript𝑠𝑖𝑑𝑖𝑠\mathcal{D}=\{(x_{i},s_{i}^{pre},s_{i}
^{dis})\}caligraphic_D = { ( italic_x start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT , italic_s start_POSTSUBSCRIPT
italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_p italic_r italic_e end_POSTSUPERSCRIPT , italic_s
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_d italic_i italic_s end_POSTSUPERSCRIPT ) },
and the number of data is |𝒟|=d𝒟𝑑|\mathcal{D}|=d| caligraphic_D | = italic_d. We choose the flexible DPO algorithm,

──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
ℒ(θ)=−𝔼(xi,sip⁢r⁢e,sid⁢i⁢s))∼𝒟[logσ(βlogπθ⁢(sip⁢r⁢e|xi)πr⁢e⁢f⁢(sip⁢r⁢e|xi)−βlogπθ⁢(sid⁢i⁢s|xi)πr⁢e⁢f⁢(sid⁢i⁢s|xi))],\displaystyle\begi│(3)
n{aligned} \mathcal{L}(\theta)=-\mathbb{E}_{(x_{i},s_{i}^{%                                                       │   
pre},s_{i}^{dis}))\sim\mathcal{D}}[\log\sigma(\beta\log\frac{\pi_{\theta}(s_{i%                                   │   
}^{pre}|x_{i})}{\pi_{ref}(s_{i}^{pre}|x_{i})}\\                                                                   │   
-\beta\log\frac{\pi_{\theta}(s_{i}^{dis}|x_{i})}{\pi_{ref}(s_{i}^{dis}|x_{i})}% )],\end{aligned}start_ROW         │   
start_CELL caligraphic_L ( italic_θ ) = - blackboard_E start_POSTSUBSCRIPT ( italic_x start_POSTSUBSCRIPT italic_i│   
end_POSTSUBSCRIPT , italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_p        │   
italic_r italic_e end_POSTSUPERSCRIPT , italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT                   │   
start_POSTSUPERSCRIPT italic_d italic_i italic_s end_POSTSUPERSCRIPT ) ) ∼ caligraphic_D end_POSTSUBSCRIPT [      │   
roman_log italic_σ ( italic_β roman_log divide start_ARG italic_π start_POSTSUBSCRIPT italic_θ end_POSTSUBSCRIPT (│   
italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_p italic_r italic_e          │   
end_POSTSUPERSCRIPT | italic_x start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT ) end_ARG start_ARG italic_π        │   
start_POSTSUBSCRIPT italic_r italic_e italic_f end_POSTSUBSCRIPT ( italic_s start_POSTSUBSCRIPT italic_i          │   
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_p italic_r italic_e end_POSTSUPERSCRIPT | italic_x                 │   
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT ) end_ARG end_CELL end_ROW start_ROW start_CELL - italic_β         │   
roman_log divide start_ARG italic_π start_POSTSUBSCRIPT italic_θ end_POSTSUBSCRIPT ( italic_s start_POSTSUBSCRIPT │   
italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_d italic_i italic_s end_POSTSUPERSCRIPT | italic_x        │   
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT ) end_ARG start_ARG italic_π start_POSTSUBSCRIPT italic_r italic_e │   
italic_f end_POSTSUBSCRIPT ( italic_s start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT        │   
italic_d italic_i italic_s end_POSTSUPERSCRIPT | italic_x start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT ) end_ARG│   
) ] , end_CELL end_ROW                                                                                            │   
──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───

where πθsubscript𝜋𝜃\pi_{\theta}italic_π start_POSTSUBSCRIPT italic_θ end_POSTSUBSCRIPT is the controller to be updated,
πr⁢e⁢fsubscript𝜋𝑟𝑒𝑓\pi_{ref}italic_π start_POSTSUBSCRIPT italic_r italic_e italic_f end_POSTSUBSCRIPT is the controller
for reference (the model after SFT in practice), β𝛽\betaitalic_β is the weighting parameter that controls the deviation
from the reference controller, and σ⁢(⋅)𝜎⋅\sigma(\cdot)italic_σ ( ⋅ ) is the logistic function.

Efficiency. One issue is the potential low efficiency in such an online self-exploration framework. A key hyperparameter
is d𝑑ditalic_d, that is, how much preference data is collected in one iteration. If we set a large d𝑑ditalic_d, it means
that we collect a large number of preference data for agent tuning in one iteration. This may cause the problem of data
bias, as the data required by the controller may change during training, while our data is generated using a fixed
controller in the beginning. If we set a small d𝑑ditalic_d, it means that we collect a small number of preference data
for agent tuning in one iteration. This may cause low efficiency, since we need to load and release the model weights
(controller, parameter gradients, tools, and verifier) frequently. To balance it, we set d𝑑ditalic_d as 500 in practice
via empirical exploration.

Training Scheme. The online self-exploration is performed after an SFT stage for controllers, since the effectinveness
of the self-exploration framework requires the controller to have the ability to generate accurate solutions. The SFT
stage is the same as MAT (Gao et al., [2024b][96]), where 20K trajectories are used to align the agent controller
(Qwen2VL-7B in practice) with desirable outputs. In the self-exploration stage, we use preference tuning to update
Qwen2-VL using 16K step-wise preference data in all, where d𝑑ditalic_d is 500 and there are 4 iterations in total. The
preference tuning process is summarized in [Algorithm 1][97].

Algorithm 1 Training process in SPORT.
0:  Seeds of multimodal tasks, initial agent controller πθsubscript𝜋𝜃\pi_{\theta}italic_π start_POSTSUBSCRIPT italic_θ
end_POSTSUBSCRIPT, and πr⁢e⁢f=πθsubscript𝜋𝑟𝑒𝑓subscript𝜋𝜃\pi_{ref}=\pi_{\theta}italic_π start_POSTSUBSCRIPT italic_r
italic_e italic_f end_POSTSUBSCRIPT = italic_π start_POSTSUBSCRIPT italic_θ end_POSTSUBSCRIPT. Preference data
𝒟=∅𝒟\mathcal{D}=\emptysetcaligraphic_D = ∅.
0:  Updated agent controller πθ∗subscript𝜋superscript𝜃\pi_{\theta^{*}}italic_π start_POSTSUBSCRIPT italic_θ
start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT end_POSTSUBSCRIPT
1:  while Not converged do
2:     Set 𝒟=∅𝒟\mathcal{D}=\emptysetcaligraphic_D = ∅.
3:     Randomly sample task seeds, and send them to an LLM to generate tasks.
4:     for Each generated task do
5:        for the i𝑖iitalic_i-step in solving the task do
6:           Sample n𝑛nitalic_n solutions
{ti1,ci1,⋯,tin,cin}superscriptsubscript𝑡𝑖1superscriptsubscript𝑐𝑖1⋯superscriptsubscript𝑡𝑖𝑛superscriptsubscript𝑐𝑖𝑛\{t_{i}^
{1},c_{i}^{1},\cdots,t_{i}^{n},c_{i}^{n}\}{ italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , ⋯ , italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT } based on the history hisubscriptℎ𝑖h_{i}italic_h start_POSTSUBSCRIPT
italic_i end_POSTSUBSCRIPT, and execute them to obtain results
{oi1,⋯,oin}superscriptsubscript𝑜𝑖1⋯superscriptsubscript𝑜𝑖𝑛\{o_{i}^{1},\cdots,o_{i}^{n}\}{ italic_o start_POSTSUBSCRIPT
italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT 1 end_POSTSUPERSCRIPT , ⋯ , italic_o start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT italic_n end_POSTSUPERSCRIPT }.
7:           Select the best solution {ti⋆,ci⋆\{t_{i}^{\star},c_{i}^{\star}{ italic_t start_POSTSUBSCRIPT italic_i
end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT , italic_c start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT
start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT}.
8:           Construct n−1𝑛1n-1italic_n - 1 preference pairs, and add them into 𝒟𝒟\mathcal{D}caligraphic_D.
9:           Add
ti⋆,ci⋆,oi⋆superscriptsubscript𝑡𝑖⋆superscriptsubscript𝑐𝑖⋆superscriptsubscript𝑜𝑖⋆t_{i}^{\star},c_{i}^{\star},o_{i}^{\star
}italic_t start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT , italic_c
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT , italic_o
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT start_POSTSUPERSCRIPT ⋆ end_POSTSUPERSCRIPT into
hisubscriptℎ𝑖h_{i}italic_h start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT.
10:        end for
11:     end for
12:     Use 𝒟𝒟\mathcal{D}caligraphic_D to update πθsubscript𝜋𝜃\pi_{\theta}italic_π start_POSTSUBSCRIPT italic_θ
end_POSTSUBSCRIPT via the preference tuning algorithm in Eq. ([3][98]).
13:  end while

## 4 Experiments

### 4.1 Setting

The performance of the proposed SPORT approach is assessed on the GTA (Wang et al., [2024c][99]) and GAIA (Mialon
et al., [2023][100]) benchmarks. Results are compared against agents powered by both closed-source models (*e.g.*,
GPT-4, GPT-4o, Claude3) and open-source models, including LLaMA-3-70B-instruct (Dubey et al., [2024][101]),
Qwen1.5-72B-chat (Bai et al., [2023][102]), LLaVA-NeXT-8B (Liu et al., [2024a][103]), InternVL2-8B (Chen et al.,
[2024c][104]), Qwen2-VL-7B (Wang et al., [2024d][105]), and MiniCPM-V-8.5B (Yao et al., [2024][106]). Specifically, we
perform direct comparisons with leading agents, such as Lego Agent (AgentLego Contributors, [2023][107]), Sibyl
Agent (Wang et al., [2024e][108]), and Warm-up Act Agent (Mialon et al., [2023][109]). As a baseline, we use the
Huggingface Agent (HF Agent) (HuggingFace Contributors, [2024][110]), which operates with the same toolset as the SPORT
Agent . We first evaluate these agents on two benchmarks, then assess the quality of produced preference data, and
finally show several visualization examples to demonstrate the effectiveness of our method.

We employ the Qwen-2-VL model as the controller. In the training process of our VLM controller, we freeze the vision
encoder and visual token compressor, and fine-tune the language model using LoRA (Hu et al., [2022][111]). We set the
rank as 32323232 and apply LoRA on query, key, and value projection matrices in all self-attention layers. We use the
AdamW optimizer with a cosine annealing scheduler. The learning rate is 1.0⁢e−61.0𝑒61.0e-61.0 italic_e - 6 and the batch
size is 2222 per device. We set the max context window as 10240102401024010240 to support complex trajectories of our
agent.

Benchmark. The GTA and GAIA benchmarks serve as robust evaluation frameworks for assessing multimodal agents. The GTA
benchmark includes 229 tasks paired with 252 images, where task completion requires 2 to 8 steps, with most tasks
involving 2 to 4 steps. This benchmark challenges multimodal agents to exhibit advanced perception, operational skills,
logical reasoning, and creative thinking based on visual data. In real-world multimodal scenarios, agents often need to
handle diverse file formats such as PPTX, PDF, and XLSX. To evaluate agent performance on such files, the GAIA benchmark
is used, comprising 446 tasks across 109 files. GAIA’s tasks are organized into three levels, with task complexity
varying from 2 steps to sequences of indefinite length. It evaluates document comprehension, web navigation, logical
reasoning, and summarization abilities.

Metric. Following existing methods (Wang et al., [2024c][112]; Gao et al., [2024b][113]), we assess agent performance
using three key metrics: *AnsAcc*, *ToolAcc*, and *CodeExec* for the GTA benchmark. *AnsAcc* gauges the accuracy of
predicted answers. *ToolAcc* evaluates the correctness of tool usage and the quality of answer summaries. *CodeExec*
measures the percentage of generated code that executes without errors. In the GAIA benchmark, we focus on measuring
*AnsAcc* at its three levels.

### 4.2 GTA Results

The results on the GTA benchmark are shown in [Table 1][114], where key metrics including *AnsAcc*, *ToolAcc*, and
*CodeExec* are reported. Our agent surpasses the Lego agent that utilizes closed-source models (*e.g.*, GPT-4 and
GPT-4o), as well as the HF agent that uses closed-source models and open-source models (*e.g.*, InternVL2-8B),
showcasing the ability of our SPORT Agent to tackle complex tasks with greater efficiency. A comparison between agents
through SFT (*i.e.*, T3-Agent) and our SPORT Agent demonstrates the effectiveness of our online self-exploration
framework and the advantages of our Step-wise optimization approach. Our method has about 7%percent77\%7 % improvements
on the final accuracy, since it calls more suitable tools (8%percent88\%8 % improvements) and reduces code error
(7%percent77\%7 % improvements). Compared with the HF agent using GPT-4o and GPT-4o mini, our agent achieves higher
*ToolAcc* and comparable *CodeExec*. This indicates that the proposed SPORT method improves the planning and reasoning
capabilities of agents again.

Table 1: Results on the GTA benchmark

───────────┬───────────────────┬────────┬─────────┬──────────
Method     │Controller         │*AnsAcc*│*ToolAcc*│*CodeExec*
───────────┼───────────────────┼────────┼─────────┼──────────
Lego Agent │GPT-4              │46.59   │-        │-         
───────────┼───────────────────┼────────┼─────────┼──────────
Lego Agent │GPT-4o             │41.52   │-        │-         
───────────┼───────────────────┼────────┼─────────┼──────────
Lego Agent │GPT-3.5-turbo      │23.62   │-        │-         
───────────┼───────────────────┼────────┼─────────┼──────────
Lego Agent │Claude3-opus       │23.44   │-        │-         
───────────┼───────────────────┼────────┼─────────┼──────────
Lego Agent │Qwen1.5-72B-chat   │13.32   │-        │-         
───────────┼───────────────────┼────────┼─────────┼──────────
Lego Agent │LLaMA3-70B-instruct│8.32    │-        │-         
───────────┼───────────────────┼────────┼─────────┼──────────
HF Agent   │GPT-4o             │57.05   │63.41    │95.12     
───────────┼───────────────────┼────────┼─────────┼──────────
HF Agent   │GPT-4o mini        │57.69   │56.10    │100.00    
───────────┼───────────────────┼────────┼─────────┼──────────
HF Agent   │LLaVA-NeXT-8B      │14.10   │14.97    │25.08     
───────────┼───────────────────┼────────┼─────────┼──────────
HF Agent   │InternVL2-8B       │32.05   │36.75    │52.18     
───────────┼───────────────────┼────────┼─────────┼──────────
HF Agent   │MiniCPM-V-8.5B     │33.97   │36.59    │56.10     
───────────┼───────────────────┼────────┼─────────┼──────────
HF Agent   │Qwen2-VL-7B        │42.31   │44.85    │65.19     
───────────┼───────────────────┼────────┼─────────┼──────────
T3-Agent   │MAT-MiniCPM-V-8.5B │52.56   │65.85    │80.49     
───────────┼───────────────────┼────────┼─────────┼──────────
T3-Agent   │MAT-Qwen2-VL-7B    │53.85   │64.63    │84.32     
───────────┼───────────────────┼────────┼─────────┼──────────
SPORT Agent│Tuned Qwen2-VL-7B  │60.26   │72.41    │91.87     
───────────┴───────────────────┴────────┴─────────┴──────────

### 4.3 GAIA Results

In [Table 2][115], we report the performance of SPORT Agent on the GAIA validation set. SPORT Agent achieves best
results among agents that use open-source models, surpassing the best-performing open-source model, Qwen2-VL-7B, about
11%percent1111\%11 % on *AnsAcc*. The consistent improvements across different levels underscore the efficacy of our
online self-exploration framework. Further

[Content truncated]
```
