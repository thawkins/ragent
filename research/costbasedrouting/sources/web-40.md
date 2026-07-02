# Web source

- URL: https://www.llmreference.com/router/azure-foundry-model-router
- Title: [LLM Reference][1][Picks][2][Models][3][Compare][4][Best][5][Providers][6][Benchmarks][7][Pulse][8][Changelog][9]
- Captured (UTC): 2026-06-29T15:42:28.123292823+00:00

```text
[LLM Reference][1][Picks][2][Models][3][Compare][4][Best][5][Providers][6][Benchmarks][7][Pulse][8][Changelog][9]
[Tools][10]
[Tools index][11][Routers & Gateways][12][Playgrounds][13][Concepts][14]
[API][15]

Router profile

# Azure AI Foundry Model Router

Microsoft

[Visit Azure AI Foundry Model Router][16]
RouterFresh · 2026-06-08

> Microsoft Azure AI Foundry's native model router that uses a trained ML model to route each prompt in real time to the
> optimal Azure-hosted model, with Balanced/Cost/Quality mode selection and automatic failover.

Type

Router

Lead directory segment

Pricing model

Passthrough

Model count pending

Hosting

Provider-native

No self-host flag

Data retention

Retains data

Verify for production policy

## At a glance


## Routes to these providers

[Microsoft Foundry][17]

Microsoft Foundry offers a comprehensive platform-as-a-service for enterprise AI operations. It provides multiple
deployment options including Serverless APIs (pay-as-you-go), Global Standard (shared managed capacity), Provisioned
Throughput Units (reserved capacity), batch processing, and bring-your-own model deployments. The platform features a
unified control plane for models, agents, tools, and observability. Its Agent Service enables building and deploying AI
agents with built-in tracing, monitoring, and governance. Evaluation and monitoring tools assess model performance,
safety, and groundedness. Foundry supports seamless upgrades from Azure OpenAI with non-destructive migration,
maintaining existing deployments while unlocking multi-provider model access and advanced platform capabilities.

[Azure OpenAI][18]

Azure OpenAI Service hosts OpenAI's GPT-4o, GPT-4, GPT-3.5, and embedding models on Microsoft Azure with enterprise
SLAs. Deployments run in customer-selected regions with private networking, role-based access control, and capacity
options spanning Standard pay-per-token, Provisioned Throughput Units (PTUs) for reserved capacity, Global Standard
shared capacity, and Batch processing. Azure OpenAI sits inside the wider Microsoft Foundry / Azure AI Studio control
plane, which adds an evaluation, monitoring, and Agent Service layer on top of the base model APIs. For workloads that
need non-OpenAI models (Claude, DeepSeek, Grok, Llama, Mistral, NVIDIA Nemotron), Microsoft Foundry is the broader
catalog; Azure OpenAI is the OpenAI-specific entry point. The service is API-compatible with the OpenAI SDK in most
flows, so teams typically swap base URLs and authentication rather than rewriting calls.

## Pricing & data handling

No separate routing fee; pay per underlying Azure model tokens. Three modes: Balanced (broad distribution by
complexity), Cost (cheapest capable model), Quality (frontier models preferred). Supports model subsets and automatic
failover. Current version: 2025-11-18.


## Sources & freshness
* [homepage, status, type, modes, pricing_model][19] · checked 2026-06-08
* [how_it_works][20] · checked 2026-06-08
* [model_catalog_entry][21] · checked 2026-06-08

Last reviewed 2026-06-08.

## Compare & related routers

Compare Azure AI Foundry Model Router against another router without mixing model rows into the same view.

[Compare with AIRouter][22]
[AIRouter][23]

Commercial LLM router that analyzes incoming requests and routes to the optimal model for cost/quality/latency via a
drop-in OpenAI-compatible API, with a privacy-preserving embedding mode that avoids sending prompt content.

[Amazon Bedrock Intelligent Prompt Routing][24]

AWS Bedrock's native intelligent prompt router that routes prompts between Anthropic Claude model tiers (Haiku/Sonnet)
based on predicted task complexity, with no extra per-routing charge.

[Martian][25]

AI-powered LLM router that analyzes each prompt in real-time to select the optimal model, targeting 20–97% cost
reduction while maintaining quality; San Francisco startup reportedly nearing $1.3B valuation.

[Neutrino AI][26]

Commercial LLM router that dynamically routes each query to the best-suited model with load balancing and fallback
handling, charging 3% of underlying AI spend.

A Data Advantage project. Updated daily.
Browse
[Models][27][Providers][28][Benchmarks][29][Best of…][30][Concepts][31][Tools][32]
Signals
[Pulse][33][Changelog][34][Frontier pricing][35]
Use
[API][36][Search index][37][Methodology][38][About][39]

© 2026 Data Advantage, LLC. All rights reserved.

[Terms & Conditions][40][Privacy Policy][41][Do Not Sell or Share My Personal Information][42]

[1]: /
[2]: /picks
[3]: /models
[4]: /compare
[5]: /best
[6]: /providers
[7]: /benchmarks
[8]: /pulse
[9]: /changelog
[10]: /tools
[11]: /tools
[12]: /routers
[13]: /playgrounds
[14]: /concepts
[15]: /api
[16]: https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/model-router
[17]: /provider/microsoft-foundry
[18]: /provider/azure-openai
[19]: https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/model-router
[20]: https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/model-router-how-it-works
[21]: https://ai.azure.com/catalog/models/model-router
[22]: /router/compare/airouter/azure-foundry-model-router
[23]: /router/airouter
[24]: /router/bedrock-intelligent-prompt-routing
[25]: /router/martian
[26]: /router/neutrino
[27]: /models
[28]: /providers
[29]: /benchmarks
[30]: /best
[31]: /concepts
[32]: /tools
[33]: /pulse
[34]: /changelog
[35]: /pulse#frontier-pricing
[36]: /api
[37]: /models
[38]: /about#methodology
[39]: /about
[40]: /terms
[41]: /privacy
[42]: /privacy#advertising
```
