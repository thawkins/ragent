# Web source

- URL: https://itnext.io/stop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddead6
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T15:41:35.932400205+00:00

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

# Stop Wasting Tokens: How to Design High-ROI AI Apps on Microsoft Foundry.

[
[Dave R - Microsoft Azure & AI MVP☁️]
][9]
[Dave R - Microsoft Azure & AI MVP☁️][10]
12 min read
·
Dec 1, 2025
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

A practical guide to cutting latency, controlling token costs, and scaling Azure AI agents without breaking your budget.

Press enter or click to view image in full size
Stop Wasting Tokens: How to Design High-ROI AI Apps on Microsoft Foundry.

Enterprises have moved beyond “Can we ship a chatbot?” to a harder question: **“Is this AI actually creating value?”**

The Microsoft Ignite 2025 session titled *“Maximizing ROI with Azure AI Foundry”* (now **Microsoft Foundry**) walks
through exactly that: how to use Foundry’s architecture, capacity SKUs, and new platform features — **model router,
Priority Processing, PTU with Dynamic Spillover, and prompt caching** — to turn AI experiments into durable,
cost-efficient products.

This article distills that talk, the accompanying slides, and current Microsoft documentation into a **deep-dive,
practitioner-focused guide** on:
* How the **Microsoft Foundry architecture** is structured (Models, Agent Service, IQ, Tools, Machine Learning, Control
  Plane).
* The **Foundry ROI Framework** and what each pillar means in real systems.
* How **Standard, Priority Processing, PTU, and Batch** offerings map to latency, cost, and scale requirements.
* Real-world stories from **Adobe Acrobat, Ada, and eClinicalWorks** and how they measure ROI.

If you’re designing agents or AI applications on Azure today, this is the mental model you want before you touch a
single PTU slider.

