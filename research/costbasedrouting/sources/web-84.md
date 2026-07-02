# Web source

- URL: https://medium.com/@badrkacimi/azure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T15:44:08.228066045+00:00

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

# Azure AI Foundry Anti‑Patterns: What Not to Do in Real Projects

[
[Badr Kacimi]
][7]
[Badr Kacimi][8]
3 min read
·
Apr 6, 2026
[
][9]

--

1

[
][10]
[][11]
[

Listen

][12]

Share

Press enter or click to view image in full size
[Nature’s patterns: Golden spirals and branching fractals — CNET][13]

> Most blog posts show how Azure AI Foundry works.
> Very few show how it quietly fails in production.

After experimenting multiple AI initiatives from proof-of-concept to scaled deployments, I’ve noticed a pattern. The
teams that struggle aren’t lacking features, models, or ambition.

They repeat the **same anti‑patterns**.

This article isn’t about shaming teams or blaming tools. Azure AI Foundry is a powerful platform. But like any platform,
how you use it matters more than what it offers.

If you’re building real customer-facing AI, not just demos

these are the mistakes to avoid.

## Anti‑Pattern #1: One Model to Rule Them All

> *“Let’s just use GPT‑4 for everything.”*

This feels efficient at first. One deployment, one prompt strategy, one approval flow.

In practice, it causes:
* Unpredictable latency
* Inflated costs
* Inconsistent behaviors across tasks
* Difficult tuning and governance

## What goes wrong

Different workloads have **different intelligence needs**:
* Classification ≠ summarization
* Extraction ≠ reasoning
* Real‑time chat ≠ batch processing

Forcing all of this through one large model creates unnecessary complexity and fragility.

## What to do instead

Adopt a **model portfolio mindset**:
* Smaller or faster models for extraction/classification
* Larger models only where reasoning truly adds value
* Explicit model–task mapping in your architecture docs

Use model router for performance and cost efficiency, see my previous article about that [**The Model Router Explained:
Intelligent Cost & Performance Optimization in Azure AI Foundry**][14]

## Anti‑Pattern #2: Prompt Sprawl Without Ownership

> *“Wait… which prompt is production using?”*

This happens faster than teams expect.

Prompts get:
* Copied across environments
* Modified by different people
* Tweaked in-flight to fix “small issues”
* Left undocumented

What goes wrong
* No auditability
* No reproducibility
* No clear “source of truth”
* Production issues no one can confidently roll back

## What to do instead

Treat prompts as **first-class assets**:
* Version them (yes, like code)
* Assign clear ownership
* Use naming conventions tied to business outcomes
* Log prompt changes alongside deployments

## Anti‑Pattern #3: Treating Foundry Like a Notebook, Not a Platform

> *“It works in the playground, let’s ship it.”*

Playgrounds are dangerous when mistaken for production environments.

## What goes wrong
* Hard-coded parameters
* Hidden assumptions (temperature, max tokens, system prompts)
* No environment separation
* No CI/CD integration.

## What to do instead

Design for **platform agnostic** from day one:
* Explicit configuration management
* Clear dev / test / prod separation
* Infrastructure-as-code patterns
* Repeatable deployment pipelines

## Anti‑Pattern #4: Ignoring Latency vs Accuracy Trade‑Offs

> *“The answer is great, but it takes 12 seconds.”*

In isolation, accuracy feels like the only metric that matters.

In reality:
* Users abandon slow systems
* APIs time out
* Downstream systems fail silently

## What goes wrong

Teams optimize for “best possible answers” without considering:
* End‑to‑end latency
* Token limits
* Streaming vs blocking responses
* User expectations per interaction

## What to do instead

Make trade‑offs explicit:
* Define acceptable latency per use case
* Test different temperature and reasoning settings
* Use progressive disclosure (fast answer → refined answer)
* Measure perceived performance, not just correctness

## Anti‑Pattern #5: Shipping Without Telemetry

> *“It worked fine last week.”*

This phrase usually appears **right before a post‑mortem **😂

## What goes wrong

Without telemetry, teams can’t answer:
* Why did this response change?
* Which prompts are failing most?
* Did a model update affect behavior?
* Are costs scaling linearly or exponentially?

Azure gives you monitoring tools, using them is no longer optional.

## What to do instead

Instrument everything:
* Prompt ID + model + parameters
* Latency and token usage
* User feedback loops
* Error and fallback paths

## The Real Pattern Behind These Anti‑Patterns

Each mistake shares a theme:

> *Treating AI systems like “smart features” instead of ****existing systems****.*

Azure AI Foundry gives you the building blocks but **architecture, governance, and humility keep systems healthy**.

***Thanks for reading my article and I hope you can take something away.***

💯 Don’t forget to *follow me on ****medium ****for more*

💯 Don’t forget to *follow me on *[***LinkedIn***][15]*** ****for more*

