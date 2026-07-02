# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/agents/concepts/limits-quotas-regions
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:43:58.641105763+00:00

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

# Foundry Agent Service limits, quotas, and regional support

Feedback
Summarize this article for me

## In this article

Foundry Agent Service enforces quotas and limits on agent artifacts, file uploads, messages, and tool registrations.
Understanding these limits helps you design applications that scale without hitting service boundaries. This article
lists default limits, supported regions, compatible models, and guidance for handling limit errors.

Note

Foundry Agent Service is generally available (GA). Some sub-features, such as [Hosted agents][8], are in public preview
and might have different constraints.

## Prerequisites
* An Azure subscription.
* A [Microsoft Foundry project][9].
* A deployed model compatible with Agent Service. Model and region availability can vary.

## Supported regions

Foundry agent service is only available for Foundry projects created in regions that support the [Azure OpenAI Responses
API][10]. Your Foundry project must be in one of these regions to use Agent Service. Some Azure OpenAI models may not be
available in the same regions. See [Region availability for Foundry Models sold by Azure][11] for details.

Important

Not all tools are available in every region. For example, file search isn't available in Italy North and Brazil South.
For the full tool-by-region matrix, see [Tool support by region and model][12].

In addition to Azure OpenAI models, Agent Service supports models from the Foundry model catalog. These models are
deployed and managed through Foundry and follow separate quotas. The following models are available for your agents to
use.

**Foundry Models sold by Azure:**
* **MAI-DS-R1**: Deterministic, precision-focused reasoning.
* **grok-4**: Frontier-scale reasoning for complex, multiple-step problem solving.
* **grok-4-fast-reasoning**: Accelerated agentic reasoning optimized for workflow automation.
* **grok-4-fast-non-reasoning**: High-throughput, low-latency generation and system routing.
* **grok-3**: Strong reasoning for complex, system-level workflows.
* **grok-3-mini**: Lightweight model optimized for interactive, high-volume use cases.
* **Llama-3.3-70B-Instruct**: Versatile model for enterprise Q&A, decision support, and system orchestration.
* **Llama-4-Maverick-17B-128E-Instruct-FP8**: FP8-optimized model that delivers fast, cost-efficient inference.
* **DeepSeek-V3-0324**: Multimodal understanding across text and images.
* **DeepSeek-V3.1**: Enhanced multimodal reasoning and grounded retrieval.
* **DeepSeek-V3.2**: Model that harmonizes high computational efficiency with superior reasoning and agent performance.
* **DeepSeek-V3.2-Speciale**: Specialized DeepSeek-V3.2 variant.
* **DeepSeek-R1-0528**: Advanced long-form and multiple-step reasoning.
* **gpt-oss-120b**: Open-ecosystem model that supports transparency and reproducibility.

Tip

Model availability can change over time. To verify what you can deploy for your project and region, use the Foundry
portal model experience.

### Sovereign clouds

Foundry Agent Service is also available in Azure Government (US Gov Virginia and US Gov Arizona) with a subset of agent
types and tools. For the full list of supported features, see [Foundry Agent Service feature availability in Azure
Government][13].

## Troubleshooting

### A model or version isn't available in your region
* Confirm you selected the right tab for your deployment type (global standard vs. provisioned).
* Try a different region that supports the [model and Responses API][14].
* If you're using gpt-5 models, [registration][15] is required. Access is granted according to Microsoft's eligibility
  criteria.

### A tool isn't available in your region
* Not all tools are supported in every region. For example, file search isn't available in Italy North and Brazil South,
  and code interpreter isn't available in all regions.
* Check the [tool support by region and model][16] table to confirm availability before you deploy.
* If a tool isn't available, choose a supported region or use a different tool.

### Provisioned throughput deployment fails
* Confirm you have enough PTUs available in the region.
* Review [Provisioned throughput][17] and [Spillover traffic management][18].

### Agent receives rate-limit (429) errors
* Implement exponential backoff with jitter in your application retry logic.
* For sustained high-throughput workloads, consider provisioned throughput deployments.
* Review [Azure OpenAI quotas and limits][19] for your deployment's tokens-per-minute and requests-per-minute caps.

## Quotas and limits

Foundry Agent Service enforces limits in two places:
* **Agent Service limits.** Limits for agent and thread artifacts, such as file uploads, vector store attachments,
  message counts, and tool registration.
* **Model limits.** Quotas and rate limits for the model deployments your agents call.

If you're using threads and messages, see [Threads, runs, and messages in Foundry Agent Service][20]. If you're using
file search, see [Vector stores for file search][21].

## Default quotas and limits for the service

The following table lists default limits enforced by the Agent Service. These limits apply to all Foundry projects
regardless of subscription type or region.

───────────────────────────────────────────────────────────┬────────────────────
Limit name                                                 │Limit value         
───────────────────────────────────────────────────────────┼────────────────────
Maximum number of files per agent/thread                   │10,000              
───────────────────────────────────────────────────────────┼────────────────────
Maximum file size for agents                               │512 MB              
───────────────────────────────────────────────────────────┼────────────────────
Maximum size for all uploaded files for agents             │300 GB              
───────────────────────────────────────────────────────────┼────────────────────
Maximum file size in tokens for attaching to a vector store│2,000,000 tokens    
───────────────────────────────────────────────────────────┼────────────────────
Maximum number of messages per thread                      │100,000             
───────────────────────────────────────────────────────────┼────────────────────
Maximum size of `text` content per message                 │1,500,000 characters
───────────────────────────────────────────────────────────┼────────────────────
Maximum number of tools registered per agent               │128                 
───────────────────────────────────────────────────────────┴────────────────────