You can watch the session here: [https://ignite.microsoft.com/en-US/sessions/BRK187?source=sessions][15]

## 1. Microsoft Foundry in One Slide: The AI App and Agent Factory

The architecture slide in the session renames **Azure AI Foundry** to **Microsoft Foundry — “The AI app and agent
factory”** and lays out five main building blocks sitting on top of a control plane:

Press enter or click to view image in full size
Microsoft Foundry
* **Models**
* **Agent Service**
* **IQ**
* **Tools**
* **Machine Learning**
* Underneath: a unified **Control Plane**, spanning **Cloud ↔ Edge**
* Across everything: **Security, compliance, and governance**

The official Microsoft description of Foundry mirrors this: a unified environment that brings together model catalog,
orchestration, data, tools, and governance to build AI apps and agents end-to-end.

Let’s break down each block.

### Models

The **Models** tile represents the **model catalog**: first-party GPT-5 family, Phi, and other Azure models, plus
third-party and open-source models exposed through Foundry. You can:
* Browse models and capabilities.
* Configure deployments (Standard, Priority, PTU, Batch).
* Attach safety, logging, and cost controls.

This is the foundation for **model router** and capacity planning decisions later.

### Agent Service

**Agent Service** is where you turn “one big prompt” into **composable agents**:
* Define agents with goals, tools, and memory.
* Orchestrate multi-step workflows across tools, RAG, and other services.
* Host agents behind APIs that your apps call.

Think of it as the runtime that executes your *agentic* flows, not just single LLM calls.

### Foundy IQ

Unifies and centralizes access to knowledge to ground every agent with the right context.
* It is an agent’s single endpoint for knowledge, delivering better context with automated source routing and advanced
  agentic retrieval, all while respecting user permissions.
* **Foundry IQ knowledge bases**: Available directly in the new Foundry portal, knowledge bases are reusable,
  topic-centric collections that ground multiple agents and applications through a single API. Building agents becomes
  simpler, no longer requiring a tangle of data tools stitched into every project
* Foundry IQ shifts that work into **knowledge bases**. Instead of wiring retrieval logic into every agent, you define a
  reusable knowledge base around a topic (such as employee policies, product documentation, or support content) and
  create it in the Foundry portal. From there, any number of agents and applications can connect and be grounded with
  that same knowledge base.
* Foundry IQ federates data across **indexed and remote knowledge sources: **M365** **SharePoint, Fabric IQ, OneLake,
  Azure Blob Storage, Azure AI Search indexes, the web, and MCP in private preview — all of which can contribute to the
  same knowledge base.

This is the layer that turns “we think this is cheaper/faster” into hard numbers.

### Tools

Foundry Tools enable you to quickly build intelligent, production-ready, and responsible applications using prebuilt,
configurable APIs and models. They support a wide range of use cases, including conversational AI, search, monitoring,
translation, speech, vision, and decision-making solutions.

In practice, tools are where **real ROI** hides: the more your agents can act on live systems, the more they can
automate revenue- or cost-impacting workflows.

### Machine Learning

The **Machine Learning** tile connects Foundry to Azure’s broader ML stack:
* Use **Azure Machine Learning** for training/fine-tuning models.
* Register custom models back into Foundry for inference and agent use.

This closes the loop between **data science** and **production agents**.

### Control Plane, Cloud, and Edge

The **Control Plane**:
* Manages deployments, networking, identity, and governance policies.
* Enforces security/compliance uniformly across models, tools, and agents — critical for regulated industries.

The **Cloud ↔ Edge** bar highlights that Foundry can push components closer to where data is generated, while the
**Security, compliance, and governance** stripe means that every ROI improvement must still pass audits and risk
thresholds.

## 2. The Foundry ROI Framework: Five Levers You Can Actually Pull

The “Foundry ROI Framework” breaks value into **five levers**:

Press enter or click to view image in full size
Foundry ROI Framework
1. **Performance Efficiency**
2. **Operational Productivity**
3. **Customer Outcomes**
4. **Governance & Risk Reduction**
5. **Innovation Scale**

Each lever corresponds to specific platform capabilities.

### 2.1 Performance Efficiency

**Features:**
* **Model router**
* **PTU (Provisioned Throughput Units)**
* **Priority Processing**

**Model router** lets you route calls to different models (or model SKUs) based on policy — e.g., GPT-5-mini for complex
queries, GPT-5-nano for simple ones — without changing app code.

**PTU** is reserved capacity for Azure OpenAI that guarantees throughput and latency for predictable workloads. It gives
you a fixed TPS and token budget with SLA-backed performance.

**Priority Processing**, now in public preview, is like “on-demand express lanes”:
* You pay a **premium per token** (typically ~75–100% above Standard according to the talk’s pricing slide).
* In return you get **low-latency, SLA-backed** processing without committing to PTUs.

These are the knobs you turn when you want **half the latency without doubling the bill**.

### 2.2 Operational Productivity

Here the star is **Observability Dashboards** — surfaced via IQ and Azure Monitor — that unify:
* Model health (latency percentiles, errors).
* Quality metrics (eval scores).
* Cost metrics (tokens, dollars per feature or tenant).

Instead of scraping logs from multiple services, you get a **single pane of glass** for “how healthy and how expensive
is this agent?”

### 2.3 Customer Outcomes

Two main ingredients:
* **Fine-tuning and RAG** for personalization and domain grounding.
* **Multimodal support** across text, images, audio, and documents.

In the Adobe Acrobat case, PDFs are ingested into **“PDF Spaces”** so users can chat with entire document collections.
Answers are grounded with citations, reducing hallucinations and building trust with knowledge workers who live inside
PDFs all day.

Customer outcome metrics include:
* Time saved per task (Adobe reports power users saving ~2 hours/day).
* Self-service resolution rate (Ada’s AI agents resolving **84%** of escalations vs traditional 70% for new human
  agents).
* Clinical documentation time freed back to doctors in the Suno.ai medical scribe scenario.

### 2.4 Governance & Risk Reduction

This pillar covers:
* **Content safety** and access controls.
* A **Governance Center** with policy enforcement and compliance dashboards.
* **Audit trails** for every decision and action.

Foundry builds on Azure’s responsible AI tooling — safety filters, abuse monitoring, and logging — to make sure ROI
isn’t wiped out by regulatory or reputational risk.

### 2.5 Innovation Scale

Finally, **Innovation Scale** is about:
* **Rapid provisioning and scaling** via PTU (and recently, **Dynamic Spillover**).
* Shipping new agents and features faster because models, tools, and governance are already integrated.

Dynamic Spillover lets PTU workloads automatically overflow into Standard capacity when utilization spikes, instead of
throwing 429s during peak events.

## 3. New Capabilities That Move the Needle on ROI

The “Announcing” slide highlights four capabilities that directly affect performance and cost:

Press enter or click to view image in full size
New Capabilities
1. **Model router in Foundry Models (GA)**
2. **Priority Processing (Public Preview)**
3. **Provisioned Throughput Dynamic Spillover (GA)**
4. **Prompt Caching (GA)**

### 3.1 Model Router: Right Model, Right Task, Right Cost

Rather than hard-coding calls to a single GPT-5 SKU, **model router** lets you:
* Define routing rules (by tenant, workload, or eval score).
* Use cheaper models for simple tasks, more capable models for complex ones.
* Experiment with new models with minimal app changes.

Press enter or click to view image in full size
Model Router — How it works.

Foundry’s team showed:
* **50% reduction in latency** (from ~20s peak to ~3–5s average).
* **85% of complex tasks routed to GPT-5-mini**.
* **15% of simple tasks routed to GPT-5-nano**.
* Immediate uptick in user satisfaction, engagement, and message volume.

Press enter or click to view image in full size
Impact

That’s the **holy trinity** of ROI: lower latency, lower cost per task, and happier users.

### 3.2 Priority Processing: Pay-as-You-Need Low Latency

Priority Processing is recommended for:
* **Real-time decision-making** (fraud, pricing, trading).
* **Conversational AI** (support agents, copilots).
* **High-value customer workloads** where every extra second hurts revenue.

Press enter or click to view image in full size
Priority Processing

It offers:
* Latency targets similar to PTU, **without committed reservations**.
* SLA-backed performance, billed per token at a premium.
* Global and **data-zone-specific** variants to meet residency needs.

**Considerations:**
* Slower ramping for sudden throughput spikes — if you know you’ll run hot continuously, PTU is still better.
* For very high and steady traffic, **PTU becomes cheaper** once you cross ~50–57% utilization compared to Priority
  Processing (as shown in the “Optimizing ROI” slide and confirmed by PTU documentation).

### 3.3 Dynamic Spillover for PTU: Always-On Capacity

PTU gives you:
* Reserved throughput.
* Predictable latency backed by SLA.
* Stable per-token pricing for steady workloads.

Press enter or click to view image in full size

**Dynamic Spillover**, now GA, ensures that when PTU is saturated:
* Requests can **spill over to Standard** capacity instead of failing.
* You can enable this per deployment; now integrated with Azure Monitor metrics and supported for agents.

For businesses like Ada handling **Black Friday spikes and incident-driven bursts** across web, email, and telephony,
this is the difference between stable NPS and a support outage.

### 3.4 Prompt Caching: Don’t Pay Twice for the Same Prompt

**Prompt caching** stores reusable parts of prompts, especially long system instructions, and:
* Cuts **time-to-first-token (TTFT)** by up to **80%**.
* In Standard, discounts cache tokens by 50–90% depending on model.
* In PTU, **cached tokens are 100% discounted** — you effectively don’t pay for them.

If your agents share long instructions and schemas, you’re leaving serious latency and cost savings on the table by not
enabling prompt caching.

## 4. Real-World Impact: Adobe, Ada, and Suno.ai

The session’s customer stories make the ROI framework concrete.

### 4.1 Adobe Acrobat: PDF Spaces and AI Assistant

Adobe’s **Acrobat AI Assistant** and **PDF Spaces** let users:
* Ask questions across one or many PDFs.
* Get cited answers, reducing hallucination risk.
* Save **~2 hours per day** for power users when consuming dense documents.

Key points:
* PDFs are chunked and indexed for retrieval.
* Foundry models (with RAG) answer user queries with citations.
* Latency targets are kept near **1.2 seconds** via PTU + routing to keep users in flow.

ROI here is straightforward: **knowledge workers reclaiming hours** of document reading every week.

### 4.2 Ada: Billion-Scale Customer Service on Foundry

Ada is an AI customer service platform processing:
* ~**1 billion conversations**,
* over **1 trillion tokens** per month,
* with agents resolving **54% of customer inquiries automatically** and **84%** of escalated cases in best customers.

Their architecture:
* Uses almost every Foundry feature: PTU, Dynamic Spillover, prompt caching, Priority Processing, and multimodal support
  for **web, email, and telephony**.
* Routes workloads by modality and SLA — email vs voice have different latency budgets.
* Optimizes PTU usage so **cost growth is sub-linear** compared to token growth.

The result: **support capacity scales with customers, not headcount**, and costs grow slower than usage.

### 4.3 eClinicalWorks + Suno.ai: Ambient Clinical Scribe

In healthcare, **Suno.ai** acts as an ambient medical scribe:
* A microphone in the exam room captures doctor–patient conversation.
* Audio is transcribed, diarized, and turned into a structured **SOAP note** (Subjective, Objective, Assessment, Plan).
* Structured fields like diagnosis codes, labs, and medications are extracted and pushed into the EHR.

ROI metrics:
* Clinicians reclaim significant documentation time per visit.
* Notes are more consistent and complete, reducing downstream billing/clinical risk.

Here Foundry’s strengths are:
* **Multimodal pipelines** (speech → text → structured notes).
* Governance and auditability for highly regulated PHI workflows.

## 5. Designing an ROI-Optimized Foundry Architecture

Let’s put this together with two reference architectures you can adapt.

### 5.1 Architecture A: Customer Support Agent with Mixed Latency SLAs

**Goal:** Minimize cost while meeting strict latency for live chat/voice, and looser latency for email/tickets.

## Get Dave R - Microsoft Azure & AI MVP☁️’s stories in your inbox

Join Medium for free to get updates from this writer.

Subscribe
Subscribe

Remember me for faster sign in

**Components:**
1. **Frontend Channels**
* Web widget, mobile app, phone IVR, email.

**2. Event Ingestion Layer**
* Normalizes requests into a common message format.

**3. Agent Service**
* Primary “Support Orchestrator” agent.
* Tools: CRM, order system, knowledge base search, ticketing system.

**4. Model Router**

Rules:
* GPT-5-mini for complex reasoning steps.
* GPT-5-nano for simple classification or FAQ responses.

**5. Capacity Strategy**
* **Priority Processing (Paygo)** for voice and live chat.
* **PTU** for steady baseline chat traffic during business hours.
* **Dynamic Spillover** enabled to Standard for peak events.
* **Batch** for nightly summarization and analytics.

**6. Prompt Caching**

Cache:
* System instructions.
* Tool schemas.
* Common output formats.

**7. IQ + Observability**

Dashboards on:
* Latency per channel.
* Resolution rate.
* Tokens & cost per tenant/feature.

**Outcome:**

You meet strict SLAs for live channels, keep cost-per-contact low through routing + PTU, and avoid outages on peak days
via spillover.

You can prototype something similar with:
* Microsoft Foundry overview:
  [https://learn.microsoft.com/en-us/azure/ai-foundry/what-is-azure-ai-foundry?WT.mc_id=AZ-MVP-5000671][16]
* Azure OpenAI PTU concepts:
  [https://learn.microsoft.com/en-us/azure/ai-services/openai/concepts/provisioned-throughput?WT.mc_id=AZ-MVP-5000671][1
  7]

### 5.2 Architecture B: Document Intelligence & Summarization at Scale

**Goal:** Run large document workloads (contracts, reports, logs) cheaply with relaxed latency.

**Components:**
1. **Ingestion & Storage**
* Blob storage or Data Lake for PDFs and text.
* Indexing for search/RAG.

**2. Batch Pipelines**
* Use **Azure OpenAI Batch** with GPT-5 models to:
* Summarize documents.
* Extract structured fields.
* Generate embeddings for later search.

**3. Capacity Strategy**

**Batch (Paygo)** for summarization and extraction:
* Up to 24-hour completion window.
* ~50% of Standard price per token, per Ignite slide.

**Standard** or **PTU** for interactive “Ask your documents” features.

**4. Prompt Caching**
* Cache extraction templates and summarization instructions to cut cost and TTFT.
* In PTU, cache tokens are free, making this especially attractive.

**5. Agent Service + Tools**
* User-facing agent to answer questions across indexed documents.
* Tools for search and metadata retrieval.

This mirrors Adobe’s PDF Spaces pattern and can be built from samples like:
* Azure AI Foundry baseline chat:
  [https://github.com/Azure-Samples/azure-ai-foundry-baseline][18]
* Azure AI Foundry samples collection:
  [https://github.com/Azure-Samples/azureai-samples][19]

## 6. Choosing Between Standard, Priority, PTU, and Batch

The “Optimizing ROI through the right AI Foundry offering” slide summarizes four offerings.

Press enter or click to view image in full size
Optimizing ROI through the right AI Foundry offering
1. **Standard (Paygo)**
* **Ideal for:** Dev/test, low-criticality apps, unpredictable low volume.
* **Latency:** Variable; best-effort.
* **Price per token:** Baseline.

**2. Priority Processing (Paygo)**
* **Ideal for:** Latency-sensitive, interactive experiences with spiky demand.
* **Latency:** Low, SLA-backed.
* **Price:** ~75–100% premium vs Standard.
* **ROI sweet spot:** High business value per call, but volume too volatile for PTU commitments.

**3. PTU (Reservations)**
* **Ideal for:** Steady, high-volume interactive traffic.
* **Latency:** Low, SLA-backed.
* **Price:** Discounted per token, best when utilization ≥50–57%.
* **Plus:** Dynamic Spillover for graceful peak handling.

**4. Batch (Paygo)**
* **Ideal for:** Large-scale, non-interactive jobs with relaxed latency.
* **Latency:** Up to 24 hours.
* **Price:** ~50% of Standard per token (per Ignite slide).

**Rule of thumb:**
* Start with **Standard** for prototyping.
* Move critical user journeys to **Priority**.
* Migrate stable, heavy traffic to **PTU + Spillover**.
* Offload offline workloads to **Batch**.

## Final Thoughts: Turning Platform Features into Business ROI

> ***ROI comes from how you combine platform features, not from a single magic model.***
* **Performance efficiency** (router, PTU, Priority, Batch, prompt caching) keeps latency and cost under control as you
  scale.
* **Operational productivity** (IQ, observability, evaluations) turns “we think this is better” into measurable
  outcomes.
* **Customer outcomes** (RAG, fine-tuning, multimodal) decide whether users actually stick around.
* **Governance & risk** make the whole thing viable in the real world.
* **Innovation scale** lets you iterate quickly instead of re-architecting every quarter.

If you’re building on Microsoft Foundry today, treat ROI as an **architecture constraint** from day one: plan capacity,
routing, and governance as first-class design decisions — not afterthoughts once the prototype works.

[*-Dave R.*][20]

[
Artificial Intelligence
][21]
[
Machine Learning
][22]
[
Technology
][23]
[
Software Development
][24]
[
Programming
][25]
[
][26]

--

[
][27]

--

[
][28]
[][29]
[
[ITNEXT]
][30]
[
[ITNEXT]
][31]
[

## Published in ITNEXT

][32]
[82K followers][33]
·[Last published 2 hours ago][34]

ITNEXT is a platform for IT developers & software engineers to share knowledge, connect, collaborate, learn and
experience next-gen technologies.

[
[Dave R - Microsoft Azure & AI MVP☁️]
][35]
[
[Dave R - Microsoft Azure & AI MVP☁️]
][36]
[

## Written by Dave R - Microsoft Azure & AI MVP☁️

][37]
[1.2K followers][38]
·[15 following][39]

Hey 👋, Let’s connect — 𝕏 @DaveRndn — [https://www.linkedin.com/in/daverndn/][40]

[

Help

][41]
[

Status

][42]
[

About

][43]
[

Careers

][44]
[

Press

][45]
[

Blog

][46]
[

Store

][47]
[

Privacy

][48]
[

Rules

][49]
[

Terms

][50]
[

Text to speech

][51]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-hi
gh-roi-ai-apps-on-microsoft-foundry-616622ddead6&source=post_page---top_nav_layout_nav-----------------------global_nav-
-----------------
[4]: https://medium.com/m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layo
ut_nav-----------------------new_post_topnav------------------
[5]: https://medium.com/search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-hi
gh-roi-ai-apps-on-microsoft-foundry-616622ddead6&source=post_page---top_nav_layout_nav-----------------------global_nav-
-----------------
[7]: https://itnext.io/?source=post_page---publication_nav-5b301f10ddcd-616622ddead6------------------------------------
---
[8]: https://itnext.io/?source=post_page---post_publication_sidebar-5b301f10ddcd-616622ddead6---------------------------
------------
[9]: https://blog.azinsider.net/?source=post_page---byline--616622ddead6---------------------------------------
[10]: https://blog.azinsider.net/?source=post_page---byline--616622ddead6---------------------------------------
[11]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fitnext%2F616622ddead6&operation=regist
er&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddea
d6&user=Dave+R+-+Microsoft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---header_actions--616622ddead6
---------------------clap_footer------------------
[12]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F616622ddead6&operation=register&
redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddead6&
user=Dave+R+-+Microsoft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---header_actions--616622ddead6---
------------------repost_header------------------
[13]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F616622ddead6&operation=registe
r&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddead
6&source=---header_actions--616622ddead6---------------------bookmark_footer------------------
[14]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D
616622ddead6&operation=register&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on
-microsoft-foundry-616622ddead6&source=---header_actions--616622ddead6---------------------post_audio_button------------
------
[15]: https://ignite.microsoft.com/en-US/sessions/BRK187
[16]: https://learn.microsoft.com/en-us/azure/ai-foundry/what-is-azure-ai-foundry?WT.mc_id=AZ-MVP-5000671
[17]: https://learn.microsoft.com/en-us/azure/ai-services/openai/concepts/provisioned-throughput?WT.mc_id=AZ-MVP-5000671
[18]: https://github.com/Azure-Samples/azure-ai-foundry-baseline
[19]: https://github.com/Azure-Samples/azureai-samples
[20]: https://blog.azinsider.net/subscribe
[21]: https://medium.com/tag/artificial-intelligence?source=post_page-----616622ddead6----------------------------------
-----
[22]: https://medium.com/tag/machine-learning?source=post_page-----616622ddead6---------------------------------------
[23]: https://medium.com/tag/technology?source=post_page-----616622ddead6---------------------------------------
[24]: https://medium.com/tag/software-development?source=post_page-----616622ddead6-------------------------------------
--
[25]: https://medium.com/tag/programming?source=post_page-----616622ddead6---------------------------------------
[26]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fitnext%2F616622ddead6&operation=regist
er&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddea
d6&user=Dave+R+-+Microsoft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---footer_actions--616622ddead6
---------------------clap_footer------------------
[27]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fitnext%2F616622ddead6&operation=regist
er&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddea
d6&user=Dave+R+-+Microsoft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---footer_actions--616622ddead6
---------------------clap_footer------------------
[28]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F616622ddead6&operation=register&
redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddead6&
user=Dave+R+-+Microsoft+Azure+%26+AI+MVP%E2%98%81%EF%B8%8F&userId=7f23df591f29&source=---footer_actions--616622ddead6---
------------------repost_footer------------------
[29]: https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F616622ddead6&operation=registe
r&redirect=https%3A%2F%2Fitnext.io%2Fstop-wasting-tokens-how-to-design-high-roi-ai-apps-on-microsoft-foundry-616622ddead
6&source=---footer_actions--616622ddead6---------------------bookmark_footer------------------
[30]: https://itnext.io/?source=post_page---post_publication_info--616622ddead6---------------------------------------
[31]: https://itnext.io/?source=post_page---post_publication_info--616622ddead6---------------------------------------
[32]: https://itnext.io/?source=post_page---post_publication_info--616622ddead6---------------------------------------
[33]: /followers?source=post_page---post_publication_info--616622ddead6---------------------------------------
[34]: /gym-service-building-a-grpc-powered-gym-management-api-with-spring-boot-4-bf52e2dd0c90?source=post_page---post_pu
blication_info--616622ddead6---------------------------------------
[35]: https://blog.azinsider.net/?source=post_page---post_author_info--616622ddead6-------------------------------------
--
[36]: https://blog.azinsider.net/?source=post_page---post_author_info--616622ddead6-------------------------------------
--
[37]: https://blog.azinsider.net/?source=post_page---post_author_info--616622ddead6-------------------------------------
--
[38]: https://blog.azinsider.net/followers?source=post_page---post_author_info--616622ddead6----------------------------
-----------
[39]: https://medium.com/@daverendon/following?source=post_page---post_author_info--616622ddead6------------------------
---------------
[40]: https://www.linkedin.com/in/daverndn/
[41]: https://help.medium.com/hc/en-us?source=post_page-----616622ddead6---------------------------------------
[42]: https://status.medium.com/?source=post_page-----616622ddead6---------------------------------------
[43]: https://medium.com/about?autoplay=1&source=post_page-----616622ddead6---------------------------------------
[44]: https://medium.com/jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----616622ddead6-------------------
--------------------
[45]: mailto:pressinquiries@medium.com
[46]: https://blog.medium.com/?source=post_page-----616622ddead6---------------------------------------
[47]: https://medium.com/store
[48]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----616622ddead6--------------------
-------------------
[49]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----616622ddead6-----------------------------
----------
[50]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----616622ddead6------------------
---------------------
[51]: https://speechify.com/medium?source=post_page-----616622ddead6---------------------------------------
```
