# Web source

- URL: https://research.google/blog/accelerating-scientific-breakthroughs-with-an-ai-co-scientist
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-30T09:41:20.042948515+00:00

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
play silent looping video pause silent looping video
unmute video mute video
1. [Home][66]
2. [Blog][67]

# Accelerating scientific breakthroughs with an AI co-scientist

February 19, 2025

Juraj Gottweis, Google Fellow, and Vivek Natarajan, Research Lead

We introduce AI co-scientist, a multi-agent AI system built with Gemini 2.0 as a virtual scientific collaborator to help
scientists generate novel hypotheses and research proposals, and to accelerate the clock speed of scientific and
biomedical discoveries.

## Quick links
* [ AI co-scientist paper ][68]
* [ Gene transfer discovery paper ][69]
* [ Transfer re-discovery paper ][70]
* Share
  * Copy link
    ×

In the pursuit of scientific advances, researchers combine ingenuity and creativity with insight and expertise grounded
in literature to generate novel and viable research directions and to guide the exploration that follows. In many
fields, this presents a breadth and depth conundrum, since it is challenging to navigate the rapid growth in the rate of
scientific publications while integrating insights from unfamiliar domains. Yet overcoming such challenges is critical,
as evidenced by the many modern breakthroughs that have emerged from transdisciplinary endeavors. For example,
Emmanuelle Charpentier and Jennifer Doudna won the [2020 Nobel Prize in Chemistry][71] for their work on [CRISPR][72],
which combined expertise ranging from microbiology to genetics to molecular biology.

Motivated by unmet needs in the modern scientific discovery process and building on [recent AI advances][73], including
the ability to synthesize across complex subjects and to perform [long-term planning and reasoning][74], we developed an
[AI co-scientist system][75]. The AI co-scientist is a [multi-agent AI system][76] that is intended to function as a
collaborative tool for scientists. Built on [Gemini 2.0, AI co-scientist is][77] designed to mirror the reasoning
process underpinning the scientific method. Beyond standard literature review, summarization and “deep research” tools,
the AI co-scientist system is intended to uncover new, original knowledge and to formulate demonstrably novel research
hypotheses and proposals, building upon prior evidence and tailored to specific research objectives.

## Empowering scientists and accelerating discoveries with the AI co-scientist

Given a scientist’s research goal that has been specified in natural language, the AI co-scientist is designed to
generate novel research hypotheses, a detailed research overview, and experimental protocols. To do so, it uses a
coalition of specialized agents — Generation, Reflection, Ranking, Evolution, Proximity and Meta-review — that are
inspired by the scientific method itself. These agents use automated feedback to iteratively generate, evaluate, and
refine hypotheses, resulting in a self-improving cycle of increasingly high-quality and novel outputs.

play silent looping video pause silent looping video
unmute video mute video

AI co-scientist overview.

Purpose-built for collaboration, scientists can interact with the system in many ways, including by directly providing
their own seed ideas for exploration or by providing feedback on generated outputs in natural language. The AI
co-scientist also uses tools, like web-search and specialized AI models, to enhance the grounding and quality of
generated hypotheses.

[AICoScientist-1-Components]

Illustration of the different components in the AI co-scientist multi-agent system and the interaction paradigm between
the system and the scientist.

The AI co-scientist parses the assigned goal into a research plan configuration, managed by a Supervisor agent. The
Supervisor agent assigns the specialized agents to the worker queue and allocates resources. This design enables the
system to flexibly scale compute and to iteratively improve its scientific reasoning towards the specified research
goal.

[AICoScientist-2-Overview]

AI co-scientist system overview. Specialized agents (**red boxes**, with unique roles and logic); scientist input and
feedback (**blue boxes**); system information flow (**dark gray arrows**); inter-agent feedback (**red arrows** within
the agent section).

## Scaling test-time compute for advanced scientific reasoning

The AI co-scientist leverages [test-time compute][78] scaling to iteratively reason, evolve, and improve outputs. Key
reasoning steps include [self-play][79]–based scientific debate for novel hypothesis generation, ranking tournaments for
hypothesis comparison, and an "evolution" process for quality improvement. The system's agentic nature facilitates
recursive self-critique, including tool use for feedback to refine hypotheses and proposals.

