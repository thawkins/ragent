# Web source

- URL: https://medium.com/@danushidk507/prompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T15:41:20.846563415+00:00

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

# Prompt Caching in LLMs and Azure AI Foundry — Complete End-to-End Guide

[
[DhanushKumar]
][7]
[DhanushKumar][8]
6 min read
·
Feb 15, 2026
[
][9]

--

[
][10]
[][11]
[

Listen

][12]

Share

As large language model (LLM) applications scale into production, one of the biggest cost and latency drivers is
repeatedly sending long prompts that contain identical instructions, schemas, tool definitions, or contextual documents.
Prompt caching is a modern inference optimization that solves this problem by reusing previously computed token
processing results when the beginning of a prompt is identical across requests. Instead of recomputing the same tokens
again and again, the model reuses cached computation states, significantly reducing inference latency and token cost
while preserving output correctness.

### Prompt Caching :

Prompt caching leverages two related ideas:

**(1) application-level caching of full responses or semantically similar responses,**

**(2) provider-level *prefix/prefill* caching that reuses the model’s intermediate attention key/value tensors (the
“prefill” or KV state) computed for the first N tokens of a prompt.**

The provider-level approach is what reduces the internal compute cost inside the LLM: when two requests share an
**identical prefix,** the provider can reuse the stored key/value tensors for those prefix tokens and only compute the
remaining tokens, so the model spends much less time and GPU cycles. This is distinct from storing final outputs
(response caching) or retrieval/embedding caches (semantic caches) which store or find responses rather than reuse model
internal tensor state. The provider-level behavior is what vendors call “prompt caching”, and it is implemented by
routing requests to a machine that has a cached prefix hash and then reusing the prefill tensors when the prefix hash
matches.

Provider implementations typically require an *exact match on a prefix* (character or token exactness after
canonicalization) and a minimum prompt length for caching to apply; OpenAI/Azure threshold behavior and routing details
are explicit in their docs. For example, both OpenAI and Azure only enable prompt caching for long prompts.(OpenAI/Azure
use 1,024 tokens as the minimum prompt length where caching becomes relevant) and route requests based on a hash of an
initial portion of tokens (often the first ~256 tokens are used for routing). If the prefix and optional customer key
match, the server returns a `cached_tokens` count in the `usage.prompt_tokens_details` that shows how many prompt tokens
were served from cache. This lets you measure hits and tune your prefix structure.

### How Prompt Caching Works Internally (Transformer Perspective)

During inference, a transformer generates attention key/value tensors for each processed token. These tensors are reused
when predicting subsequent tokens. Prompt caching extends this concept across requests.

If two requests share the same prefix tokens:
1. The service computes a prefix hash.
2. It checks whether the same prefix already exists in the cache.
3. If present, the cached attention tensors are reused.
4. Only the remaining tokens are processed.

This dramatically reduces GPU compute time because the most expensive part of inference (processing long prefix context)
is skipped.

### What exactly is cached, and provider constraints must design for

Providers cache the *prefill/attention key/value tensors* that are generated while processing input tokens (the model’s
intermediate state after consuming a prefix). That intermediate representation is typically much smaller to persist than
raw token text, but it is tied to a particular model architecture and model version. Providers therefore constrain cache
behavior: they require identical tokenized prefixes (not “similar” text) to hit the cache; a single character difference
in the prefix results in a miss. Providers may also limit caching to specific model families or deployment types, and
will expose usage fields showing `cached_tokens`. Extended retention options exist for some models but are not
universal. For example, OpenAI supports an in-memory default (minutes) and an extended retention option (up to 24h for
some recent models) which moves K/V tensors to GPU-local storage for longer retention. Azure Foundry / Azure OpenAI
states that caches are cleared within 24 hours and that caching discounts on input tokens can vary by deployment type
(Standard vs Provisioned). These provider constraints are the single most important operational factor when you design
how prompts are authored or templated.

### When prompt caching is a win (and when it isn’t)

Prompt caching is highly effective for workloads where the majority of the prompt is static across requests, for
instance: system instructions, assistant tool schemas, long few-shot examples, static documents (policy text, legal
boilerplate) that are used as context for many queries, or structured output schemas appended as a prefix. It is less
useful when most requests differ at the start of the prompt (user-specific personalization or variable instructions
inserted before the repeated content), when prompts are short (<1,024 tokens) or when you need semantic similarity
rather than exact matches. If you need similarity matching of prompts (i.e., different text but same intent), combine
semantic caching (embedding + kNN or approximate matching) at the application layer; provider prompt caching requires
exact prefix identity.

