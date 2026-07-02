# Web source

- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC12824272
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T16:30:05.719844304+00:00

```text
[ Skip to main content ][1]

An official website of the United States government

Here's how you know
Here's how you know

**Official websites use .gov**
A **.gov** website belongs to an official government organization in the United States.

**Secure .gov websites use HTTPS**
A **lock** ( [Lock] ) or **https://** means you've safely connected to the .gov website. Share sensitive information
only on official, secure websites.

[ [NCBI home page] ][2]
Search
Log in
* [ Dashboard ][3]
* [ Publications ][4]
* [ Account settings ][5]
* Log out
Search… Search NCBI

Primary site navigation

[Close] Search [Search]

Logged in as: ****
* [ Dashboard ][6]
* [ Publications ][7]
* [ Account settings ][8]

Log in
[PMC search open icon] [PMC search close ison]
Search PMC Full-Text Archive Search in PMC [Search]
* [ Journal List ][9]
* [ User Guide ][10]
* [Open resources icon]
* [ [View on publisher site icon] ][11]
* [ [Download PDF icon] ][12]
* [Collections icon] [Collections icon]
* [Cite icon]
* [Show article permalink icon]
  
  ## PERMALINK
  
  [Copy icon] Copy
[Open article navigation icon]
As a library, NLM provides access to scientific literature. Inclusion in an NLM database does not imply endorsement of,
or agreement with, the contents by NLM or the National Institutes of Health.
Learn more: [PMC Disclaimer][13] | [ PMC Copyright Notice ][14]
[Scientific Reports logo]
Sci Rep
. 2026 Jan 20;16:2640. doi: [10.1038/s41598-025-33854-2][15]
* [Search in PMC][16]
* [Search in PubMed][17]
* [View in NLM Catalog][18]
* [Add to search][19]

# Knowledge-based question answering using graph neural networks and contextual language representations

[Mohamed Samir][20]

### Mohamed Samir

¹Information Systems Department, Faculty of Computer and Information Sciences, Ain Shams University, Cairo, Egypt
Find articles by [Mohamed Samir][21]
^{1,}^{✉}, [Naglaa Fathy][22]

### Naglaa Fathy

¹Information Systems Department, Faculty of Computer and Information Sciences, Ain Shams University, Cairo, Egypt
Find articles by [Naglaa Fathy][23]
¹, [Walaa Gad][24]

### Walaa Gad

¹Information Systems Department, Faculty of Computer and Information Sciences, Ain Shams University, Cairo, Egypt
Find articles by [Walaa Gad][25]
¹
* Author information
* Article notes
* Copyright and License information
¹Information Systems Department, Faculty of Computer and Information Sciences, Ain Shams University, Cairo, Egypt
^{✉}

Corresponding author.

Received 2025 Sep 19; Accepted 2025 Dec 22; Collection date 2026.

© The Author(s) 2025

**Open Access** This article is licensed under a Creative Commons Attribution 4.0 International License, which permits
use, sharing, adaptation, distribution and reproduction in any medium or format, as long as you give appropriate credit
to the original author(s) and the source, provide a link to the Creative Commons licence, and indicate if changes were
made. The images or other third party material in this article are included in the article’s Creative Commons licence,
unless indicated otherwise in a credit line to the material. If material is not included in the article’s Creative
Commons licence and your intended use is not permitted by statutory regulation or exceeds the permitted use, you will
need to obtain permission directly from the copyright holder. To view a copy of this licence, visit
[http://creativecommons.org/licenses/by/4.0/][26].

[PMC Copyright notice][27]
PMCID: PMC12824272  PMID: [41559152][28]

## Abstract

This work introduces a novel question answering (QA) framework that integrates commonsense knowledge from ConceptNet
with deep contextual embeddings from BERT using a graph neural network for structured reasoning. For each
question–answer pair, the system constructs a relevant subgraph from ConceptNet, which is then processed using Graph
Attention Network v2 (GATv2) to capture semantic relationships among concepts. In parallel, BERT encodes the
question–answer pair to provide contextual language representations. These two representations are fused into a joint
embedding that combines structured knowledge with unstructured text understanding, enabling richer inference.
Evaluations on the CommonsenseQA and OpenBookQA datasets show accuracy improvements of 82.3% and 86.21%, respectively,
surpassing existing leading methods. These results highlight the effectiveness of combining knowledge graphs with
language models for complex QA tasks requiring commonsense reasoning.

**Keywords:** Knowledge graph, Graph neural networks, Language models, Question answering, QA system

**Subject terms:** Computational biology and bioinformatics, Mathematics and computing

## Introduction

The field of Natural Language Processing (NLP) has witnessed rapid progress, especially with the emergence of large
pre-trained language models (PLMs), which have significantly improved machines’ ability to comprehend and answer
questions submitted in natural language^{[2][29]}. These models—such as BERT, RoBERTa, and GPT—have shown strong
performance across a variety of NLP tasks, including reading comprehension, summarization, and question answering.
Despite these advancements, such models still face difficulties when confronted with reasoning tasks that necessitate
structured commonsense knowledge—which is often missing from raw textual data. Commonsense knowledge refers to the broad
and implicit understanding of everyday situations, facts, and relationships that humans typically take for granted,
enabling them to make inferences beyond explicitly stated information.

A growing body of research has shown that enriching language models with structured knowledge can improve their
reasoning capabilities. To bridge this gap, researchers have turned to structured resources like knowledge graphs (KGs),
structured repositories that encode semantic relationships between concepts. Popular KGs like ConceptNet^{[1][30]},
WordNet^{[6][31]}, and Wikidata^{[7][32]} serve as rich sources of relational knowledge. ConceptNet is valuable for
commonsense reasoning due to its broad coverage of everyday entities and their interconnections, making it a suitable
candidate for enhancing question answering (QA) systems.

Several recent models have attempted to integrate knowledge graphs with neural language models. Approaches like MHGRN
and QA-GNN have shown that leveraging graph neural networks (GNNs) can improve the interpretability and performance of
QA systems. However, these models often rely on fixed graph structures or early fusion methods that may limit their
adaptability to diverse question types.

In this study, a hybrid architecture for multiple-choice QA is introduced, in which structured information from
ConceptNet^{[1][33]} is combined with contextual insights from the BERT language model^{[2][34]}. At the heart of this
architecture lies Graph Attention Network v2 (GATv2)^{[3][35]}, which is employed to identify and reason over
question-relevant subgraphs. Unlike previous GNN variants, GATv2 enables more flexible and expressive attention
mechanisms, allowing the model to better capture salient connections between entities. To our knowledge, this is the
first application of GATv2 within the domain of knowledge graph-based QA.

The key contributions of this work include:
1. Dynamically constructing subgraphs from ConceptNet based on each question and its candidate answer choices, enabling
   focused reasoning through GATv2.
2. Effective fusion of these graph-derived embeddings with BERT-based
3. textual representations to improve answer prediction.
4. Rigorous evaluation of the proposed architecture using two prominent commonsense QA
   benchmarks—CommonsenseQA^{[4][36]} and OpenBookQA^{[5][37]}—both of which consist of multiple-choice questions,
   requiring deeper reasoning.

This work aims to contribute to the ongoing discussion around hybrid neural-symbolic approaches in NLP, demonstrating
how the synergy between structured knowledge and deep contextual representations can lead to more accurate and
explainable QA systems.

The remainder of this paper is organized as follows: Section "[Related Work][38]" presents a comprehensive review of
related work in the area of knowledge-enhanced question answering. Section "[Methodology][39]" outlines the proposed
methodology, with a detailed explanation of the integration of knowledge graphs with language models. Section
"[ConceptNet Alignment][40]" describes the experimental setup, including datasets, implementation details, and
evaluation metrics. Section "[Experimental results][41]" presents and discusses the results of the experiments. Finally,
Section "[Conclusion and Future Work][42]" concludes the paper and outlines potential directions for future research.

## Related work

Question Answering (QA) has been a long-standing challenge in Natural Language Processing (NLP), witnessing significant
advancements following the emergence of large-scale pre-trained language models (PLMs) such as BERT^{[2][43]},
RoBERTa^{[8][44]}, and T5^{[9][45]}. These models have demonstrated high accuracy across various QA benchmarks,
especially for factoid-based questions where information can be directly extracted from textual data. . However, their
capabilities are often limited when it comes to commonsense reasoning, primarily due to the absence of structured,
external knowledge in their training data.

To extend these models’ reasoning capabilities, many researchers have explored the integration of external knowledge
through Knowledge Graphs (KGs). Table [1][46] provides a comparative summary of key related works, highlighting their
strengths and limitations.

### Table 1.

Comparison of Related Work in Knowledge Graph-Based Question Answering.

───────────┬─────────────────────────┬──────────────────┬──────────────────────────────────┬────────────────────────────
Research   │Base model               │KG integration    │Advantages                        │Limitations                 
paper      │                         │method            │                                  │                            
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
Lin et     │RoBERTa                  │Multi-hop         │Enriches contextual embeddings    │Static paths; lacks dynamic 
al.^{[10][4│                         │ConceptNet paths  │with external knowledge           │adaptability                
7]}        │                         │                  │                                  │                            
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
Bauer et   │Custom (KG-based)        │Reasoning chains  │Supports multi-hop inference      │Dependent on graph          
al.^{[11][4│                         │over KG paths     │grounded in KG                    │construction quality        
8]}        │                         │                  │                                  │                            
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
De Cao et  │RoBERTa + GCN            │GCN over entity   │Effective for multi-hop reasoning │Fixed structure; limited    
al.^{[12][4│                         │nodes             │in HotpotQA                       │adaptability to sparse      
9]}        │                         │                  │                                  │graphs                      
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
Fang et    │BERT + GCN               │GCN for biomedical│Strong in domain-specific         │Requires rich               
al.^{[13][5│                         │QA                │reasoning                         │domain-specific graphs      
0]}        │                         │                  │                                  │                            
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
Yasunaga et│BERT + GAT               │QA-GNN with static│Edge-aware attention; improved QA │Static attention weights;   
al.^{[14][5│                         │relevance scores  │accuracy                          │handcrafted paths           
1]}        │                         │                  │                                  │                            
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
Feng et    │MHGRN                    │Gated multi-hop   │Dynamic path selection; salient   │Computationally expensive;  
al.^{[15][5│                         │GNN               │subgraph focus                    │complexity in training      
2]}        │                         │                  │                                  │                            
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
Feroze et  │DeBERTa with disentangled│None              │Hybrid data curation, temporal    │No external KG reasoning;   
al.^{[20][5│attention                │                  │attention improves SOTA           │synthetic data noise        
3]}        │                         │                  │performance                       │                            
───────────┼─────────────────────────┼──────────────────┼──────────────────────────────────┼────────────────────────────
Feroze et  │Small LMs (Phi-3, etc.)  │None              │Shows instruction tuning can      │No explicit KG; limited     
al.^{[21][5│with instruction tuning  │                  │elicit reasoning behavior         │relational reasoning        
4]}        │                         │                  │                                  │                            
───────────┴─────────────────────────┴──────────────────┴──────────────────────────────────┴────────────────────────────
[Open in a new tab][55]

In^{[10][56]}, Lin et al. proposed enriching contextual embeddings by incorporating multi-hop paths from ConceptNet into
a RoBERTa-based architecture for commonsense QA. Similarly, Bauer et al.^{[11][57]} introduced a method to construct
chains of reasoning over KG paths, enabling multi-hop inference grounded in real-world knowledge. Both works emphasize
the importance of using structured knowledge to complement text-based representations.

A prominent research direction involves Graph Neural Networks (GNNs) that performs structured reasoning over KGs.
In^{[12][58]}, De Cao et al. employed Graph Convolutional Networks (GCNs) alongside RoBERTa to enhance multi-hop
question answering on HotpotQA, showing that propagating information over entity nodes improves reasoning. Likewise,
Fang et al.^{[13][59]} utilized GCN with BERT to handle relational paths in biomedical QA tasks, underlining GCN’s
strength in domain-specific reasoning.

On the other hand, several studies have explored Graph Attention Networks (GATs) to learn edge-aware representations.
Yasunaga et al.^{[14][60]} introduced QA-GNN, which uses GATs over ConceptNet^{[1][61]} combined with BERT-based
encoding to jointly reason over question–answer pairs. While QA-GNN shows promising results, it relies on static
relevance scores and handcrafted graph paths, which may hinder scalability.Additionally, Feng et al.^{[15][62]} proposed
MHGRN, a multi-hop GNN that dynamically selects paths using learned gating mechanisms, enabling the model to focus on
salient subgraphs. Both models were evaluated on CommonsenseQA^{[4][63]} and OpenBookQA^{[5][64]}, highlighting the
importance of integrating structured knowledge and contextual language models.

Feroze et al.^{[20][65]} introduced a disentangled attention-based framework with a hybrid data strategy to improve
temporal commonsense understanding, demonstrating that structured temporal cues substantially improve model reasoning.
Similarly, Feroze et al.^{[21][66]} investigated commonsense reasoning in small language models, showing how lightweight
models can benefit from external knowledge resources and prompting strategies to compensate for limited parameter
capacity.

Despite these contributions, existing approaches often suffer from limited flexibility, especially when handling sparse
or heterogeneous graph structures. Static attention mechanisms used in GAT or fixed graph paths in GCN may fail to
capture context-sensitive relationships that vary across questions or domains.

To address these limitations, we propose leveraging Graph Attention Network v2 (GATv2)^{[3][67]}, an improvement over
the original GAT. GATv2 introduces a dynamic and expressive attention mechanism that allows the model to adaptively
weigh neighboring nodes based on the context. This enhancement is particularly valuable when performing commonsense
reasoning over noisy or loosely connected subgraphs extracted from large KGs.

As shown, while prior approaches have made significant progress in integrating structured knowledge into QA systems,
many suffer from rigidity in attention mechanisms or reliance on predefined graph structures.

## Methodology

This section explains the proposed system that integrates structured commonsense knowledge from ConceptNet^{[1][68]}
with contextual information from a pre-trained language model (BERT)^{[2][69]}, structured within a graph neural network
(GNN) based architecture. The proposed architecture, illustrated in Fig. [1][70], is composed of three primary
components:
1. Subgraph construction,
2. Graph-based reasoning via GATv2^{[3][71]}, and
3. Fusion with language model embeddings.

### Fig. 1.

[[Fig. 1]][72]

[Open in a new tab][73]

Proposed architecture for KG-based QA.

### Subgraph construction from ConceptNet

To incorporate external knowledge into the question answering process, ConceptNet^{[1][74]} is used to extract a
relevant subgraph for each question–answer pair in both the CommonsenseQA^{[4][75]} and OpenBookQA^{[5][76]} datasets.
This is done by identifying the key concepts mentioned in the question and its answer choices, and then retrieving all
1-hop (i.e., directly connected) and 2-hop (i.e., connected through one intermediate node) neighbors connected to these
concepts. The resulting subgraph is expected to provide commonsense relationships that support or refute the
plausibility of each answer option. Figure [2][77] shows an example of a multiple-choice question from the CommonsenseQA
dataset, together with the resulting subgraph for the third choice, to be used for reasoning.

#### Fig. 2.

[[Fig. 2]][78]

[Open in a new tab][79]

Commonsense question-candidate choices example, and the retrieved ConceptNet subgraph for the third choice (c).

Each subgraph is represented as a directed graph G = (V,E), where:
* V is the set of concept nodes (terms from ConceptNet),
* E is the set of semantic relations (edges) connecting them.

To identify the key terms that seed the KG retrieval process, we employ a multi-stage extraction strategy combining
syntactic, statistical, and semantic signals:
1. **Syntactic candidate extraction**
   
   We first process the question and answer choices using a dependency parser and extract all noun phrases (NPs), verb
   phrases (VPs), and named entities. This ensures that concept extraction is grounded in the syntactic structure of the
   text rather than relying solely on token-level heuristics.
2. **Part-of-speech and semantic filtering**
   
   From the syntactic candidates, we retain only nouns, noun compounds, and verb heads, as these categories most
   frequently align with ConceptNet nodes. Function words and adjectives not tied to a noun phrase are removed to reduce
   noise.
3. **TF–IDF relevance scoring**
   
   To prioritize semantically meaningful tokens, we compute TF–IDF scores over the training corpus and retain candidates
   in the top 30% by relevance. This step filters generic terms (e.g., “thing”, “use”) and emphasizes informative
   concepts.
   
   The cutoff of 30% was selected empirically based on validation experiments comparing multiple thresholds (20–40%).
   This value provided a stable balance between filtering generic tokens and retaining sufficient semantic coverage for
   effective subgraph construction.
4. **ConceptNet alignment**
   
   Each remaining candidate is mapped to ConceptNet using a hybrid approach:
5. Lexical normalization (lemmatization, lowercasing, stopword removal).
6. Embedding-based similarity using ConceptNet-numberbatch vectors. A candidate is retained as a valid key term if its
   cosine similarity to a ConceptNet node exceeds 0.4, ensuring semantic alignment.
7. The similarity threshold of 0.4 was selected empirically based on validation experiments comparing multiple cutoff
   values (0.3–0.6), and was found to provide a stable trade-off between semantic precision and coverage.
8. **Final candidate set**
   
   The resulting set of key terms typically includes 2–5 concepts per question and seed the subgraph retrieval procedure
   described next.
   
   This multi-step pipeline ensures that extracted terms are syntactically grounded, semantically meaningful, and
   aligned with ConceptNet, thereby improving subgraph quality and downstream reasoning.

To ensure that the resulting subgraph remains both informative and computationally tractable, a relevance scoring
mechanism is employed. Specifically, a semantic relevance score is computed for each node by measuring the contextual
similarity between the node’s textual representation and the textual representations of its neighbors. This process
involves the following steps:
1. Each concept node from the knowledge graph is embedded using BERT^{[2][80]} to obtain its textual representation
   vector.
2. The cosine similarity between each concept’s embedding and the embeddings of its neighbors is then computed as a
   relevance score.

The cosine similarity between two vectors A (concept node embedding) and B (concept node embedding) is given by:

────────────────────────────────────┬─
[graphic file with name d33e752.gif]│1
────────────────────────────────────┴─

where:—A · B is the dot product of the two vectors,—||A|| and ||B|| are the Euclidean norms (magnitudes) of vectors A
and B, respectively. This score reflects the relevance of the concept vector to the QA context in the embedding space—a
higher score indicates greater semantic relevance.

For instance, given the question *"What is used to cut paper?"* with the answer choice *“scissors”*. A concept like
*“sharp”* might have a high cosine similarity with the QA embedding and thus be retained in the subgraph. In contrast, a
less relevant concept like *“animal”* would yield a low similarity score and be pruned. Nodes with low relevance
scores—i.e., those that are semantically distant from the QA context—are pruned, along with any resulting disconnected
components. This ensures that the final subgraph retains only contextually pertinent concepts, enabling more focused and
efficient graph-based reasoning.

### Graph-based reasoning with GATv2

In this work, Graph Attention Network v2 (GATv2)^{[3][81]} is leveraged as the core mechanism for graph-based reasoning
over subgraphs extracted from knowledge graphs.

In QA tasks that require commonsense or multi-hop reasoning, language models often struggle to connect distant or
implicit relationships solely from unstructured text. To address this, reasoning over structured subgraphs—derived from
external knowledge sources such as ConceptNet^{[1][82]}—becomes essential. These subgraphs capture semantic connections
among key concepts mentioned in the question and candidate answers. Effectively traversing and aggregating information
from these graphs enables models to infer missing links, assess plausibility, and support explainable predictions.
Hence, a reasoning module is critical for operating over the structured knowledge and integrating it with the language
model’s understanding.

GATv2 is a recent improvement over the original GAT architecture, proposed to address a critical limitation in the
standard attention formulation used in GNNs. Specifically, traditional GAT employs a static attention mechanism in which
the attention coefficients computed between two connected nodes are independent of the query node’s identity, leading to
an inability to adaptively re-rank neighbors based on context. This restricts the model’s expressive power, especially
when modeling more complex or asymmetric relations that are common in commonsense and factual reasoning.

GATv2 resolves this issue by modifying the computation order within the attention mechanism. Instead of applying a
linear transformation to the input features before computing the attention scores (as in GAT), GATv2 first computes a
joint representation by concatenating the untransformed features of both source and target nodes, and only then applies
a shared learnable transformation followed by a LeakyReLU activation^{[16][83]}. This subtle yet powerful change enables
dynamic attention, allowing the network to contextually adjust how it weighs neighboring nodes during message passing.

Formally, for a given node pair (i, j), the attention coefficient [Inline graphic] in GATv2 is computed as follows:

────────────────────────────────────┬─
[graphic file with name d33e799.gif]│2
────────────────────────────────────┴─

where, [Inline graphic] denotes the concatenation of the raw features of nodes i and j, [Inline graphic] is a shared
linear transformation, and a is a learnable vector that projects the transformed pairwise representation into a scalar
attention score. By deferring the projection until after feature concatenation, GATv2^{[3][84]} ensures that the
attention scores are sensitive to both nodes’ raw features, thereby enhancing the model’s ability to differentiate and
prioritize edges in a context-aware manner.

In this work, GATv2 layers are applied over subgraphs derived from the knowledge graph, where nodes represent concepts
and edges denote semantic relations. This architecture allows the model to reason over graph structure by learning which
neighboring nodes contribute most to the inference task, guided by both the topology and the semantic content of node
features. Furthermore, GATv2 supports multi-head attention, enabling the model to capture diverse relational
perspectives and enhance generalization.

In practice, we use a three-layer GATv2 architecture with ReLU activations and dropout regularization between layers.
This setup enables hierarchical reasoning over node neighborhoods, with the first layer aggregating immediate context
and the second layer refining those representations for downstream classification. The overall architecture is
illustrated in Fig. [3][85].

#### Fig. 3.

[Fig. 3]

[Open in a new tab][86]

Three-layer GATv2 architecture.

Specifically, our contribution differs from prior GNN-based QA systems in the following ways:
* **Task-specific subgraph construction with semantic filtering:** Unlike existing approaches, which operate on large,
  fixed ConceptNet expansions, our method performs T5-based concept extraction and cosine-similarity filtering to
  generate a *compact, question-conditioned* subgraph for each candidate answer. GATv2 is applied on these tailored
  subgraphs, enabling more targeted relational reasoning.
* **Relevance-guided pruning before message passing:** We introduce a pruning strategy that removes nodes with low
  semantic relevance prior to GATv2 propagation. This drastically reduces noise—an issue that earlier GNN-based QA
  systems often faced—and changes how attention operates on graph neighborhoods.
* **Alignment to language-space semantics through projection and fusion:** Our approach includes a learnable projection
  layer mapping the GATv2 graph embedding into the BERT semantic space, followed by an optimized fusion module built
  specifically for sparse subgraphs. This design is essential for small ConceptNet graphs (often < 15 nodes), where the
  standard GATv2 configuration used in the literature is insufficient.

In summary, GATv2’s dynamic attention mechanism^{[3][87]} enhances the capacity of graph neural networks to perform
nuanced reasoning over complex knowledge graphs. Its superior expressiveness and practical efficiency make it a suitable
choice for integrating graph-based knowledge in question answering tasks that require structured relational
understanding.

### Fusion with language model embeddings

While graph-based reasoning over ConceptNet subgraphs^{[1][88]} enables relational understanding of commonsense
knowledge, it is often insufficient to fully capture the nuances of natural language. To complement this, we integrate
contextualized language representations obtained from a pre-trained transformer-based language model^{[2][89]}. This
fusion allows our model to reason jointly over structured knowledge graphs and unstructured text, improving overall
performance in multiple-choice question answering.

For each question–answer pair, we first construct a combined input string in the form of:

[CLS] question [SEP] answer choice [SEP],

Where [CLS] token, short for *classification*, is a special token that is prepended to the beginning of every input
sequence, [SEP] token, short for *separator*, is used to delineate different segments within the input.

The combined input string passes through a pre-trained BERT-base model^{[2][90]} to generate a dense contextual
embedding. We use the final hidden state corresponding to the [CLS] token as the global semantic representation of the
question–answer pair. This embedding captures rich syntactic and semantic features derived from the input text and is
particularly effective in modeling fine-grained distinctions among the answer choices.

To perform multimodal reasoning, the BERT-derived embedding is fused with the GATv2-based graph representation.
Specifically, the output node embeddings from the final GATv2 layer are aggregated using mean pooling to form a
fixed-size vector representing the subgraph. This vector is concatenated with the BERT-based textual embedding to obtain
a unified representation:

────────────────────────────────────┬─
[graphic file with name d33e889.gif]│3
────────────────────────────────────┴─

Here, [Inline graphic] denotes the [CLS] embedding from BERT, [Inline graphic] is the pooled GATv2 output, and ∥ denotes
vector concatenation.

In this work, we adopt vector concatenation as the fusion mechanism between the BERT-based textual representation and
the GATv2-derived graph embedding. This choice offers a balanced trade-off between performance, interpretability, and
computational efficiency. Unlike attention-based fusion or cross-modal transformer layers—which introduce substantial
additional parameters, training overhead, and complexity—concatenation preserves scalability and ensures that the
contributions of the graph and language components remain explicitly disentangled. While prior systems such as
GreaseLM^{[17][91]} employ more sophisticated fusion modules, they typically rely on much larger backbone models and
heavier computation. Our ablation study further confirms that, within our architecture, concatenation achieves
competitive performance relative to more elaborate fusion strategies, yielding strong results on both
CommonsenseQA^{[4][92]} and OpenBookQA^{[5][93]} while maintaining a lean and reproducible model design.

The fused representation is passed through a feed-forward network that outputs a logit vector [Inline graphic] for the C
candidate answers:

────────────────────────────────────┬─
[graphic file with name d33e923.gif]│4
────────────────────────────────────┴─

The logits are normalized with a softmax to obtain probability scores:

────────────────────────────────────┬─
[graphic file with name d33e929.gif]│5
────────────────────────────────────┴─

The model is trained using the categorical cross-entropy loss:

────────────────────────────────────┬─
[graphic file with name d33e936.gif]│6
────────────────────────────────────┴─

where y is the index of the correct answer choice. This multi-class formulation is appropriate for datasets such as
CommonsenseQA (C = 5) and OpenBookQA (C = 4).

This fusion strategy effectively combines the strong linguistic reasoning capabilities of pretrained language models
with the structured relational information captured by GATv2 over the knowledge graph.

#### Fusion of graph representations and language representations

For each question–answer pair, BERT produces a contextual embedding h_CLS ∈ ℝᴰ, where D = 768 for BERT-base. In
parallel, the constructed ConceptNet subgraph is encoded using a two-layer GATv2 network. Each node v receives an
embedding vector zᵥ ∈ ℝᴳ, with G = 256. After message passing, we apply a graph-level readout function:

────────────────────────────────────┬─
[graphic file with name d33e947.gif]│7
────────────────────────────────────┴─

Yielding a single graph embedding g ∈ ℝᴳ representing relational structures relevant to the candidate answer.

Because D ≠ G, we map the graph embedding into the same semantic space as the language embedding using a learnable
linear projection:

────────────────────────────────────┬─
[graphic file with name d33e955.gif]│8
────────────────────────────────────┴─

The fused representation is obtained through concatenation followed by a non-linear transformation:

────────────────────────────────────┬─
[graphic file with name d33e961.gif]│9
────────────────────────────────────┴─

where [Inline graphic], H = 512, and σ is GELU activation.

Finally, the fused vector is passed to a classification layer to compute the probability for each answer choice:

────────────────────────────────────┬──
[graphic file with name d33e972.gif]│10
────────────────────────────────────┴──

The entire architecture is trained end-to-end, optimizing cross-entropy over the four or five answer candidates. Both
BERT and GATv2 receive gradients from the shared loss, ensuring joint learning of textual and relational signals.

### Relevance scoring mechanism

To compute the relevance of each candidate answer, we first obtain two representations: the contextual language
embedding [Inline graphic] from BERT and the graph-level embedding g′ obtained from the GATv2-encoded subgraph. After
concatenation and non-linear transformation, the fused vector [Inline graphic] represents the joint reasoning space that
incorporates both semantic context and relational knowledge. The final relevance score for each answer choice is
computed as:

────────────────────────────────────┬──
[graphic file with name d33e990.gif]│11
────────────────────────────────────┴──

where [Inline graphic] is the fused representation corresponding to answer choice *i*. The scores for all choices are
normalized using a softmax function:

─────────────────────────────────────┬──
[graphic file with name d33e1003.gif]│12
─────────────────────────────────────┴──

These probabilities reflect how relevant each answer choice is with respect to both the textual context and the
relational structure encoded in the subgraph. During training, the model maximizes the probability of the correct
answer, allowing BERT and GATv2 to jointly learn which semantic and relational features contribute most to answer
relevance.

## Experimental setup

This section provides a description of the datasets used for evaluation, the implementation details of the proposed
model, the training procedures, and the baselines used for comparison.

### Datasets

The proposed approach is evaluated based on two well-established benchmarks that emphasize commonsense reasoning:

CommonsenseQA^{[4][94]}: a multiple‑choice dataset built upon ConceptNet^{[1][95]}, designed to challenge models with
questions that require external commonsense knowledge. It comprises 12,247 questions, each featuring five answer options
with only one correct answer as shown in Fig. [4][96]. The questions are categorized by the relation underlying the
concept from ConceptNet (e.g., *IsA*, *HasProperty*, *AtLocation*), fostering diverse reasoning types across commonsense
domains.

#### Fig. 4.

[[Fig. 4]][97]

[Open in a new tab][98]

Multiple-choice question examples from CommonsenseQA dataset.

**OpenBookQA**^{[5][99]}: a multiple-choice dataset focused on elementary-level science, designed to resemble an
"open-book" exam format, as shown in Fig. [5][100]. It contains approximately 5,960 questions, each with four answer
choices, and is accompanied by an “open book” of 1,326 core science facts. Although it is anchored in scientific facts,
solving its questions typically requires combining the provided science knowledge with additional commonsense or
everyday reasoning. Therefore, in this study, we use it as a hybrid science-and-commonsense reasoning dataset, rather
than classifying it strictly as a commonsense benchmark.

#### Fig. 5.

[[Fig. 5]][101]

[Open in a new tab][102]

Multiple-choice question examples from OpenBookQA dataset.

For both datasets, we use the standard training, validation, and test splits established in previous studies. Model
performance is reported using accuracy as the primary evaluation metric.

For every question and its associated answer candidates, we perform the following steps:
1. **Concept Extraction:** Key terms are identified in the question and each answer choice using a custom
   concept-matching module grounded in the ConceptNet vocabulary^{[1][103]}. This module employs a T5-small model
   fine-tuned on a manually annotated subset of 1,200 CommonsenseQA instances, where each question and its answer
   options were labeled with their core underlying concepts. The model is trained with a learning rate of 3 × 10⁻4,
   batch size 16, maximum input length 64, maximum output length 32, beam size 4, and length penalty 0.8, for 5 epochs
   with early stopping based on validation loss. The fine-tuned model achieves an extraction accuracy of 87.4% on a
   held-out validation split, measured by exact lexical overlap with the human-annotated concepts.
   
   At inference time, the model generates candidate concept phrases for each question and answer choice. These
   candidates are filtered using a syntactic/POS-based pipeline and ranked using TF–IDF relevance over the training
   corpus. Each remaining phrase is mapped to ConceptNet via a hybrid alignment procedure combining lexical
   normalization (lemmatization, lowercasing, and function-word removal) with embedding-based similarity using
   ConceptNet-Numberbatch vectors. A phrase is accepted as a valid ConceptNet concept only if its cosine similarity to
   an existing ConceptNet node exceeds 0.40. This automated pipeline typically yields 2–5 high-confidence
   ConceptNet-aligned concepts per item, which then serve as seed nodes for subgraph retrieval.
2. **Subgraph retrieval:** From ConceptNet^{[1][104]}, we extract a subgraph by retrieving.
3. 1-hop and 2-hop neighbors connected to the identified concepts.
4. **Graph cleaning:** Disconnected nodes and irrelevant components are removed.
5. We retain only the largest connected subgraph to ensure coherence.
6. **Graph construction:** We build the graph’s adjacency matrix and node feature matrix, which serve as inputs to the
   GATv2 layers^{[3][105]}.

To improve efficiency and reduce noise, less useful edges are filtered out by retaining only meaningful relation types
(e.g., *IsA*, *PartOf*, *UsedFor*).

To prune weak or irrelevant edges from the extracted ConceptNet subgraphs, we apply cosine similarity filtering based on
pretrained ConceptNet-numberbatch embeddings. We evaluated pruning thresholds in the range 0.25–0.45 using a small grid
search on a held-out portion of the training data. The threshold 0.35 yielded the best trade-off between eliminating
noisy relations and preserving sufficient connectivity for multi-hop reasoning. Lower thresholds (< 0.30) produced
excessively dense subgraphs that introduced noise, while higher thresholds (> 0.40) resulted in overly sparse structures
that degraded GNN performance. Therefore, we adopt cosine similarity ≥ 0.35 as our final pruning criterion.

To ensure fair evaluation and prevent data leakage, we implement the following steps when constructing ConceptNet-based
subgraphs:
1. **Knowledge base segmentation** The ConceptNet knowledge base is split into distinct subsets, with a clear separation
   between the training and test sets. The training subset is used for model training, while the test subset is strictly
   reserved for evaluation, ensuring that no test-specific knowledge influences the model’s training process.
2. **Relation filtering** We filter out any relations and concepts that directly overlap with the test set in
   CommonsenseQA. This includes excluding any answer choices or question–answer pairs from ConceptNet subgraph
   construction that appear in the test set.
3. **Cross-validation** We perform cross-validation by randomly splitting the dataset into training and validation sets
   and constructing subgraphs only from knowledge not included in the corresponding test split. This procedure ensures
   that the model is evaluated on entirely unseen data.

By implementing these steps, we aim to mitigate the risk of data leakage and ensure that our evaluation is fair and
reflects the model’s generalization ability rather than memorization of test data.

### Models setup and training procedures

Tables [2][106] and [3][107] provide the description of the models employed in the experiment along with the training
procedures, respectively.

#### Table 2.

Setup of the GNN model.

───────────────────────────────────────────────────────────────────────────────────────────
Graph reasoning                                                                            
───────────────────────┬───────────────────────────────────────────────────────────────────
GNN variant            │Graph attention network v2 (GATv2)                                 
───────────────────────┼───────────────────────────────────────────────────────────────────
Layers                 │3                                                                  
───────────────────────┼───────────────────────────────────────────────────────────────────
Attention Heads        │4                                                                  
───────────────────────┼───────────────────────────────────────────────────────────────────
Hidden Dimension       │128                                                                
───────────────────────┼───────────────────────────────────────────────────────────────────
Node Features          │Pre-trained embeddings (e.g., BERT-based vectors)                  
───────────────────────┴───────────────────────────────────────────────────────────────────
***Language understanding***                                                               
───────────────────────┬───────────────────────────────────────────────────────────────────
Model                  │BERT-base (uncased)                                                
───────────────────────┼───────────────────────────────────────────────────────────────────
Input                  │[CLS] Question [SEP] Answer [SEP]                                  
───────────────────────┼───────────────────────────────────────────

[Content truncated]
```