The system's self-improvement relies on the [Elo][80] auto-evaluation metric derived from its tournaments. Due to their
core role, we assessed whether higher Elo ratings correlate with higher output quality. We analyzed the concordance
between Elo auto-ratings and [GPQA benchmark][81] accuracy on its diamond set of challenging questions, and we found
that higher Elo ratings positively correlate with a higher probability of correct answers.

[AICoScientist-3-Elo]

Average accuracy of the AI co-scientist (blue line) and reference Gemini 2.0 (red line) responses on GPQA diamond
questions, grouped by Elo rating. The Elo is an auto-evaluation and is not based on an independent ground truth.

Seven domain experts curated 15 open research goals and best guess solutions in their field of expertise. Using the
automated Elo metric we observed that the AI co-scientist outperformed other state-of-the-art agentic and reasoning
models for these complex problems. The analysis reproduced the benefits of scaling test-time compute using inductive
biases derived from the scientific method. As the system spends more time reasoning and improving, the self-rated
quality of results improve and surpass models and unassisted human experts.

[AICoScientist-4-BestHypothesis]
[AICoScientist-5-Top10Hypothesis]

Performance of the AI co-scientist improves as the system spends more time in computation. This can be seen in the
automated Elo metric gradually improving over other baselines. **Top:** Elo progression of the best rated hypothesis.
**Bottom:** Elo progression of the average of top-10 hypotheses.

On a smaller subset of 11 research goals, experts assessed the novelty and impact of the AI co-scientist–generated
results compared to other relevant baselines; they also provided overall preference. While the sample size was small,
experts assessed the AI co-scientist to have higher potential for novelty and impact, and preferred its outputs compared
to other models. Further, these human expert preferences also appeared to be concordant with the previously introduced
Elo auto-evaluation metric.

[AICoScientist-6-Novelty]
[AICoScientist-7-Ranking]

Human experts assessed the AI co-scientist results to have higher potential for novelty and impact (**left**) and
preferred it compared to other models (**right**).

## Validation of novel AI co-scientist hypotheses with real-world laboratory experiments

To assess the practical utility of the system’s novel predictions, we evaluated end-to-end laboratory experiments
probing the AI co-scientist–generated hypotheses and research proposals in three key biomedical applications: drug
repurposing, proposing novel treatment targets, and elucidating the mechanisms underlying antimicrobial resistance.
These settings all involved expert-in-the-loop guidance and spanned an array of complexities:

[AICoScientist-11-Table]

### Drug repurposing for acute myeloid leukaemia

Drug development is an [increasingly time-consuming and expensive process][82] in which new therapeutics require many
aspects of the discovery and development process to be restarted for each indication or disease. Drug repurposing
addresses this challenge by discovering new therapeutic applications for existing drugs beyond their original intended
use. But, due to the complexity of the task, it demands extensive interdisciplinary expertise.

We applied the AI co-scientist to assist with the prediction of drug repurposing opportunities and, with our partners,
validated predictions through computational biology, expert clinician feedback, and in vitro experiments.

Notably, the AI co-scientist proposed novel repurposing candidates for [acute myeloid leukemia][83] (AML). Subsequent
experiments validated these proposals, confirming that the suggested drugs inhibit tumor viability at clinically
relevant concentrations in multiple AML cell lines.

[AICoScientist-8-DoseResponse]

[Dose-response curves][84] of one of the three novel AI co-scientist–predicted AML repurposing drugs. KIRA6 inhibits
KG-1 (AML cell line) viability at clinically relevant concentrations. Being able to reduce cancer cell viability at
lower drug concentrations is advantageous for multiple reasons, e.g., as it reduces the potential for off-target side
effects.

### Advancing target discovery for liver fibrosis

Identifying novel treatment targets is more complex than drug repurposing, and often leads to inefficient hypothesis
selection and poor prioritization for in vitro and in vivo experiments. AI-assisted target discovery helps to streamline
the process of experimental validation, potentially helping to reduce development time costs.