### Azure AI Foundry Prompt Caching — Core Behavior

Azure AI Foundry implements prompt caching automatically for supported Azure OpenAI models and operations. The system
temporarily stores processed token computations and reuses them when identical prefixes are detected.

Prompt caching provides:
* Reduced latency
* Reduced input token cost
* Identical model outputs
* Automatic activation without additional configuration

Prompt caches are cleared within approximately 24 hours and are isolated per Azure subscription.

### Activation Requirements (Critical Engineering Constraint)

For Azure prompt caching to activate:
* The prompt must be at least 1024 tokens long.
* The first 1024 tokens must be identical across requests.
* After 1024 tokens, additional matches occur every 128 identical tokens.

Even a single character change within the first 1024 tokens results in a cache miss, making prompt canonicalization
extremely important in production system.

### Routing and cache lookup mechanism

Azure routes requests using a **hash of the prefix**, typically computed from the first ~256 tokens.

This routing determines which compute node contains the cached prefix state. If the request reaches the node holding the
matching prefix, a cache hit occurs; otherwise the prefix is recomputed.

Azure exposes cache hits through:

usage.prompt_tokens_details.cached_tokens

This value shows how many prompt tokens were served from cache.

From a monitoring standpoint, this is the primary metric used to measure cache efficiency.

### What exactly Azure caches

Azure supports caching for:
* Entire messages array (system, developer, user, assistant)
* Tool definitions
* Structured output schemas
* Images in prompts (if image parameters remain identical)

This is extremely important for production systems using:
* tool calling
* function schemas
* long policy instructions
* multi-modal prompts

All of those can be cached when placed at the prompt prefix.

### prompt_cache_key — routing optimization mechanism

Azure allows specifying a parameter:

prompt_cache_key

This value is combined with the prefix hash to improve routing efficiency and increase hit probability when many
requests share common prefixes.

However, if the same prefix + key receives too many requests (≈15 requests/min), overflow routing may reduce cache
effectiveness.

Engineering implication:
Use keys to group logical workloads (same template), but distribute load across keys if traffic is extremely high.

### Practical architecture pattern for production systems

A production GenAI architecture using Azure prompt caching typically follows:
1. Prompt templating layer
   System instructions, schema definitions, long context documents are placed first.
2. Canonicalization layer
   Ensures deterministic formatting (whitespace, JSON ordering) so the first 1024 tokens remain identical.
3. Provider prompt caching
   Azure automatically caches prefix computation.
4. Application-level semantic/response cache (optional)
   Redis / vector semantic cache for additional savings.
5. Observability layer
   Tracks cached_tokens, latency reduction, and cost savings.

This layered caching strategy yields maximum performance improvements.

### Production Python implementation example (Azure OpenAI)

Below is a minimal production-ready example showing prompt caching usage.

from openai import AzureOpenAI

client = AzureOpenAI(
    api_key="AZURE_KEY",
    azure_endpoint="https://<endpoint>.openai.azure.com",
    api_version="2024-10-01-preview"
)

messages = [
    {"role": "system", "content": "Long reusable policy text..."},
    {"role": "user", "content": "User question"}
]

response = client.chat.completions.create(
    model="gpt-4o",
    messages=messages,
    extra_body={
        "prompt_cache_key": "policy-template-v1"
    }
)

print(response.usage.prompt_tokens_details.cached_tokens)

Engineering recommendations:
* Always place reusable content at the beginning.
* Version prompt templates using cache keys.
* Monitor cached_tokens ratio to measure savings.
* Avoid modifying prefixes dynamically.

### Design trade-offs and best practices

Prompt caching provides significant cost and latency benefits but introduces strict prefix-identity requirements.
Therefore:
* Template versioning must be controlled.
* Canonical formatting must be enforced.
* High-QPS systems should distribute cache keys to avoid routing overflow.
* Sensitive prompts should be evaluated under retention policies since cache lifetime can reach 24 hours.

When implemented correctly, prompt caching can reduce large-context inference cost by orders of magnitude for repeated
enterprise workloads such as RAG systems, copilots, or enterprise knowledge assistants.

Prompt caching is a foundational optimization for modern GenAI platforms that reuse long contextual prefixes. Azure AI
Foundry implements prefix-based computation reuse automatically for supported models, requiring identical first-token
prefixes and providing measurable cost and latency benefits. Proper prompt engineering, template stability, monitoring,
and architecture integration are essential to fully realize its value.

