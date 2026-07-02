# Web source

- URL: https://medium.com/microsoft-azure-in-practice/choosing-the-right-ai-model-in-microsoft-foundry-bc9098450940
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T15:43:51.275911986+00:00

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
[Mastodon][7]
[

## Microsoft Azure in Practice

][8]
·
[
[Microsoft Azure in Practice]
][9]

Code-first Azure publication covering AI, .NET, Data, DevOps, Security, and cost optimization — real projects,
reproducible examples, and measurable outcomes.

# Choosing the Right AI Model in Microsoft Foundry

[
[Divyesh Govaerdhanan]
][10]
[Divyesh Govaerdhanan][11]
8 min read
·
Nov 28, 2025
[
][12]

--

[
][13]
[][14]
[

Listen

][15]

Share

*From 11,000+ options to a short, confident shortlist*

If you’ve opened the **Microsoft Foundry** model catalog recently, you’ve probably had that moment:

> *“There are more than ****11,000 models**** here. Where do I even start?”*

***The good news***: you don’t need to understand every model.

Press enter or click to view image in full size
Source: [https://ai.azure.com/][16]

What you *do* need is:
* A simple way to map **“job to be done → model type”**
* A basic feel for **which providers shine where**
* And a habit of using **benchmarks and evaluation tools** instead of guessing

In this blog, I will cover:
1. A mental model for choosing AI models in Microsoft Foundry
2. The major model families (OpenAI, Phi, Claude, Llama, etc.) and what they’re good at
3. How to use **Azure AI Benchmarks and leaderboards** to compare models
4. Concrete “if this, then that” patterns for real workloads
5. A tiny .NET snippet you can reuse to test any Foundry model
6. Simple decision checklist to find the right model

*Note: Microsoft Foundry is the evolution of Azure AI Studio / Azure AI Foundry. The model catalog and benchmarking
tools now live in this unified experience.*

## 1. Start with the workload, not the model

Before touching the catalog, answer four questions with your team:
1. **What’s the job to be done?**
* Chatbot? Analyst copilot? Code assistant? Call center triage? Document summarization?

**2. What matters more: quality, speed, or cost?**
* Are you okay with a “good enough” answer at high volume, or do you need near-perfect analysis for fewer, high-value
  queries?

**3. Which modalities are involved?**
* Text only? Text + images? Do you need to generate images, audio, or video?

**4. Where must it run?**
* Pure cloud? Hybrid? On-prem/edge with strict data constraints?

Once you answer these, 11,000+ models shrink to a realistic shortlist.

## 2. The Microsoft Foundry model landscape (who’s who)

Microsoft Foundry gives you a single catalog and endpoint to access models from OpenAI, Microsoft, Anthropic, Cohere,
Meta, NVIDIA, and open-source communities.

***Publishers Catalog:*** [https://ai.azure.com/catalog/publishers/][17]

Press enter or click to view image in full size
AI Model Providers

You don’t need the full list; you just need to know the **families**:

Press enter or click to view image in full size
* **Frontier reasoning models** (GPT-5, Claude Opus) for the hardest problems
* **Everyday copilots** (GPT-4.1-mini, Claude Sonnet, Phi-4-mini) for day-to-day tasks
* **Open-weight / small models** (Phi, Llama, gpt-oss) for custom and hybrid scenarios

## 3. Don’t guess: use Microsoft Foundry Benchmarks & leaderboards

This is the part many teams miss: Azure doesn’t just give you models — it provides you with **benchmarks**.

In the **Foundry model catalog**:
* Models with benchmark data show a **bar graph icon**.
* When you open a model, there’s a **Benchmarks** tab where you can:
* See a **Quality index** and other index scores (quality, cost, latency, throughput).
* View **comparative charts** that show how this model stacks up against others.
* Open a **metric comparison table** with scores per benchmark (reasoning, QA, math, coding, etc.).

There’s also a dedicated **Benchmarks** page at `ai.azure.com/explore/benchmarks` where you can browse leaderboards and
compare models on standard datasets.

Press enter or click to view image in full size
ai.azure.com/explore/benchmarks

From there, you can:
1. **Open the model details** → *Benchmarks* tab
2. Click **“Compare with more models”** to see side-by-side charts.

And if you want to bring this closer to your real workloads:
* Use the **Evaluation** features (“Try with your own data”) to run an evaluation against your prompts / JSONL datasets.
* In the **Playgrounds**, you can try up to **three models in parallel** with the same prompt and compare outputs
  visually.

*Important: Benchmarks are a starting point, not the final answer. They give you ****evidence**** about quality, cost,
and latency — but you should still test on your own data and, ideally, check multiple sources (Foundry Benchmarks,
Hugging Face leaderboards, etc.).*

## 4. Model archetypes and when to use them (with examples)

Let’s walk through common scenarios and match them to model types.

### 4.1 Deep reasoning, strategy & complex workflows

**Use cases**
* Reviewing complex contracts or SoWs for risk and missing clauses
* Synthesizing long research docs into clear decisions
* Driving multi-tool agents that plan, call APIs, and summarize

**Models to consider**
* **GPT-5.1 **for the deepest reasoning when quality is critical
* **Claude Opus / Sonnet** if you like Anthropic’s reasoning style and longer context windows

**Example prompt**

> *“Given these three SoWs and our internal risk playbook, list the top 5 risks, highlight missing data protection
> clauses, and propose safer alternative wording.”*

Start by looking at the **Quality index** and reasoning benchmarks for GPT-5 vs Claude Opus in your region, then pilot
with both on a small subset of your docs.

### 4.2 Everyday chatbots & copilots

**Use cases**
* HR or IT helpdesk copilots
* Customer-facing FAQ bots
* “Sidekick” panels inside your web app

**Models to consider**
* **GPT-4.1-mini** or similar “mini” models: great balance of quality, latency, and cost
* **Claude Sonnet / Haiku** for conversational, safe responses with good performance
* **Phi-4-mini** if you want **very cost-efficient** responses and plan to ground heavily on your own data.

**Example prompt**

> *“Build a chatbot that answers travel policy questions and generates the correct request template.”*

Here, start from the **Cost** and **Latency indexes** in Benchmarks and pick a small model. You can always escalate
certain paths (“/deep” or “/legal”) to GPT-5 later.

### 4.3 Code assistants & developer productivity

**Use cases**
* Code generation, refactoring, and unit test creation
* Framework migrations (e.g., from .NET Framework to .NET 10)
* Infrastructure-as-code, pipeline, or script generation

**Models to consider**
* **GPT-5-Codex** / **GPT-5.1-codex** for the strongest code-aware intelligence
* GPT-4.1 family, if you want a more cost-balanced option and still strong coding skills

**Example prompt**

> *“Refactor this C# service into a clean architecture style with separate application and domain layers, and generate
> xUnit tests.”*

Look at benchmarks specific to **coding**, then try a side-by-side comparison in the Playground for 5–10 prompts your
team actually cares about.

### 4.4 RAG, semantic search & knowledge copilots

**Use cases**
* “Ask your data” copilots over PDFs, Confluence, tickets, and logs
* Semantic search and clustering of user feedback
* Intelligent search in support portals

You usually need two pieces:
1. An **embedding model** (for vector search)
2. A **chat/reasoning model** (for final answers)

**Models to consider**
* Embeddings from **Cohere**, **OpenAI**, or other models explicitly tagged as `embedding` in the catalog
* A “mini” or mid-range chat model (GPT-4.1-mini, Claude Sonnet, Phi-4-mini)

**Example prompt**

> *“Show me incidents in the last 90 days where customers hit SSL handshake errors, and summarize the fixes we used.”*

Use Azure AI Search / Foundry IQ for retrieval, and let Benchmarks help you pick the chat + embedding combination with
the best quality vs cost.

### 4.5 Vision & multimodal reasoning

**Use cases**
* Explaining charts, dashboards, prototypes, or architecture diagrams
* Inspecting forms, invoices, receipts, and lab reports
* Analysing UI screenshots and suggesting UX improvements

**Models to consider**
* **Phi-4-multimodal-instruct** and other multimodal models that accept image + text inputs
* Multimodal GPT-5 variants (where available) for more advanced reasoning across text + images

**Example prompt**

> *“Here’s a screenshot of our sales dashboard. Explain what changed in Q3 and propose 3 experiments to improve
> conversion.”*

Check the **Multimodal** filters and vision benchmarks in the model catalog before testing in the Playground.

### 4.6 Image, audio & video generation

**Use cases**
* Generating hero images, diagrams, or UI mockups
* Creating audio snippets or synthesized voices
* Producing short marketing/explainer videos

**Models to consider**
* **GPT-Image-1 / GPT-Image-1-Mini** for images (blogs, diagrams, UI concepts)
* **GPT-Audio** models for text-to-speech and audio content
* Video models (like **Sora-2**) from the video playground, where you can generate and preview clips directly inside
  Foundry.

**Example prompt**

> *“Generate a simple architecture illustration of a multi-tool AI agent using Microsoft Foundry: one box for the agent,
> one for tools, one for data.”*

These are great for **developer experience**, documentation, and demos.

### 4.7 Open-weight & small models (Phi, Llama, gpt-oss)

**Use cases**
* Strict data residency requirements
* Offline / edge scenarios
* Highly specialized domains (legal, medical, industrial) where you may fine-tune

**Models to consider**
* Open-weight models like **gpt-oss** and **Llama** hosted via **Foundry Models**
* **Phi-4 / Phi-4-mini** deployed with **Foundry Local** or on Windows via **Windows AI Foundry / Copilot+ PCs** for
  on-device inferencing.

These aren’t “worse” — they’re **different tools**. You trade some raw capability for control, cost, and location.

## 5. Plugging a Foundry model into your .NET app (minimal example)

Once you’ve picked a model and deployed it via Microsoft Foundry, calling it from .NET is straightforward with the
**Azure.AI.Inference** SDK.

## GitHub Repository

💻 **Explore the Full Source Code:**
👉 [GitHub — Multi-Tool Azure AI Agent (.NET)][18]

The repo includes:
* Full .NET console app
* Demo commands
* Example output & metrics

Star ⭐ the repo if you find it helpful — it supports ongoing improvements and helps others discover it!

## 6. A simple decision checklist for your team

When your team is choosing models, you can literally walk through this checklist:
1. **Define the job clearly**
* What is the user trying to do? (1–2 sentences)

**2. Pick 2–3 candidate models**
* From the **model catalog filters** (provider, modality, “chat” vs “embedding”, etc.).

**3. Look at the Benchmarks first**
* Quality index vs cost vs latency on the Benchmarks tab
* Use “Compare with more models” for side-by-side charts.

**4. Test in the Playground**
* Run your real prompts against 2–3 models in parallel.

**5. Evaluate with your data**
* Create an Evaluation run (JSONL, your own dataset) and review scores + traces.

**6. Deploy more than one**
* Use a “primary” model and keep a “fallback/experiment” model so you can adapt as the ecosystem evolves.

## 7. Wrap-up: model choice is now a product decision

With **more than 11,000 models** available in Microsoft Foundry, model choice is no longer a purely technical question —
it’s a **product decision**.

The good news is:
* You don’t have to guess. You have **Benchmarks, leaderboards, and evaluations** built into the portal.
* You don’t have to commit forever — you can deploy multiple models, route traffic, and switch as new models arrive.
* And you don’t have to do this alone — the catalog, Benchmarks page, and Playgrounds are all designed to help teams
  co-decide, not just individual developers.

If you’re already building with .NET and Azure, you’re in a great position. You can keep your existing stack and let
Foundry handle model selection, routing, and benchmarking as the ecosystem keeps evolving.

[
Microsoft Foundry
][19]
[
Azure Ai
][20]
[
Ai Model
][21]
[
Azure
][22]
[
Microsoft Ai Agent
][23]
[
][24]

--

[
][25]

--

[
][26]
[][27]
[
[Microsoft Azure in Practice]
][28]
[
[Microsoft Azure in Practice]
][29]
[

## Published in Microsoft Azure in Practice

][30]
[30 followers][31]
·[Last published Jun 20, 2026][32]

Code-first Azure publication covering AI, .NET, Data, DevOps, Security, and cost optimization — real projects,
reproducible examples, and measurable outcomes.

[
[Divyesh Govaerdhanan]
][33]
[
[Divyesh Govaerdhanan]
][34]
[

## Written by Divyesh Govaerdhanan

][35]
[74 followers][36]
·[26 following][37]

Microsoft MVP, Technical Team Lead at Cloud Assert. Writing production-grade .NET, Azure, and AI agent guides with real
code and companion GitHub repos.

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
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-m
odel-in-microsoft-foundry-bc9098450940&source=post_page---top_nav_layout_nav-----------------------global_nav-----------
-------
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-m
odel-in-microsoft-foundry-bc9098450940&source=post_page---top_nav_layout_nav-----------------------global_nav-----------
-------
[7]: https://me.dm/@divyeshg94
[8]: https://medium.com/microsoft-azure-in-practice?source=post_page---publication_nav-a34acb0c483c-bc9098450940--------
-------------------------------
[9]: https://medium.com/microsoft-azure-in-practice?source=post_page---post_publication_sidebar-a34acb0c483c-bc909845094
0---------------------------------------
[10]: /@divyeshg94?source=post_page---byline--bc9098450940---------------------------------------
[11]: /@divyeshg94?source=post_page---byline--bc9098450940---------------------------------------
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fmicrosoft-azure-in-practice%2Fbc9098450940&operation=reg
ister&redirect=https%3A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft-foundry
-bc9098450940&user=Divyesh+Govaerdhanan&userId=32e75d034c87&source=---header_actions--bc9098450940---------------------c
lap_footer------------------
[13]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Fbc9098450940&operation=register&redirect=https%3A%
2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft-foundry-bc9098450940&user=Divye
sh+Govaerdhanan&userId=32e75d034c87&source=---header_actions--bc9098450940---------------------repost_header------------
------
[14]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Fbc9098450940&operation=register&redirect=https%3
A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft-foundry-bc9098450940&source=-
--header_actions--bc9098450940---------------------bookmark_footer------------------
[15]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3Dbc9098450940&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft
-foundry-bc9098450940&source=---header_actions--bc9098450940---------------------post_audio_button------------------
[16]: https://ai.azure.com/
[17]: https://ai.azure.com/catalog/publishers/
[18]: https://github.com/divyeshg94/AzureAIAgent_Multi-Tool
[19]: /tag/microsoft-foundry?source=post_page-----bc9098450940---------------------------------------
[20]: /tag/azure-ai?source=post_page-----bc9098450940---------------------------------------
[21]: /tag/ai-model?source=post_page-----bc9098450940---------------------------------------
[22]: /tag/azure?source=post_page-----bc9098450940---------------------------------------
[23]: /tag/microsoft-ai-agent?source=post_page-----bc9098450940---------------------------------------
[24]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fmicrosoft-azure-in-practice%2Fbc9098450940&operation=reg
ister&redirect=https%3A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft-foundry
-bc9098450940&user=Divyesh+Govaerdhanan&userId=32e75d034c87&source=---footer_actions--bc9098450940---------------------c
lap_footer------------------
[25]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fmicrosoft-azure-in-practice%2Fbc9098450940&operation=reg
ister&redirect=https%3A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft-foundry
-bc9098450940&user=Divyesh+Govaerdhanan&userId=32e75d034c87&source=---footer_actions--bc9098450940---------------------c
lap_footer------------------
[26]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Fbc9098450940&operation=register&redirect=https%3A%
2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft-foundry-bc9098450940&user=Divye
sh+Govaerdhanan&userId=32e75d034c87&source=---footer_actions--bc9098450940---------------------repost_footer------------
------
[27]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Fbc9098450940&operation=register&redirect=https%3
A%2F%2Fmedium.com%2Fmicrosoft-azure-in-practice%2Fchoosing-the-right-ai-model-in-microsoft-foundry-bc9098450940&source=-
--footer_actions--bc9098450940---------------------bookmark_footer------------------
[28]: https://medium.com/microsoft-azure-in-practice?source=post_page---post_publication_info--bc9098450940-------------
--------------------------
[29]: https://medium.com/microsoft-azure-in-practice?source=post_page---post_publication_info--bc9098450940-------------
--------------------------
[30]: https://medium.com/microsoft-azure-in-practice?source=post_page---post_publication_info--bc9098450940-------------
--------------------------
[31]: /microsoft-azure-in-practice/followers?source=post_page---post_publication_info--bc9098450940---------------------
------------------
[32]: /microsoft-azure-in-practice/from-local-to-azure-deploy-your-mcp-server-to-production-78b05bb1e5a9?source=post_pag
e---post_publication_info--bc9098450940---------------------------------------
[33]: /@divyeshg94?source=post_page---post_author_info--bc9098450940---------------------------------------
[34]: /@divyeshg94?source=post_page---post_author_info--bc9098450940---------------------------------------
[35]: /@divyeshg94?source=post_page---post_author_info--bc9098450940---------------------------------------
[36]: /@divyeshg94/followers?source=post_page---post_author_info--bc9098450940---------------------------------------
[37]: /@divyeshg94/following?source=post_page---post_author_info--bc9098450940---------------------------------------
[38]: https://help.medium.com/hc/en-us?source=post_page-----bc9098450940---------------------------------------
[39]: https://status.medium.com/?source=post_page-----bc9098450940---------------------------------------
[40]: /about?autoplay=1&source=post_page-----bc9098450940---------------------------------------
[41]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----bc9098450940-------------------------------------
--
[42]: mailto:pressinquiries@medium.com
[43]: https://blog.medium.com/?source=post_page-----bc9098450940---------------------------------------
[44]: https://medium.com/store
[45]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----bc9098450940--------------------
-------------------
[46]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----bc9098450940-----------------------------
----------
[47]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----bc9098450940------------------
---------------------
[48]: https://speechify.com/medium?source=post_page-----bc9098450940---------------------------------------
```
