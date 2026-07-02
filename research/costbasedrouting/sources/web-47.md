# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/model-router
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:42:39.365530253+00:00

```text
[ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][3] [ More info about Internet Explorer and Microsoft Edge ][4]
Table of contents Exit editor mode
Ask Learn Ask Learn
Reading mode Table of contents [ Read in English ][5] Add Add to plan [ Edit ][6] Copy Markdown Print

Note

Access to this page requires authorization. You can try [signing in][7] or changing directories.

Access to this page requires authorization. You can try changing directories.

# Model router for Microsoft Foundry

Feedback
Summarize this article for me

## In this article

Model router is a trained language model that intelligently routes your prompts in real time to the most suitable large
language model (LLM). You deploy model router like any other Foundry model. Thus, it delivers high performance while
saving on costs, reducing latencies, and increasing responsiveness, while maintaining comparable quality, all packaged
as a single model deployment.

To try model router quickly, follow [How to use model router][8]. After you deploy model router, send a request to the
deployment. Model router selects an underlying model for each request based on your routing settings. For a deep dive
into the routing pipeline, training, and decision logic, see [How model router works][9].

Note

You do not need to separately deploy the supported LLMs for use with model router, with the exception of the Claude
models. To use model router with your Claude models, first deploy them from the model catalog. The deployments are
invoked by model router if they're selected for routing.

## How model router works

As a trained language model, model router analyzes your prompts in real time based on complexity, reasoning, task type,
and other attributes. It does not store your prompts. It routes only to eligible models based on your access and
deployment types, honoring data zone boundaries.

Important

The effective context window is limited by the smallest underlying model. For larger contexts, use [model subset][10] to
select models that support your requirements.
* In Balanced mode (default), it considers all underlying models within a small quality range (for example, 1% to 2%
  compared with the highest-quality model for that prompt) and picks the most cost-effective model.
* In Cost mode, it considers a larger quality band (for example, 5% to 6% compared with the highest-quality model for
  that prompt) and chooses the most cost-effective model.
* In Quality mode, it picks the highest quality rated model for the prompt, ignoring the cost.

## Why use model router?

Model router optimizes costs and latencies while maintaining comparable quality. Smaller and cheaper models are used
when they're sufficient for the task, but larger and more expensive models are available for more complex tasks. Also,
reasoning models are available for tasks that require complex reasoning, and non-reasoning models are used otherwise.
Model router provides a single deployment and chat experience that combines the best features from all of the underlying
chat models.

The current version, `2025-11-18` (latest), includes the following capabilities:
1. Support Global Standard and Data Zone Standard deployments.
2. Adds support for new models: `grok-4`, `grok-4-fast-reasoning`, `DeepSeek-V3.1`, `DeepSeek-V3.2`, `gpt-oss-120b`,
   `Llama-4-Maverick-17B-128E-Instruct-FP8`, `gpt-4o`, `gpt-4o-mini`, `gpt-5.2`, `gpt-5.2-chat`, `claude-haiku-4-5`,
   `claude-sonnet-4-5`, `claude-opus-4-1`, and `claude-opus-4-6`.
3. Quick deploy or Custom deploy with **routing mode** and **model subset** options.
4. **Routing mode**: Optimize the routing logic for your needs. Supported options: `Quality`, `Cost`, `Balanced`
   (default).
5. **Model subset**: Select your preferred models to create your model subset for routing.
6. Support for agentic scenarios including tools so you can now use it in the Foundry Agent Service.

## Versioning

Model router uses date-stamped versions. The current version is `2025-11-18` (latest), which is actively maintained —
new underlying models and features are added to this version over time without changing the version identifier.

Older versions (`2025-08-07`, `2025-05-19`) are frozen and don't receive new model additions.

────────────┬───────────────────┬──────────────────────────────────────────
Version     │Status             │Description                               
────────────┼───────────────────┼──────────────────────────────────────────
`2025-11-18`│**Active (latest)**│Receives ongoing model and feature updates
────────────┼───────────────────┼──────────────────────────────────────────
`2025-08-07`│Frozen             │Fixed set of models; no new additions     
────────────┼───────────────────┼──────────────────────────────────────────
`2025-05-19`│Frozen             │Fixed set of models; no new additions     
────────────┴───────────────────┴──────────────────────────────────────────

Tip

You don't need to wait for a new version number to access newly supported models. The `2025-11-18` version is updated in
place as new models become available.

If you select **Auto-update** at the deployment step (see [Model updates][11]), your model router deployment
automatically updates when new versions become available. When that happens, the set of underlying models also changes,
which could affect the overall performance of the model and costs.

## Supported models

Note

You don't need to separately deploy the supported LLMs for use with model router, with the exception of the Claude
models. To use model router with your Claude models, first deploy them from the model catalog. The deployments will get
invoked by Model router if they're selected for routing.

───┬────────────────────────────┬───────────────────────────────────────────────────┬───────────────────────────────────
Mod│Format                      │Model                                              │Version                            
el │                            │                                                   │                                   
rou│                            │                                                   │                                   
ter│                            │                                                   │                                   
ver│                            │                                                   │                                   
sio│                            │                                                   │                                   
n  │                            │                                                   │                                   
───┼────────────────────────────┼───────────────────────────────────────────────────┼───────────────────────────────────
`20│OpenAI                      │`gpt-4.0`                                          │`2024-11-20`                       
25-│OpenAI                      │`gpt-4.0-mini`                                     │`2024-07-18`                       
11-│OpenAI                      │`gpt-4.1`                                          │`2025-04-14`                       
18`│OpenAI                      │`gpt-4.1-mini`                                     │`2025-04-14`                       
(la│OpenAI                      │`gpt-4.1-nano`                                     │`2025-04-14`                       
tes│OpenAI                      │`o4-mini`                                          │`2025-04-16`                       
t) │OpenAI                      │`gpt-5-nano`                                       │`2025-08-07`                       
   │OpenAI                      │`gpt-5-mini`                                       │`2025-08-07`                       
   │OpenAI                      │`gpt-5`                                            │`2025-08-07`                       
   │OpenAI                      │`gpt-5-chat`                                       │`2025-08-07`                       
   │OpenAI                      │`gpt-5.2`                                          │`2025-12-11`                       
   │OpenAI                      │`gpt-5.2-chat`                                     │`2025-12-11`                       
   │OpenAI                      │`gpt-5.3-chat`                                     │`2026-03-03`                       
   │OpenAI                      │`gpt-5.4-nano`                                     │`2026-03-17`                       
   │OpenAI                      │`gpt-5.4-mini`                                     │`2026-03-17`                       
   │OpenAI                      │`gpt-5.4`                                          │`2026-03-05`                       
   │OpenAI                      │`gpt-5.5`                                          │`2026-04-24`                       
   │DeepSeek                    │`Deepseek-V3.1`²                                   │`1`                                
   │DeepSeek                    │`Deepseek-V3.2`²                                   │`1`                                
   │OpenAI                      │`gpt-oss-120b`²                                    │`1`                                
   │Meta                        │`Llama-4-Maverick-17B-128E-Instruct-FP8`²          │`1`                                
   │xAI                         │`grok-4`²                                          │`1`                                
   │xAI                         │`grok-4-fast-reasoning`²                           │`1`                                
   │Anthropic                   │`claude-haiku-4-5`³                                │`20251001`                         
   │Anthropic                   │`claude-sonnet-4-5`³                               │`20250929`                         
   │Anthropic                   │`claude-opus-4-1`³                                 │`20250805`                         
   │Anthropic                   │`claude-opus-4-6`³                                 │`1`                                
   │Anthropic                   │`claude-opus-4-7`³                                 │`1`                                
───┼────────────────────────────┼───────────────────────────────────────────────────┼───────────────────────────────────
`20│OpenAI                      │`gpt-4.1`                                          │`2025-04-14`                       
25-│OpenAI                      │`gpt-4.1-mini`                                     │`2025-04-14`                       
08-│OpenAI                      │`gpt-4.1-nano`                                     │`2025-04-14`                       
07`│OpenAI                      │`o4-mini`                                          │`2025-04-16`                       
   │OpenAI                      │`gpt-5`¹                                           │`2025-08-07`                       
   │OpenAI                      │`gpt-5-mini`                                       │`2025-08-07`                       
   │OpenAI                      │`gpt-5-nano`                                       │`2025-08-07`                       
   │OpenAI                      │`gpt-5-chat`                                       │`2025-08-07`                       
───┼────────────────────────────┼───────────────────────────────────────────────────┼───────────────────────────────────
`20│OpenAI                      │`gpt-4.1`                                          │`2025-04-14`                       
25-│OpenAI                      │`gpt-4.1-mini`                                     │`2025-04-14`                       
05-│OpenAI                      │`gpt-4.1-nano`                                     │`2025-04-14`                       
19`│OpenAI                      │`o4-mini`                                          │`2025-04-16`                       
───┴────────────────────────────┴───────────────────────────────────────────────────┴───────────────────────────────────
* ¹Requires registration.
* ²Model router support is in preview.
* ³Model router support is in preview. Requires deployment of model for use with Model router.

## Routing mode

With the latest version, if you choose custom deployment, you can select the **routing mode** to optimize for quality or
cost while maintaining a baseline level of performance. Setting a routing mode is optional, and if you don’t set one,
your deployment defaults to the Balanced mode.

Available routing modes:

──────────────────┬────────────────────────────────────────────────────────────────────────────────────
Mode              │Description                                                                         
──────────────────┼────────────────────────────────────────────────────────────────────────────────────
Balanced (default)│Considers both cost and quality dynamically. Perfect for general-purpose scenarios  
──────────────────┼────────────────────────────────────────────────────────────────────────────────────
Quality           │Prioritizes for maximum accuracy. Best for complex reasoning or critical outputs    
──────────────────┼────────────────────────────────────────────────────────────────────────────────────
Cost              │Prioritizes for more cost savings. Ideal for high-volume, budget-sensitive workloads
──────────────────┴────────────────────────────────────────────────────────────────────────────────────

## Govern model router deployments

If your organization uses Azure Policy to control which models can be deployed, model router honors the same built-in
Foundry model deployment policy that governs standard model deployments. The policy applies to the model subset that a
developer can include in a model router deployment, and it's enforced consistently across the Foundry portal, REST API,
Azure CLI, and ARM templates. For the IT admin assignment steps and the developer experience, see [Govern model router
deployments with Azure Policy][12].

## Model subset

The latest version of model router supports model subsets: You can specify which underlying models to include in routing
decisions. This gives you more control over cost, compliance, and performance characteristics.

When new base models become available, they're not included in your selection unless you explicitly add them to your
deployment's inclusion list.

## Automatic failover

Model router now includes built-in automatic failover. When using the default deployment to route to all supported
models, model router transparently redirects the request to the next most appropriate model, so transient issues with
any single model don't disrupt your application. Failover is enabled by default — no additional configuration is
required.

For custom deployment configurations:
* Your selected routing mode (Balanced, Cost, or Quality) continues to apply during failover.
* Your configured model subset also works as your fallback set to prevent your prompts getting processed by unapproved
  models. Therefore, be sure to select model subsets with at least two models to benefit from the fallback capability.

## Prompt caching

Model router supports prompt caching because requests are processed by the underlying models that support it. When model
router delegates a request to a model that supports prompt caching, cached tokens are used automatically — no extra
configuration is needed.

Cache behavior depends on which underlying model the router selects for a given request. Because routing decisions might
vary, caching benefits apply only when the same model handles consecutive requests with overlapping prompt prefixes.

For details on how prompt caching works and which models support it, see [Prompt caching][13].

## Limitations

### Resource limitations

──────────────┬───────────────────────────────────
Region        │Deployment types supported         
──────────────┼───────────────────────────────────
East US 2     │Global Standard, Data Zone Standard
──────────────┼───────────────────────────────────
Sweden Central│Global Standard, Data Zone Standard
──────────────┴───────────────────────────────────

Also see [Azure OpenAI in Microsoft Foundry models][14] for current region availability.

To overcome the limits on context window and parameters, use the Model subset feature to select your models for routing
that support your desired properties.

Note

The context window limit listed for model router is the limit of the smallest underlying model. Other underlying models
are compatible with larger context windows, which means an API call with a larger context will succeed only if the
prompt happens to be routed to the right model. To review context windows for the underlying models, see [Azure OpenAI
in Microsoft Foundry models][15].

To shorten the context window, you can do one of the following:
* Summarize the prompt before passing it to the model
* Truncate the prompt into more relevant parts
* Use document embeddings and have the chat model retrieve relevant sections. For more information, see [What is Azure
  AI Search?][16]

### Quota tiers

Model router limits scale with your subscription's usage tier. For information on how tiers work, see [Quota tiers][17].

──────┬──────────────────┬──────────────────┬────────────────────┬────────────────────
Tier  │GlobalStandard RPM│GlobalStandard TPM│DataZoneStandard RPM│DataZoneStandard TPM
──────┼──────────────────┼──────────────────┼────────────────────┼────────────────────
Tier 1│1,000             │1,000,000         │300                 │300,000             
──────┼──────────────────┼──────────────────┼────────────────────┼────────────────────
Tier 2│2,000             │2,000,000         │670                 │670,000             
──────┼──────────────────┼──────────────────┼────────────────────┼────────────────────
Tier 3│4,000             │4,000,000         │1,000               │1,000,000           
──────┼──────────────────┼──────────────────┼────────────────────┼────────────────────
Tier 4│7,000             │7,000,000         │2,000               │2,000,000           
──────┼──────────────────┼──────────────────┼────────────────────┼────────────────────
Tier 5│10,000            │10,000,000        │3,000               │3,000,000           
──────┼──────────────────┼──────────────────┼────────────────────┼────────────────────
Tier 6│15,000            │15,000,000        │4,000               │4,000,000           
──────┴──────────────────┴──────────────────┴────────────────────┴────────────────────

For other rate limit information, see [Quotas and limits][18].

Model router accepts image inputs for [Vision enabled chats][19] (all of the underlying models can accept image input),
but the routing decision is based on the text input only.

Model router doesn't process audio input.

## Troubleshooting

──────────────────────────┬──────────────────────────────────────────────────────────────────────────────────────────
Issue                     │Resolution                                                                                
──────────────────────────┼──────────────────────────────────────────────────────────────────────────────────────────
Deployment fails          │Verify your Foundry resource is in East US 2 or Sweden Central.                           
──────────────────────────┼──────────────────────────────────────────────────────────────────────────────────────────
Claude models not routing │Ensure Claude models are deployed separately before enabling in model router.             
──────────────────────────┼──────────────────────────────────────────────────────────────────────────────────────────
Context exceeded error    │Reduce prompt size or use model subset to select models with larger context windows.      
──────────────────────────┼──────────────────────────────────────────────────────────────────────────────────────────
Unexpected model selection│Review your routing mode setting (Balanced, Cost, Quality) and model subset configuration.
──────────────────────────┴──────────────────────────────────────────────────────────────────────────────────────────

For detailed deployment troubleshooting, see [How to use model router][20].

## Billing information

Model router usage is charged for input prompts at the rate listed on the pricing page.

You can monitor the costs of your model router deployment in the Azure portal.

## Next step

[How to use model router][21]

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-06-02

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][22]
[ Your Privacy Choices][23]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][24]
* [Previous Versions][25]
* [Blog][26]
* [Contribute][27]
* [Privacy][28]
* [Consumer Health Privacy][29]
* [Terms of Use][30]
* [Trademarks][31]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/concepts/model-router.md
[7]: #
[8]: ../how-to/model-router
[9]: model-router-how-it-works
[10]: #model-subset
[11]: ../how-to/working-with-models#model-updates
[12]: ../../how-to/model-router-policy
[13]: ../how-to/prompt-caching
[14]: ../../foundry-models/concepts/models-sold-directly-by-azure
[15]: ../../foundry-models/concepts/models-sold-directly-by-azure
[16]: ../../../search/search-what-is-azure-search
[17]: ../quotas-limits#quota-tiers
[18]: ../quotas-limits
[19]: ../how-to/gpt-with-vision
[20]: ../how-to/model-router
[21]: ../how-to/model-router
[22]: #
[23]: https://aka.ms/yourcaliforniaprivacychoices
[24]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[25]: https://learn.microsoft.com/en-us/previous-versions/
[26]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[27]: https://learn.microsoft.com/en-us/contribute
[28]: https://go.microsoft.com/fwlink/?LinkId=521839
[29]: https://go.microsoft.com/fwlink/?linkid=2259814
[30]: https://learn.microsoft.com/en-us/legal/termsofuse
[31]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
