# Web source

- URL: https://technspire.com/blog/prompt-caching-2026-real-cost-wins
- Title: [
- Captured (UTC): 2026-06-29T15:41:19.472796044+00:00

```text
[
technspire][1]
[Home][2]
Services
[Solution Examples][3][Team][4][Blog][5][Contact Us][6]
EN
[Back to all posts][7]
AI & Cloud Infrastructure

# Prompt Caching in 2026: Anthropic, OpenAI, Azure Compared

By Technspire Team·May 15, 2026·2739 views

Prompt caching is the highest-ROI cost lever on long-context LLM workloads in 2026. Done well, it cuts input-token cost
by 30 to 50% on agent loops and RAG pipelines, with no quality change. Done poorly, it silently does nothing. The
difference is placement, breakpoint discipline, and a measurement habit.

## How the Three Providers Price Caching

Anthropic, OpenAI, and Azure OpenAI all offer prompt caching, with different mechanics. The numbers below reflect public
pricing in mid-2026; check the live pricing pages before architecting around them.
* **Anthropic.** Explicit cache_control breakpoints. Writes cost 1.25x normal input rate; reads cost 10% of normal input
  rate (a 90% discount on cached input). Cache TTL is 5 minutes by default, with a 1-hour option at a higher write rate.
* **OpenAI.** Automatic caching above a token threshold (1,024 tokens of stable prefix). Cached prefix is billed at 50%
  of normal input rate. No explicit breakpoints; the system detects the longest matching prefix.
* **Azure OpenAI.** Mirrors the OpenAI behaviour for OpenAI models; same automatic prefix matching, same pricing
  structure. Regional caches per deployment.

The Anthropic model is more controllable. The OpenAI model is less work to wire up. For long, stable system prompts and
tool definitions, both produce similar effective discounts.

## The Cache Breakpoint Pattern

Caches hit on prefixes. The placement principle: put everything stable first, everything variable last. The order that
produces the best hit rate:
1. System prompt (most stable).
2. Tool definitions (stable per agent version).
3. Long static context (e.g. corpus excerpts that recur across queries).
4. Slowly changing context (conversation history older than a few turns).
5. The current user message (most variable).

Anything ordered after a variable element does not cache. A timestamp injected into the system prompt at the top of the
request invalidates the entire cache. A request ID dropped into the tool list breaks the tool-list cache. The single
most common mistake is putting a variable string before the long stable content.

`// Anthropic: explicit breakpoints on the long stable parts
const response = await anthropic.messages.create({
  model: 'claude-opus-4-7',
  max_tokens: 1024,
  system: [
    { type: 'text',
      text: SYSTEM_PROMPT,
      cache_control: { type: 'ephemeral' } },         // breakpoint 1
    { type: 'text',
      text: KNOWLEDGE_BASE_EXCERPT,
      cache_control: { type: 'ephemeral' } },         // breakpoint 2
  ],
  tools,                                              // tools cache too
  messages: conversationHistory.concat([
    { role: 'user', content: userMessage }            // variable, not cached
  ]),
});`

## Where the Cost Wins Are Largest

The economics work hardest when three conditions hold:
* **The cacheable prefix is large.** 5,000+ tokens of stable content makes a real difference. 500 tokens does not move
  the bill noticeably.
* **The same prefix is reused many times.** Agent loops (5 to 15 calls with the same system prompt and tools), RAG with
  a stable instruction template, batched evaluation runs.
* **The variable part is comparatively small.** If the user message is 50 tokens and the cached prefix is 8,000, the
  cache discount applies to 99% of the input cost.

The opposite case: short prompts with mostly-variable content. A 200-token classification request has almost nothing to
cache. Caching adds complexity here for almost no benefit.

## Measuring Hit Rate

Both Anthropic and OpenAI return cache statistics in the response. Track them. The dashboard you actually want has three
lines per workload: input tokens, cached input tokens, and cache hit rate (cached/input). A hit rate below 50% on a
long-context workload is a sign the breakpoint is in the wrong place.

`// Recording cache metrics from each response
const usage = response.usage;
metrics.record({
  workload: 'incident_triage_agent',
  input_tokens: usage.input_tokens,
  cache_creation_tokens: usage.cache_creation_input_tokens ?? 0,
  cache_read_tokens: usage.cache_read_input_tokens ?? 0,
  output_tokens: usage.output_tokens,
  hit_rate: (usage.cache_read_input_tokens ?? 0) /
            Math.max(1, usage.input_tokens),
});`

## Where the Cache Silently Goes Cold

Four failure modes worth specific attention:
* **TTL expiry on bursty traffic.** Anthropic's 5-minute default TTL is generous for steady traffic and tight for
  sporadic. If requests arrive once every 7 minutes, every request pays the write cost and never reads. The 1-hour TTL
  option is worth the write premium for these patterns.
* **Regional or model split.** A workload that load-balances across regions or across model versions does not share
  caches across them. A single deployment per workload caches better than a multi-region round-robin.
* **Tool definition churn.** Editing a single tool description invalidates the entire tool-list cache for everyone using
  it. A versioned tool schema with rare changes caches; one edited weekly does not.
* **Conversation history reorganisation.** If the application periodically rewrites or compacts conversation history,
  the cache for that prefix is destroyed. Compact infrequently and at predictable boundaries.

## A Worked Example

An agent with a 4,000-token system prompt and 2,000-token tool definitions, running 8 model calls per session, with
average per-call user content of 500 tokens.

Without caching, input cost per session is roughly: 8 × (4,000 + 2,000 + accumulated context). The accumulated context
grows; round to 8 × 8,000 = 64,000 input tokens.

With caching on the system prompt and tools (6,000 tokens cached on the second call onward): 6,000 (write) + 7 × 6,000
(read at 10% of input rate, on Anthropic) + 8 × 2,000 (variable, full price) = 6,000 + 4,200-equivalent + 16,000. Total
effective input cost equivalent: about 26,200 tokens. A 59% reduction in input-token billable equivalents.

The exact numbers vary with workload and provider, but the order of magnitude is consistent. Long stable prefix, many
repeated calls, no variable content above the prefix: caching cuts input cost dramatically.

## When Not to Bother

Three workloads where the caching investment does not pay back:
* **Short, mostly-variable requests.** A 300-token classification prompt has nothing meaningful to cache.
* **Very low volume.** A workload that runs ten times a day will pay the cache write cost each time and rarely read.
* **Output-bound workloads.** Long-form generation where the output dominates the cost (a 50-token prompt producing
  5,000 tokens of output) is unaffected by input caching.

## The 30-Minute Audit

A pragmatic order of investigation for an existing LLM workload:
1. Pull the usage data for a week. Find the top three workloads by input-token volume.
2. For each, identify the stable prefix length. If it is over 2,000 tokens, caching is worth a try.
3. Verify the prefix order: stable first, variable last. Move anything out of order.
4. Add explicit cache breakpoints (Anthropic) or verify automatic caching is engaged (OpenAI / Azure OpenAI).
5. Log cache_read_input_tokens for a week. Plot hit rate by workload.
6. Adjust breakpoint placement until hit rate stabilises above 60% on long-context workloads.

This is one of the rare optimisations where the code change is small and the bill change is large. The cost-conscious
teams will already have done this audit. The teams that have not are leaving a quarter to a half of their LLM input bill
on the table.

## Tags

[Prompt Caching][8][Cost Optimization][9][Anthropic][10][OpenAI][11][Azure OpenAI][12]

## Discuss your AI or cloud project with us

Technspire helps Swedish and European B2B teams ship AI, Azure, and Next.js work that holds up in production. Short
conversations are free.

[Schedule a free consultation][13]
[Back to all posts][14]
[
technspire][15]

Leading provider of AI services, cloud development, and digital transformation solutions for Swedish enterprises and
government agencies.

Org.nr: 559022-9422
VAT: SE559022942201

### Services
* [Azure OpenAI Integration][16]
* [Next.js & React Development][17]
* [TypeScript Modernization][18]
* [Payment System Integration][19]
* [On-Premise AI Solutions][20]
* [Cloud Migration][21]

### Company
* [Solution Examples][22]
* [Training Courses][23]
* [Our Team][24]
* [Blog][25]
* [Contact][26]

### Contact
* Markörvägen 1a
  Stockholm
  Sweden
* [hello@technspire.com][27]

© 2026 Technspire AB. All rights reserved.
[Privacy Policy][28][Terms of Service][29][Cookie Policy][30]

[1]: /
[2]: /
[3]: /case-studies
[4]: /team
[5]: /blog
[6]: /#contact
[7]: /en/blog
[8]: /en/blog/tags/Prompt%20Caching
[9]: /en/blog/tags/Cost%20Optimization
[10]: /en/blog/tags/Anthropic
[11]: /en/blog/tags/OpenAI
[12]: /en/blog/tags/Azure%20OpenAI
[13]: /en#contact
[14]: /en/blog
[15]: /
[16]: /services/azure-openai-integration
[17]: /services/nextjs-react-development
[18]: /services/typescript-modernization
[19]: /services/payment-system-integration
[20]: /services/on-premise-ai-solutions
[21]: /services/cloud-migration
[22]: /case-studies
[23]: /training
[24]: /team
[25]: /blog
[26]: /#contact
[27]: mailto:hello@technspire.com
[28]: /privacy
[29]: /terms
[30]: /cookies
```