[
Prompt Caching
][13]
[
Azure Ai
][14]
[
Azureopenai
][15]
[
LLM
][16]
[
][17]

--

[
][18]

--

[
][19]
[][20]
[
[DhanushKumar]
][21]
[
[DhanushKumar]
][22]
[

## Written by DhanushKumar

][23]
[1.2K followers][24]
·[72 following][25]

A guy who is curious to learn and blog ... Data Science @Deloitte AI | Data Science | Azure AI

[

Help

][26]
[

Status

][27]
[

About

][28]
[

Careers

][29]
[

Press

][30]
[

Blog

][31]
[

Store

][32]
[

Privacy

][33]
[

Rules

][34]
[

Terms

][35]
[

Text to speech

][36]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai
-foundry-complete-end-to-end-guide-6df1d5a8c082&source=post_page---top_nav_layout_nav-----------------------global_nav--
----------------
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai
-foundry-complete-end-to-end-guide-6df1d5a8c082&source=post_page---top_nav_layout_nav-----------------------global_nav--
----------------
[7]: /@danushidk507?source=post_page---byline--6df1d5a8c082---------------------------------------
[8]: /@danushidk507?source=post_page---byline--6df1d5a8c082---------------------------------------
[9]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F6df1d5a8c082&operation=register&redirect=https%3A%2F%
2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082&user
=DhanushKumar&userId=bef30f7c46ed&source=---header_actions--6df1d5a8c082---------------------clap_footer----------------
--
[10]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F6df1d5a8c082&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082&u
ser=DhanushKumar&userId=bef30f7c46ed&source=---header_actions--6df1d5a8c082---------------------repost_header-----------
-------
[11]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F6df1d5a8c082&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082
&source=---header_actions--6df1d5a8c082---------------------bookmark_footer------------------
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D6df1d5a8c082&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete
-end-to-end-guide-6df1d5a8c082&source=---header_actions--6df1d5a8c082---------------------post_audio_button-------------
-----
[13]: /tag/prompt-caching?source=post_page-----6df1d5a8c082---------------------------------------
[14]: /tag/azure-ai?source=post_page-----6df1d5a8c082---------------------------------------
[15]: /tag/azureopenai?source=post_page-----6df1d5a8c082---------------------------------------
[16]: /tag/llm?source=post_page-----6df1d5a8c082---------------------------------------
[17]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F6df1d5a8c082&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082&use
r=DhanushKumar&userId=bef30f7c46ed&source=---footer_actions--6df1d5a8c082---------------------clap_footer---------------
---
[18]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F6df1d5a8c082&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082&use
r=DhanushKumar&userId=bef30f7c46ed&source=---footer_actions--6df1d5a8c082---------------------clap_footer---------------
---
[19]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F6df1d5a8c082&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082&u
ser=DhanushKumar&userId=bef30f7c46ed&source=---footer_actions--6df1d5a8c082---------------------repost_footer-----------
-------
[20]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F6df1d5a8c082&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40danushidk507%2Fprompt-caching-in-llms-and-azure-ai-foundry-complete-end-to-end-guide-6df1d5a8c082
&source=---footer_actions--6df1d5a8c082---------------------bookmark_footer------------------
[21]: /@danushidk507?source=post_page---post_author_info--6df1d5a8c082---------------------------------------
[22]: /@danushidk507?source=post_page---post_author_info--6df1d5a8c082---------------------------------------
[23]: /@danushidk507?source=post_page---post_author_info--6df1d5a8c082---------------------------------------
[24]: /@danushidk507/followers?source=post_page---post_author_info--6df1d5a8c082---------------------------------------
[25]: /@danushidk507/following?source=post_page---post_author_info--6df1d5a8c082---------------------------------------
[26]: https://help.medium.com/hc/en-us?source=post_page-----6df1d5a8c082---------------------------------------
[27]: https://status.medium.com/?source=post_page-----6df1d5a8c082---------------------------------------
[28]: /about?autoplay=1&source=post_page-----6df1d5a8c082---------------------------------------
[29]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----6df1d5a8c082-------------------------------------
--
[30]: mailto:pressinquiries@medium.com
[31]: https://blog.medium.com/?source=post_page-----6df1d5a8c082---------------------------------------
[32]: https://medium.com/store
[33]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----6df1d5a8c082--------------------
-------------------
[34]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----6df1d5a8c082-----------------------------
----------
[35]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----6df1d5a8c082------------------
---------------------
[36]: https://speechify.com/medium?source=post_page-----6df1d5a8c082---------------------------------------
```
