# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/foundry-models/quotas-limits
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:41:33.158215751+00:00

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

# Microsoft Foundry Models quotas and limits

Feedback
Summarize this article for me

## In this article

This article provides a quick reference and detailed description of the quotas and limits for [Foundry Models sold by
Azure][8]. For quotas and limits specific to the Azure OpenAI in Foundry Models, see [Quotas and limits in Azure
OpenAI][9].

## Updates to quota management after 05/07/2026

Microsoft Foundry is introducing an update to quota management to bring consistency and predictability to how quota is
managed across deployments. Starting with Realtime Translate and Realtime Whisper, quota for deployments is tracked at
the subscription level—shared across all resources and regions—rather than being allocated separately per resource or
per region.

This change consolidates quota into shared pools:
* Global Standard: Deployments of the same model and version share one quota pool across all regions in a subscription.
* Data Zone Standard: Deployments of the same model and version share one quota pool per data zone (for example, US or
  EU).

## What’s changing for me?

For the models that are onboarded to the new quota management system:
* All Global Standard deployments of the same model and version under a subscription now draw from a single shared quota
  pool across all regions.
* All Data Zone Standard deployments of the same model and version under a subscription now draw from a shared quota
  pool within each data zone.
* Existing approved quota is retained and automatically applies at the subscription level—no action required.

This consolidation allows Microsoft Foundry to offer supported models consistently across all Foundry regions,
regardless of how quota is distributed across resources or regions.

Important

The updated quota management currently applies only to Realtime Translate and Realtime Whisper. For all other Foundry
Models covered in this article, quotas and limits are managed per region, per subscription, and per model or deployment
type. In the future, these quota guidelines will also apply to some existing models and to new Foundry Model launches.

## Quotas and limits reference

The following sections provide a quick guide to the default quotas and limits that apply to Foundry Models. Quotas and
limits aren't enforced at the tenant level. Instead, the highest level of quota restrictions is scoped at the Azure
subscription level. Tokens per minute (TPM) and requests per minute (RPM) limits are defined per region, per
subscription, and per model or deployment type.

### Resource limits (per Azure subscription, per region)

──────────────────────────────────────────────────────────────────────────┬───────────
Limit name                                                                │Limit value
──────────────────────────────────────────────────────────────────────────┼───────────
Foundry resources per region per Azure subscription                       │100        
──────────────────────────────────────────────────────────────────────────┼───────────
Max projects per resource                                                 │250        
──────────────────────────────────────────────────────────────────────────┼───────────
Max deployments per resource (model deployments within a Foundry resource)│32         
──────────────────────────────────────────────────────────────────────────┴───────────

### Rate limits

The following table lists limits for Foundry Models for the following rates:
* Tokens per minute
* Requests per minute
* Concurrent request

───────────────────────────────────────────┬────────────────────────────┬─────────────────────────────┬─────────────────
Models                                     │Tokens per minute           │Requests per minute          │Concurrent       
                                           │                            │                             │requests         
───────────────────────────────────────────┼────────────────────────────┼─────────────────────────────┼─────────────────
Azure OpenAI models                        │Varies per model and SKU.   │Varies per model and SKU. See│Varies. See      
                                           │See [limits for Azure       │[limits for Azure            │[Azure OpenAI    
                                           │OpenAI][10].                │OpenAI][11].                 │limits][12].     
───────────────────────────────────────────┼────────────────────────────┼─────────────────────────────┼─────────────────
- DeepSeek-R1                              │5,000,000                   │5,000                        │300              
- DeepSeek-V3-0324                         │                            │                             │                 
───────────────────────────────────────────┼────────────────────────────┼─────────────────────────────┼─────────────────
- Llama 3.3 70B Instruct                   │400,000                     │1,000                        │300              
- Llama-4-Maverick-17B-128E-Instruct-FP8   │                            │                             │                 
- Grok 3                                   │                            │                             │                 
- Grok 3 mini                              │                            │                             │                 
───────────────────────────────────────────┼────────────────────────────┼─────────────────────────────┼─────────────────
- Flux.2-Pro                               │not applicable              │- Low (Default): 15          │not applicable   
                                           │                            │- Medium: 30                 │                 
                                           │                            │- High (Enterprise): 100     │                 
───────────────────────────────────────────┼────────────────────────────┼─────────────────────────────┼─────────────────
- Flux-Pro 1.1                             │not applicable              │2 capacity units (6 requests │not applicable   
- Flux.1-Kontext Pro                       │                            │per minute)                  │                 
───────────────────────────────────────────┼────────────────────────────┼─────────────────────────────┼─────────────────
Rest of models                             │400,000                     │1,000                        │300              
───────────────────────────────────────────┴────────────────────────────┴─────────────────────────────┴─────────────────

To increase your quota, use [Microsoft Foundry Service: Request for Quota Increase][13] to submit your request. Due to
high demand, requests to increase quota are evaluated individually. For more information on quota increase requests, see
[request increases to the default limits][14].

### Other limits

─────────────────────────────────────────────┬───────────
Limit name                                   │Limit value
─────────────────────────────────────────────┼───────────
Max number of custom headers in API requests¹│10         
─────────────────────────────────────────────┴───────────