We probed the AI co-scientist system's ability to propose, rank, and generate hypotheses and experimental protocols for
target discovery hypotheses, focusing on [liver fibrosis][85]. The AI co-scientist demonstrated its potential by
identifying epigenetic targets grounded in preclinical evidence with significant anti-fibrotic activity in [human
hepatic organoids][86] (3D, multicellular tissue cultures derived from human cells and designed to mimic the structure
and function of the human liver). These findings will be detailed in an upcoming report led by collaborators at Stanford
University.

[AICoScientist-9a-LiverFibrosis]

Comparison of treatments derived from AI co-scientist–suggested liver fibrosis targets versus a fibrosis inducer
(negative control) and an inhibitor (positive control). All treatments suggested by AI co-scientist show promising
activity (p-values for all suggested drugs are <0.01), including candidates that possibly reverse a disease phenotype.
Results are detailed in an upcoming report from our Stanford University collaborators.

### Explaining mechanisms of antimicrobial resistance

As a third validation, we focused on generating hypotheses to explain bacterial gene transfer evolution mechanisms
related to antimicrobial resistance (AMR) — microbes' evolved mechanisms to resist infection-treating drugs. This is
another complex challenge that involves understanding the molecular mechanisms of gene transfer ([conjugation][87],
[transduction][88], and [transformation][89]) alongside the ecological and evolutionary pressures that drive AMR genes
to spread.

For this test, expert researchers instructed the AI co-scientist to explore a topic that had already been subject to
novel discovery in their group, but had not yet been revealed in the public domain, namely, to explain how
[capsid-forming phage-inducible chromosomal islands][90] (cf-PICIs) exist across multiple bacterial species. The AI
co-scientist system independently proposed that cf-PICIs interact with diverse phage tails to expand their host range.
This in silico discovery, which had been experimentally validated in the original novel laboratory experiments performed
prior to use of the AI co-scientist system, are described in co-timed manuscripts ([1][91], [2][92]) with our
collaborators at the Fleming Initiative and Imperial College London. This illustrates the value of the AI co-scientist
system as an assistive technology, as it was able to leverage decades of research comprising all prior open access
literature on this topic.

[AICoScientist-10-RediscoveryTimeline]

Timeline of AI co-scientist re-discovery of a novel gene transfer mechanism. **Blue:** Experimental research pipeline
timeline for cf-PICI mobilization discovery. **Red:** AI co-scientist development and recapitulation of these key
findings (without prior knowledge).

## Limitations and outlook

In our report we address several limitations of the system and opportunities for improvement, including enhanced
literature reviews, factuality checking, cross-checks with external tools, auto-evaluation techniques, and larger-scale
evaluation involving more subject matter experts with varied research goals. The AI co-scientist represents a promising
advance toward AI-assisted technologies for scientists to help accelerate discovery. Its ability to generate novel,
testable hypotheses across diverse scientific and biomedical domains — some already validated experimentally — and its
capacity for recursive self-improvement with increased compute, demonstrate its potential to accelerate scientists'
efforts to address grand challenges in science and medicine. We look forward to responsible exploration of the potential
of the AI co-scientist as an assistive tool for scientists. This project illustrates how collaborative and human-centred
AI systems might be able to augment human ingenuity and accelerate scientific discovery.

## Announcing Trusted Tester access to the AI co-scientist system

We are excited by the early promise of the AI co-scientist system and believe it is important to evaluate its strengths
and limitations in science and biomedicine more broadly. To facilitate this responsibly we will be enabling access to
the system for research organizations through a Trusted Tester Program. We encourage interested research organizations
around the world to consider joining this program [here][93].

## Acknowledgements