💯 ***leave some feedback***

## #[MVP Communities — Microsoft][16]

## That’s all !

[
AI
][17]
[
LLM
][18]
[
Antipattern
][19]
[
Azure
][20]
[
][21]

--

[
][22]

--

1

[
][23]
[][24]
[
[Badr Kacimi]
][25]
[
[Badr Kacimi]
][26]
[

## Written by Badr Kacimi

][27]
[95 followers][28]
·[4 following][29]

Senior IT Consultant | Tech Mentor

[

Help

][30]
[

Status

][31]
[

About

][32]
[

Careers

][33]
[

Press

][34]
[

Blog

][35]
[

Store

][36]
[

Privacy

][37]
[

Rules

][38]
[

Terms

][39]
[

Text to speech

][40]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-n
ot-to-do-in-real-projects-7d0896cb0977&source=post_page---top_nav_layout_nav-----------------------global_nav-----------
-------
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-n
ot-to-do-in-real-projects-7d0896cb0977&source=post_page---top_nav_layout_nav-----------------------global_nav-----------
-------
[7]: /@badrkacimi?source=post_page---byline--7d0896cb0977---------------------------------------
[8]: /@badrkacimi?source=post_page---byline--7d0896cb0977---------------------------------------
[9]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F7d0896cb0977&operation=register&redirect=https%3A%2F%
2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977&user=Badr+Kac
imi&userId=e28ee0b5617c&source=---header_actions--7d0896cb0977---------------------clap_footer------------------
[10]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F7d0896cb0977&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977&user=Badr+
Kacimi&userId=e28ee0b5617c&source=---header_actions--7d0896cb0977---------------------repost_header------------------
[11]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F7d0896cb0977&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977&source=-
--header_actions--7d0896cb0977---------------------bookmark_footer------------------
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D7d0896cb0977&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-
projects-7d0896cb0977&source=---header_actions--7d0896cb0977---------------------post_audio_button------------------
[13]: https://www.cnet.com/pictures/natures-patterns-golden-spirals-and-branching-fractals/
[14]: /@badrvkacimi/the-model-router-explained-intelligent-cost-performance-optimization-in-azure-ai-foundry-c2614a40347
1
[15]: https://www.linkedin.com/in/badr-kacimi/
[16]: https://mvp.microsoft.com/
[17]: /tag/ai?source=post_page-----7d0896cb0977---------------------------------------
[18]: /tag/llm?source=post_page-----7d0896cb0977---------------------------------------
[19]: /tag/antipattern?source=post_page-----7d0896cb0977---------------------------------------
[20]: /tag/azure?source=post_page-----7d0896cb0977---------------------------------------
[21]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F7d0896cb0977&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977&user=Badr+Ka
cimi&userId=e28ee0b5617c&source=---footer_actions--7d0896cb0977---------------------clap_footer------------------
[22]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F7d0896cb0977&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977&user=Badr+Ka
cimi&userId=e28ee0b5617c&source=---footer_actions--7d0896cb0977---------------------clap_footer------------------
[23]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F7d0896cb0977&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977&user=Badr+
Kacimi&userId=e28ee0b5617c&source=---footer_actions--7d0896cb0977---------------------repost_footer------------------
[24]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F7d0896cb0977&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40badrkacimi%2Fazure-ai-foundry-anti-patterns-what-not-to-do-in-real-projects-7d0896cb0977&source=-
--footer_actions--7d0896cb0977---------------------bookmark_footer------------------
[25]: /@badrkacimi?source=post_page---post_author_info--7d0896cb0977---------------------------------------
[26]: /@badrkacimi?source=post_page---post_author_info--7d0896cb0977---------------------------------------
[27]: /@badrkacimi?source=post_page---post_author_info--7d0896cb0977---------------------------------------
[28]: /@badrkacimi/followers?source=post_page---post_author_info--7d0896cb0977---------------------------------------
[29]: /@badrkacimi/following?source=post_page---post_author_info--7d0896cb0977---------------------------------------
[30]: https://help.medium.com/hc/en-us?source=post_page-----7d0896cb0977---------------------------------------
[31]: https://status.medium.com/?source=post_page-----7d0896cb0977---------------------------------------
[32]: /about?autoplay=1&source=post_page-----7d0896cb0977---------------------------------------
[33]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----7d0896cb0977-------------------------------------
--
[34]: mailto:pressinquiries@medium.com
[35]: https://blog.medium.com/?source=post_page-----7d0896cb0977---------------------------------------
[36]: https://medium.com/store
[37]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----7d0896cb0977--------------------
-------------------
[38]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----7d0896cb0977-----------------------------
----------
[39]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----7d0896cb0977------------------
---------------------
[40]: https://speechify.com/medium?source=post_page-----7d0896cb0977---------------------------------------
```
