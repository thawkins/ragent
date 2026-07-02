# Web source

- URL: https://www.microsoft.com/en-us/research/blog/mmctagent-enabling-multimodal-reasoning-over-large-video-and-image-collections
- Title: [Skip to main content][1] [ [Microsoft] ][2] [ Research ][3] [Publications][4] [Code & data][5] [People][6] [Microsoft
- Captured (UTC): 2026-06-30T09:39:26.225838531+00:00

```text
[Skip to main content][1] [ [Microsoft] ][2] [ Research ][3] [Publications][4] [Code & data][5] [People][6] [Microsoft
Research blog][7] [Artificial intelligence][8] [Audio & acoustics][9] [Computer vision][10] [Graphics & multimedia][11]
[Human-computer interaction][12] [Human language technologies][13] [Search & information retrieval][14] [Data platforms
and analytics][15] [Hardware & devices][16] [Programming languages & software engineering][17] [Quantum computing][18]
[Security, privacy & cryptography][19] [Systems & networking][20] [Algorithms][21] [Mathematics][22] [Ecology &
environment][23] [Economics][24] [Medical, health & genomics][25] [Social sciences][26] [Technology for emerging
markets][27] [Academic programs][28] [Events & academic conferences][29] [Microsoft Research Forum][30] [Behind the Tech
podcast][31] [Microsoft Research blog][32] [Microsoft Research Forum][33] [Microsoft Research podcast][34] [About
Microsoft Research][35] [Careers & internships][36] [People][37] [Emeritus program][38] [News & awards][39] [Microsoft
Research newsletter][40] [Africa][41] [AI for Science][42] [AI Frontiers][43] [Asia-Pacific][44] [Cambridge][45] [Health
Futures][46] [India][47] [Montreal][48] [New England][49] [New York City][50] [Redmond][51] [Applied Sciences][52]
[Mixed Reality & AI - Cambridge][53] [Mixed Reality & AI - Zurich][54] [ Register: Research Forum ][55] [Microsoft
Security][56] [Azure][57] [Dynamics 365][58] [Microsoft 365][59] [Microsoft Teams][60] [Windows 365][61] [Microsoft
AI][62] [Azure Space][63] [Mixed reality][64] [Microsoft HoloLens][65] [Microsoft Viva][66] [Quantum computing][67]
[Sustainability][68] [Education][69] [Automotive][70] [Financial services][71] [Government][72] [Healthcare][73]
[Manufacturing][74] [Retail][75] [Find a partner][76] [Become a partner][77] [Partner Network][78] [Microsoft
Marketplace][79] [Software companies][80] [Blog][81] [Microsoft Advertising][82] [Developer Center][83]
[Documentation][84] [Events][85] [Licensing][86] [Microsoft Learn][87] [Microsoft Research][88] [View Sitemap][89]
[ Return to Blog Home ][90]

## Microsoft Research Blog

# MMCTAgent: Enabling multimodal reasoning over large video and image collections

Published November 12, 2025

By [ Akshay Nambi ][91] , Principal Researcher [ Kavyansh Chourasia ][92] , Research SDE 2 [ Tanuja Ganu ][93] ,
Director of Research Engineering

Share this page
* [ Share on Facebook ][94]
* [ Share on X ][95]
* [ Share on LinkedIn ][96]
* [ Share on Reddit ][97]
* [ Subscribe to our RSS feed ][98]

[Three white icons on a blue-to-purple gradient background: the first icon shows an image/photo; the second icon depicts
a computer monitor with vertical bars; the third icon displays three connected circles with user silhouettes.]

Modern multimodal AI models can recognize objects, describe scenes, and answer questions about images and short video
clips, but they struggle with long-form and large-scale visual data, where real-world reasoning requires moving beyond
object recognition and short-clip analysis.

Real-world reasoning increasingly involves analyzing long-form video content, where context spans minutes or hours, far
beyond the context limits of most models. It also entails querying across massive multimodal libraries of videos,
images, and transcripts, where finding and integrating relevant evidence requires more than retrieval—it requires
strategic reasoning. Existing models typically perform single-pass inference, producing one-shot answers. This limits
their ability to handle tasks that require temporal reasoning, cross-modal grounding, and iterative refinement.

## MMCTAgent

To meet these challenges, we developed the [Multi-modal Critical Thinking Agent][99], or MMCTAgent, for structured
reasoning over long-form video and image data, available on [GitHub (opens in new tab)][100] and featured on [Azure AI
Foundry Labs (opens in new tab)][101].

Built on [AutoGen][102], Microsoft’s open-source multi-agent system, MMCTAgent provides multimodal question-answering
with a Planner–Critic architecture. This design enables planning, reflection, and tool-based reasoning, bridging
perception and deliberation in multimodal tasks. It links language, vision, and temporal understanding, transforming
static multimodal tasks into dynamic reasoning workflows.  

Unlike conventional models that produce one-shot answers, MMCTAgent has modality-specific agents, including ImageAgent
and VideoAgent, which include tools like get_relevant_query_frames() or object_detection-tool(). These agents
perform deliberate, iterative reasoning—selecting the right tools for each modality, evaluating intermediate results,
and refining conclusions through a Critic loop. This enables MMCTAgent to analyze complex queries across long videos and
large image libraries with explainability, extensibility, and scalability.

[MMCTAgent on Azure AI Foundry Labs][103]

Spotlight: Microsoft research newsletter

## Microsoft Research Newsletter

Stay connected to the research community at Microsoft.

[ Subscribe today ][104]
Opens in a new tab

## How MMCTAgent works

MMCTAgent integrates two coordinated agents, Planner and Critic, orchestrated through AutoGen. The Planner agent
decomposes a user query, identifies the appropriate reasoning tools, performs multimodal operations, and drafts a
preliminary answer. The Critic agent reviews the Planner’s reasoning chain, validates evidence alignment, and refines or
revises the response for factual accuracy and consistency.

This iterative reasoning loop enables MMCTAgent to improve its answers through structured self-evaluation—bringing
reflection into AI reasoning. A key strength of MMCTAgent lies in its modular extensibility. Developers can easily
integrate new, domain-specific tools—such as medical image analyzers, industrial inspection models, or specialized
retrieval modules—by adding them to ImageQnATools or VideoQnATools. This design makes MMCTAgent adaptable across
domains.

### VideoAgent: From ingestion to long-form multimodal reasoning

[MMCTAgent’s Planner–Critic architecture enables multimodal reasoning over long-form video through structured ingestion,
retrieval, and iterative feedback. ]Figure 1. MMCTAgent’s Planner–Critic architecture enables multimodal reasoning over
long-form video through structured ingestion, retrieval, and iterative feedback

The VideoAgent extends this architecture to long-form video reasoning. It operates in two connected phases: library
creation (ingestion) and query-time reasoning.

#### Phase 1 – Video ingestion and library creation

Before reasoning, long-form videos undergo an ingestion pipeline that aligns multimodal information for retrieval and
understanding:
1. **Transcription **and** translation**: Converts audio to text and, if multilingual, translates transcripts into a
   consistent language 
2. **Key-frame identification**: Extracts representative frames marking major visual or scene changes
3. **Semantic chunking **and** chapter generation**: Combines transcript segments and visual summaries into coherent,
   semantically segmented chapters with associated key frames. Inspired by Microsoft’s [Deep Video Discovery agentic
   search tool][105], this step also extracts detailed descriptions of objects, on-screen text, and characters present
   within each video segment, integrating these insights directly into the corresponding chapters. 
4. **Multimodal embedding creation**: Generates image embeddings for key frames, linking them to their corresponding
   transcript and chapter data

All structured metadata, including transcripts, visual summaries, chapters, and embeddings, is indexed in the Multimodal
Knowledgebase using [Azure AI Search (opens in new tab)][106], which forms the foundation for scalable semantic
retrieval and downstream reasoning.

#### Phase 2 – Video question answering and reasoning

When a user submits a query, the VideoAgent retrieves, analyzes, and reasons across the indexed video content using
specialized planner and critic tools.

##### Planner tools
* **get_video_analysis**: Finds the most relevant video, provides a summary, and lists detected objects 
* **get_context**: Retrieves contextual information and relevant chapters from the Azure AI Search index
* **get_relevant_frames**: Selects key frames most relevant to the user query
* **query_frame**: Performs detailed visual and textual reasoning over selected frames
* **get_context** and **get_relevant_frames** work in tandem to ensure that reasoning begins from the most semantically
  relevant evidence

##### Critic tool
* **critic_tool**: Evaluates the reasoning output for temporal alignment, factual accuracy, and coherence between visual
  and textual modalities

This two-phase design, which involves structured ingestion followed by agentic reasoning, enables MMCTAgent to deliver
accurate, interpretable insights for long information-dense videos. 

### ImageAgent: Structured reasoning for static visuals

While the VideoAgent handles temporal reasoning across long-form videos, the ImageAgent applies the same Planner–Critic
paradigm to static visual analysis. It performs modular, tool-based reasoning over images, combining perception tools
for recognition, detection, and optical character recognition with language-based reasoning for interpretation and
explanation.

##### Planner tools
* **vit_tool**: Leverages Vision Transformer (ViT) or Vision Languague Model (VLM) for high-level visual understanding
  and description 
* **recog_tool**: Performs scene, face, and object recognition
* **object_detection_tool**: Localizes and labels entities within an image
* **ocr_tool**: Extracts embedded text from visual elements

##### Critic tool
* **critic_tool**: Validates the Planner’s conclusions for factual alignment and consistency, refining the final
  response 

This lightweight ImageAgent provides fine-grained, explainable reasoning over image collections—supporting visual
question answering, content inspection, and multimodal retrieval—while maintaining architectural symmetry with the
VideoAgent.

## Evaluation Results 

To assess the effectiveness of MMCTAgent, we evaluated both the ImageAgent and VideoAgent with multiple base LLM models
and a range of benchmark datasets and real-world scenarios. Some key results are presented here. 

──────────────┬──────┬────────────────┬─────┬────────────────┬─────┬───────────────
Image Datasets│GPT-4V│MMCT with GPT-4V│GPT4o│MMCT with GPT-4o│GPT-5│MMCT with GPT-5
──────────────┼──────┼────────────────┼─────┼────────────────┼─────┼───────────────
MM-Vet [1]    │60.20 │74.24           │77.98│79.36           │80.51│81.65          
──────────────┼──────┼────────────────┼─────┼────────────────┼─────┼───────────────
MMMU [2]      │56.80 │63.57           │69.10│73.00           │84.20│85.44          
──────────────┴──────┴────────────────┴─────┴────────────────┴─────┴───────────────

──────────────┬─────┬────────────────
Video Datasets│GPT4o│MMCT with GPT-4o
──────────────┼─────┼────────────────
VideoMME [3]  │72.10│76.70           
──────────────┴─────┴────────────────

MMCTAgent enhances base model performance by augmenting their capabilities with appropriate tools such as object
detection and optical character recognition (OCR) for weaker models, or domain-specific tools for stronger models,
thereby leading to substantial improvements. For example, integrating these tools raised GPT-4V’s accuracy from 60.20%
to 74.24% on MM-Vet dataset. Additionally, the configurable Critic agent provides additional validation, which is
especially valuable in critical domains. The additional evaluation results are available [here (opens in new tab)][107].

## Takeaways and next steps

MMCTAgent demonstrates a scalable agentic approach to multimodal reasoning with a Planner–Critic architecture. Its
unified multimodal design supports both image and video pipelines, while the extensible toolchain enables rapid
integration of domain-specific tools and capabilities. It provides Azure-native deployment and supports configurability
within the broader open-source ecosystem.

Looking ahead, we aim to improve efficiency and adaptability in retrieval and reasoning workflows, and to extend
MMCTAgent’s applications beyond current agricultural evaluations, exploring new real-world domains through initiatives
like [Project Gecko][108] to advance the creation of accessible, innovative multimodal applications for people around
the globe. 

## Acknowledgements

We would like to thank our team members for their valuable contributions to this work: Aman Patkar, [Ogbemi
Ekwejunor-Etchie][109], Somnath Kumar, Soumya De, and Yash Gadhia.

**References****** 

[1] W. Yu, Z. Yang, L. Li, J. Wang, K. Lin, Z. Liu, X. Wang, and L. Wang. “MM-VET: Evaluating large multimodal models
for integrated capabilities”, 2023. 

[2] X. Yue, Y. Ni, K. Zhang, T. Zheng, R. Liu, G. Zhang, S. Stevens, D. Jiang, W. Ren, Y. Sun, C. Wei, B. Yu, R. Yuan,
R. Sun, M. Yin, B. Zheng, Z. Yang, Y. Liu, W. Huang, H. Sun, Y. Su, and W. Chen. “MMMU: A massive multi-discipline
multimodal understanding and reasoning benchmark for expert AGI”, 2023. 

[3] Chaoyou Fu, Yuhan Dai, Yondong Luo, Lei Li, Shuhuai Ren, Renrui Zhang, Zihan Wang, Chenyu Zhou, Yunhang Shen,
Mengdan Zhang, et al. “Video-MME: The first-ever comprehensive evaluation benchmark of multi-modal llms in video
analysis”, 2024. 

Opens in a new tab

## Related publications

### [MMCTAgent: Multi-modal Critical Thinking Agent Framework for Complex Visual Reasoning ][110]

## Meet the authors

[Portrait of Akshay Nambi]

### Akshay Nambi

Principal Researcher

[Learn more][111]
[Portrait of Kavyansh Chourasia]

### Kavyansh Chourasia

Research SDE 2

[Learn more][112]
[Portrait of Tanuja Ganu]

### Tanuja Ganu

Director of Research Engineering

[Learn more][113]

## Research Areas
* [ Artificial intelligence ][114]

## Related Tools
* [ MMCTAgent ][115]

## Related projects
* [ Project Gecko ][116]

## Related labs
* [ Microsoft Research Lab - India ][117]

## Related stories
* [ Advancing AI to meet needs of the global majority ][118]

Follow us:
* [ Follow on X ][119]
* [ Like on Facebook ][120]
* [ Follow on LinkedIn ][121]
* [ Subscribe on Youtube ][122]
* [ Follow on Instagram ][123]
* [ Subscribe to our RSS feed ][124]

Share this page:
* [ Share on X ][125]
* [ Share on Facebook ][126]
* [ Share on LinkedIn ][127]
* [ Share on Reddit ][128]

[Surface Pro][129] [Surface Laptop][130] [Surface Laptop Ultra][131] [Surface RTX Spark Dev Box][132] [Copilot for
organizations][133] [Copilot for personal use][134] [Explore Microsoft products][135] [Windows 11 apps][136] [Account
profile][137] [Download Center][138] [Microsoft Store support][139] [Returns][140] [Order tracking][141] [Certified
Refurbished][142] [Microsoft Store Promise][143] [Flexible Payments][144] [Microsoft in education][145] [Devices for
education][146] [Microsoft Teams for Education][147] [Microsoft 365 Education][148] [How to buy for your school][149]
[Educator training and development][150] [Deals for students and parents][151] [AI for education][152]
[Microsoft AI][153] [Microsoft Security][154] [Dynamics 365][155] [Microsoft 365][156] [Microsoft Power Platform][157]
[Microsoft Teams][158] [Microsoft 365 Copilot][159] [Small Business][160] [Azure][161] [Microsoft Developer][162]
[Microsoft Learn][163] [Support for AI marketplace apps][164] [Microsoft Tech Community][165] [Microsoft
Marketplace][166] [Software companies][167] [Visual Studio][168] [Careers][169] [About Microsoft][170] [Company
news][171] [Privacy at Microsoft][172] [Investors][173] [Diversity and inclusion][174] [Accessibility][175]
[Sustainability][176]
[ [Your Privacy Choices Opt-Out Icon] Your Privacy Choices ][177] [ [Your Privacy Choices Opt-Out Icon] Your Privacy
Choices ][178]
[Consumer Health Privacy][179] [Sitemap][180] [Contact Microsoft][181] [Privacy ][182] [Manage cookies][183] [Terms of
use][184] [Trademarks][185] [Safety & eco][186] [Recycling][187] [About our ads][188]

[1]: 
[2]: https://www.microsoft.com
[3]: /en-us/research/
[4]: /en-us/research/publications/
[5]: /en-us/research/tools/
[6]: /en-us/research/people/
[7]: /en-us/research/blog/
[8]: /en-us/research/focus-area/ai-and-microsoft-research/
[9]: /en-us/research/research-area/audio-acoustics/
[10]: /en-us/research/research-area/computer-vision/
[11]: /en-us/research/research-area/graphics-and-multimedia/
[12]: /en-us/research/research-area/human-computer-interaction/
[13]: /en-us/research/research-area/human-language-technologies/
[14]: /en-us/research/research-area/search-information-retrieval/
[15]: /en-us/research/research-area/data-platform-analytics/
[16]: /en-us/research/research-area/hardware-devices/
[17]: /en-us/research/research-area/programming-languages-software-engineering/
[18]: /en-us/research/research-area/quantum/
[19]: /en-us/research/research-area/security-privacy-cryptography/
[20]: /en-us/research/research-area/systems-and-networking/
[21]: /en-us/research/research-area/algorithms/
[22]: /en-us/research/research-area/computational-sciences-mathematics/
[23]: /en-us/research/research-area/ecology-environment/
[24]: /en-us/research/research-area/economics/
[25]: /en-us/research/research-area/medical-health-genomics/
[26]: /en-us/research/research-area/social-sciences/
[27]: /en-us/research/research-area/technology-for-emerging-markets/
[28]: /en-us/research/academic-programs/
[29]: /en-us/research/events-conferences/
[30]: https://researchforum.microsoft.com
[31]: https://www.microsoft.com/en-us/behind-the-tech 
[32]: /en-us/research/blog
[33]: https://researchforum.microsoft.com
[34]: /en-us/research/podcast/
[35]: /en-us/research/about-microsoft-research/
[36]: /en-us/research/careers/
[37]: /en-us/research/people/
[38]: /en-us/research/microsoft-research-emeritus-program/
[39]: /en-us/research/news-and-awards/
[40]: https://info.microsoft.com/ww-landing-microsoft-research-newsletter.html?wt.mc_id=S-webpage_msr-homepage
[41]: /en-us/research/lab/microsoft-research-lab-africa-nairobi/
[42]: /en-us/research/lab/microsoft-research-ai-for-science/
[43]: /en-us/research/lab/ai-frontiers/
[44]: /en-us/research/lab/microsoft-research-asia/
[45]: /en-us/research/lab/microsoft-research-cambridge/
[46]: /en-us/research/lab/microsoft-health-futures/
[47]: /en-us/research/lab/microsoft-research-india/
[48]: /en-us/research/lab/microsoft-research-montreal/
[49]: /en-us/research/lab/microsoft-research-new-england/
[50]: /en-us/research/lab/microsoft-research-new-york/
[51]: /en-us/research/lab/microsoft-research-redmond/
[52]: /en-us/research/lab/applied-sciences-group/
[53]: /en-us/research/lab/mixed-reality-ai-lab-cambridge/
[54]: /en-us/research/lab/mixed-reality-ai-zurich/
[55]: https://researchforum.microsoft.com
[56]: https://www.microsoft.com/en-us/security
[57]: https://azure.microsoft.com/en-us/
[58]: https://dynamics.microsoft.com/en-us/
[59]: https://www.microsoft.com/en-us/microsoft-365/business/
[60]: https://www.microsoft.com/en-us/microsoft-teams/group-chat-software
[61]: https://www.microsoft.com/en-us/windows-365
[62]: https://www.microsoft.com/en-us/ai?icid=DSM_AllCommercial_AI
[63]: https://azure.microsoft.com/en-us/solutions/space/
[64]: https://www.microsoft.com/en-us/mixed-reality/windows-mixed-reality
[65]: https://www.microsoft.com/en-us/hololens
[66]: https://www.microsoft.com/en-us/microsoft-viva
[67]: https://azure.microsoft.com/en-us/solutions/quantum-computing/
[68]: https://www.microsoft.com/en-us/corporate-responsibility/sustainability?icid=DSM_AllCommercial_Sustainability
[69]: https://www.microsoft.com/en-us/education
[70]: https://www.microsoft.com/en-us/industry/automotive
[71]: https://www.microsoft.com/en-us/industry/financial-services/banking
[72]: https://www.microsoft.com/en-us/industry/government
[73]: https://www.microsoft.com/en-us/industry/health/microsoft-cloud-for-healthcare
[74]: https://www.microsoft.com/en-us/industry/manufacturing/microsoft-cloud-for-manufacturing
[75]: https://www.microsoft.com/en-us/industry/consumer-goods
[76]: https://partner.microsoft.com/en-US/
[77]: https://partner.microsoft.com/en-US/membership/cloud-solution-provider
[78]: https://partner.microsoft.com/en-us/membership
[79]: https://marketplace.microsoft.com?icid=DSM_AllCommercial_Marketplace&ocid=cmm3c8ee9bs
[80]: https://www.microsoft.com/software-development-companies?icid=DSM_AllCommercial_SoftwareCompanies&ocid=cmm3c8ee9bs
[81]: https://blogs.microsoft.com/
[82]: https://about.ads.microsoft.com/en-us?s_cid=dig-src_uhfcomm
[83]: https://developer.microsoft.com/en-us/
[84]: https://learn.microsoft.com/docs/
[85]: https://www.microsoft.com/en-us/events
[86]: https://www.microsoft.com/en-us/licensing/
[87]: https://learn.microsoft.com/
[88]: https://www.microsoft.com/en-us/research/
[89]: https://www.microsoft.com/en-us/sitemap
[90]: https://www.microsoft.com/en-us/research/blog/
[91]: https://www.microsoft.com/en-us/research/people/akshayn/
[92]: https://www.microsoft.com/en-us/research/people/kchourasia/
[93]: https://www.microsoft.com/en-us/research/people/taganu/
[94]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fmmctagent
-enabling-multimodal-reasoning-over-large-video-and-image-collections%2F
[95]:  			https://x.com/intent/tweet?text=MMCTAgent%3A%20Enabling%20multimodal%20reasoning%20over%20large%20video%20and%20i
mage%20collections&url=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fmmctagent-enabling-multimodal-reasoni
ng-over-large-video-and-image-collections%2F			
[96]:  			https://www.linkedin.com/shareArticle?mini=true&url=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fm
mctagent-enabling-multimodal-reasoning-over-large-video-and-image-collections%2F&title=MMCTAgent%3A%20Enabling%20multimo
dal%20reasoning%20over%20large%20video%20and%20image%20collections&summary=MMCTAgent%3A%20Enabling%20multimodal%20reason
ing%20over%20large%20video%20and%20image%20collections&source=Microsoft%20Research			
[97]:  			http://www.reddit.com/submit?title=MMCTAgent%3A%20Enabling%20multimodal%20reasoning%20over%20large%20video%20and%
20image%20collections&url=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fmmctagent-enabling-multimodal-reas
oning-over-large-video-and-image-collections%2F			
[98]: https://www.microsoft.com/en-us/research/feed/
[99]: https://www.microsoft.com/en-us/research/publication/mmctagent-multi-modal-critical-thinking-agent-framework-for-c
omplex-visual-reasoning/?msockid=153992cb7df169482b9487167c0968e9
[100]: https://github.com/microsoft/MMCTAgent
[101]: https://labs.ai.azure.com/projects/mmct-agent/
[102]: https://www.microsoft.com/en-us/research/project/autogen
[103]: https://labs.ai.azure.com/projects/mmct-agent/
[104]: https://info.microsoft.com/ww-landing-microsoft-research-newsletter.html
[105]: https://www.microsoft.com/en-us/research/publication/deep-video-discovery-agentic-search-with-tool-use-for-long-f
orm-video-understanding/
[106]: https://learn.microsoft.com/en-us/azure/search/search-what-is-azure-search
[107]: https://github.com/microsoft/MMCTAgent/blob/main/EVALUATIONS.md
[108]: https://www.microsoft.com/en-us/research/project/project-gecko
[109]: https://www.microsoft.com/en-us/research/people/ogbemie
[110]: https://www.microsoft.com/en-us/research/publication/mmctagent-multi-modal-critical-thinking-agent-framework-for-
complex-visual-reasoning/
[111]: https://www.microsoft.com/en-us/research/people/akshayn/
[112]: https://www.microsoft.com/en-us/research/people/kchourasia/
[113]: https://www.microsoft.com/en-us/research/people/taganu/
[114]: https://www.microsoft.com/en-us/research/research-area/artificial-intelligence/
[115]: https://labs.ai.azure.com/projects/mmct-agent/
[116]: https://www.microsoft.com/en-us/research/project/project-gecko/
[117]: https://www.microsoft.com/en-us/research/lab/microsoft-research-india/
[118]: https://www.microsoft.com/en-us/research/story/advancing-ai-to-meet-needs-of-the-global-majority/
[119]: https://x.com/intent/follow?original_referrer=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fmmctage
nt-enabling-multimodal-reasoning-over-large-video-and-image-collections%2F&screen_name=MSFTResearch
[120]: https://www.facebook.com/microsoftresearch/
[121]: https://www.linkedin.com/showcase/microsoftresearch/
[122]: https://www.youtube.com/user/MicrosoftResearch
[123]: https://www.instagram.com/msft_research/
[124]: https://www.microsoft.com/en-us/research/feed/
[125]: https://x.com/intent/tweet?text=MMCTAgent%3A%20Enabling%20multimodal%20reasoning%20over%20large%20video%20and%20i
mage%20collections&url=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fmmctagent-enabling-multimodal-reasoni
ng-over-large-video-and-image-collections%2F
[126]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fmmctagen
t-enabling-multimodal-reasoning-over-large-video-and-image-collections%2F
[127]:  									https://www.linkedin.com/shareArticle?mini=true&url=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2F
mmctagent-enabling-multimodal-reasoning-over-large-video-and-image-collections%2F&title=MMCTAgent%3A%20Enabling%20multim
odal%20reasoning%20over%20large%20video%20and%20image%20collections&summary=MMCTAgent%3A%20Enabling%20multimodal%20reaso
ning%20over%20large%20video%20and%20image%20collections&source=Microsoft%20Research									
[128]:  									http://www.reddit.com/submit?title=MMCTAgent%3A%20Enabling%20multimodal%20reasoning%20over%20large%20video%20and
%20image%20collections&url=https%3A%2F%2Fwww.microsoft.com%2Fen-us%2Fresearch%2Fblog%2Fmmctagent-enabling-multimodal-rea
soning-over-large-video-and-image-collections%2F									
[129]: https://www.microsoft.com/surface/devices/surface-pro
[130]: https://www.microsoft.com/surface/devices/surface-laptop
[131]: https://www.microsoft.com/en-us/surface/devices/surface-laptop-ultra?icid=DSM_Footer_WhatsNew_SurfaceLaptopUltra
[132]: https://www.microsoft.com/en-us/surface/devices/surface-rtx-spark-dev-box?icid=DSM_Footer_WhatsNew_SurfaceRTXSpar
kDevBox
[133]: https://www.microsoft.com/en-us/microsoft-copilot/organizations?icid=DSM_Footer_CopilotOrganizations
[134]: https://www.microsoft.com/en-us/microsoft-copilot/for-individuals?form=MY02PT&OCID=GE_web_Copilot_Free_868g3t5nj
[135]: https://www.microsoft.com/en-us/microsoft-products-and-apps
[136]: https://www.microsoft.com/en-us/windows/apps-for-windows?icid=DSM_Footer_WhatsNew_Windows11apps
[137]: https://account.microsoft.com/
[138]: https://www.microsoft.com/en-us/download
[139]: https://go.microsoft.com/fwlink/?linkid=2139749
[140]: https://www.microsoft.com/en-us/store/b/returns
[141]: https://www.microsoft.com/en-us/store/b/order-tracking
[142]: https://www.microsoft.com/en-us/store/b/certified-refurbished-products
[143]: https://www.microsoft.com/en-us/store/b/why-microsoft-store?icid=footer_why-msft-store_7102020
[144]: https://www.microsoft.com/en-us/store/b/payment-financing-options?icid=footer_financing_vcc
[145]: https://www.microsoft.com/en-us/education
[146]: https://www.microsoft.com/en-us/education/devices/overview
[147]: https://www.microsoft.com/en-us/education/products/teams
[148]: https://www.microsoft.com/en-us/education/products/microsoft-365
[149]: https://www.microsoft.com/education/how-to-buy
[150]: https://education.microsoft.com/
[151]: https://www.microsoft.com/en-us/store/b/education
[152]: https://www.microsoft.com/en-us/education/ai-in-education
[153]: https://www.microsoft.com/en-us/ai?icid=DSM_Footer_AI
[154]: https://www.microsoft.com/en-us/security
[155]: https://www.microsoft.com/en-us/dynamics-365
[156]: https://www.microsoft.com/en-us/microsoft-365/business
[157]: https://www.microsoft.com/en-us/power-platform
[158]: https://www.microsoft.com/en-us/microsoft-teams/group-chat-software
[159]: https://www.microsoft.com/en-us/microsoft-365-copilot?icid=DSM_Footer_Microsoft365Copilot
[160]: https://www.microsoft.com/en-us/store/b/business?icid=CNavBusinessStore
[161]: https://azure.microsoft.com/en-us/
[162]: https://developer.microsoft.com/en-us/
[163]: https://learn.microsoft.com/
[164]: https://www.microsoft.com/software-development-companies/offers-benefits/isv-success?icid=DSM_Footer_SupportAIMar
ketplace&ocid=cmm3atxvn98
[165]: https://techcommunity.microsoft.com/
[166]: https://marketplace.microsoft.com?icid=DSM_Footer_Marketplace&ocid=cmm3atxvn98
[167]: https://www.microsoft.com/software-development-companies?icid=DSM_Footer_SoftwareCompanies&ocid=cmm3atxvn98
[168]: https://visualstudio.microsoft.com/
[169]: https://careers.microsoft.com/
[170]: https://www.microsoft.com/about
[171]: https://news.microsoft.com/source/?icid=DSM_Footer_Company_CompanyNews
[172]: https://www.microsoft.com/en-us/privacy?icid=DSM_Footer_Company_Privacy
[173]: https://www.microsoft.com/investor/default.aspx
[174]: https://www.microsoft.com/en-us/diversity/default?icid=DSM_Footer_Company_Diversity
[175]: https://www.microsoft.com/en-us/accessibility
[176]: https://www.microsoft.com/en-us/corporate-responsibility/sustainability?icid=DSM_Footer_Sustainability
[177]: https://aka.ms/yourcaliforniaprivacychoices
[178]: https://aka.ms/yourcaliforniaprivacychoices
[179]: https://go.microsoft.com/fwlink/?linkid=2259814
[180]: https://www.microsoft.com/en-us/sitemap1.aspx
[181]: https://support.microsoft.com/contactus
[182]: https://go.microsoft.com/fwlink/?LinkId=521839
[183]: #
[184]: https://go.microsoft.com/fwlink/?LinkID=206977
[185]: https://go.microsoft.com/fwlink/?linkid=2196228
[186]: https://go.microsoft.com/fwlink/?linkid=2196227
[187]: https://www.microsoft.com/en-us/legal/compliance/recycling
[188]: https://choice.microsoft.com
```