The research described here is a joint effort between many Google Research, Google Deepmind and Google Cloud AI teams.
We thank our co-authors at Fleming Initiative and Imperial College London, Houston Methodist Hospital, Sequome, and
Stanford University — José R Penadés, Tiago R D Costa, Vikram Dhillon, Eeshit Dhaval Vaishnav, Byron Lee, Jacob Blum and
Gary Peltz. We appreciate Subhashini Venugopalan and Yun Liu for their detailed feedback on the manuscripts described
here. We are also grateful to the many incredible scientists across institutions providing detailed technical and expert
feedback — please refer to our report to see the voices and minds that aided this work. We also thank our teammates
Resham Parikh, Taylor Goddu, Siyi Kou, Rachelle Sico, Amanda Ferber, Cat Kozlowski, Alison Lentz, KK Walker, Roma
Ruparel, Jenn Sturgeon, Lauren Winer, Juanita Bawagan, Tori Milner, MK Blake, Kalyan Pamarthy for their support.
Finally, we also thank John Platt, Michael Brenner, Zoubin Ghahramani, Dale Webster, Joelle Barral, Michael Howell,
Susan Thomas, Jason Freidenfelds, Karen DeSalvo, Vladimir Vuskovic, Greg Corrado, Ronit Levavi Morad, Ali Eslami, Anna
Koivuniemi, Royal Hansen, Andy Berndt, Noam Shazeer, Oriol Vinyals, Burak Gokturk, Amin Vahdat, Katherine Chou, Avinatan
Hassidim, Koray Kavukcuoglu, Pushmeet Kohli, Yossi Matias, James Manyika, Jeff Dean and Demis Hassabis for their
support.
* Labels:
* [Generative AI][94]
* [Health & Bioscience][95]
* [Human-Computer Interaction and Visualization][96]

## Quick links
* [ AI co-scientist paper ][97]
* [ Gene transfer discovery paper ][98]
* [ Transfer re-discovery paper ][99]
* Share
  * Copy link
    ×

## Other posts of interest
* [
  
  June 24, 2026
  
  Thinking to recall: How reasoning unlocks parametric knowledge in LLMs
  * Generative AI ·
  * Machine Intelligence ·
  * Natural Language Processing
  ][100]
* [
  
  June 12, 2026
  
  Research into how AI can help users understand skin conditions
  * Health & Bioscience ·
  * Human-Computer Interaction and Visualization
  ][101]
* [
  
  June 4, 2026
  
  Towards passive heart health monitoring via smartphone camera
  * Health & Bioscience ·
  * Human-Computer Interaction and Visualization ·
  * Machine Intelligence
  ][102]

× ❮ ❯
[AICoScientist-3-Elo]
[AICoScientist-2-Overview]
[AICoScientist-11-Table]
[AICoScientist-5-Top10Hypothesis]
[AICoScientist-1-Components]
[AICoScientist-4-BestHypothesis]
[AICoScientist-9a-LiverFibrosis]
[AICoScientist-7-Ranking]
[AICoScientist-8-DoseResponse]
[AICoScientist-6-Novelty]
[AICoScientist-10-RediscoveryTimeline]

## Follow us
* [ ][103]
* [ ][104]
* [ ][105]
* [ ][106]

## Explore our other initiatives

### Google AI

Discover how Google AI is committed to enriching knowledge and solving complex challenges
* [
  Products
  ][107]
* [
  Build
  ][108]
* [
  Research
  ][109]
* [
  Responsibility
  ][110]
* [
  Societal Impact
  ][111]
* [
  About
  ][112]

### Google Cloud

High-performance infrastructure for cloud computing, data analytics & machine learning
* [
  Overview
  ][113]
* [
  Solutions
  ][114]
* [
  Products
  ][115]
* [
  Pricing
  ][116]
* [
  Resources
  ][117]

### Google DeepMind

Our mission is to build AI responsibly to benefit humanity
* [
  Models
  ][118]
* [
  Research
  ][119]
* [
  Science
  ][120]
* [
  About
  ][121]

### Google Labs

Explore the future of AI responsibly with Google Labs
* [
  About
  ][122]
* [
  Experiments
  ][123]
* [
  Stay connected
  ][124]
