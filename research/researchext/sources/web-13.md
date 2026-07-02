# Web source

- URL: https://arxiv.org/html/2503.13275v2
- Title: 1. [1 Introduction][1]
- Captured (UTC): 2026-06-30T09:39:17.995431820+00:00

```text
1. [1 Introduction][1]
2. [2 Related Work][2]
   1. [2.1 Retrieval-augmented generation (RAG)][3]
   2. [2.2 Multi-Step Question Answering][4]
   3. [2.3 Multi-Step Retrieval Optimization][5]
3. [3 Proposed Approach][6]
   1. [3.1 Core Components][7]
   2. [3.2 Optimization Strategy][8]
   3. [3.3 Multi-Agent Extension][9]
4. [4 Experiments][10]
   1. [4.1 Experimental Settings][11]
      1. [4.1.1 Tasks and Datasets][12]
         1. [Multi-hop QA Datasets.][13]
         2. [Single-hop QA Datasets:][14]
      2. [4.1.2 Retrieval Configuration][15]
      3. [4.1.3 Agent Configurations and Baselines][16]
         1. [System Configurations.][17]
         2. [Language Models for Agents.][18]
         3. [QA Module.][19]
      4. [4.1.4 Evaluation Metrics][20]
      5. [4.1.5 Implementation.][21]
   2. [4.2 Results][22]
      1. [4.2.1 Effectiveness of the Agent System][23]
      2. [4.2.2 Multi-Agent Extension][24]
         1. [Agent Scaling.][25]
         2. [Effect of Agent LLM Configuration.][26]
         3. [Effect of the Generator LLM.][27]
         4. [Overall Findings and Takeaways.][28]
      3. [4.2.3 Comparison with Single-Step Baseline][29]
      4. [4.2.4 Comparison with Iterative Retrieval Methods][30]
      5. [4.2.5 Efficiency Analysis][31]
         1. [Observations on Efficiency:][32]
         2. [Cost-Benefit Analysis][33]
      6. [4.2.6 Qualitative Analysis: Query Planning Examples][34]
5. [5 Conclusions][35]
6. [6 Limitations][36]

# Knowledge-Aware Iterative Retrieval for Multi-Agent Systems

Seyoung Song
{ssong}@gatech.edu

###### Abstract

We introduce a novel large language model (LLM)-driven agent framework, which iteratively refines queries and filters
contextual evidence by leveraging dynamically evolving knowledge. A defining feature of the system is its decoupling of
external sources from an internal knowledge cache that is progressively updated to guide both query generation and
evidence selection. This design mitigates bias-reinforcement loops and enables dynamic, trackable search exploration
paths, thereby optimizing the trade-off between exploring diverse information and maintaining accuracy through
autonomous agent decision-making. Our approach is evaluated on a broad range of open-domain question answering
benchmarks, including multi-step tasks that mirror real-world scenarios where integrating information from multiple
sources is critical, especially given the vulnerabilities of LLMs that lack explicit reasoning or planning capabilities.
The results show that the proposed system not only outperforms single-step baselines regardless of task difficulty but
also, compared to conventional iterative retrieval methods, demonstrates pronounced advantages in complex tasks through
precise evidence-based reasoning and enhanced efficiency. The proposed system supports both competitive and
collaborative sharing of updated context, enabling multi-agent extension. The benefits of multi-agent configurations
become especially prominent as task difficulty increases. The number of convergence steps scales with task difficulty,
suggesting cost-effective scalability. When agents utilize a lightweight LLM, the system achieves results that are
comparable or superior to those obtained with heavier counterparts, indicating that the agent system architecture
enhances reasoning capabilities independently of the underlying LLM’s reasoning abilities. This also points to the
potential for further optimization of agent collaboration structures.

## 1 Introduction

Large Language Models (LLMs) are probabilistic language generation models that do not incorporate explicit reasoning
systems or logical planning modules. Consequently, in tasks that require synthesizing information over multiple steps,
the reasoning performed at each stage is not clearly delineated, and intermediate reasoning occurs implicitly, making
the process susceptible to errors. Furthermore, the difficulty of rigorously validating each step exacerbates the
accumulation of errors throughout the overall process.

To overcome these challenges, it is often necessary to retrieve external knowledge that compensates for the inherent
limitations of LLMs, especially in real-world scenarios. Approaches such as Retrieval Augmented Generation (RAG) play a
significant role by acquiring information not contained within the model in real time, thereby enabling more precise
responses.

Multi-step question answering (QA) is a representative challenge that demands both high precision in intermediate
reasoning and the integration of diverse information. It not only exposes the limitations of LLMs but has also emerged
as an important benchmark for real-world problems that seek to transcend these limitations.

In this context, we propose Knowledge-Aware Iterative Retrieval for Multi-Agent Systems, a retrieval optimization system
that employs an agent-based framework. It iteratively optimizes search queries through agent-guided knowledge
accumulation, with a focus on query refinement, the iterative process of modifying or enhancing an initial query to
improve search results. The system dynamically optimizes search queries using LLM-based agents and operates through the
following core stages:
1. 1.
   
   Query Planning: Agents leverage accumulated knowledge to propose or refine search queries while avoiding redundant
   searches and ensuring that unresolved sub-goals are addressed.
2. 2.
   
   Knowledge Extraction: Relevant passages from retrieved documents are distilled into verified facts and identification
   of unresolved gaps, which are then used to update each agent’s knowledge base.
3. 3.
   
   Contextual Filtering: Utilizing the accumulated knowledge, the system filters out extraneous or inconsistent text to
   retain only the most pertinent document segments, thereby reducing reasoning overload and mitigating potential
   confusion or hallucination.

If information gaps persist, the system reiterates the cycle of query planning, knowledge extraction, and contextual
filtering. Moreover, in a multi-agent configuration, agents can operate in parallel by sharing knowledge or refined
context, thereby more efficiently exploring the search space.

The novelty of the proposed system lies in its ability to balance the conflicting objectives of collecting a diverse
range of information while ensuring accuracy. Conventional approaches in multi-step question answering generally do not
refine intermediate search results because the process of information filtering, such as determining relevance, is a
multi-step challenge that LLMs struggle to address. In contrast, the proposed method optimizes the trade-off between the
conflicting objectives through agent-based autonomous decision-making. By enabling targeted gap detection and
re-querying mechanisms supported by dynamically updated internal knowledge, the system mitigates LLM biases toward
specific keywords or contexts, thereby facilitating self-correction. Moreover, search queries are progressively refined,
and both the queries and the accumulated knowledge are maintained in a structured form, enhancing traceability and
enabling the identification of errors at specific stages.

During the processes of knowledge extraction, contextual filtering, and query execution, we leverage an LLM as an
optimization tool. Additionally, by sharing refined knowledge or processed documents, the system can be scaled to a
multi-agent configuration, facilitating more efficient exploration of the search space.

The proposed method has been extensively evaluated across a diverse array of open-domain QA benchmarks, including those
that require multi-step reasoning. Our findings are as follows:
* •
  
  The agent-based retrieval system outperforms single-step retrieval baselines (e.g., naive RAG) regardless of the task.
  In our evaluation, we considered both retrieval effectiveness and downstream QA performance.
* •
  
  Compared to conventional iterative retrieval approaches based on static search strategies, the proposed method offers
  several advantages. It reduces computational cost through systematic query expansion, improves scalability, and
  enables precise evidence-based reasoning. As tasks require more compositional reasoning, the proposed approach
  prioritizes preserving precision over indiscriminate context expansion, resulting in efficiency gains over
  conventional methods.
* •
  
  When multi-agent extensions are applied, the performance boost compared to a single agent becomes particularly evident
  for more complex tasks, indicating that the optimal resolution strategy is task-dependent. However, the number of
  convergence steps in the agent system scales with task difficulty, ensuring that efficiency is maintained across
  varying levels of complexity. In competitive multi-agent extensions, implicit role differentiation among agents
  emerges, thereby enhancing the proposed system’s systematic gap resolution mechanism, which plays a key role in
  addressing challenging tasks.
* •
  
  Agent scaling results indicate that performance gains do not increase linearly with the number of agents, and that
  LLMs reported to have superior reasoning capabilities are not necessarily better suited for collaborative frameworks.
  For example, a 2-agent GPT-4o-mini configuration achieved cost-effective, optimal results compared to configurations
  using GPT-4o. This finding suggests that the benefits of the agent architecture can enhance problem-solving
  capabilities independently of the underlying LLM’s reasoning abilities, and underscores the potential for further
  optimization of agent collaboration mechanisms.

## 2 Related Work

### 2.1 Retrieval-augmented generation (RAG)

Retrieval-Augmented Generation (RAG) combines traditional information retrieval with generative models to handle
open-domain QA tasks, improving factual accuracy by retrieving relevant documents from external sources and generating
answers. However, simple implementation of RAG, where often one model instance is responsible for query understanding,
retrieval, and answer generation, often face significant challenges due to cascading errors, a phenomenon where errors
made early in the retrieval or reasoning process propagate through later stages, compounding mistakes and reducing
overall system accuracy. [[9][37]] empirically demonstrate that retrieval-augmented language models are highly sensitive
to the relevance of the retrieved context: while relevant passages enhance performance, irrelevant ones can lead to
cascading errors, particularly in multi-hop reasoning scenarios. To address this, they propose a modular, black-box
solution that leverages a natural language inference (NLI) model to filter out irrelevant passages without altering the
underlying LLM’s parameters.

### 2.2 Multi-Step Question Answering

Multi-step question answering requires models to systematically integrate information across sequential reasoning steps,
a task particularly challenging for conventional LLMs. While humans naturally decompose complex queries into modular
sub-problems, LLMs often struggle with implicit error propagation due to their lack of explicit reasoning mechanisms.
This limitation has spurred the development of specialized benchmarks to rigorously evaluate multi-step reasoning
capabilities.

For instance, [[2][38]] introduces dependency-chained sub-questions where each step’s resolution hinges on the prior
step’s correctness, effectively eliminating shortcut solutions. Similarly, [[3][39]] provides structured reasoning paths
to trace whether models genuinely perform multi-hop inference rather than answer surface matching. These benchmarks
underscore the critical need for explicit intermediate verification, a gap addressed by our proposed knowledge-aware
iterative retrieval framework.

### 2.3 Multi-Step Retrieval Optimization

In response to the limitations of conventional single-step retrieval methods such as RAG, recent research has
increasingly focused on iterative retrieval approaches. For instance, [[7][40]] interleaves retrieval with
chain-of-thought reasoning, refining queries at each iteration based on intermediate inferences from a generator such as
an LLM. Although these methods tend to show high accuracy in downstream QA benchmarks, they incur high computational
costs as iterations repeat and risk exceeding context-window limits since all intermediate results are fed back into the
model.

To address these limitations, an adaptive retrieval approach was proposed in [[8][41]] that employs a dedicated
classifier to determine when multi-step retrieval is needed. By triggering multi-step retrieval based on task
difficulty, this method aims to mitigate the computational overhead associated with repeated retrieval and reasoning
cycles. However, this method is still fundamentally based on an iterative retrieval approach, employing a separate
model, specifically a classifier trained to classify task difficulty.

## 3 Proposed Approach

The proposed approach employs iterative retrieval but distinguishes itself through the following key mechanisms:

The system enables diversity in search exploration via targeted query formulation. Unlike conventional methods that
accumulate all intermediate reasoning outputs from a separate generative model (e.g., chain-of-thought), our approach
maintains a dedicated knowledge base derived from LLM-generated outputs while decoupling it from query formulation. By
explicitly designing queries to isolate necessary information at each step, the system preserves a transparent reasoning
trajectory, critical for real-world applications requiring verifiable intermediate steps. Furthermore, this separation
mitigates model-inherent biases and provides a foundation for multi-agent extensions through shared knowledge
cross-pollination.

In addition, the system ensures accuracy through dynamic context filtering. While existing methods naively retain all
intermediate contexts, leading to computational overload and LLM confusion, our approach bounds context size by
selectively filtering irrelevant or redundant information. Although technically challenging (requiring multi-step
summarization and extraction), the filtering process leverages an agent-curated knowledge base independent of external
documents, thereby reducing susceptibility to hallucinations.

Finally, the system optimizes the trade-off between diversity and accuracy through agent-based autonomous
decision-making and multi-agent cross-validation. By dynamically prioritizing either exploration (diverse search paths)
or exploitation (evidence convergence) based on task requirements, the framework achieves robustness against conflicting
objectives. This optimization not only improves processing efficiency but also enables scalable multi-agent extensions
while maintaining performance across complexity levels.

### 3.1 Core Components

The proposed agent-based knowledge enhanced retrieval system implements an iterative inference process as detailed in
Algorithm [1][42]. The architecture optimizes the accuracy–diversity trade-off through interconnected components:
* •
  
  Knowledge Update Mechanism: At each step, the agent independently makes decisions based on the LLM, including query
  formulation, knowledge updating, and document filtering. To facilitate this, the system maintains two dynamic memory
  structures:
  * –
    
    What is Known: 𝒦t={k1,…,kn}subscript𝒦𝑡subscript𝑘1…subscript𝑘𝑛\mathcal{K}_{t}=\{k_{1},\dots,k_{n}\}caligraphic_K
    start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT = { italic_k start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT , … , italic_k
    start_POSTSUBSCRIPT italic_n end_POSTSUBSCRIPT } where each kisubscript𝑘𝑖k_{i}italic_k start_POSTSUBSCRIPT italic_i
    end_POSTSUBSCRIPT represents verified facts.
  * –
    
    What is Required: ℛt={r1,…,rm}subscriptℛ𝑡subscript𝑟1…subscript𝑟𝑚\mathcal{R}_{t}=\{r_{1},\dots,r_{m}\}caligraphic_R
    start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT = { italic_r start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT , … , italic_r
    start_POSTSUBSCRIPT italic_m end_POSTSUBSCRIPT } where each rjsubscript𝑟𝑗r_{j}italic_r start_POSTSUBSCRIPT italic_j
    end_POSTSUBSCRIPT denotes an unresolved information gap, which is a specific piece of missing knowledge required to
    resolve the overarching query.
  
  At each step, the system evaluates the relevance of content relative to the query and extracts the core information
  from the source documents, which may include summarization or editorial refinement. In doing so, the system
  structurally decouples the relevance assessment from the filtering process, ensuring that only information directly
  pertinent to the inquiry is retained (What is Known).
  
  The system dynamically defines and tracks the unresolved information gaps that must be addressed (What is Required).
  This enables the autonomous decomposition of user inputs into components that are more targeted and progressively
  refined.
  
  The knowledge structure undergoes continuous refinement through interactions with the external environment, such as
  document repositories. As the system collects new documents via search tools to address unresolved information gaps in
  ℛtsubscriptℛ𝑡\mathcal{R}_{t}caligraphic_R start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT, it extracts and integrates
  relevant information into 𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT
  (verified facts) while updating ℛtsubscriptℛ𝑡\mathcal{R}_{t}caligraphic_R start_POSTSUBSCRIPT italic_t
  end_POSTSUBSCRIPT to reflect remaining gaps. This process establishes a closed feedback loop: updated knowledge in
  𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT directly informs subsequent
  query formulation to avoid redundant searches, while revised gaps in ℛtsubscriptℛ𝑡\mathcal{R}_{t}caligraphic_R
  start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT guide the prioritization of unexplored information needs. By
  iteratively aligning retrieved evidence with both verified facts and unresolved requirements, the system dynamically
  adapts its search strategy, ensuring contextually informed retrieval that balances the exploration of new search paths
  with the exploitation of confirmed evidence. This iterative refinement mechanism enables progressive convergence
  toward resolving the overarching query while maintaining computational efficiency.
* •
  
  Query Planning: The agent dynamically formulates search queries by referring to:
  * –
    
    Current state of What is Known (𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K start_POSTSUBSCRIPT italic_t
    end_POSTSUBSCRIPT)
  * –
    
    Unresolved information gaps in What is Required (ℛtsubscriptℛ𝑡\mathcal{R}_{t}caligraphic_R start_POSTSUBSCRIPT
    italic_t end_POSTSUBSCRIPT)
  * –
    
    Query history (𝒬tsubscript𝒬𝑡\mathcal{Q}_{t}caligraphic_Q start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT) to prevent
    redundancy
  
  While prioritizing the resolution of unresolved sub-problems, the agent also autonomously identifies missing
  information gaps through its internal reasoning. This dual mechanism, which is grounded in both explicit goals and
  agent-derived hypotheses, enables progressive decomposition of complex objectives and exploration of diverse search
  paths, thereby enhancing adaptability in dynamic environments.
* •
  
  Contextual Filtering: The system employs a dual-stage filtering process that leverages the dynamically updated
  knowledge 𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT. As described in
  the Knowledge Update Mechanism, potentially conflicting information in 𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K
  start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT is fact-checked against new documents
  𝒟tsubscript𝒟𝑡\mathcal{D}_{t}caligraphic_D start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT (integrating or discarding
  data based on consistency checks). Then, the system refines 𝒟tsubscript𝒟𝑡\mathcal{D}_{t}caligraphic_D
  start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT at the passage level:
  
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  𝒟f⁢i⁢l⁢t⁢e⁢r⁢e⁢d=⋃d∈𝒟{s∈d∣ϕ⁢(s,𝒦t)>τ},subscript𝒟𝑓𝑖𝑙𝑡𝑒𝑟𝑒𝑑subscript𝑑𝒟conditional-set𝑠𝑑italic-ϕ𝑠subscript𝒦𝑡𝜏\mathcal{D}_{filter
  ed}=\bigcup_{d\in\mathcal{D}}\{s\in d\mid\phi(s,\mathcal{K}% _{t})>\tau\},caligraphic_D start_POSTSUBSCRIPT italic_f
  italic_i italic_l italic_t italic_e italic_r italic_e italic_d end_POSTSUBSCRIPT = ⋃ start_POSTSUBSCRIPT italic_d ∈ 
  caligraphic_D end_POSTSUBSCRIPT { italic_s ∈ italic_d ∣ italic_ϕ ( italic_s , caligraphic_K start_POSTSUBSCRIPT     
  italic_t end_POSTSUBSCRIPT ) > italic_τ } ,                                                                         
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  
  where ϕitalic-ϕ\phiitalic_ϕ is a semantic matching function (e.g., an LLM-based approach for measuring textual
  similarity) that computes the relevance between a text segment s𝑠sitalic_s and the current knowledge
  𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT. Since
  𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT is iteratively updated based
  on new documents and inference steps, each filtering operation adapts to the latest state of verified facts.

### 3.2 Optimization Strategy

The proposed system adheres to the core principles of agent autonomy and goal-directed behavior through two
interconnected mechanisms: an iterative decision cycle and dynamic goal management. These ensure self-directed
adaptation to evolving information needs while maintaining alignment with overarching objectives.
* •
  
  Autonomous Decision Cycle: The agent executes the following steps without external intervention, demonstrating
  autonomy through closed-loop reasoning:
  1. 1.
     
     Query Generation:
     
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     qt+1=fL⁢L⁢M⁢(qt,𝒟t|𝒦t,ℛt,𝒬1:t),subscript𝑞𝑡1subscript𝑓𝐿𝐿𝑀subscript𝑞𝑡conditionalsubscript𝒟𝑡subscript𝒦𝑡subscriptℛ𝑡subsc
     ript𝒬:1𝑡q_{t+1}=f_{LLM}(\,q_{t},\mathcal{D}_{t}\;|\;\mathcal{K}_{t},\mathcal{R}_{t},%                            
     \mathcal{Q}_{1:t}\,),italic_q start_POSTSUBSCRIPT italic_t + 1 end_POSTSUBSCRIPT = italic_f start_POSTSUBSCRIPT  
     italic_L italic_L italic_M end_POSTSUBSCRIPT ( italic_q start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ,         
     caligraphic_D start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT | caligraphic_K start_POSTSUBSCRIPT italic_t        
     end_POSTSUBSCRIPT , caligraphic_R start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT , caligraphic_Q                 
     start_POSTSUBSCRIPT 1 : italic_t end_POSTSUBSCRIPT ) ,                                                           
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     
     where the agent synthesizes queries using verified facts (𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K
     start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT), unresolved gaps (ℛtsubscriptℛ𝑡\mathcal{R}_{t}caligraphic_R
     start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT), and query history (𝒬1:tsubscript𝒬:1𝑡\mathcal{Q}_{1:t}caligraphic_Q
     start_POSTSUBSCRIPT 1 : italic_t end_POSTSUBSCRIPT) to avoid redundancy.
  2. 2.
     
     Document Retrieval:
     
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     𝒟add,t+1=Retrieve⁢(qt+1),subscript𝒟add𝑡1Retrievesubscript𝑞𝑡1\mathcal{D}_{\text{add},t+1}=\text{Retrieve}\bigl{(}q_
     {t+1}\bigr{)},caligraphic_D start_POSTSUBSCRIPT add , italic_t + 1 end_POSTSUBSCRIPT = Retrieve ( italic_q       
     start_POSTSUBSCRIPT italic_t + 1 end_POSTSUBSCRIPT ) ,                                                           
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     
     expanding evidence while preserving exploration diversity.
  3. 3.
     
     Knowledge Update:
     
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     𝒦t+1,ℛt+1=gL⁢L⁢M⁢(𝒟t|𝒦t,ℛt),subscript𝒦𝑡1subscriptℛ𝑡1subscript𝑔𝐿𝐿𝑀conditionalsubscript𝒟𝑡subscript𝒦𝑡subscriptℛ𝑡\mathca
     l{K}_{t+1},\;\mathcal{R}_{t+1}=g_{LLM}\bigl{(}\mathcal{D}_{t}\;\big{|}%                                          
     \;\mathcal{K}_{t},\;\mathcal{R}_{t}\bigr{)},caligraphic_K start_POSTSUBSCRIPT italic_t + 1 end_POSTSUBSCRIPT ,   
     caligraphic_R start_POSTSUBSCRIPT italic_t + 1 end_POSTSUBSCRIPT = italic_g start_POSTSUBSCRIPT italic_L italic_L
     italic_M end_POSTSUBSCRIPT ( caligraphic_D start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT | caligraphic_K        
     start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT , caligraphic_R start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ) ,
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     
     refining verified facts and unresolved gaps through agent-driven validation.
  4. 4.
     
     Filter Document:
     
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     𝒟t+1=hL⁢L⁢M⁢((𝒟t∪𝒟add,t+1)|𝒦t+1).subscript𝒟𝑡1subscriptℎ𝐿𝐿𝑀conditionalsubscript𝒟𝑡subscript𝒟add𝑡1subscript𝒦𝑡1\mathcal{
     D}_{t+1}=h_{LLM}\Bigl{(}\bigl{(}\mathcal{D}_{t}\cup\mathcal{D}_{\text%                                           
     {add},t+1}\bigr{)}\;\Big{|}\;\mathcal{K}_{t+1}\Bigr{)}.caligraphic_D start_POSTSUBSCRIPT italic_t + 1            
     end_POSTSUBSCRIPT = italic_h start_POSTSUBSCRIPT italic_L italic_L italic_M end_POSTSUBSCRIPT ( ( caligraphic_D  
     start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ∪ caligraphic_D start_POSTSUBSCRIPT add , italic_t + 1            
     end_POSTSUBSCRIPT ) | caligraphic_K start_POSTSUBSCRIPT italic_t + 1 end_POSTSUBSCRIPT ) .                       
     ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────
     
     retaining only knowledge-aligned passages to minimize reasoning noise.
* •
  
  Goal Management: The system balances dual objectives through adaptive termination:
  * –
    
    Primary objective: Minimize unresolved gaps: min⁡|ℛt|subscriptℛ𝑡\min|\mathcal{R}_{t}|roman_min | caligraphic_R
    start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT |, while maximizing evidence relevance:
    max⁡Relevance⁢(𝒟t)Relevancesubscript𝒟𝑡\max\text{Relevance}(\mathcal{D}_{t})roman_max Relevance ( caligraphic_D
    start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ).
  * –
    
    Dynamic termination condition:
    
    ──────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
    Endt=𝕀⁢[ℛt=∅∨Sufficiency⁢(Mt,𝒟t)≥τ]subscriptEnd𝑡𝕀delimited-[]subscriptℛ𝑡Sufficiencysubscript𝑀𝑡subscript𝒟𝑡𝜏\text{│(1)
    End}_{t}=\mathbb{I}\Bigl{[}\mathcal{R}_{t}=\emptyset\;\lor\;\text{%                                           │   
    Sufficiency}(M_{t},\mathcal{D}_{t})\geq\tau\Bigr{]}End start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT =       │   
    blackboard_I [ caligraphic_R start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT = ∅ ∨ Sufficiency ( italic_M      │   
    start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT , caligraphic_D start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT │   
    ) ≥ italic_τ ]                                                                                                │   
    ──────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───
    
    where ℛt=∅subscriptℛ𝑡\mathcal{R}_{t}=\emptysetcaligraphic_R start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT = ∅
    indicates that all required information gaps have been resolved; Mtsubscript𝑀𝑡M_{t}italic_M start_POSTSUBSCRIPT
    italic_t end_POSTSUBSCRIPT is the moderator (optional, activated when external validation is available) that
    assesses whether the current refined context 𝒟tsubscript𝒟𝑡\mathcal{D}_{t}caligraphic_D start_POSTSUBSCRIPT italic_t
    end_POSTSUBSCRIPT sufficiently answers the user query; and τ𝜏\tauitalic_τ is a moderator-determined relevance
    threshold.

The inherent conflicts between accuracy and diversity poses a critical challenge: while diverse search exploration
fosters creative connections, it risks introducing unverified information. Conversely, strict adherence to verified
facts limits novel insights. To resolve this trade-off, the proposed system integrates the following optimization
mechanisms for autonomous balancing of context preservation (minimizing hallucination) and information diversity
(maximizing exploration):
* •
  
  Progressive Diversity Promotion:
  
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
  qt+1=argmaxq′[α⁢Div⁢(q′∣𝒬1:t)+(1−α)Rel(q′∣ℛt)]subscript𝑞𝑡1subscriptsuperscript𝑞′𝛼Divconditionalsuperscript𝑞′subscr│(2)
  ipt𝒬:1𝑡1𝛼Rel∣superscript𝑞′subscriptℛ𝑡\begin{split}q_{t+1}=\arg\max_{q^{\prime}}\Bigl{[}&\alpha\,\text{Div}(q^{% │   
  \prime}\mid\mathcal{Q}_{1:t})\\                                                                                 │   
  &+(1-\alpha)\,\text{Rel}(q^{\prime}\mid\mathcal{R}_{t})\Bigr{]}\end{split}start_ROW start_CELL italic_q         │   
  start_POSTSUBSCRIPT italic_t + 1 end_POSTSUBSCRIPT = roman_arg roman_max start_POSTSUBSCRIPT italic_q           │   
  start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT end_POSTSUBSCRIPT [ end_CELL start_CELL italic_α Div ( italic_q     │   
  start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT ∣ caligraphic_Q start_POSTSUBSCRIPT 1 : italic_t end_POSTSUBSCRIPT )│   
  end_CELL end_ROW start_ROW start_CELL end_CELL start_CELL + ( 1 - italic_α ) Rel ( italic_q                     │   
  start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT ∣ caligraphic_R start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ) ]  │   
  end_CELL end_ROW                                                                                                │   
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───
  
  where q′superscript𝑞′q^{\prime}italic_q start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT denotes a candidate query
  generated during the query formulation process.
  Div⁢(q′∣Q1:t)Divconditionalsuperscript𝑞′subscript𝑄:1𝑡\text{Div}(q^{\prime}\mid Q_{1:t})Div ( italic_q
  start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT ∣ italic_Q start_POSTSUBSCRIPT 1 : italic_t end_POSTSUBSCRIPT ) measures
  how different q′superscript𝑞′q^{\prime}italic_q start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT is from the previously
  attempted queries Q1:tsubscript𝑄:1𝑡Q_{1:t}italic_Q start_POSTSUBSCRIPT 1 : italic_t end_POSTSUBSCRIPT, thereby
  promoting information diversity. Meanwhile, Rel⁢(q′∣Rt)Relconditionalsuperscript𝑞′subscript𝑅𝑡\text{Rel}(q^{\prime}\mid
  R_{t})Rel ( italic_q start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT ∣ italic_R start_POSTSUBSCRIPT italic_t
  end_POSTSUBSCRIPT ) quantifies how relevant q′superscript𝑞′q^{\prime}italic_q start_POSTSUPERSCRIPT ′
  end_POSTSUPERSCRIPT is to the current requirements Rtsubscript𝑅𝑡R_{t}italic_R start_POSTSUBSCRIPT italic_t
  end_POSTSUBSCRIPT. The weight α𝛼\alphaitalic_α is dynamically adjusted based on the agent’s assessment of query
  diversity needs versus task priority.
* •
  
  Context-Preserving Filtering:
  
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────┬───
  Accept(d′)=𝕀[ExactMatch⁢(d′,d)=1∧maxk∈𝒦tRel(d′,k)>τ2]Acceptsuperscript𝑑′𝕀delimited-[]ExactMatchsuperscript𝑑′𝑑1sub│(3)
  script𝑘subscript𝒦𝑡Relsuperscript𝑑′𝑘subscript𝜏2\begin{split}\text{Accept}(d^{\prime})=\mathbb{I}\Bigl{[}&\text{Ex│   
  actMatch}(d^% {\prime},d)=1\;\land\;\\                                                                          │   
  &\max_{\mathclap{k\in\mathcal{K}_{t}}}\text{Rel}(d^{\prime},k)>\tau_{2}\Bigr{]% }\end{split}start_ROW start_CELL│   
  Accept ( italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT ) = blackboard_I [ end_CELL start_CELL ExactMatch │   
  ( italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT , italic_d ) = 1 ∧ end_CELL end_ROW start_ROW start_CELL │   
  end_CELL start_CELL roman_max start_POSTSUBSCRIPT italic_k ∈ caligraphic_K start_POSTSUBSCRIPT italic_t         │   
  end_POSTSUBSCRIPT end_POSTSUBSCRIPT Rel ( italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT , italic_k ) >   │   
  italic_τ start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT ] end_CELL end_ROW                                             │   
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────┴───
  
  where Rel⁢(d′,d)Relsuperscript𝑑′𝑑\text{Rel}(d^{\prime},d)Rel ( italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT ,
  italic_d ) captures how closely the refined context d′superscript𝑑′d^{\prime}italic_d start_POSTSUPERSCRIPT ′
  end_POSTSUPERSCRIPT remains aligned to the original document d𝑑ditalic_d (ensuring that the context is preserved), and
  
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  ExactMatch⁢(d′,d)={1,if ⁢d′⊆d,0,otherwise.ExactMatchsuperscript𝑑′𝑑cases1if superscript𝑑′𝑑0otherwise\text{ExactMatch}(d
  ^{\prime},d)=\begin{cases}1,&\text{if }d^{\prime}\subseteq d% ,\\[6.0pt] 0,&\text{otherwise}.\end{cases}ExactMatch (
  italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT , italic_d ) = { start_ROW start_CELL 1 , end_CELL start_CELL  
  if italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT ⊆ italic_d , end_CELL end_ROW start_ROW start_CELL 0 ,      
  end_CELL start_CELL otherwise . end_CELL end_ROW                                                                    
  ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  
  which guarantees that d′superscript𝑑′d^{\prime}italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT does not include
  any modified passages (e.g., it prevents transformations such as p1→x1→subscript𝑝1subscript𝑥1p_{1}\rightarrow
  x_{1}italic_p start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT → italic_x start_POSTSUBSCRIPT 1 end_POSTSUBSCRIPT).
  Additionally,
  maxk∈𝒦t⁡Rel⁢(d′,k)subscript𝑘subscript𝒦𝑡Relsuperscript𝑑′𝑘\max_{k\in\mathcal{K}_{t}}\text{Rel}(d^{\prime},k)roman_max
  start_POSTSUBSCRIPT italic_k ∈ caligraphic_K start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT end_POSTSUBSCRIPT Rel (
  italic_d start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT , italic_k ) measures how well d′superscript𝑑′d^{\prime}italic_d
  start_POSTSUPERSCRIPT ′ end_POSTSUPERSCRIPT aligns with the knowledge base 𝒦tsubscript𝒦𝑡\mathcal{K}_{t}caligraphic_K
  start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT, enforcing factual consistency. The function
  𝕀⁢[⋅]𝕀delimited-[]⋅\mathbb{I}[\cdot]blackboard_I [ ⋅ ] is an indicator function, and τ2subscript𝜏2\tau_{2}italic_τ
  start_POSTSUBSCRIPT 2 end_POSTSUBSCRIPT is an agent-determined relevance threshold.
* •
  
  Knowledge Update:
  * –
    
    Binary Fact-Checking:
    
    ────────────────────────────────────┬─────────────────────────────────────────────────────────────────────────┬───
    FlagOrDiscard⁢(s)FlagOrDiscard𝑠\displ│=𝕀[∄k∈𝒦t such                                                            │(4)
    aystyle\text{FlagOrDiscard}(s)FlagOr│that\displaystyle=\mathbb{I}\Bigl{[}\nexists\,k\in\mathcal{K}_{t}\text{  │   
    Discard ( italic_s )                │such that }= blackboard_I [ ∄ italic_k ∈ caligraphic_K                   │   
                                        │start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT such that                 │   
    ────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┼───
                                        │Supported(s,k)=1]\displaystyle\quad\text{Supported}(s,k)=1\Bigr{]}Support│   
                                        │ed ( italic_s , italic_k ) = 1 ]                                         │   
    ────────────────────────────────────┴─────────────────────────────────────────────────────────────────────────┴───
    
    where Supported⁢(s,k)Supported𝑠𝑘\text{Supported}(s,k)Supported ( italic_s , italic_k ) is a binary function that
    returns 1 if segment s𝑠sitalic_s is directly supported by a verified fact
    k∈𝒦t𝑘subscript𝒦𝑡k\in\mathcal{K}_{t}italic_k ∈ caligraphic_K start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT, and 0
    otherwise, ensuring that any segment not supported by the current knowledge is flagged or discarded.
  * –
    
    Dynamic Knowledge Integration:
    
    ──────────────────────────────────────┬───────────────────────────────────────────────────────────────────────┬───
    𝒦t+1subscript𝒦𝑡1\displaystyle\mathcal{│=𝒦t∪{s|Supported(s,d)=1\displaystyle=\mathcal{K}_{t}\cup\Bigl{\{}s\,\Bi│(5)
    K}_{t+1}caligraphic_K                 │g{|}\,\text{Supported}(s,d)=1= caligraphic_K start_POSTSUBSCRIPT       │   
    start_POSTSUBSCRIPT italic_t + 1      │italic_t end_POSTSUBSCRIPT ∪ { italic_s | Supported ( italic_s ,       │   
    end_POSTSUBSCRIPT                     │italic_d ) = 1                                                         │   
    ──────────────────────────────────────┼───────────────────────────────────────────────────────────────────────┼───
                                          │∧Relevant(s,ℛt)=1}\displaystyle\quad\land\,\text{Relevant}\bigl{(}s,\ma│   
                                          │thcal{R}_{t}\bigr{)}=1% \Bigr{\}}∧ Relevant ( italic_s , caligraphic_R │   
                                          │start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ) = 1 }                 │   
    ──────────────────────────────────────┴───────────────────────────────────────────────────────────────────────┴───
    
    where Relevant⁢(s,ℛt)Relevant𝑠subscriptℛ𝑡\text{Relevant}(s,\mathcal{R}_{t})Relevant ( italic_s , caligraphic_R
    start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT ) returns 1 if segment s𝑠sitalic_s addresses the unresolved
    information gaps in ℛtsubscriptℛ𝑡\mathcal{R}_{t}caligraphic_R start_POSTSUBSCRIPT italic_t end_POSTSUBSCRIPT,
    modeling the iterative refinement process by which new evidence is integrated into the knowledge base, ensuring that
    only supported and relevant information is added.
  
  Together, these mechanisms help to reduce the risk of unfounded claims and ensure that the system’s knowledge base is
  continuously updated in a contextually informed manner.

In summary, each mechanism addresses a distinct subproblem:
1. 1.
   
   Progressive Diversity Promotion balances the need for exploring new or alternative search paths against their
   relevance to task priority in order to fill unresolved information gaps.
2. 2.
   
   Context-Preserving Filtering ensures that any refined context remains consistent with both verified knowledge and
   source documents.
3. 3.
   
   Knowledge Update ensures factual consistency by flagging or discarding any segments that are not supported by the
   current knowledge base or do not meet the moderator’s criteria. Then, it dynamically integrates relevant and verified
   evidence into the knowledge base, thereby continuously refining its understanding.

By decoupling these mechanisms, the system optimizes the accuracy–diversity trade-off.

Algorithm 1 Knowledge-Aware Agent Retrieval Algorithm

Require: Refinement Step, Knowledge Update Step, Filtering Step, Search Tool T𝑇Titalic_T, (External) Router
Rextsubscript𝑅extR_{\mathrm{ext}}italic_R start_POSTSUBSCRIPT roman_ext end_POSTSUBSCRIPT
* •
  
  Status Cache Definitions:
  * –
    
    𝐊⊆ℐ𝐊ℐ\mathbf{K}\subseteq\mathcal{I}bold_K ⊆ caligraphic_I (Known Information)
  * –
    
    𝐑⊆ℐ𝐑ℐ\mathbf{R}\subseteq\mathcal{I}bold_R ⊆ caligraphic_I (Required Information)
  * –
    
    𝐐⊆Σ∗𝐐superscriptΣ\mathbf{Q}\subseteq\Sigma^{*}bold_Q ⊆ roman_Σ start_POSTSUPERSCRIPT ∗ end_POSTSUPERSCRIPT (Query
    History)
  * –
    
    𝐃⊆𝒟𝐃𝒟\mathbf{D}\subseteq\mathcal{D}bold_D ⊆ caligraphic_D (Refined Context)
* •
  
  Input: User question x𝑥xitalic_x
* •
  
  Output: Refined context 𝐃𝐃\mathbf{D}bold_D sufficient to answer x𝑥xitalic_x

1:Initialize:
𝐊←∅,𝐑←∅,𝐐←∅,𝐃←∅formulae-sequence←𝐊formulae-sequence←𝐑formulae-sequence←𝐐←𝐃\mathbf{K}\leftarrow\emptyset,\;\mathbf{R}\lef
tarrow\emptyset,\;\mathbf{Q}% \leftarrow\emptyset,\;\mathbf{D}\leftarrow\emptysetbold_K ← ∅ , bold_R ← ∅ , bold_Q ← ∅ ,
bold_D ← ∅.
2:Check External Router Rextsubscript𝑅extR_{\mathrm{ext}}italic_R start_POSTSUBSCRIPT roman_ext end_POSTSUBSCRIPT for
retrieval need.
3:if Retrieve == Yes then
4:     (Initial Retrieval): Call T⁢(x)𝑇𝑥T(x)italic_T ( italic_x ).
5:     →→\quad\rightarrow→ Obtain rephrased queries 𝐪0subscript𝐪0\mathbf{q}_{0}bold_q start_POSTSUBSCRIPT 0
end_POSTSUBSCRIPT and passages 𝐝0subscript𝐝0\mathbf{d}_{0}bold_d start_POSTSUBSCRIPT 0 end_POSTSUBSCRIPT.
6:     [Knowledge Update]: Update 𝐊,𝐑𝐊𝐑\mathbf{K},\mathbf{R}bold_K , bold_R using 𝐝0subscript𝐝0\mathbf{d}_{0}bold_d
start_POSTSUBSCRIPT 0 end_POSTSUBSCRIPT.
7:     [Filter Context]: Derive refined context 𝐃𝐃\mathbf{D}bold_D from 𝐝0subscript𝐝0\mathbf{d}_{0}bold_d
start_POSTSUBSCRIPT 0 end_POSTSUBSCRIPT using 𝐊𝐊\mathbf{K}bold_K, 𝐑𝐑\mathbf{R}bold_R.
8:     𝐐←𝐐∪{𝐪0}←𝐐𝐐subscript𝐪0\mathbf{Q}\leftarrow\mathbf{Q}\cup\{\mathbf{q}_{0}\}bold_Q ← bold_Q ∪ { bold_q
start_POSTSUBSCRIPT 0 end_POSTSUBSCRIPT }; 𝐃←𝐝0←𝐃subscript𝐝0\mathbf{D}\leftarrow\mathbf{d}_{0}bold_D ← bold_d
start_POSTSUBSCRIPT 0 end_POSTSUBSCRIPT;
9:     i←1←𝑖1i\leftarrow 1italic_i ← 1
10:     repeat
11:         [Refine]: Call T⁢(𝐪i−1)𝑇subscript𝐪𝑖1T(\mathbf{q}_{i-1})italic_T ( bold_q start_POSTSUBSCRIPT italic_i - 1
end_POSTSUBSCRIPT ). Obtain 𝐪isubscript𝐪𝑖\mathbf{q}_{i}bold_q start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT,
𝐝isubscript𝐝𝑖\mathbf{d}_{i}bold_d start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT.
12:         𝐐←𝐐∪{𝐪i}←𝐐𝐐subscript𝐪𝑖\mathbf{Q}\leftarrow\mathbf{Q}\cup\{\mathbf{q}_{i}\}bold_Q ← bold_Q ∪ { bold_q
start_POSTSUBSCRIPT italic_i end_POSTSUBSCRIPT };
𝐃←𝐃∪𝐝i←𝐃𝐃subscript𝐝𝑖\mathbf{D}\leftarrow\mathbf{D}\cup\mathbf{d}_{i}bold_D ← bold_D ∪ bold_d start_POSTSUBSCRIPT
italic_i end_POSTSUBSCRIPT;
13:         [Knowledge Update]: Update 𝐊,𝐑𝐊𝐑\mathbf{K},\mathbf{R}bold_K , bold_R using 𝐃𝐃\mathbf{D}bold_D.
14:         [Filter Context]: Re-filter 𝐃𝐃\mathbf{D}bold_D with the updated 𝐊

[Content truncated]
```
