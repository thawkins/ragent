# Web source

- URL: https://learn.microsoft.com/en-us/answers/questions/5904380/caching-in-microsoft-foundry-serverless-deployment
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:41:14.763328509+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Caching in Microsoft Foundry serverless deployments

[ Justin Encabo ][8] 0 Reputation points
2026-05-28T06:22:01.8666667+00:00

Hello. I would like to ask if Microsoft Foundry models like DeepSeek V4 Pro and MiniMax 2.7via Fireworks have prompt
caching discounts. I want to personally use my balance and use Foundry as an endpoint for prototyping with AI-driven
development with an agentic harness (Pi Coding Agent). Are these models available for prompt caching discounts? Or is
everything going to be a cache miss?

Foundry Models
[ Foundry Models ][9]

A catalog of AI models in Microsoft Foundry that you can discover, compare, and deploy using Azure’s built‑in tools for
evaluation, fine‑tuning, and inference

Sign in to follow Follow
1 comment Hide comments for this question Report a concern
I have the same question (0)
1. [ SRILAKSHMI C ][10] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
   2026-06-09T07:09:14.4766667+00:00
   
   Hi [@Justin Encabo][11],
   
   Did you get any chance to review the above response. Do let me know if you have any further queries.
   
   Thank you!
   
   0 votes Report a concern
[ Sign in to comment ][12]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 4 answers

Sort by: Most helpful
[Most helpful][13] [Newest][14] [Oldest][15]
1. [ Carlin Scott ][16] • Follow 0 Reputation points
   2026-06-18T20:54:38.6433333+00:00
   
   DeepSeek-V4-Pro is available from both Azure direct, and Fireworks. So choose the correct model if you want caching.
   
   The [Foundry Models pricing | DeepSeek][17] does not mention caching at all. There is no caching.
   
   The [Foundry Models pricing | Fireworks][18] mentions caching. They do offer MiniMax 2.5 and DeepSeek-V4-Pro through
   Fireworks. But 2.7 seems to be a "self-hosted" only option. Meaning you have to deploy the model inside your account.
   
   Was this answer helpful?
   
   Yes No
   1 person found this answer helpful.
   0 comments No comments Report a concern
   [ Sign in to comment ][19]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