[ About Google ][125]
[ Google Products ][126]
[ Privacy ][127]
[ Terms ][128]
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
[68]: https://arxiv.org/abs/2502.18864
[69]: https://www.biorxiv.org/content/10.1101/2025.02.11.637232v1
[70]: https://www.cell.com/cell/fulltext/S0092-8674%2825%2900973-0?ref=sinodrugwatch.com
[71]: https://www.nobelprize.org/uploads/2020/10/popular-chemistryprize2020.pdf
[72]: https://en.wikipedia.org/wiki/CRISPR
[73]: https://arxiv.org/abs/2403.05530
[74]: https://gemini.google/overview/deep-research/
[75]: https://storage.googleapis.com/coscientist_paper/ai_coscientist.pdf
[76]: https://research.google/blog/towards-a-science-of-scaling-agent-systems-when-and-why-agent-systems-work/
[77]: https://blog.google/technology/google-deepmind/google-gemini-ai-update-december-2024/
[78]: https://arxiv.org/abs/2408.03314
[79]: https://deepmind.google/discover/blog/alphago-zero-starting-from-scratch/
[80]: https://en.wikipedia.org/wiki/Elo_rating_system
[81]: https://arxiv.org/abs/2311.12022
[82]: https://en.wikipedia.org/wiki/Eroom%27s_law
[83]: https://en.wikipedia.org/wiki/Acute_myeloid_leukemia
[84]: https://www.merckmanuals.com/professional/clinical-pharmacology/pharmacodynamics/dose-response-relationships
[85]: https://pmc.ncbi.nlm.nih.gov/articles/PMC546435/
[86]: https://pubmed.ncbi.nlm.nih.gov/28878125/
[87]: https://www.nature.com/scitable/definition/conjugation-prokaryotes-290/#:~:text=Conjugation%20is%20the%20process%2
0by,factor%2C%20or%20F%2Dfactor.
[88]: https://en.wikipedia.org/wiki/Transduction_(genetics)
[89]: https://en.wikipedia.org/wiki/Genetic_transformation
[90]: https://pubmed.ncbi.nlm.nih.gov/36596306/
[91]: https://www.biorxiv.org/content/10.1101/2025.02.11.637232v1
[92]: https://storage.googleapis.com/coscientist_paper/penades2025ai.pdf
[93]: https://docs.google.com/forms/d/e/1FAIpQLSdvw_8IPrc8O7ZM8FKF46i8BnOYMeSeyLeBNiuk_yGWIlnxYA/viewform
[94]: /blog/label/generative-ai
[95]: /blog/label/health-bioscience
[96]: /blog/label/human-computer-interaction-and-visualization
[97]: https://arxiv.org/abs/2502.18864
[98]: https://www.biorxiv.org/content/10.1101/2025.02.11.637232v1
[99]: https://www.cell.com/cell/fulltext/S0092-8674%2825%2900973-0?ref=sinodrugwatch.com
[100]: /blog/thinking-to-recall-how-reasoning-unlocks-parametric-knowledge-in-llms/
[101]: /blog/research-into-how-ai-can-help-users-understand-skin-conditions/
[102]: /blog/towards-passive-heart-health-monitoring-via-smartphone-camera/
[103]: https://x.com/GoogleResearch
[104]: https://www.linkedin.com/showcase/googleresearch/
[105]: https://www.youtube.com/c/GoogleResearch
[106]: https://github.com/google-research
[107]: https://ai.google/products/
[108]: https://ai.google/build/
[109]: https://ai.google/research/
[110]: https://ai.google/public-policy-perspectives/
[111]: https://ai.google/societal-impact/
[112]: https://ai.google/our-ai-journey/?section=intro
[113]: https://cloud.google.com/
[114]: https://cloud.google.com/solutions
[115]: https://cloud.google.com/products
[116]: https://cloud.google.com/pricing
[117]: https://cloud.google.com/resources
[118]: https://deepmind.google/models/
[119]: https://deepmind.google/research/
[120]: https://deepmind.google/science/
[121]: https://deepmind.google/about/
[122]: https://labs.google/#about
[123]: https://labs.google/#experiments
[124]: https://labs.google/#stay-connected
[125]: https://about.google/
[126]: https://about.google/intl/en/products/
[127]: https://policies.google.com/privacy
[128]: https://policies.google.com/terms
```
