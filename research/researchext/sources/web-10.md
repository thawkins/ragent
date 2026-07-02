# Web source

- URL: https://research.google/blog/unlocking-dependable-responses-with-gemini-enterprise-agent-platforms-agentic-rag
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-30T09:39:13.158548219+00:00

```text
[Skip to main content][1]

## Explore our many areas of focus

[
Explore all research areas
][2]
Applied AI & sciences
[
[earth_AI_nav]
Earth AI
][3] [
[health_AI_nav]
Health AI
][4] [
[science_AI_nav]
Science AI
][5] [
[sustainability_crisis_resilience_nav]
Sustainability & crisis resilience
][6]
Foundational ML & algorithms
[
[algorithms_theory_nav]
Algorithms & theory
][7] [
[information_retrieval_nav]
Information retrieval
][8] [
[machine_intelligence_nav]
Machine intelligence
][9] [
[machine_perception_nav]
Machine perception
][10] [
[NLP_nav]
Natural language processing
][11]
People, systems & quantum AI
[
[human_computer_interaction_nav]
Human-computer interaction and visualization
][12] [
[networking_nav]
Networking
][13] [
[quantum_AI_nav]
Quantum AI
][14] [
[responsible_AI_nav]
Responsible AI
][15] [
[anti_abuse_nav]
Anti abuse
][16] [
[software_engineering_nav]
Software engineering
][17] [
[software_systems_nav1]
Software systems
][18]
Learn More
[
[publications_nav]
Publications
][19] [
[projects_nav]
Projects
][20]

## Building a collaborative ecosystem

[
[dataset_nav]
Datasets
Access high-quality datasets to accelerate your research.
][21] [
[models_products_nav]
Tools & services
Explore our latest AI models and products.
][22]
[
[software_engineering_nav]
Open source
Discover open-source code and collaborate with the community.
][23]

## Shaping the future together

[
See all programs
][24]
[
[faculty_programs_nav]
Faculty programs
Participating in the academic research community through meaningful engagement with university faculty.
][25] [
[student_programs_nav]
Student programs
Supporting the next generation of researchers through a wide range of programming.
][26]
[
[locations_nav]
Locations
Find your place in our global offices and research labs.
][27]

## Translating discovery into real-world impact

[
[earth_AI_nav]
People
Our researchers drive advancements in computer science through both fundamental and applied research.
][28]
[
[teams_nav]
Teams
Collaborative groups tackling the world's most challenging AI problems.
][29]
Research

## Explore our many areas of focus

[
Explore all research areas
][30]
Applied AI & sciences
[
[earth_AI_nav]
Earth AI
][31] [
[health_AI_nav]
Health AI
][32] [
[science_AI_nav]
Science AI
][33] [
[sustainability_crisis_resilience_nav]
Sustainability & crisis resilience
][34]
Foundational ML & algorithms
[
[algorithms_theory_nav]
Algorithms & theory
][35] [
[information_retrieval_nav]
Information retrieval
][36] [
[machine_intelligence_nav]
Machine intelligence
][37] [
[machine_perception_nav]
Machine perception
][38] [
[NLP_nav]
Natural language processing
][39]
People, systems & quantum AI
[
[human_computer_interaction_nav]
Human-computer interaction and visualization
][40] [
[networking_nav]
Networking
][41] [
[quantum_AI_nav]
Quantum AI
][42] [
[responsible_AI_nav]
Responsible AI
][43] [
[anti_abuse_nav]
Anti abuse
][44] [
[software_engineering_nav]
Software engineering
][45] [
[software_systems_nav1]
Software systems
][46]
Learn More
[
[publications_nav]
Publications
][47] [
[projects_nav]
Projects
][48]
Resources

## Building a collaborative ecosystem

[
[dataset_nav]
Datasets
Access high-quality datasets to accelerate your research.
][49] [
[models_products_nav]
Tools & services
Explore our latest AI models and products.
][50] [
[software_engineering_nav]
Open source
Discover open-source code and collaborate with the community.
][51]
[ Conferences & events ][52]
Careers

## Shaping the future together

[
See all programs
][53]
[
[faculty_programs_nav]
Faculty programs
Participating in the academic research community through meaningful engagement with university faculty.
][54] [
[student_programs_nav]
Student programs
Supporting the next generation of researchers through a wide range of programming.
][55] [
[locations_nav]
Locations
Find your place in our global offices and research labs.
][56]
[ Blog ][57]
About

## Translating discovery into real-world impact

[
[earth_AI_nav]
People
Our researchers drive advancements in computer science through both fundamental and applied research.
][58] [
[teams_nav]
Teams
Collaborative groups tackling the world's most challenging AI problems.
][59]
[ Google Research ][60]
[
Google AI
Learn about all our AI
][61] [
Google DeepMind
Explore the frontier of AI
][62] [
Google Labs
Try our AI experiments
][63]
Research
Resources
[ Conferences & events ][64]
Careers
[ Blog ][65]
About
Search
1. [Home][66]
2. [Blog][67]

# Unlocking dependable responses with Gemini Enterprise Agent Platform’s Agentic RAG

June 5, 2026

Cyrus Rashtchian, Research Scientist, and Da-Cheng Juan, Engineering Manager, Google Research

We introduce our new agentic RAG framework. Based on a collaboration between Google Research and Google Cloud, our
multi-agent workflow goes beyond standard RAG by breaking down complex enterprise queries and iteratively searching for
sufficient context before generating dependable responses.

## Quick links
* [ RAG Engine Cross Corpus Retrieval ][68]
* Share
  * Copy link
    ×

Current single-step [retrieval-augmented generation][69] (RAG) systems weren’t designed for the multi-source, multi-hop
queries of modern business workflows. If, for example, the query is, "What are the specs of the server used in Project
X?", the system might find documents about Project X, but those documents might only mention a server ID. It won't know
to take that ID and perform a second search in another database to find the specs. The result is a partial answer or a
"not found" response because the information is spread across different "islands" of data, requiring deeper exploration
to find the facts.

Enter “agentic RAG”, which plans, reasons, and iteratively interacts with data sources, enabling the handling of complex
queries to increase dependability and accuracy.

Today, we’re excited to introduce Google’s [Gemini Enterprise Agent Platform][70]-hosted version of [Cross-Corpus
Retrieval powered by Agentic RAG][71]. Like [other multi-agent RAG][72] frameworks, ours employs various agents that
work together to reliably answer complex queries. Unlike other multi-agent frameworks, ours incorporates [sufficient
context][73] to confirm if there is enough information for an accurate answer. Compared to standard RAG, our framework
increases accuracy on factuality datasets by up to 34%. We also evaluated our system with proprietary, internal datasets
and found that we achieve better grounding and improved reasoning accuracy on multiple domain-specific tasks.

## How multi-agent architectures work: Planning, rewriting, and routing

It helps to think of multi-agent RAG not as a single search engine but as an organized research department. In a
"monolithic" or “[Vanilla][74]” RAG system, the retrieval component just looks at your question and tries to find
matching documents before an LLM generates a response.

In a multi-agent framework, the system breaks the job down into specialized roles:
1. The Orchestrator evaluates your complex request and decides, "This isn't a one-step job", and delegates the work to
   agents.
2. The Planner Agent maps out the information pathways. If you ask about a project’s budget and its timeline, for
   example, the Planner Agent decides: "First, we need to check the finance database, then we need to check the project
   management logs."
3. The Query Rewriter translates your request into multiple search queries. It turns "What's up with Project X?" into
   "Status report for Project X Q3" and "Key blockers for Project X team."
4. The Search Fanout Agent takes those refined queries and sends them to various retrieval sources to collect snippets
   of information.
5. Finally, an LLM aggregates all the context to deliver a final response.

play silent looping video pause silent looping video
unmute video mute video

Demonstrating a standard agentic RAG system. While this has multiple agents, it does not include iterative retrieval or
specialized cross-corpus support.

## What makes our agentic RAG different from others

The key difference with our new agentic RAG framework is persistence. Compared to other RAG solutions, our framework is
effective because it knows when it is missing information and continues searching until the context is complete. This
prevents the AI from "guessing" when the first search comes up empty, or from simply saying, “I don’t have enough
information.” While this is an appropriate response in some cases, sometimes the information is there and we just need
to find it.

For example, imagine a doctor asking about a patient’s medications, diet, and allergies:

"What are the discharge medications and dietary restrictions for John Doe after his knee surgery, and did he have any
allergic reactions during his stay? Do not include medications only administered during hospital inpatient or emergency
department visits except for heparin IV drip or Tenecteplase."

In response, our framework kicks off many specialized agents. We give an overview of our solution in the figure below
and then describe it in more detail afterwards.

play silent looping video pause silent looping video
unmute video mute video

Illustrating our multi-agent RAG solution, which includes a sufficient context agent, as well as the ability to
iteratively retrieve more information before answering the query.

### Phase 1: Orchestration

The Root Agent parses the doctor's request and delegates the tasks to sub-agents. The Planner Agent identifies that it
needs to check three distinct areas: Pharmacy, Nutrition, and Clinical Notes. The Query Rewriter breaks the long request
into simple, searchable questions so the retriever can more accurately find relevant content.


### Phase 2: Search (standard step)

The RAG Agent searches the patient's records for all the query fanouts at once. It finds the medications and the diet
information, but it can’t find any mention of allergies in the most obvious files. In a standard or “Vanilla” RAG
system, the process might end here with an incomplete answer.


### Phase 3: Sufficient Context Agent (new research innovation)

Think of the Sufficient Context Agent as a quality-control inspector standing at the end of an assembly line. It
examines three specific findings before allowing a response to be generated:

#### 1. Retrieved snippets

The Sufficient Context Agent evaluates the actual text chunks pulled from the database by the RAG Agent. In the doctor's
example, these could be the specific paragraphs found in the "Discharge Summary" and "Nutrition Notes." It reads these
to see if the information needed to answer the query is present in those sentences.

#### 2. Intermediate draft

The system also creates a "rough draft" response. The Sufficient Context Agent then reviews the prompt, draft, and
retrieved snippets to evaluate whether the model has everything it needs to provide a comprehensive and grounded answer.
If the prompt asks for three things (meds, diet, allergies) but the snippets only contain information about two, the
Sufficient Context Agent flags it as “insufficient context.”

#### 3. Missing pieces analysis

This is the most critical part. The Sufficient Context Agent identifies exactly what is not there. It doesn't just
output that "this is insufficient"; it generates a specific "Reason" and "Feedback" log. For example:

Finding: "We have the medication list and the low-sodium diet instructions."

Gap: "We are missing information from the source documents about allergic reactions or adverse events during the stay."

The Sufficient Context Agent compares what was found against the original request and asks: "Did we answer the allergy
question?” If not, it then issues an "Insufficient Context" signal and provides specific feedback: "You found meds and
diet, but you missed allergies. Go back and search specifically for 'rashes' or 'adverse events'." In a multi-source
situation, it can also request more information or decide that the source isn’t relevant to the query.

### Phase 4: Iteration

Because of the Sufficient Context Agent feedback, the Query Rewriter creates a new search for "rashes." Then, the RAG
Agent dives deeper into files it ignored the first time and finds the missing information.

### Phase 5: Synthesis (final answer)

The Sufficient Context Agent checks the data one last time. Now that it has the meds, diet, and allergy info, it
determines we can stop searching. Finally, the Synthesis Agent writes a clean, accurate summary for the doctor.

## Experiments and results

We evaluated agentic RAG on [FramesQA][75], which is based on the [FRAMES][76] paper. An example multi-hop question is:

“Of the top two most watched television season finales (as of June 2024), which finale ran the longest in length and by
how much?”

The RAG system needs to perform multiple steps to arrive at the correct answer. First, it has to identify that the two
most watched finales are from the shows [M*A*S*H][77] and [Cheers][78]. Then, it has to find their running times, and
calculate the length difference. In many RAG settings (Vanilla RAG or agentic RAG without sufficient context), we could
end up in a situation where the model says something like:

“Despite multiple scans, I found no explicit runtimes for M*A*S*H or Cheers. The documents provide viewership data, but
not the duration in minutes or hours.”

This does not answer the question.

Fortunately, our agentic RAG can solve this by first searching for the TV shows, then using the Query Rewriter and
Sufficient Context Agent to have a targeted search for the run time of M*A*S*H or Cheers. Then, Gemini can easily
determine which finale ran the longest in length and by how much:

“The M*A*S*H finale ran for 150 minutes, making it the longest of the top two. It was 52 minutes longer than the Cheers
finale, which ran for approximately 98 minutes.”

We ran an experiment to test this ability at scale (FramesQA has 824 queries along with a corpus containing 2,676 PDF
documents). In the “Vanilla” RAG setting, we use Google’s [RAG Engine][79] (which has an advanced retrieval engine, LLM
parser, and re-ranker). We compared this with our agentic RAG in two settings. In the single-corpus setting, we retrieve
from the FramesQA documents. In the cross-corpus setting, we also include three other distracting datasets, where the
Planner Agent must determine where to retrieve from. This cross-corpus setting mimics use cases where companies have
databases managed by separate teams. We compute accuracy by using an LLM-as-a-judge to compare the system responses to
the ground truth answers in the dataset.

In the cross-corpus setting, our system nearly matches its single-corpus accuracy. Even when the Planner Agent must
select the correct corpus out of 4 possibilities, we successfully route the search queries and answer 90.1% of questions
correctly. Also, the latency of both single- and cross-corpus versions is about the same (within 3% on average). This
demonstrates that our Agentic RAG system can reason over multiple, unrelated data sources, which opens up possibilities
for more flexible retrieval scenarios.

[AgenticRAG3_Comparison]

Comparison of **cross-corpus** retrieval versus single-corpus and Vanilla RAG on FramesQA, demonstrating that our
agentic solutions achieve high accuracy.

## Conclusion

By combining advanced query planning, routing, and sufficient context, our agentic RAG system ensures that AI-generated
responses are auditable, traceable, and grounded. We look forward to seeing how the machine learning community leverages
these new agentic capabilities to build the next generation of dependable AI systems. This new feature is now available
as [a public preview offering in Gemini Enterprise Agent Platform][80].

## Acknowledgments

This project is joint work with Bo Li, Zhongjie Mao, Tiger Jin, Yuhong Kan, Mohd Abdullah (Obito), Chun-Sung Ferng,
Pooneh Mortazavi, Roger (Peng) Yu, Eran Lewis, and Ivan Kuznetsov. We thank Kimberly Schwede for designing the graphics
and Mark Simborg for writing assistance. We also thank our key enterprise partners for critical user feedback, data, and
insights.
* Labels:
* [Data Management][81]
* [Machine Intelligence][82]
* [Natural Language Processing][83]
* [Product][84]

## Quick links
* [ RAG Engine Cross Corpus Retrieval ][85]
* Share
  * Copy link
    ×

## Other posts of interest
* [
  
  June 26, 2026
  
  Accelerating Gemini Nano models on Pixel with frozen Multi-Token Prediction
  * Machine Intelligence ·
  * Mobile Systems ·
  * Natural Language Processing
  ][86]
* [
  
  June 25, 2026
  
  Optimizing cloud economics with linear elastic caching
  * Algorithms & Theory ·
  * Data Management
  ][87]
* [
  
  June 24, 2026
  
  Thinking to recall: How reasoning unlocks parametric knowledge in LLMs
  * Generative AI ·
  * Machine Intelligence ·
  * Natural Language Processing
  ][88]

× ❮ ❯
[AgenticRAG3_Comparison]

## Follow us
* [ ][89]
* [ ][90]
* [ ][91]
* [ ][92]

## Explore our other initiatives

### Google AI

Discover how Google AI is committed to enriching knowledge and solving complex challenges
* [
  Products
  ][93]
* [
  Build
  ][94]
* [
  Research
  ][95]
* [
  Responsibility
  ][96]
* [
  Societal Impact
  ][97]
* [
  About
  ][98]

### Google Cloud

High-performance infrastructure for cloud computing, data analytics & machine learning
* [
  Overview
  ][99]
* [
  Solutions
  ][100]
* [
  Products
  ][101]
* [
  Pricing
  ][102]
* [
  Resources
  ][103]

### Google DeepMind

Our mission is to build AI responsibly to benefit humanity
* [
  Models
  ][104]
* [
  Research
  ][105]
* [
  Science
  ][106]
* [
  About
  ][107]

### Google Labs

Explore the future of AI responsibly with Google Labs
* [
  About
  ][108]
* [
  Experiments
  ][109]
* [
  Stay connected
  ][110]
[ About Google ][111]
[ Google Products ][112]
[ Privacy ][113]
[ Terms ][114]
Cookies management controls
×

[1]: #page-content
[2]: /research-areas/
[3]: /research-areas/google-earth-ai/
[4]: /research-areas/health-ai/
[5]: /research-areas/science-ai/
[6]: /research-areas/sustainability-crisis-resilience/
[7]: /research-areas/algorithms-and-theory/
[8]: /research-areas/information-retrieval/
[9]: /research-areas/machine-intelligence/
[10]: /research-areas/machine-perception/
[11]: /research-areas/natural-language-processing/
[12]: /research-areas/human-computer-interaction-and-visualization/
[13]: /research-areas/networking/
[14]: /research-areas/quantum-computing/
[15]: /research-areas/responsible-ai/
[16]: /research-areas/anti-abuse/
[17]: /research-areas/software-engineering/
[18]: /research-areas/software-systems/
[19]: /pubs/
[20]: /resources/our-projects/
[21]: https://research.google/resources/#datasets-1
[22]: https://research.google/resources/#tools-services-2
[23]: https://research.google/resources/#open-source-3
[24]: /programs-and-events/
[25]: /programs-and-events/faculty-engagement/
[26]: /programs-and-events/student-engagement/
[27]: /careers/
[28]: /people/
[29]: /teams/
[30]: /research-areas/
[31]: /research-areas/google-earth-ai/
[32]: /research-areas/health-ai/
[33]: /research-areas/science-ai/
[34]: /research-areas/sustainability-crisis-resilience/
[35]: /research-areas/algorithms-and-theory/
[36]: /research-areas/information-retrieval/
[37]: /research-areas/machine-intelligence/
[38]: /research-areas/machine-perception/
[39]: /research-areas/natural-language-processing/
[40]: /research-areas/human-computer-interaction-and-visualization/
[41]: /research-areas/networking/
[42]: /research-areas/quantum-computing/
[43]: /research-areas/responsible-ai/
[44]: /research-areas/anti-abuse/
[45]: /research-areas/software-engineering/
[46]: /research-areas/software-systems/
[47]: /pubs/
[48]: /resources/our-projects/
[49]: https://research.google/resources/#datasets-1
[50]: https://research.google/resources/#tools-services-2
[51]: https://research.google/resources/#open-source-3
[52]: /conferences-and-events/
[53]: /programs-and-events/
[54]: /programs-and-events/faculty-engagement/
[55]: /programs-and-events/student-engagement/
[56]: /careers/
[57]: /blog/
[58]: /people/
[59]: /teams/
[60]: /
[61]: https://ai.google/?utm_source=deepmind.google&utm_medium=referral&utm_campaign=gdm&utm_content=
[62]: https://deepmind.google?utm_source=deepmind.google&utm_medium=referral&utm_campaign=gdm&utm_content=/
[63]: https://labs.google/?utm_source=deepmind.google&utm_medium=referral&utm_campaign=gdm&utm_content=
[64]: /conferences-and-events/
[65]: /blog/
[66]: /
[67]: /blog/
[68]: https://docs.cloud.google.com/gemini-enterprise-agent-platform/build/rag-engine/cross-corpus-retrieval
[69]: https://en.wikipedia.org/wiki/Retrieval-augmented_generation
[70]: https://cloud.google.com/blog/products/ai-machine-learning/introducing-gemini-enterprise-agent-platform?e=48754805
[71]: https://docs.cloud.google.com/gemini-enterprise-agent-platform/build/rag-engine/cross-corpus-retrieval
[72]: https://huggingface.co/learn/cookbook/multiagent_rag_system
[73]: https://research.google/blog/deeper-insights-into-retrieval-augmented-generation-the-role-of-sufficient-context/
[74]: https://bytebridge.medium.com/vanilla-rag-vs-agentic-rag-4d756ddb611f
[75]: https://huggingface.co/datasets/google/frames-benchmark
[76]: https://arxiv.org/abs/2409.12941
[77]: https://en.wikipedia.org/wiki/M*A*S*H_(TV_series)
[78]: https://en.wikipedia.org/wiki/Cheers
[79]: https://docs.cloud.google.com/vertex-ai/generative-ai/docs/rag-engine/rag-overview
[80]: https://docs.cloud.google.com/gemini-enterprise-agent-platform/build/rag-engine/cross-corpus-retrieval
[81]: /blog/label/data-management
[82]: /blog/label/machine-intelligence
[83]: /blog/label/natural-language-processing
[84]: /blog/label/product
[85]: https://docs.cloud.google.com/gemini-enterprise-agent-platform/build/rag-engine/cross-corpus-retrieval
[86]: /blog/accelerating-gemini-nano-models-on-pixel-with-frozen-multi-token-prediction/
[87]: /blog/optimizing-cloud-economics-with-linear-elastic-caching/
[88]: /blog/thinking-to-recall-how-reasoning-unlocks-parametric-knowledge-in-llms/
[89]: https://x.com/GoogleResearch
[90]: https://www.linkedin.com/showcase/googleresearch/
[91]: https://www.youtube.com/c/GoogleResearch
[92]: https://github.com/google-research
[93]: https://ai.google/products/
[94]: https://ai.google/build/
[95]: https://ai.google/research/
[96]: https://ai.google/public-policy-perspectives/
[97]: https://ai.google/societal-impact/
[98]: https://ai.google/our-ai-journey/?section=intro
[99]: https://cloud.google.com/
[100]: https://cloud.google.com/solutions
[101]: https://cloud.google.com/products
[102]: https://cloud.google.com/pricing
[103]: https://cloud.google.com/resources
[104]: https://deepmind.google/models/
[105]: https://deepmind.google/research/
[106]: https://deepmind.google/science/
[107]: https://deepmind.google/about/
[108]: https://labs.google/#about
[109]: https://labs.google/#experiments
[110]: https://labs.google/#stay-connected
[111]: https://about.google/
[112]: https://about.google/intl/en/products/
[113]: https://policies.google.com/privacy
[114]: https://policies.google.com/terms
```
