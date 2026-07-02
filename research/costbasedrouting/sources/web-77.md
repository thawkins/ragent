# Web source

- URL: https://itnext.io/making-smarter-model-choices-on-microsoft-foundry-848ff5760dab
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T15:44:02.027574875+00:00

```text
[Sitemap][1]
[Open in app][2]

Sign up

[Sign in][3]

Get app
[
Write
][4]
[
Search
][5]

Sign up

[Sign in][6]

[Unknown user]
[

## ITNEXT

][7]
·
[
[ITNEXT]
][8]

ITNEXT is a platform for IT developers & software engineers to share knowledge, connect, collaborate, learn and
experience next-gen technologies.

# Making Smarter Model Choices on Microsoft Foundry.

[
[Dave R - Microsoft Azure & AI MVP☁️]
][9]
[Dave R - Microsoft Azure & AI MVP☁️][10]
10 min read
·
Dec 10, 2025
[
][11]

--

[
][12]
[][13]
[

Listen

][14]

Share

A practical guide to model selection, offers, deployment patterns, and cost controls for large-scale agentic apps.

Press enter or click to view image in full size
Making Smarter Model Choices on Microsoft Foundry.

AI apps and agents are no longer science projects; they are production systems that must scale, meet SLAs, respect data
boundaries, and stay inside a budget.

This article* *walks through how Microsoft Foundry positions itself as an **AI app and agent factory** and how you can
pick the right models, offers, and deployment options for real-world workloads, and how customers like Manus and Wolters
Kluwer are wiring this together into production architectures.

Feel free to take a look at the Microsoft Ignite session related [here][15].

## The Platform Shift: From Single Prompts to Agentic Systems

Press enter or click to view image in full size
The Platform Shifts.

We’re in the middle of a **platform shift**: we’re moving from isolated prompt calls to **AI apps and agents** that
reason, plan, and act in long-running workflows.

### Microsoft Foundry — The AI App and Agent Factory

The following image shows **Microsoft Foundry** as *“the AI app and agent factory”*, with five core layers:

Press enter or click to view image in full size
Microsoft Foundry
* **Models** — Frontier, SLMs, multimodal, industry models, OSS, partner models.
* **Agent Service** — declarative and hosted agents, multi-agent workflows, built-in memory, channel integration, agent
  controls.
* **IQ (Knowledge)** — RAG, indexes, live and structured/unstructured data.
* **Tools** — prebuilt and MCP tools, business system connectors, custom tool catalogs.
* **Machine Learning** — evaluation, experimentation, and continuous improvement.

Underneath sits **Cloud + Edge** with **security, compliance, and governance** baked in. This is your mental reference
diagram: Foundry is not just “a model endpoint”; it is a full stack for agents.

## Microsoft Foundry — Build Anywhere, Deploy Anywhere

The following diagram illustrates how this stack appears in practice.

Press enter or click to view image in full size
Microsoft Foundry.

**Build anywhere**
* **Code-first**: SDKs, APIs, agent frameworks.
* **Design-first**: visual builders, prompt and agent design tools.

**Deploy anywhere**
* **Local / on-device** (via Foundry Local). [Microsoft Learn][16]
* **Cloud**, across regions and data zones.

On the **knowledge** side, Foundry provides:
* **One API for all RAG workflows** — unify how you ground on indexes, live data, and structured/unstructured content.
* **Agentic retrieval** — more than “vector search + LLM”, adding multi-step reasoning over knowledge.

On the **models** side, you can **access and compare over 11,000 models** and rely on **intelligent model routing** to
optimize performance in real time.

Finally, **tools + observability**:
* 1,400+ business system connections, MCP support, and a bring-your-own-tool catalog.
* Tracing, logging, evaluations, monitoring, and controls for quality, safety, security, cost, and throughput.
* Enterprise guardrails: network isolation, AI gateway, access controls, encryption, classification, policies, and
  compliance.

This is the baseline architecture you should mentally overlay on any agentic design you build.

## Anthropic Claude in Foundry Models — Best of Both Worlds

A key announcement in BRK195 is **Anthropic Claude in Foundry Models**: Sonnet 4.5, Opus 4.1, and Haiku 4.5 are
officially available through Microsoft Foundry.

Press enter or click to view image in full size
Anthropic Claude in Foundry Models
* **Trust-oriented** — low hallucination rate with citations and a bias toward saying *“I don’t know”* over guessing.
* **Agentic** — #1 on SWE-bench Verified for coding tasks; Sonnet 4.5 can compress *weeks* of software work into hours.
* **Enterprise-scale** — used across 8/10 top Fortune 500 companies, driving critical workflows.
* **MCP-native** — Anthropic pioneered the **Model Context Protocol** (MCP), which Foundry can leverage for rich tool
  and data integrations.

Within Foundry, Claude plugs into:
* Developer productivity via **GitHub Copilot** and multi-provider agent frameworks.
* Enterprise agent factories (Foundry) and end-user productivity surfaces (Microsoft 365 Copilot, Copilot Studio).

## Claude End-to-End: From Discovery to Deployed Agents

**Claude is integrated across every layer of Microsoft Foundry**.

Press enter or click to view image in full size
Claude in Microsoft Foundry.

The pipeline looks like this:
1. **Discover**
* Browse *Claude in Foundry Models* (Sonnet 4.5, Opus 4.1, Haiku 4.5).
* Compare capabilities, pricing, and regions.

**2. Experiment**
* Use **Agent Builder & Playground** to try prompts, evaluate outputs, and compare Claude against other models (e.g.,
  GPT-4.1, Grok, DeepSeek).

**3. Build**
* Use **Claude Code**, the **Anthropic SDK**, and the **Claude Agent SDK** to build agentic coding experiences, tools,
  and skills.

**4. Orchestrate**
* Run Claude in **Agent Service** for reasoning, planning, and action sequencing in multi-agent workflows.

**5. Optimize**
* Plug Claude into the **model router** for cost–performance optimization, automatic fallback, and intelligent routing
  as demand spikes.

**6. Extend**
* Attach **skills** (web search, web fetch, code execution, MCP tools) so Claude can act on real systems and knowledge
  bases.

Think of this as the “Claude lifecycle” in Foundry: from *“which Claude variant do I pick?”* to *“how do I route,
observe, and evolve my Claude-powered agents in production?”*

## What Is a General Agent? And Why Foundry Helps

The **Manus** customer spotlight introduces the idea of a **general agent**:

Press enter or click to view image in full size
What is a General Agent?
* Not limited to a single task; able to solve open-ended problems.
* Exhibits **agency** — it thinks, plans, and acts autonomously, not just waiting for commands.
* Gains power through **atomic capabilities**: content creation (images, video, audio, web pages), digital interaction
  (browser automation, file operations), software development, database ops, etc.

Microsoft Foundry fits this pattern because it combines:
* **Multimodal models** — text, image, video, audio.
* **Advanced tools** — MCP tools, business integrations, and RAG.
* **Enterprise-grade infra** — observability, SLAs, and governance.

As you design your own general agents, think of the following checklist:
1. Do I have the right **atomic capabilities** wired in as tools?
2. Can my agent **plan and act** across them (Agent Service)?
3. Do I have the right **models** and **routing** strategies for each step?

Press enter or click to view image in full size
Atomic Capabilities.

## The Model Catalog — 11,000+ Frontier & Open Models

The Model Catalog is the “candy store”: Foundry exposes **11,000+ models**, combining Azure OpenAI, DeepSeek, xAI Grok,
Mistral, Meta Llama, Phi SLM, and many others, plus industry-specific and partner models like Cohere and Anthropic.

Press enter or click to view image in full size
Find the best models for your AI Applications.

Key points:
* **All Foundry Models** share unified access, scalable deployment, and enterprise-ready controls.

**Sold directly by Microsoft** models come with:
* SLAs and compliance guarantees.
* Reserved capacity and cost-efficient PTU quotas.
* Safety guardrails on by default.

Cohere in **Direct from Azure** is highlighted for high-performance retrieval, classification, and generation workflows
(Command A, Embed 4, Rerank 3.5).

Press enter or click to view image in full size
Direct Models From Foundry

From a *practitioner* perspective, this slide answers: *“Can I keep my architecture stable while changing models over
time?”* The answer is yes — Foundry’s model inference APIs and router decouple model choice from the rest of the stack.

## Model Offerings — Standard, Priority Processing, Provisioned, Batch

One of the most cost-relevant sections is **Microsoft Foundry Model Offerings**. This is where you map performance,
latency, and cost to concrete offer types.

Press enter or click to view image in full size
Microsoft Foundry Model Offerings.

### Offers

**Standard**
* Recommended for: production workloads, development & testing, prototyping, PoCs.
* Good default when you don’t yet know traffic patterns.

**Priority Processing** (Public Preview)
* Premium, low-latency access with SLA-backed performance.
* Ideal for high-value, latency-sensitive workloads (conversational AI, real-time decisioning).
* Same model and same API — you can switch tiers without rewriting code.

**Provisioned**
* For throughput-heavy production workloads with predictable volume.
* Recommended for high-volume data processing and real-time workloads.

**Batch**
* For large-scale data processing and content generation.
* Great for offline workloads: content generation at scale, data transformation, evaluation runs.

Press enter or click to view image in full size
Chossing the best Offer.

Now let’s explore the Deployment Options.

## Deployment Options — Global, Data Zone, Regional

The following illustration gives you a residency and latency matrix.

Press enter or click to view image in full size
Choosing the best deployment.

**Global** — best when:
* You need consistent experience across many regions.
* Latency and cost matter, but data residency is flexible.

**Data Zones (EU / US)** — best when:
* You need **data residency** within a broader zone.
* You want cost savings and access to latest models.

**Regional** — best when:
* You need strict regional compliance and local processing.
* You want low latency by being physically close to users.

Foundry backs this with **99.9% reliability service-wide** and **99% latency SLA for Provisioned** offers, across 28
regions plus EU/US data zones.

## Cost & Performance Levers: Prompt Caching and Dynamic Spillover

Two features directly impact your cloud bill and reliability:

### Prompt Caching (GA)

Prompt caching targets **repeatable prompt patterns**.

Press enter or click to view image in full size
Prompt Caching.
* Faster time-to-first-token.
* Higher throughput via cache hits.
* Cached tokens are **50–100% cheaper**.

For production apps with “template-like” prompts (multi-step agents, chain-of-thought system prompts, evaluation flows),
you should systematically design around cacheable segments.

### Dynamic Spillover (GA)

Dynamic Spillover helps you **avoid 429s and downtime** by transparently spilling traffic to alternative capacity when
you hit throughput limits.

Press enter or click to view image in full size
Dynamic Spillover
* Keeps apps running during spikes.
* Lets you “load test in production” with more confidence.
* Supports cost-efficient scaling because you can combine Provisioned + spillover instead of massively
  over-provisioning.

Along with the model router and offers, these are your main tools for choosing models more intelligently and increasing
profitability.

## Customer Architectures

### Manus — Building a True General Agent

Manus showcases a **general agent** architecture using Foundry:
* A central agent with **agency** (plan/act), orchestrating across tools for content creation, browsing, file
  operations, and software development.
* Powered by multimodal models and enterprise tools from Foundry.
* Deployed on top of Foundry’s governance, observability, and AI gateway.

This is a template for large enterprises wanting “one agent frontend” to multiple lines of business without
re-implementing infrastructure each time.

### Wolters Kluwer — Clinical AI with Expert in the Loop

Wolters Kluwer’s architecture is a textbook example of **expert-in-the-loop** AI for regulated industries:

Press enter or click to view image in full size
Wolters Kluwer AI Platform.
* **Wolters Kluwer AI Platform** deals with security, governance, scalability, and adaptability.
* **AI User Experiences & Agents** sit on top, including **UpToDate Expert AI** for clinicians.

**Platform services and tools** integrate:
* Foundry Models
* AI Search
* 3rd-party integrations (Copilot, MCP, A2A)

Press enter or click to view image in full size
UpToDate Expert AI.

The **orchestration agent** coordinates four specialized agents:
* **Intent Agent** — understands clinician intent.
* **Plan Agent** — breaks intent into steps.
* **Retrieval Agent** — pulls evidence from UpToDate via AI Search.
* **Answer Agent** — synthesizes an expert-quality response.

All of this runs inside a **continuous improvement cycle**: engineers, early access users, and SMEs collaborate to
refine evaluation metrics and model/tool choices.

## Get Dave R - Microsoft Azure & AI MVP☁️’s stories in your inbox

Join Medium for free to get updates from this writer.

Subscribe
Subscribe

Remember me for faster sign in

This is the pattern you should mirror for any safety-critical vertical: clear agents, explainable flows, and humans in
the loop.

## A Practical Framework for “Smarter Model Choices”

Here’s a concrete decision playbook:

Press enter or click to view image in full size
Model selection simplified.
1. **Classify the workload**
* Is it interactive (chat, agents, copilots) or batch (summarization, evaluation, content generation)?
* Is it latency-sensitive or throughput-heavy?
* Is it general reasoning, coding, retrieval, multimodal, or industry-specific?

**2. Pick the model family**
* **Claude** for agentic coding, low hallucination, and strong reasoning.
* **Azure OpenAI / GPT-x** for broad ecosystem integration and multimodal patterns.
* **Cohere** for retrieval-heavy classification/search.
* **Other families** (DeepSeek, Grok, Phi, Mistral, etc.) when they align with your benchmarks or cost targets.

**3. Select the offer**
* **Standard** — default for most workloads (dev/prod with moderate traffic).
* **Priority Processing** — for high-value, latency-sensitive user experiences.
* **Provisioned** — for stable, high-volume workloads.
* **Batch** — for offline bulk processing and eval runs.

**4. Choose deployment type**
* **Global** if residency is flexible and you want broad reach.
* **Data Zone** if you need EU/US residency plus access to latest models.
* **Regional** where strict compliance and ultra-low latency are required.

**5. Add cost/perf controls**
* Enable **prompt caching** wherever prompts repeat.
* Use **Dynamic Spillover** to protect against 429s under load.
* Configure **model router** to experiment with and gradually migrate to better models.

**6. Instrument and iterate**
* Use Foundry’s tracing, logging, evaluation, and monitoring for continuous improvement.

## Final Thoughts

We’re past the stage where “use an LLM” is a strategy. Real-world AI systems live or die on the details: **which model
you choose, how you deploy it, how you govern it, and how fast you can evolve it**.

Microsoft Foundry isn’t just a place to try models; it’s an execution environment for **long-lived, agentic systems**
that have to earn their keep in production. Anthropic Claude, Azure OpenAI, Cohere, DeepSeek, Grok, Phi, Mistral, Llama
— individually, they’re impressive. Inside Foundry, with model routing, multiple offers (Standard, Priority,
Provisioned, Batch), prompt caching, and Dynamic Spillover, they become **replaceable, optimizable components** in a
larger architecture.

“Smarter model choices” is not a one-time decision; it’s an operating model.

Start small:
* Benchmark two or three models in the Foundry playground against your real prompts.
* Wire prompt caching into your most expensive flows.
* Turn on the model router and Dynamic Spillover for at least one critical path.
* Instrument everything with tracing, evals, and cost telemetry.

Then iterate. Treat your AI stack the way you treat your core application architecture: with discipline, visibility, and
an unapologetic focus on value.

If this deep dive helped you design (or rethink) your own Foundry architecture, consider saving it, sharing it with your
team, and following along.

[*-Dave R.*][17]

[
Artificial Intelligence
][18]
[
Machine Learning
][19]
[
Technology
][20]
[
Programming
][21]
[
Software Development
][22]
[
][23]

--

[
][24]

--

[
][25]
[][26]
[
[ITNEXT]
][27]
[
[ITNEXT]
][28]
[

## Published in ITNEXT

][29]
[82K followers][30]
·[Last published 2 hours ago][31]

ITNEXT is a platform for IT developers & software engineers to share knowledge, connect, collaborate, learn and
experience next-gen technologies.

[
[Dave R - Microsoft Azure & AI MVP☁️]
][32]
[
[Dave R - Microsoft Azure & AI MVP☁️]
][33]
[

## Written by Dave R - Microsoft Azure & AI MVP☁️

][34]
[1.2K followers][35]
·[15 following][36]

Hey 👋, Let’s connect — 𝕏 @DaveRndn — [https://www.linkedin.com/in/daverndn/][37]

[

Help

][38]
[

Status

][39]
[

About

][40]
[

Careers

][41]
[

Press

][42]
[

Blog

][43]
[

Store

][44]
[

Privacy

][45]
[

Rules

][46]
[

Terms

][47]
[

Text to speech

][48]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-micr
osoft-foundry-848ff5760dab&source=post_page---top_nav_layout_nav-----------------------global_nav------------------
[4]: https://medium.com/m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layo
ut_nav-----------------------new_post_topnav------------------
[5]: https://medium.com/search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-micr
osoft-foundry-848ff5760dab&source=post_page---top_nav_layout_nav-----------------------global_nav------------------
[7]: https://itnext.io/?source=post_page---publication_nav-5b301f10ddcd-848ff5760dab------------------------------------
---
[8]: https://itnext.io/?source=post_page---post_publication_sidebar-5b301f10ddcd-848ff5760dab---------------------------
------------
[9]: https://blog.azinsider.net/?source=post_page---byline--848ff5760dab---------------------------------------
[10]: https://blog.azinsider.net/?source=post_page---byline--848ff5760dab---------------------------------------
[11]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fitnext%2F848ff5760dab&operation=regist
er&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848ff5760dab&user=Dave+R+-+Micro
soft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---header_actions--848ff5760dab---------------------c
lap_footer------------------
[12]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F848ff5760dab&operation=register&
redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848ff5760dab&user=Dave+R+-+Microsof
t+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---header_actions--848ff5760dab---------------------repo
st_header------------------
[13]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F848ff5760dab&operation=registe
r&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848ff5760dab&source=---header_act
ions--848ff5760dab---------------------bookmark_footer------------------
[14]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D
848ff5760dab&operation=register&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848
ff5760dab&source=---header_actions--848ff5760dab---------------------post_audio_button------------------
[15]: https://mediusdl.event.microsoft.com/video-7530165/d1eeaba503/BRK195.mp4?sv=2018-03-28&sr=c&sig=314mM7vp9A8hf1OJAR
8piq27OEZxEgtNSMvfF5ukx7M%3D&se=2030-11-19T07%3A09%3A58Z&sp=r
[16]: https://learn.microsoft.com/en-us/azure/ai-foundry/foundry-local/get-started?view=foundry-classic&utm_source=chatg
pt.com
[17]: https://blog.azinsider.net/subscribe
[18]: https://medium.com/tag/artificial-intelligence?source=post_page-----848ff5760dab----------------------------------
-----
[19]: https://medium.com/tag/machine-learning?source=post_page-----848ff5760dab---------------------------------------
[20]: https://medium.com/tag/technology?source=post_page-----848ff5760dab---------------------------------------
[21]: https://medium.com/tag/programming?source=post_page-----848ff5760dab---------------------------------------
[22]: https://medium.com/tag/software-development?source=post_page-----848ff5760dab-------------------------------------
--
[23]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fitnext%2F848ff5760dab&operation=regist
er&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848ff5760dab&user=Dave+R+-+Micro
soft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---footer_actions--848ff5760dab---------------------c
lap_footer------------------
[24]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fitnext%2F848ff5760dab&operation=regist
er&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848ff5760dab&user=Dave+R+-+Micro
soft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---footer_actions--848ff5760dab---------------------c
lap_footer------------------
[25]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F848ff5760dab&operation=register&
redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848ff5760dab&user=Dave+R+-+Microsof
t+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---footer_actions--848ff5760dab---------------------repo
st_footer------------------
[26]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F848ff5760dab&operation=registe
r&redirect=https%3A%2F%2Fitnext.io%2Fmaking-smarter-model-choices-on-microsoft-foundry-848ff5760dab&source=---footer_act
ions--848ff5760dab---------------------bookmark_footer------------------
[27]: https://itnext.io/?source=post_page---post_publication_info--848ff5760dab---------------------------------------
[28]: https://itnext.io/?source=post_page---post_publication_info--848ff5760dab---------------------------------------
[29]: https://itnext.io/?source=post_page---post_publication_info--848ff5760dab---------------------------------------
[30]: /followers?source=post_page---post_publication_info--848ff5760dab---------------------------------------
[31]: /gym-service-building-a-grpc-powered-gym-management-api-with-spring-boot-4-bf52e2dd0c90?source=post_page---post_pu
blication_info--848ff5760dab---------------------------------------
[32]: https://blog.azinsider.net/?source=post_page---post_author_info--848ff5760dab-------------------------------------
--
[33]: https://blog.azinsider.net/?source=post_page---post_author_info--848ff5760dab-------------------------------------
--
[34]: https://blog.azinsider.net/?source=post_page---post_author_info--848ff5760dab-------------------------------------
--
[35]: https://blog.azinsider.net/followers?source=post_page---post_author_info--848ff5760dab----------------------------
-----------
[36]: https://medium.com/@daverendon/following?source=post_page---post_author_info--848ff5760dab------------------------
---------------
[37]: https://www.linkedin.com/in/daverndn/
[38]: https://help.medium.com/hc/en-us?source=post_page-----848ff5760dab---------------------------------------
[39]: https://status.medium.com/?source=post_page-----848ff5760dab---------------------------------------
[40]: https://medium.com/about?autoplay=1&source=post_page-----848ff5760dab---------------------------------------
[41]: https://medium.com/jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----848ff5760dab-------------------
--------------------
[42]: mailto:pressinquiries@medium.com
[43]: https://blog.medium.com/?source=post_page-----848ff5760dab---------------------------------------
[44]: https://medium.com/store
[45]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----848ff5760dab--------------------
-------------------
[46]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----848ff5760dab-----------------------------
----------
[47]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----848ff5760dab------------------
---------------------
[48]: https://speechify.com/medium?source=post_page-----848ff5760dab---------------------------------------
```
