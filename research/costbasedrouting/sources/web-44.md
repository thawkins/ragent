# Web source

- URL: https://www.linkedin.com/posts/brett-favro_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6
- Title: Agree & Join LinkedIn
- Captured (UTC): 2026-06-29T15:42:34.756569212+00:00

```text
Agree & Join LinkedIn

By clicking Continue to join or sign in, you agree to LinkedIn’s [User Agreement][1], [Privacy Policy][2], and [Cookie
Policy][3].

`` `` `` `` `` `` `` [ Skip to main content ][4] [ LinkedIn ][5]
* [ Top Content ][6]
* [ People ][7]
* [ Learning ][8]
* [ Jobs ][9]
* [ Games ][10]
[ Sign in ][11] [ Join now ][12]

# How to Optimize Prompt Routing in Multi-LLM Solutions

This title was summarized by AI from the post below.
[ Brett Favro ][13]
11mo
* [ Report this post ][14]

Prompt Routing in Multi-LLM Solutions: Prompt routing can range from a simple to complex orchestration of prompts to
models for optimal results.  Depending on your solution domain and task types, you likely require multiple models or
agents in your solution. One size seldom fits all and sending all prompts to the biggest model is wrong! Optimal results
mean achieving your user’s intended accurate outcomes and achieving optimal response times, balanced against cost.
Different models have varying token costs, so optimizing a solution’s run rate can be critical. Dynamic prompt routing
typically uses an LLM to analyze user-provided prompts to classify intent. With LLM assisted routing you first present a
multi-classification problem (assuming >2 classes) to an LLM. It responds with a predicted label or class for the
prompt. Based on this classification response, you then route the prompt to the best model for a final response. The
best model is the one meeting all your objectives. Your custom routing function encapsulates all objectives and
determines the model endpoint for the request. Modern frameworks allow us to implement prompt routing. For example, in
LangChain you create an LLMChain object for each relevant model. You then create a routing chain via RunnableLambda.
This determines the prompt’s classification by leveraging your custom routing function. The classification response
determines the “chain” to which the prompt gets routed.   Cloud providers have seized an opportunity here. They now
offer solutions for multi-LLM GenAI solutions. Their solutions allow you to externalize prompt routing, removing
potential software maintenance concerns inherent with codifying prompt routing in your solution. Using solutions like
Amazon Bedrock Intelligent Prompt Routing (GA) or Model Router for Azure AI Foundry (in preview), you can offload some
or all of this from your code. Intelligent Prompt Routing allows you to leverage preconfigured prompt routers or design
your own. Default prompt routers work within models in the Anthropic, Meta, or Amazon model families hosted on Bedrock.
 Configured prompt routers enable you to create hybrid routing solutions to leverage models outside Bedrock but still
within AWS. Within Azure AI Foundry, Model Router is a proxy model you deploy based on your use case. If you want Q&A,
then choosing the chat completion model router associates your requests with the gpt 4.1 models. Just like Intelligent
Prompt Router, Model Router acts as an intelligent middleman, routing to its associated models based on accuracy and
complexity requirements of user-entered prompts. Can all your objectives can be meet with these solutions? For a closer
look at each: Intelligent Prompt Routing: [https://lnkd.in/eSq27pEp][15] Model Router: [https://lnkd.in/ejw32kj5][16]

[ How to use model router for Azure AI Foundry (preview) learn.microsoft.com ][17]
`` ``
[ 3 ][18] `` `` `` `` `` `` `` [ 1 Comment ][19]
[ Like ][20] [ Comment ][21] `` ``
Share
* Copy
* LinkedIn
* Facebook
* X
[ Ram Perumalla ][22] 11mo
* [ Report this comment ][23]

Insightful, thanks for sharing

[
Like
][24] [
Reply
][25] 1 Reaction

To view or add a comment, [sign in][26]

``

## More Relevant Posts
* [ Dee Katauskas ][27]
  8mo
  * [ Report this post ][28]
  
  Just read a sharp breakdown of two powerful protocols shaping how AI interacts with tools and data. 🔍 • Model Context
  Protocol (MCP) gives LLMs human‑style discovery and adaptability — perfect when environments change.  • gRPC delivers
  lightning‑fast, high‑throughput communication — ideal when performance counts. The takeaway? Use MCP for flexibility
  and gRPC for speed — and let them play together for best‑in‑class AI systems. [https://lnkd.in/gZHCBzZA][29]
  [#GenerativeAI][30] [#CloudArchitecture][31] [#SolutionsArchitect][32] [#AWS][33] [#Azure][34]
  
  [ MCP vs gRPC : Comparing AI Protocols for Real-World Applications geeky-gadgets.com ][35]
  `` ``
  [ 1 ][36] `` `` `` `` `` `` ``
  [ Like ][37] [ Comment ][38] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][39]
  
  ``
* [ Karina Taveras ][40]
  8mo
  * [ Report this post ][41]
  
  🚨 IBM just dropped a bombshell that could reshape the entire AI landscape. While everyone's obsessing over ChatGPT
  and Claude, IBM quietly released Granite 3.0 – and the numbers are staggering. Here's what most people are missing: ✅
  These models OUTPERFORM Meta's Llama on key benchmarks ✅ They cost 3x-23x LESS than frontier models like GPT-4 ✅
  They're 100% open-source under Apache 2.0 license ✅ Built specifically for enterprise use (not consumer chatbots) The
  game-changer? IBM's new approach combines smaller, efficient models with enterprise data using their InstructLab
  technique. Result: Task-specific performance that rivals massive models at a fraction of the cost. What makes Granite
  3.0 different: 🔹 Granite Guardian 3.0 provides comprehensive safety guardrails 🔹 Mixture-of-Experts models enable
  CPU-based deployments 🔹 Time Series models outperform models 10x larger 🔹 Available on HuggingFace, Google Cloud,
  NVIDIA NIM, and more The real insight: While competitors chase bigger and more expensive models, IBM is proving that
  smarter beats bigger. This isn't just another AI release – it's a fundamental shift toward practical, cost-effective
  enterprise AI. For businesses tired of paying premium prices for overkill solutions, Granite 3.0 could be the answer.
  What's your take – will open-source, enterprise-focused models like Granite 3.0 challenge the dominance of
  consumer-focused AI giants?
  
  [ IBM Introduces Granite 3.0: High Performing AI Models Built for Business newsroom.ibm.com ][42]
  `` ``
  [ 1 ][43] `` `` `` `` `` `` ``
  [ Like ][44] [ Comment ][45] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][46]
  
  ``
* [ Evandro Lopes de Sousa ][47]
  9mo
  * [ Report this post ][48]
  
  Exciting news in the world of AI! Today, Microsoft released the Microsoft Agent Framework, a new open-source framework
  designed for building, orchestrating, and deploying AI agents. This marks a major step forward for developers,
  researchers, and AI enthusiasts looking to create smarter, more autonomous, and scalable solutions. I'm excited to
  explore its possibilities and start experimenting with this powerful tool! 🔍 If you work with AI or are curious about
  intelligent agents, this is definitely worth checking out. We’re living in an incredible time for technology!
  [#ArtificialIntelligence][49] [#MicrosoftAgentFramework][50] [#OpenSource][51] [#TechNews][52] [#AI][53]
  [#IntelligentAgents][54] [#Innovatio][55] [https://lnkd.in/deYZPDyw][56]
  
  [ Introducing Microsoft Agent Framework | Microsoft Azure Blog https://azure.microsoft.com/en-us/blog ][57]
  `` ``
  [ 19 ][58] `` `` `` `` `` `` ``
  [ Like ][59] [ Comment ][60] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][61]
  
  ``
* [ Uplatz ObserveIQ ][62]
  
  62 followers
  
  8mo
  * [ Report this post ][63]
  
  🤖 Observability for LLMs and AI Workloads: Monitoring the Unpredictable As enterprises scale AI and LLM-based
  workloads, they face a new challenge: You can’t monitor intelligence the same way you monitor infrastructure.
  Traditional observability works for predictable systems — fixed thresholds, repeatable patterns, consistent
  performance. AI systems? They behave probabilistically. One input can produce wildly different outputs depending on
  data, model state, or even prompt context. That’s why Uplatz ObserveIQ extends observability beyond servers and
  services to the AI cognition layer — bringing transparency, accountability, and control to LLM-driven actions. --- 🧠
  Why Monitoring AI Is Different AI models don’t just fail — they drift, degrade, and hallucinate. Their quality depends
  on factors traditional metrics ignore: 1. Inference latency (how fast responses are generated) 2. Response quality and
  consistency (semantic accuracy) 3. Data drift (how live data differs from training data) 4. GPU performance and cost
  per token These variables shift continuously, making reactive monitoring obsolete. --- 🔍 How ObserveIQ Solves It
  Uplatz ObserveIQ gives end-to-end visibility across AI pipelines — from ingestion to inference. 1️⃣ Model Metrics
  Observability – Tracks token latency, GPU utilization, and model throughput in real time. 📘: Detecting a latency
  spike in a chatbot’s inference layer and triggering automated GPU scaling. 2️⃣ Data & Concept Drift Detection –
  Identifies when live data diverges from training baselines. 📘: Spotting new fraud patterns post-holiday season before
  false negatives increase. 3️⃣ Output Quality Monitoring for LLMs – Evaluates factual accuracy, coherence, and toxicity
  using semantic validators. 📘: A summarization model starts producing vague outputs — ObserveIQ flags a prompt update
  as the root cause. 4️⃣ AI Cost Observability – Links inference volume, GPU time, and token cost to actual model
  performance. 📘: Detecting inefficient prompts consuming excessive context tokens in production. --- ⚙️ A Unified AI
  Observability Stack ObserveIQ integrates logs, traces, metrics, and AI telemetry in one intelligent platform. It
  unifies DevOps, MLOps, and DataOps — enabling teams to move from firefighting to foresight. ✅ Detect model decay
  early ✅ Correlate performance with cost and accuracy ✅ Automate alerting and remediation ✅ Build trust and
  explainability into AI systems --- 🔮 The Future: Trustworthy AI Through Observability In the age of LLMs and
  autonomous systems, observability equals reliability. You can’t secure, scale, or trust what you can’t see. With
  Uplatz ObserveIQ, organizations gain continuous visibility into both infrastructure and intelligence — ensuring every
  prediction, prompt, and decision is observable, explainable, optimizable. ──────────── Contact Us
  [support@uplatz.com][64] +44 7459 302492 ──────────── [#Uplatz][65] [#ObserveIQ][66] [#CloudNative][67] [#DataOps][68]
  [#ModelDrift][69] [#Observability][70] [#LLMMonitoring][71]
  * View C2PA information
  
  `` ``
  [ 2 ][72] `` `` `` `` `` `` ``
  [ Like ][73] [ Comment ][74] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][75]
  
  ``
* [ Kent Arnspiger ][76]
  8mo
  * [ Report this post ][77]
  
  New capabilities in Azure AI Foundry that make it easier for developers to build, observe, and govern multi-agent
  systems, while helping organizations close the trust gap in AI.
  
  [ Introducing Microsoft Agent Framework | Microsoft Azure Blog https://azure.microsoft.com/en-us/blog ][78]
  `` ``
  [ Like ][79] [ Comment ][80] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][81]
  
  ``
* [ Sebastian Iglesias ][82]
  8mo
  * [ Report this post ][83]
  
  Deep Research in Azure AI Foundry is an API and software development kit (SDK)-based offering that leverages OpenAI’s
  advanced agentic research capability, fully integrated with Azure’s enterprise-grade agentic platform. With Deep
  Research, developers have the ability to build agents that can deeply plan, analyze, and synthesize information from
  across the web. This tool automates complex research tasks, generates transparent and auditable outputs, and allows
  for the seamless composition of multi-step workflows with other tools and agents within Azure AI Foundry.
  
  [ Introducing Deep Research in Azure AI Foundry Agent Service | Microsoft Azure Blog
  https://azure.microsoft.com/en-us/blog ][84]
  `` ``
  [ 2 ][85] `` `` `` `` `` `` ``
  [ Like ][86] [ Comment ][87] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][88]
  
  ``
* [ Andrew Abrahams ][89]
  8mo
  * [ Report this post ][90]
  
  In October we’re announcing new capabilities in Azure AI Foundry that make it easier for developers to build, observe,
  and govern multi-agent systems, while helping organizations close the trust gap in AI. [https://lnkd.in/ebKk2zpj][91]
  
  [ Introducing Microsoft Agent Framework | Microsoft Azure Blog https://azure.microsoft.com/en-us/blog ][92]
  `` ``
  [ 2 ][93] `` `` `` `` `` `` ``
  [ Like ][94] [ Comment ][95] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][96]
  
  ``
* [ Jonathon Wells ][97]
  8mo
  * [ Report this post ][98]
  
  The hardest part of AI isn’t building the models—it’s making them work where you need, at a price you can afford.
  ✨Red Hat AI 3 tackles this challenge with efficient inference, simpler data access, and practical tools for
  delivering AI agents.
  
  [ Red Hat Brings Distributed AI Inference to Production AI Workloads with Red Hat AI 3 redhat.com ][99] View C2PA
  information
  `` ``
  [ 3 ][100] `` `` `` `` `` `` ``
  [ Like ][101] [ Comment ][102] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][103]
  
  ``
* [ Laure Nerdeux-Drouet ][104]
  8mo
  * [ Report this post ][105]
  
  The hardest part of AI isn’t building the models—it’s making them work where you need, at a price you can afford.
  ✨Red Hat AI 3 tackles this challenge with efficient inference, simpler data access, and practical tools for
  delivering AI agents.
  
  [ Red Hat Brings Distributed AI Inference to Production AI Workloads with Red Hat AI 3 redhat.com ][106] View C2PA
  information
  `` ``
  [ 2 ][107] `` `` `` `` `` `` ``
  [ Like ][108] [ Comment ][109] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][110]
  
  ``
* [ Paul Lancaster ][111]
  8mo
  * [ Report this post ][112]
  
  The hardest part of AI isn’t building the models—it’s making them work where you need, at a price you can afford.
  ✨Red Hat AI 3 tackles this challenge with efficient inference, simpler data access, and practical tools for
  delivering AI agents.
  
  [ Red Hat Brings Distributed AI Inference to Production AI Workloads with Red Hat AI 3 redhat.com ][113] View C2PA
  information
  `` ``
  [ Like ][114] [ Comment ][115] `` ``
  Share
  * Copy
  * LinkedIn
  * Facebook
  * X
  
  To view or add a comment, [sign in][116]
  
  ``

653 followers
* [ 14 Posts ][117]
* [ 1 Article ][118]

[ View Profile ][119] [ Follow ][120]

## More from this author
* [
  
  ### AI evaluation requires a product mindset:
  
  Brett Favro 1mo
  ][121]

## Explore related topics
* [How to Optimize Prompts for Improved Outcomes ][122]
* [LLM Frameworks for Multi-Model AI Solutions ][123]
* [How to Optimize AI Prompt Design ][124]
* [Preventing Prompt Issues in Large Language Models ][125]
* [Understanding LLM Self-Routing in Inference ][126]
* [LLM Routing Using Confidence Scoring Methods ][127]
* [Intelligent Query Routing for AWS Agent Workflows ][128]
* [How to Build a Two-Prompt LLM Workflow ][129]
* [Message Routing Strategies Using LLMs ][130]
* [Smart Task Routing Using AI ][131]

Show more Show less

## Explore content categories
* [Career][132]
* [Productivity][133]
* [Finance][134]
* [Soft Skills & Emotional Intelligence][135]
* [Project Management][136]
* [Education][137]
* [Technology][138]
* [Leadership][139]
* [Ecommerce][140]
* [User Experience][141]

Show more Show less
* LinkedIn © 2026
* [ About ][142]
* [ Accessibility ][143]
* [ User Agreement ][144]
* [ Privacy Policy ][145]
* [ Cookie Policy ][146]
* [ Copyright Policy ][147]
* [ Brand Policy ][148]
* [ Guest Controls ][149]
* [ Community Guidelines ][150]
* * العربية (Arabic)
  * বাংলা (Bangla)
  * Čeština (Czech)
  * Dansk (Danish)
  * Deutsch (German)
  * Ελληνικά (Greek)
  * **English (English)**
  * Español (Spanish)
  * فارسی (Persian)
  * Suomi (Finnish)
  * Français (French)
  * हिंदी (Hindi)
  * Magyar (Hungarian)
  * Bahasa Indonesia (Indonesian)
  * Italiano (Italian)
  * עברית (Hebrew)
  * 日本語 (Japanese)
  * 한국어 (Korean)
  * मराठी (Marathi)
  * Bahasa Malaysia (Malay)
  * Nederlands (Dutch)
  * Norsk (Norwegian)
  * ਪੰਜਾਬੀ (Punjabi)
  * Polski (Polish)
  * Português (Portuguese)
  * Română (Romanian)
  * Русский (Russian)
  * Svenska (Swedish)
  * తెలుగు (Telugu)
  * ภาษาไทย (Thai)
  * Tagalog (Tagalog)
  * Türkçe (Turkish)
  * Українська (Ukrainian)
  * Tiếng Việt (Vietnamese)
  * 简体中文 (Chinese (Simplified))
  * 正體中文 (Chinese (Traditional))
  Language

## Sign in to view more content

Create your free account or sign in to continue your search

`` `` `` `` `` `` `` `` `` ``
Email or phone
Password
Show
[Forgot password?][151] Sign in
Sign in with Email

or

New to LinkedIn? [Join now][152]

By clicking Continue to join or sign in, you agree to LinkedIn’s [User Agreement][153], [Privacy Policy][154], and
[Cookie Policy][155].

``

[1]: /legal/user-agreement?trk=linkedin-tc_auth-button_user-agreement
[2]: /legal/privacy-policy?trk=linkedin-tc_auth-button_privacy-policy
[3]: /legal/cookie-policy?trk=linkedin-tc_auth-button_cookie-policy
[4]: #main-content
[5]: /?trk=public_post_nav-header-logo
[6]: https://www.linkedin.com/top-content?trk=public_post_guest_nav_menu_topContent
[7]: https://www.linkedin.com/pub/dir/+/+?trk=public_post_guest_nav_menu_people
[8]: https://www.linkedin.com/learning/search?trk=public_post_guest_nav_menu_learning
[9]: https://www.linkedin.com/jobs/search?trk=public_post_guest_nav_menu_jobs
[10]: https://www.linkedin.com/games?trk=public_post_guest_nav_menu_games
[11]: https://www.linkedin.com/login?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favro_how-to-us
e-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&fromSignIn=true&trk=public_post_nav-header-signin
[12]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_nav-header-join
[13]: https://www.linkedin.com/in/brett-favro?trk=public_post_feed-actor-name
[14]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fbrett-favro_how-to-use-model-router-for-azure
-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guestReportConten
tType=POST&_f=guest-reporting
[15]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Flnkd%2Ein%2FeSq27pEp&urlhash=DEZh&trk=public_post-text
[16]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Flnkd%2Ein%2Fejw32kj5&urlhash=gFZR&trk=public_post-text
[17]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Flearn%2Emicrosoft%2Ecom%2Fen-us%2Fazure%2Fai-foundry%2Fo
penai%2Fhow-to%2Fmodel-router&urlhash=ab_1&trk=public_post_feed-article-content
[18]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_social-actions-reaction
s
[19]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_social-actions-comments
[20]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_like-cta
[21]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_comment-cta
[22]: https://www.linkedin.com/in/ram-perumalla-a161ba3?trk=public_post_comment_actor-name
[23]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fbrett-favro_how-to-use-model-router-for-azure
-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_comment_ellipsis-menu-semaphore-sign-in-redirect&guestRepo
rtContentType=COMMENT&_f=guest-reporting
[24]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_comment_like
[25]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_comment_reply
[26]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-favr
o_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_feed-cta-banner-cta
[27]: https://www.linkedin.com/in/dee-katauskas-cloud-architect?trk=public_post_feed-actor-name
[28]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fdee-katauskas-cloud-architect_mcp-vs-grpc-com
paring-ai-protocols-for-activity-7387539702796546048-4I92&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guest
ReportContentType=POST&_f=guest-reporting
[29]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Flnkd%2Ein%2FgZHCBzZA&urlhash=_65P&trk=public_post-text
[30]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fgener
ativeai&trk=public_post-text
[31]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fcloud
architecture&trk=public_post-text
[32]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fsolut
ionsarchitect&trk=public_post-text
[33]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Faws&t
rk=public_post-text
[34]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fazure
&trk=public_post-text
[35]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fwww%2Egeeky-gadgets%2Ecom%2Fmcp-vs-grpc-ai-protocols%2F&
urlhash=TrDa&trk=public_post_feed-article-content
[36]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fdee-katauskas-
cloud-architect_mcp-vs-grpc-comparing-ai-protocols-for-activity-7387539702796546048-4I92&trk=public_post_social-actions-
reactions
[37]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fdee-katauskas-
cloud-architect_mcp-vs-grpc-comparing-ai-protocols-for-activity-7387539702796546048-4I92&trk=public_post_like-cta
[38]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fdee-katauskas-
cloud-architect_mcp-vs-grpc-comparing-ai-protocols-for-activity-7387539702796546048-4I92&trk=public_post_comment-cta
[39]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fdee-katauskas-
cloud-architect_mcp-vs-grpc-comparing-ai-protocols-for-activity-7387539702796546048-4I92&trk=public_post_feed-cta-banner
-cta
[40]: https://www.linkedin.com/in/karinataveras?trk=public_post_feed-actor-name
[41]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkarinataveras_ibm-introduces-granite-30-high-
performing-activity-7386305327798992896-ixLk&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guestReportContent
Type=POST&_f=guest-reporting
[42]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fnewsroom%2Eibm%2Ecom%2F2024-10-21-ibm-introduces-granite
-3-0-high-performing-ai-models-built-for-business&urlhash=pROG&trk=public_post_feed-article-content
[43]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkarinataveras_
ibm-introduces-granite-30-high-performing-activity-7386305327798992896-ixLk&trk=public_post_social-actions-reactions
[44]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkarinataveras_
ibm-introduces-granite-30-high-performing-activity-7386305327798992896-ixLk&trk=public_post_like-cta
[45]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkarinataveras_
ibm-introduces-granite-30-high-performing-activity-7386305327798992896-ixLk&trk=public_post_comment-cta
[46]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkarinataveras_
ibm-introduces-granite-30-high-performing-activity-7386305327798992896-ixLk&trk=public_post_feed-cta-banner-cta
[47]: https://br.linkedin.com/in/evandro-lopes-de-sousa-a9013926?trk=public_post_feed-actor-name
[48]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fevandro-lopes-de-sousa-a9013926_introducing-m
icrosoft-agent-framework-microsoft-activity-7379277622997925888-GJtF&trk=public_post_ellipsis-menu-semaphore-sign-in-red
irect&guestReportContentType=POST&_f=guest-reporting
[49]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fartif
icialintelligence&trk=public_post-text
[50]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fmicro
softagentframework&trk=public_post-text
[51]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fopens
ource&trk=public_post-text
[52]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Ftechn
ews&trk=public_post-text
[53]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fai&tr
k=public_post-text
[54]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fintel
ligentagents&trk=public_post-text
[55]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Finnov
atio&trk=public_post-text
[56]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Flnkd%2Ein%2FdeYZPDyw&urlhash=LCx-&trk=public_post-text
[57]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fazure%2Emicrosoft%2Ecom%2Fen-us%2Fblog%2Fintroducing-mic
rosoft-agent-framework%2F&urlhash=E3YR&trk=public_post_feed-article-content
[58]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fevandro-lopes-
de-sousa-a9013926_introducing-microsoft-agent-framework-microsoft-activity-7379277622997925888-GJtF&trk=public_post_soci
al-actions-reactions
[59]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fevandro-lopes-
de-sousa-a9013926_introducing-microsoft-agent-framework-microsoft-activity-7379277622997925888-GJtF&trk=public_post_like
-cta
[60]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fevandro-lopes-
de-sousa-a9013926_introducing-microsoft-agent-framework-microsoft-activity-7379277622997925888-GJtF&trk=public_post_comm
ent-cta
[61]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fevandro-lopes-
de-sousa-a9013926_introducing-microsoft-agent-framework-microsoft-activity-7379277622997925888-GJtF&trk=public_post_feed
-cta-banner-cta
[62]: https://uk.linkedin.com/company/uplatz-observeiq?trk=public_post_feed-actor-name
[63]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fuplatz-observeiq_uplatz-observeiq-cloudnative
-activity-7383991403460182016-nawB&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guestReportContentType=POST&
_f=guest-reporting
[64]: https://www.linkedin.com/redir/redirect?url=mailto%3Asupport%40uplatz%2Ecom&urlhash=VkJd&trk=public_post-text
[65]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fuplat
z&trk=public_post-text
[66]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fobser
veiq&trk=public_post-text
[67]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fcloud
native&trk=public_post-text
[68]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fdatao
ps&trk=public_post-text
[69]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fmodel
drift&trk=public_post-text
[70]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fobser
vability&trk=public_post-text
[71]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Ffeed%2Fhashtag%2Fllmmo
nitoring&trk=public_post-text
[72]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fuplatz-observe
iq_uplatz-observeiq-cloudnative-activity-7383991403460182016-nawB&trk=public_post_social-actions-reactions
[73]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fuplatz-observe
iq_uplatz-observeiq-cloudnative-activity-7383991403460182016-nawB&trk=public_post_like-cta
[74]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fuplatz-observe
iq_uplatz-observeiq-cloudnative-activity-7383991403460182016-nawB&trk=public_post_comment-cta
[75]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fuplatz-observe
iq_uplatz-observeiq-cloudnative-activity-7383991403460182016-nawB&trk=public_post_feed-cta-banner-cta
[76]: https://www.linkedin.com/in/kentarnspiger?trk=public_post_feed-actor-name
[77]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkentarnspiger_introducing-microsoft-agent-fra
mework-microsoft-activity-7381334133530271744-95wd&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guestReportC
ontentType=POST&_f=guest-reporting
[78]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fazure%2Emicrosoft%2Ecom%2Fen-us%2Fblog%2Fintroducing-mic
rosoft-agent-framework%2F&urlhash=E3YR&trk=public_post_feed-article-content
[79]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkentarnspiger_
introducing-microsoft-agent-framework-microsoft-activity-7381334133530271744-95wd&trk=public_post_like-cta
[80]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkentarnspiger_
introducing-microsoft-agent-framework-microsoft-activity-7381334133530271744-95wd&trk=public_post_comment-cta
[81]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fkentarnspiger_
introducing-microsoft-agent-framework-microsoft-activity-7381334133530271744-95wd&trk=public_post_feed-cta-banner-cta
[82]: https://www.linkedin.com/in/sebastianiglesias?trk=public_post_feed-actor-name
[83]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fsebastianiglesias_introducing-deep-research-i
n-azure-ai-foundry-activity-7387153652597092352-EIOy&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guestRepor
tContentType=POST&_f=guest-reporting
[84]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fazure%2Emicrosoft%2Ecom%2Fen-us%2Fblog%2Fintroducing-dee
p-research-in-azure-ai-foundry-agent-service%2F&urlhash=Ovlf&trk=public_post_feed-article-content
[85]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fsebastianigles
ias_introducing-deep-research-in-azure-ai-foundry-activity-7387153652597092352-EIOy&trk=public_post_social-actions-react
ions
[86]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fsebastianigles
ias_introducing-deep-research-in-azure-ai-foundry-activity-7387153652597092352-EIOy&trk=public_post_like-cta
[87]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fsebastianigles
ias_introducing-deep-research-in-azure-ai-foundry-activity-7387153652597092352-EIOy&trk=public_post_comment-cta
[88]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fsebastianigles
ias_introducing-deep-research-in-azure-ai-foundry-activity-7387153652597092352-EIOy&trk=public_post_feed-cta-banner-cta
[89]: https://www.linkedin.com/in/andrew-abrahams-a254511?trk=public_post_feed-actor-name
[90]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fandrew-abrahams-a254511_introducing-microsoft
-agent-framework-microsoft-activity-7380982232540286976-jqFR&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&gu
estReportContentType=POST&_f=guest-reporting
[91]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Flnkd%2Ein%2FebKk2zpj&urlhash=rtAR&trk=public_post-text
[92]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fazure%2Emicrosoft%2Ecom%2Fen-us%2Fblog%2Fintroducing-mic
rosoft-agent-framework%2F&urlhash=E3YR&trk=public_post_feed-article-content
[93]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fandrew-abraham
s-a254511_introducing-microsoft-agent-framework-microsoft-activity-7380982232540286976-jqFR&trk=public_post_social-actio
ns-reactions
[94]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fandrew-abraham
s-a254511_introducing-microsoft-agent-framework-microsoft-activity-7380982232540286976-jqFR&trk=public_post_like-cta
[95]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fandrew-abraham
s-a254511_introducing-microsoft-agent-framework-microsoft-activity-7380982232540286976-jqFR&trk=public_post_comment-cta
[96]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fandrew-abraham
s-a254511_introducing-microsoft-agent-framework-microsoft-activity-7380982232540286976-jqFR&trk=public_post_feed-cta-ban
ner-cta
[97]: https://www.linkedin.com/in/jonathon-wells-54880292?trk=public_post_feed-actor-name
[98]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fjonathon-wells-54880292_red-hat-brings-distri
buted-ai-inference-to-activity-7384604052988440576-JkjF&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guestRe
portContentType=POST&_f=guest-reporting
[99]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fwww%2Eredhat%2Ecom%2Fen%2Fabout%2Fpress-releases%2Fred-h
at-brings-distributed-ai-inference-production-ai-workloads-red-hat-ai-3&urlhash=TokH&trk=public_post_feed-article-conten
t
[100]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fjonathon-well
s-54880292_red-hat-brings-distributed-ai-inference-to-activity-7384604052988440576-JkjF&trk=public_post_social-actions-r
eactions
[101]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fjonathon-well
s-54880292_red-hat-brings-distributed-ai-inference-to-activity-7384604052988440576-JkjF&trk=public_post_like-cta
[102]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fjonathon-well
s-54880292_red-hat-brings-distributed-ai-inference-to-activity-7384604052988440576-JkjF&trk=public_post_comment-cta
[103]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fjonathon-well
s-54880292_red-hat-brings-distributed-ai-inference-to-activity-7384604052988440576-JkjF&trk=public_post_feed-cta-banner-
cta
[104]: https://fr.linkedin.com/in/laure-nerdeux-drouet-987627a?trk=public_post_feed-actor-name
[105]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Flaure-nerdeux-drouet-987627a_red-hat-brings-
distributed-ai-inference-to-activity-7383866545413791744-KvLj&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&g
uestReportContentType=POST&_f=guest-reporting
[106]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fwww%2Eredhat%2Ecom%2Fen%2Fabout%2Fpress-releases%2Fred-
hat-brings-distributed-ai-inference-production-ai-workloads-red-hat-ai-3&urlhash=TokH&trk=public_post_feed-article-conte
nt
[107]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Flaure-nerdeux
-drouet-987627a_red-hat-brings-distributed-ai-inference-to-activity-7383866545413791744-KvLj&trk=public_post_social-acti
ons-reactions
[108]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Flaure-nerdeux
-drouet-987627a_red-hat-brings-distributed-ai-inference-to-activity-7383866545413791744-KvLj&trk=public_post_like-cta
[109]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Flaure-nerdeux
-drouet-987627a_red-hat-brings-distributed-ai-inference-to-activity-7383866545413791744-KvLj&trk=public_post_comment-cta
[110]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Flaure-nerdeux
-drouet-987627a_red-hat-brings-distributed-ai-inference-to-activity-7383866545413791744-KvLj&trk=public_post_feed-cta-ba
nner-cta
[111]: https://www.linkedin.com/in/paullancaster?trk=public_post_feed-actor-name
[112]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fpaullancaster_red-hat-brings-distributed-ai-
inference-to-activity-7384267570394165248-bwXw&trk=public_post_ellipsis-menu-semaphore-sign-in-redirect&guestReportConte
ntType=POST&_f=guest-reporting
[113]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fwww%2Eredhat%2Ecom%2Fen%2Fabout%2Fpress-releases%2Fred-
hat-brings-distributed-ai-inference-production-ai-workloads-red-hat-ai-3&urlhash=TokH&trk=public_post_feed-article-conte
nt
[114]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fpaullancaster
_red-hat-brings-distributed-ai-inference-to-activity-7384267570394165248-bwXw&trk=public_post_like-cta
[115]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fpaullancaster
_red-hat-brings-distributed-ai-inference-to-activity-7384267570394165248-bwXw&trk=public_post_comment-cta
[116]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fposts%2Fpaullancaster
_red-hat-brings-distributed-ai-inference-to-activity-7384267570394165248-bwXw&trk=public_post_feed-cta-banner-cta
[117]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fin%2Fbrett-favro%
2Frecent-activity%2F&trk=public_post_follow-posts
[118]: https://www.linkedin.com/today/author/brett-favro?trk=public_post_follow-articles
[119]: https://www.linkedin.com/in/brett-favro?trk=public_post_follow-view-profile
[120]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Ffeed%2Fupdate%2Fu
rn%3Ali%3Aactivity%3A7352061757151924229&trk=public_post_follow
[121]: https://www.linkedin.com/pulse/ai-evaluation-requires-product-mindset-brett-favro-yt0xe?trk=public_post
[122]: https://www.linkedin.com/top-content/artificial-intelligence/ai-prompt-improvement/how-to-optimize-prompts-for-im
proved-outcomes/
[123]: https://www.linkedin.com/top-content/artificial-intelligence/ai-model-development/llm-frameworks-for-multi-model-
ai-solutions/
[124]: https://www.linkedin.com/top-content/artificial-intelligence/ai-prompt-improvement/how-to-optimize-ai-prompt-desi
gn/
[125]: https://www.linkedin.com/top-content/artificial-intelligence/ai-prompt-improvement/preventing-prompt-issues-in-la
rge-language-models/
[126]: https://www.linkedin.com/top-content/artificial-intelligence/understanding-ai-systems/understanding-llm-self-rout
ing-in-inference/
[127]: https://www.linkedin.com/top-content/technology/llm-system-optimization/llm-routing-using-confidence-scoring-meth
ods/
[128]: https://www.linkedin.com/top-content/artificial-intelligence/ai-workflow-enhancement/intelligent-query-routing-fo
r-aws-agent-workflows/
[129]: https://www.linkedin.com/top-content/project-management/optimizing-workflow-processes/how-to-build-a-two-prompt-l
lm-workflow/
[130]: https://www.linkedin.com/top-content/supply-chain-management/llm-security-management/message-routing-strategies-u
sing-llms/
[131]: https://www.linkedin.com/top-content/productivity/using-ai-for-task-management/smart-task-routing-using-ai/
[132]: https://www.linkedin.com/top-content/career/
[133]: https://www.linkedin.com/top-content/productivity/
[134]: https://www.linkedin.com/top-content/finance/
[135]: https://www.linkedin.com/top-content/soft-skills-emotional-intelligence/
[136]: https://www.linkedin.com/top-content/project-management/
[137]: https://www.linkedin.com/top-content/education/
[138]: https://www.linkedin.com/top-content/technology/
[139]: https://www.linkedin.com/top-content/leadership/
[140]: https://www.linkedin.com/top-content/ecommerce/
[141]: https://www.linkedin.com/top-content/user-experience/
[142]: https://about.linkedin.com?trk=d_public_post_footer-about
[143]: https://www.linkedin.com/accessibility?trk=d_public_post_footer-accessibility
[144]: https://www.linkedin.com/legal/user-agreement?trk=d_public_post_footer-user-agreement
[145]: https://www.linkedin.com/legal/privacy-policy?trk=d_public_post_footer-privacy-policy
[146]: https://www.linkedin.com/legal/cookie-policy?trk=d_public_post_footer-cookie-policy
[147]: https://www.linkedin.com/legal/copyright-policy?trk=d_public_post_footer-copyright-policy
[148]: https://brand.linkedin.com/policies?trk=d_public_post_footer-brand-policy
[149]: https://www.linkedin.com/psettings/guest-controls?trk=d_public_post_footer-guest-controls
[150]: https://www.linkedin.com/legal/professional-community-policies?trk=d_public_post_footer-community-guide
[151]: https://www.linkedin.com/uas/request-password-reset?trk=csm-v2_forgot_password
[152]: https://www.linkedin.com/signup/cold-join?session_redirect=https%3A%2F%2Fwww%2Elinkedin%2Ecom%2Fposts%2Fbrett-fav
ro_how-to-use-model-router-for-azure-ai-foundry-activity-7352061757151924229-M9G6&trk=public_post_contextual-sign-in-mod
al_join-link
[153]: /legal/user-agreement?trk=linkedin-tc_auth-button_user-agreement
[154]: /legal/privacy-policy?trk=linkedin-tc_auth-button_privacy-policy
[155]: /legal/cookie-policy?trk=linkedin-tc_auth-button_cookie-policy
```