¹ Current APIs allow up to 10 custom headers, which the pipeline passes through and returns. If you exceed this header
count, your request results in an HTTP 431 error. To resolve this error, reduce the header volume. **Future API versions
won't pass through custom headers**. Don't depend on custom headers in future system architectures.

## Usage tiers

Global Standard deployments use Azure's global infrastructure to dynamically route customer traffic to the data center
with best availability for the customer's inference requests. This infrastructure enables more consistent latency for
customers with low to medium levels of traffic. Customers with high sustained levels of usage might see more
variabilities in response latency.

The Usage Limit determines the level of usage beyond which customers might see larger variability in response latency. A
customer's usage is defined per model and is the total tokens consumed across all deployments in all subscriptions in
all regions for a given tenant.

## Request increases to the default limits

Submit the [quota increase request form][15] to request quota increases for [Foundry Models sold by Azure][16], Azure
OpenAI models, and Anthropic models. Except for Anthropic models, [Models from partners and community][17] don't support
quota increases.

Quota increase requests are processed in the order they're received, and priority goes to customers who actively use
their existing quota allocation. Requests that don't meet this condition might be denied.

## General best practices to stay within rate limits

To minimize issues related to rate limits, use the following techniques:
* Implement retry logic in your application.
* Avoid sharp changes in the workload. Increase the workload gradually.
* Test different load increase patterns.
* Increase the quota assigned to your deployment. Move quota from another deployment, if necessary.

## Setting client-side timeout

Set the client-side timeout explicitly based on the following guidance.

Note

If not explicitly set, the client side timeout exists as per the library used, and might not be the same limits as
above.
* Reasoning models (models that generate intermediate reasoning tokens before producing a summarized response): up to 29
  minutes.
* Non-reasoning models:
  * For streaming, up to 60 seconds.
  * For non-streaming requests, up to 29 minutes.

29 minutes here doesn't mean all requests take 29 minutes but rather depending on context tokens, generated tokens, and
cache hit rates, requests can take up to 29 minutes.

Set a timeout that's less than these values, tuned to your traffic patterns.

For reasoning models including streaming requests, all the reasoning tokens are first generated and then summarized
before sending the first response token back to the user.

You can modify the [reasoning effort][18] parameter to control the number of reasoning tokens generated in the process.

## Troubleshooting

─────────────────────────┬───────────────────────────────────┬──────────────────────────────────────────────────────────
Symptom                  │Cause                              │Resolution                                                
─────────────────────────┼───────────────────────────────────┼──────────────────────────────────────────────────────────
HTTP 429 Too Many        │Token-per-minute or                │Implement retry logic with exponential backoff. Use the   
Requests                 │request-per-minute limit exceeded  │`Retry-After` header value.                               
─────────────────────────┼───────────────────────────────────┼──────────────────────────────────────────────────────────
HTTP 431 Request Header  │More than 10 custom headers sent   │Reduce custom headers to 10 or fewer.                     
Fields Too Large         │                                   │                                                          
─────────────────────────┼───────────────────────────────────┼──────────────────────────────────────────────────────────
Quota page shows 0       │Subscription or regional quota     │Move unused quota from another deployment. To increase    
available                │fully allocated                    │your limit, [request a quota increase][19].               
─────────────────────────┼───────────────────────────────────┼──────────────────────────────────────────────────────────
Model not available in   │Model isn't deployed or supported  │Check [model availability][20] and choose an available    
region                   │in the selected region             │region.                                                   
─────────────────────────┴───────────────────────────────────┴──────────────────────────────────────────────────────────

## Related content
* [Models available in Foundry Models][21]
* [Manage and increase quotas for Foundry resources][22]
* [Quotas and limits in Azure OpenAI][23]

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-05-18

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][24]
[ Your Privacy Choices][25]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][26]
* [Previous Versions][27]
* [Blog][28]
* [Contribute][29]
* [Privacy][30]
* [Consumer Health Privacy][31]
* [Terms of Use][32]
* [Trademarks][33]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/foundry-models/quotas-limits.md
[7]: #
[8]: concepts/models-sold-directly-by-azure
[9]: ../openai/quotas-limits
[10]: ../openai/quotas-limits
[11]: ../openai/quotas-limits
[12]: ../openai/quotas-limits
[13]: https://aka.ms/oai/stuquotarequest
[14]: #request-increases-to-the-default-limits
[15]: https://aka.ms/oai/stuquotarequest
[16]: concepts/models-sold-directly-by-azure
[17]: concepts/models-from-partners
[18]: ../openai/how-to/reasoning
[19]: #request-increases-to-the-default-limits
[20]: concepts/models-sold-directly-by-azure
[21]: concepts/models-sold-directly-by-azure
[22]: ../../foundry-classic/openai/how-to/quota
[23]: ../openai/quotas-limits
[24]: #
[25]: https://aka.ms/yourcaliforniaprivacychoices
[26]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[27]: https://learn.microsoft.com/en-us/previous-versions/
[28]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[29]: https://learn.microsoft.com/en-us/contribute
[30]: https://go.microsoft.com/fwlink/?LinkId=521839
[31]: https://go.microsoft.com/fwlink/?linkid=2259814
[32]: https://learn.microsoft.com/en-us/legal/termsofuse
[33]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