2. [ SRILAKSHMI C ][20] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
   2026-05-28T16:16:16.2533333+00:00
   
   Hello [@Justin Encabo ][21]
   
   Thank you for Reaching out to Microsoft Q&A,
   
   Based on the current Azure AI Foundry serverless deployment behavior, models such as:
   
   • DeepSeek V4 Pro
   
   • MiniMax 2.7 via Fireworks
   
   are currently billed on a standard pay-per-use/per-token basis, and there is no broadly documented or guaranteed
   built-in prompt caching discount mechanism exposed through Azure AI Foundry serverless endpoints today.
   
   In practical terms, you should generally assume:
   
   • Every request is processed independently
   
   • Repeated prompts/shared prompt prefixes are effectively treated as cache misses from a billing perspective
   
   • There is currently no customer-visible cache hit/miss telemetry or cached-token discount model exposed through the
   Foundry serverless abstraction layer for these provider integrations
   
   While some underlying model providers may internally use KV-cache optimizations for runtime efficiency, Azure AI
   Foundry serverless deployments do not currently expose deterministic prompt-cache billing discounts similar to some
   native provider APIs.
   
   Because these models are served through third-party provider integrations (for example via Fireworks), caching
   semantics can vary depending on:
   
   • The underlying provider implementation
   
   • Whether internal KV-cache reuse exists
   
   • Whether cache reuse is request-scoped or session-scoped
   
   • Whether Azure Foundry passes through any provider-native caching capabilities
   
   • Whether billing systems expose cached-token pricing separately
   
   For your scenario using Foundry endpoints for AI-driven development and agentic workflows such as Pi Coding Agent
   this is especially important because repeated long-context prompts can significantly increase token consumption and
   cost.
   
   At this time, Azure AI Foundry serverless endpoints should generally be treated as:
   
   “Full token billing per invocation unless explicit caching support is documented for that model/provider.”
   
   If you would like to reduce or optimize costs, here are a few approaches you can consider:
   1. Implement your own application-side cache layer
      • If your agent frequently submits identical or near-identical prompts, you can cache responses in your own
      database or application layer
      • This is often the most effective workaround today for agentic workflows with repeated context reuse
   2. Consider provisioned/reserved capacity options
      • If your workload volume becomes more predictable, provisioned or reserved-capacity deployments may provide
      better cost efficiency compared to pure serverless pay-per-token usage
   3. Explore managed/real-time endpoints
      • Managed compute deployments give you more control over runtime behavior and allow you to implement your own
      warm-worker or caching strategies within the application/service layer
   4. Minimize repeated static context
      • For agentic harnesses, reducing repeated system prompts or shared context can significantly reduce token spend
      when no caching discounts exist
   5. Compare with native provider APIs
      • Some providers may expose prompt caching or cached-token billing more explicitly through their direct APIs than
      through the Azure Foundry abstraction layer
   
   At the moment, there is no publicly documented indication that DeepSeek V4 Pro in Foundry or MiniMax 2.7 via
   Fireworks
   
   Currently support customer-visible prompt caching discounts through Azure AI Foundry serverless deployments.
   
   Please refer this
   
   Deploy Models via Serverless API: [https://learn.microsoft.com/azure/ai-foundry/how-to/deploy-models-serverless][22]
   
   Microsoft Foundry Models overview (serverless deployments):
   [https://learn.microsoft.com/azure/foundry/concepts/foundry-models-overview#serverless-deployments][23]
   
   Deploy Models via Managed Compute (real-time endpoint):
   [https://learn.microsoft.com/azure/ai-foundry/how-to/deploy-models-managed?tabs=azure-studio#deploy-open-models][24]
   
   Thank you!
   
   Was this answer helpful?
   
   Yes No
   1 comment Show comments for this answer Report a concern
   1. [ SRILAKSHMI C ][25] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
      2026-06-10T07:29:08.9633333+00:00
      
      Hi [@Justin Encabo][26],
      
      We haven’t heard from you on the last response and was just checking back to see if you have a resolution yet. In
      case if you have any resolution please do share that same with the community as it can be helpful to others.
      Otherwise, will respond with more details and we will try to help.
      
      Thank you!
      
      0 votes Report a concern
   [ Sign in to comment ][27]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
3. [ Jerald Felix ][28] • Follow 14,965 Reputation points • Volunteer Moderator
   2026-05-28T07:43:17.6333333+00:00
   
   Hello **[Justin Encabo][29],**
   
   Greetings! Thanks for raising this question in Q&A forum.
   
   Great question, and the good news is that the answer is much better than you might expect! Yes, DeepSeek V4 Pro and
   MiniMax models via Fireworks on Microsoft Foundry **do have prompt caching pricing** — with a dedicated cached token
   rate that is significantly lower than the regular input token rate. Let me break this down clearly for you.
   
   **DeepSeek V4 Pro — Prompt Caching on Foundry**
   
   DeepSeek V4 Pro on Microsoft Foundry via Fireworks has the following serverless pricing: $1.75 per 1M input tokens,
   $0.15 per 1M cached tokens, and $3.48 per 1M output tokens.
   
   So cached tokens are billed at roughly **91% less** than regular input tokens — a very significant discount that
   makes agentic use cases with repeated context much more cost-efficient.
   
   **MiniMax / Kimi K2.6 — Prompt Caching on Foundry**
   
   Kimi K2.6 (MoonshotAI) has serverless pricing of $0.95 per 1M input tokens, $0.16 per 1M cached tokens, and $4.00 per
   1M output tokens. Both models are available per-token serverless and via PTU through the Foundry model catalog with a
   single Azure endpoint and the same enterprise controls.
   
   **How does the caching work on Fireworks models?**
   
   For Fireworks models, cached input tokens are by default priced at 50% for all text and vision language models unless
   otherwise specified. However, as you can see from the Foundry-specific pricing above, DeepSeek V4 Pro and Kimi K2.6
   on Foundry have even deeper cached token discounts (around 91%) compared to the standard 50% discount.
   
   **Practical tips for your Pi Coding Agent agentic harness**
   
   To maximize cache hits and minimize costs in your agentic workflow, here are the key things to keep in mind:
   
   **Step 1: Structure your prompts for cache hits** Keep the beginning of your prompts — especially the system prompt,
   static context, tools definitions, and code context — consistent and identical across calls. The caching mechanism
   rewards prompts where the prefix is unchanged between requests.
   
   **Step 2: Use serverless pay-per-token for prototyping** Serverless pay-per-token inference is ideal for
   experimenting securely and quickly with Data Zone Standard — this is exactly the right option for prototyping with
   your balance without committing to PTUs.
   
   **Step 3: Switch to PTUs when your usage patterns stabilize** Once you've validated your agentic harness and have
   predictable throughput, provisioned throughput units (PTUs) offer predictable, steady-state performance for base or
   custom models which is better for production agentic workflows.
   
   **Step 4: Monitor your cached vs non-cached token usage** In the Foundry portal, go to your project > Monitoring to
   track token usage. Look at the ratio of cached to total input tokens — a good agentic setup with stable system
   prompts should achieve high cache hit rates over time, keeping your costs very close to the cached token rate.
   
   **Step 5: Check for the latest pricing before committing** Pricing for these models can change. Always verify the
   current rates at the Microsoft Foundry model catalog page for DeepSeek V4 Pro and MiniMax before planning your
   budget. The pricing shown above is current as of May 2026.
   
   If this answer helps you kindly accept the answer which will help others who have similar questions.
   
   Best Regards,
   
   Jerald Felix.
   
   Was this answer helpful?
   
   Yes No
   1 comment Show comments for this answer Report a concern
   1. [ Ni Yao ][30] • Follow 0 Reputation points • Microsoft Employee
      2026-06-09T14:24:12.6133333+00:00
      
      Hi Jerald, is this documented somewhere? It seems like this knowledge is only available in this one forum post.
      
      1 vote Report a concern
   [ Sign in to comment ][31]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
4. AI answer
   
   2026-05-28T06:22:16.6366667+00:00
   
   Prompt caching support and discounts are determined by whether the underlying model and deployment type support Azure
   OpenAI-style prompt caching, not just by being a Microsoft Foundry model.
   
   From the available information:
   * Prompt caching is described and priced specifically for Azure OpenAI models (for example GPT‑4o, GPT‑4.1, GPT‑5.x
     series) under Foundry/OpenAI endpoints. Cached tokens are billed at a discount for Standard deployments and up to
     100% discount for Provisioned deployments, with behavior controlled via parameters like `prompt_cache_retention`
     and (optionally) `prompt_cache_key`.
   * The prompt caching documentation lists only GPT‑4.x and GPT‑5.x families as supporting extended prompt cache
     retention, and states that all Azure OpenAI models GPT‑4o or newer support in‑memory prompt cache retention.
   * Fireworks-based models (such as DeepSeek and MiniMax variants) are listed separately in the Foundry catalog as
     partner models, with per‑token or PTU offers, but there is no mention in the provided material that they
     participate in Azure OpenAI prompt caching or receive prompt caching discounts.
   
   Based on the context provided, prompt caching discounts are documented only for Azure OpenAI models (GPT‑4o and
   newer, GPT‑4.1, GPT‑5.x families) and not for Fireworks partner models like DeepSeek or MiniMax. For those partner
   models, behavior should be assumed to be regular per‑token billing with no documented prompt caching discount, and
   any reuse of computation via caching is not described.
   
   To get prompt caching discounts for an agentic development harness, use one of the Azure OpenAI models that
   explicitly support prompt caching (for example GPT‑4o+ or GPT‑4.1 / GPT‑5.x families) on Standard or Provisioned
   deployments and structure prompts so that the first 1,024 tokens are identical across calls.
   
   References:
   * [Prompt caching][32]
   * [Fireworks models on Microsoft Foundry (preview)][33]
   * [Microsoft Foundry Models overview][34]
   * [Explore Microsoft Foundry Models in Azure Machine Learning][35]
   * [Foundry Models frequently asked questions][36]
   
   AI-generated content may be incorrect. Read our [transparency notes][37] for more information.
   
   Was this answer helpful?
   
   Yes No
   Report a concern
[ Sign in to answer ][38]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-us][39]
[ Your Privacy Choices][40]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][41]
* [Previous Versions][42]
* [Blog][43]
* [Contribute][44]
* [Privacy][45]
* [Consumer Health Privacy][46]
* [Terms of Use][47]
* [Code of Conduct][48]
* [Trademarks][49]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5904
380%2Fcaching-in-microsoft-foundry-serverless-deployment 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2
F5904380%2Fcaching-in-microsoft-foundry-serverless-deployment 											&text=Caching%20in%20Microsoft%20Foundry%20serverless%20de
ployments 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5904380%2Fcaching-in-m
icrosoft-foundry-serverless-deployment
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5904380%2F
caching-in-microsoft-foundry-serverless-deployment
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Caching%20in%20Microsoft%20Foundry%20serverless%20deployments&body=Cachi
ng in Microsoft Foundry serverless deploymentshttps%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5904380%
2Fcaching-in-microsoft-foundry-serverless-deployment
[8]: /en-us/users/na/?userid=01f6e118-0b72-47be-b740-dbc9304231bc
[9]: /en-us/answers/tags/1584/foundry-models/
[10]: /en-us/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[11]: https://learn.microsoft.com/en-us/users/na/?userid=01f6e118-0b72-47be-b740-dbc9304231bc
[12]: #
[13]: ?orderby=helpful&page=1#answers
[14]: ?orderby=newest&page=1#answers
[15]: ?orderby=oldest&page=1#answers
[16]: /en-us/users/na/?userid=f6f5c3ee-7ff5-4417-a52e-01b402bd04ce
[17]: https://azure.microsoft.com/en-us/pricing/details/ai-foundry-models/deepseek/
[18]: https://azure.microsoft.com/en-us/pricing/details/ai-foundry-models/fireworks/
[19]: #
[20]: /en-us/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[21]: https://learn.microsoft.com/en-us/users/na/?userid=01f6e118-0b72-47be-b740-dbc9304231bc
[22]: https://learn.microsoft.com/azure/ai-foundry/how-to/deploy-models-serverless
[23]: https://learn.microsoft.com/azure/foundry/concepts/foundry-models-overview#serverless-deployments
[24]: https://learn.microsoft.com/azure/ai-foundry/how-to/deploy-models-managed?tabs=azure-studio#deploy-open-models
[25]: /en-us/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[26]: https://learn.microsoft.com/en-us/users/na/?userid=01f6e118-0b72-47be-b740-dbc9304231bc
[27]: #
[28]: /en-us/users/na/?userid=f5bf268b-7ffe-0006-0000-000000000000
[29]: https://learn.microsoft.com/en-us/users/na/?userid=01f6e118-0b72-47be-b740-dbc9304231bc
[30]: /en-us/users/na/?userid=06e34342-91ed-4168-bf19-0c4f325e8842
[31]: #
[32]: https://learn.microsoft.com/azure/foundry/openai/how-to/prompt-caching
[33]: https://learn.microsoft.com/azure/foundry/how-to/fireworks/enable-fireworks-models#available-catalog-models
[34]: https://learn.microsoft.com/azure/foundry/concepts/foundry-models-overview#serverless-deployments
[35]: https://learn.microsoft.com/azure/machine-learning/foundry-models-overview?view=azureml-api-2#serverless-deploymen
ts
[36]: https://learn.microsoft.com/azure/foundry-classic/foundry-models/faq#general
[37]: /answers/support/ai-first-overview
[38]: #
[39]: #
[40]: https://aka.ms/yourcaliforniaprivacychoices
[41]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[42]: https://learn.microsoft.com/en-us/previous-versions/
[43]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[44]: https://learn.microsoft.com/en-us/contribute
[45]: https://go.microsoft.com/fwlink/?LinkId=521839
[46]: https://go.microsoft.com/fwlink/?linkid=2259814
[47]: https://learn.microsoft.com/en-us/legal/termsofuse
[48]: https://aka.ms/msftqacodeconduct
[49]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