The Agent Service limits in this table are fixed and apply uniformly across all subscription types. Agent Service
doesn't impose separate rate limits on API calls. Rate limiting is applied at the model deployment level. See [Azure
OpenAI quotas and limits][22] for model-specific rate limits.

## Limit error reference

When you exceed a limit, the Agent Service returns an error. Handle these errors gracefully in your application.

─────────────────────────┬───────────┬────────────────────────┬──────────────────────────────────
Error scenario           │HTTP status│Error code              │Recommended action                
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
File too large           │400        │`file_size_exceeded`    │Split content into smaller files  
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Vector store token limit │400        │`token_limit_exceeded`  │Reduce file content or split files
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Thread message cap       │400        │`message_limit_exceeded`│Create a new thread               
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Message content too large│400        │`content_size_exceeded` │Use file search for large content 
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Too many tools           │400        │`tool_limit_exceeded`   │Remove unused tools               
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Rate limit exceeded      │429        │`rate_limit_exceeded`   │Implement exponential backoff     
─────────────────────────┴───────────┴────────────────────────┴──────────────────────────────────

For example:
* **File exceeds the maximum size.** Uploading the file fails. Split the content into smaller files or reduce file size
  before you upload.
* **Vector store token limit.** Attaching a file to a vector store fails if the file exceeds the token limit. Reduce the
  file content or split it into multiple files.
* **Thread message cap.** Adding messages can fail after a thread reaches the message limit. Create a new thread for a
  new conversation session, or archive and rotate threads as part of your application design.
* **Message content size.** Creating a message can fail if the `text` content is too large. Send smaller messages, or
  move large content into files and use file search.
* **Tool registration cap.** Creating or updating an agent can fail if you register too many tools. Register only the
  tools you need, and prefer fewer, reusable tools.
* **Rate limit exceeded.** API calls to the model deployment are throttled. Implement exponential backoff with jitter.

For file search scenarios, see [Vector stores for file search][23] for guidance on managing vector store growth.

## Best practices to stay within limits

Use the following practices to reduce limit-related failures:
* **Keep files small and focused.** Prefer multiple smaller documents over a single large document.
* **Avoid very large messages.** Put long content in uploaded files and query it by using file search.
* **Plan for long conversations.** Treat threads as session state and rotate to new threads when conversations become
  very long.
* **Register only required tools.** Remove unused tools from agent definitions.
* **Monitor usage trends.** Track agent activity by using [Foundry Agent Service metrics][24] to identify growth before
  you hit limits.

## Quotas and limits for models

Agents follow the quotas and rate limits for the model deployments they use.

For current model quotas and limits, see:
* [Azure OpenAI quotas and limits][25].
* [Microsoft Foundry Models quotas and limits][26].

To view or request more model quota, see [Manage and increase quotas for resources with Microsoft Foundry (Foundry
projects)][27].

## Request a limit increase

The limits in this article are default values for Foundry Agent Service. If your workload requires higher limits:
* **Model quotas.** You can request increases for model deployment quotas. See [Manage and increase quotas for resources
  with Microsoft Foundry][28].
* **Agent Service limits.** The file, message, and tool limits listed in this article are fixed service limits and can't
  be increased. Design your application to work within these constraints by using the best practices described earlier.

## Related content
* [Threads, runs, and messages in Foundry Agent Service][29]
* [Tool support by region and model][30]
* [Vector stores for file search][31]
* [Monitor Foundry Agent Service][32]
* [Azure OpenAI quotas and limits][33]
* [Manage and increase quotas for resources with Microsoft Foundry][34]

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-06-22

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][35]
[ Your Privacy Choices][36]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][37]
* [Previous Versions][38]
* [Blog][39]
* [Contribute][40]
* [Privacy][41]
* [Consumer Health Privacy][42]
* [Terms of Use][43]
* [Trademarks][44]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/limits-quotas-regions.md
[7]: #
[8]: hosted-agents
[9]: ../../how-to/create-projects
[10]: ../../openai/how-to/responses#supported-regions
[11]: ../../foundry-models/concepts/models-sold-directly-by-azure-region-availability
[12]: tool-best-practice#tool-support-by-region-and-model
[13]: azure-government
[14]: #supported-regions
[15]: https://aka.ms/openai/gpt-5/2025-08-07
[16]: tool-best-practice#tool-support-by-region-and-model
[17]: ../../openai/concepts/provisioned-throughput
[18]: ../../openai/how-to/spillover-traffic-management
[19]: ../../openai/quotas-limits
[20]: runtime-components
[21]: vector-stores
[22]: ../../openai/quotas-limits
[23]: vector-stores
[24]: ../../observability/how-to/how-to-monitor-agents-dashboard
[25]: ../../openai/quotas-limits
[26]: ../../foundry-models/quotas-limits
[27]: ../../how-to/quota
[28]: ../../how-to/quota
[29]: runtime-components
[30]: tool-best-practice#tool-support-by-region-and-model
[31]: vector-stores
[32]: ../../observability/how-to/how-to-monitor-agents-dashboard
[33]: ../../openai/quotas-limits
[34]: ../../how-to/quota
[35]: #
[36]: https://aka.ms/yourcaliforniaprivacychoices
[37]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[38]: https://learn.microsoft.com/en-us/previous-versions/
[39]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[40]: https://learn.microsoft.com/en-us/contribute
[41]: https://go.microsoft.com/fwlink/?LinkId=521839
[42]: https://go.microsoft.com/fwlink/?linkid=2259814
[43]: https://learn.microsoft.com/en-us/legal/termsofuse
[44]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
