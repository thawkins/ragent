# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/concepts/evaluation-regions-limits-virtual-network
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:43:59.254903435+00:00

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

# Rate limits, region support, and enterprise features for evaluation

Feedback
Summarize this article for me

## In this article

This article provides an overview of which regions support AI-assisted evaluators, the rate limits that apply to
evaluation runs, how to configure virtual network support for network isolation, and using your own storage account to
run evaluations.

## Regional availability

### Supported regions for Agent playground evaluations

The agent playground evaluations are supported in the following regions:

────────────────┬────────────────────
Americas        │Europe              
────────────────┼────────────────────
East US 2       │France Central      
────────────────┼────────────────────
West US         │Norway East         
────────────────┼────────────────────
West US 2       │Sweden Central      
────────────────┼────────────────────
West US 3       │Germany West Central
────────────────┼────────────────────
Central US      │Italy North         
────────────────┼────────────────────
East US         │Poland Central      
────────────────┼────────────────────
North Central US│Spain Central       
────────────────┼────────────────────
South Central US│                    
────────────────┴────────────────────

### Supported regions for batch evaluations

The batch evaluations are supported in the following regions:

────────────────┬────────────────────┬──────────────┬────────────────────
Americas        │Europe              │Asia Pacific  │Middle East & Africa
────────────────┼────────────────────┼──────────────┼────────────────────
Brazil South    │France Central      │Australia East│South Africa North  
────────────────┼────────────────────┼──────────────┼────────────────────
Canada Central  │Germany West Central│Central India │UAE North           
────────────────┼────────────────────┼──────────────┼────────────────────
Canada East     │Italy North         │East Asia     │                    
────────────────┼────────────────────┼──────────────┼────────────────────
Central US      │North Europe        │Japan East    │                    
────────────────┼────────────────────┼──────────────┼────────────────────
East US         │Norway East         │Japan West    │                    
────────────────┼────────────────────┼──────────────┼────────────────────
East US 2       │Poland Central      │Korea Central │                    
────────────────┼────────────────────┼──────────────┼────────────────────
North Central US│Spain Central       │South India   │                    
────────────────┼────────────────────┼──────────────┼────────────────────
South Central US│Sweden Central      │Southeast Asia│                    
────────────────┼────────────────────┼──────────────┼────────────────────
West Central US │Switzerland North   │              │                    
────────────────┼────────────────────┼──────────────┼────────────────────
West US         │UK South            │              │                    
────────────────┼────────────────────┼──────────────┼────────────────────
West US 2       │West Europe         │              │                    
────────────────┼────────────────────┼──────────────┼────────────────────
West US 3       │                    │              │                    
────────────────┴────────────────────┴──────────────┴────────────────────

### Risk and safety evaluators and AI red teaming region support

The following safety evaluators and AI red teaming are supported in these regions: Hate and unfairness, Sexual, Violent,
Self-harm, Indirect attack, Code vulnerabilities, Ungrounded attributes, and AI red teaming.

────────────────┬────────────────┬──────────────
Americas        │Europe          │Asia Pacific  
────────────────┼────────────────┼──────────────
East US 2       │France Central  │Australia East
────────────────┼────────────────┼──────────────
North Central US│Sweden Central  │              
────────────────┼────────────────┼──────────────
                │Switzerland West│              
────────────────┴────────────────┴──────────────

Supported regions for Groundedness Pro:
* East US 2
* Sweden Central

Supported regions for Protected material:
* East US 2

### Azure OpenAI graders regional availability

For the Azure OpenAI graders regional list, see [Regional availability][8].

## Rate limits

The following rate limits apply to evaluation runs:

─────────────────────────────────┬───────
Limit                            │Value  
─────────────────────────────────┼───────
Maximum size per row             │2 MB   
─────────────────────────────────┼───────
Maximum rows per batch evaluation│100,000
─────────────────────────────────┴───────

Evaluation run creations are rate-limited at the tenant, subscription, and project levels. If you exceed the limit:
* The response includes a `retry-after` header with the wait time.
* The response body contains rate limit details.

Use exponential backoff when retrying failed requests.

## Virtual network support for evaluation

For network isolation, you can bring your own virtual network for evaluation. To learn more, see [How to configure a
private link][9].

Virtual network support for evaluation requires network injection (subnet delegation), but if you **only need evaluation
capabilities** and do not require full agent support (Cosmos DB, AI Search, or project capability host), consider using
the simplified [evaluation-only setup template (15a)][10] instead. It deploys a minimal network-secured environment
tailored for evaluation scenarios with fewer resources and reduced complexity.

Note

If you connect Application Insights, evaluation data is sent to it.

Important

To prevent evaluation and red teaming run failures, assign the Foundry User role to the project's Managed Identity
during initial project setup.

Important

The Foundry RBAC roles were recently renamed. **Foundry User**, **Foundry Owner**, **Foundry Account Owner**, and
**Foundry Project Manager** were previously named Azure AI User, Azure AI Owner, Azure AI Account Owner, and Azure AI
Project Manager. You might still see the previous names in some places while the rename rolls out. The role IDs and core
permissions are unchanged by the rename.

### Virtual network region support

Bringing your own virtual network for evaluation is supported in the following regions:

────────────────┬────────────────────┬──────────────┬────────────────────
Americas        │Europe              │Asia Pacific  │Middle East & Africa
────────────────┼────────────────────┼──────────────┼────────────────────
Brazil South    │France Central      │Australia East│South Africa North  
────────────────┼────────────────────┼──────────────┼────────────────────
Canada Central  │Germany West Central│Japan East    │UAE North           
────────────────┼────────────────────┼──────────────┼────────────────────
Canada East     │Italy North         │Korea Central │                    
────────────────┼────────────────────┼──────────────┼────────────────────
East US         │Norway East         │South India   │                    
────────────────┼────────────────────┼──────────────┼────────────────────
East US 2       │Poland Central      │Southeast Asia│                    
────────────────┼────────────────────┼──────────────┼────────────────────
North Central US│Spain Central       │              │                    
────────────────┼────────────────────┼──────────────┼────────────────────
South Central US│Sweden Central      │              │                    
────────────────┼────────────────────┼──────────────┼────────────────────
West US         │Switzerland North   │              │                    
────────────────┼────────────────────┼──────────────┼────────────────────
West US 2       │UK South            │              │                    
────────────────┼────────────────────┼──────────────┼────────────────────
West US 3       │West Europe         │              │                    
────────────────┴────────────────────┴──────────────┴────────────────────

## Bring your own storage

You can use your own storage account to run evaluations for your Foundry project, whether the project is configured with
a virtual network or without one.

For projects without a virtual network, you can use the storage account connection template. For projects with a virtual
network, the storage setup is already included in the [evaluation-only setup template (15a)][11].
1. For projects without a virtual network, create and connect your storage account to your Foundry project at the
   resource level. You can [use a Bicep template][12], which provisions and connects a storage account to your Foundry
   project with key authentication.
2. Make sure the connected storage account has access to all projects.
3. If you connected your storage account by using Microsoft Entra ID, make sure to give managed identity **Storage Blob
   Data Owner** permissions to both your account and the Foundry project resource in the Azure portal.

## Related content
* [How to configure a private link][13]
* [Observability for generative AI applications][14]
* [Assign Azure roles for access to blob data][15]

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-06-15

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][16]
[ Your Privacy Choices][17]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][18]
* [Previous Versions][19]
* [Blog][20]
* [Contribute][21]
* [Privacy][22]
* [Consumer Health Privacy][23]
* [Terms of Use][24]
* [Trademarks][25]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/concepts/evaluation-regions-limits-virtua
l-network.md
[7]: #
[8]: ../../foundry-classic/openai/how-to/evaluations#regional-availability
[9]: ../how-to/configure-private-link
[10]: https://github.com/microsoft-foundry/foundry-samples/tree/main/infrastructure/infrastructure-setup-bicep/15a-priva
te-network-evaluation-only-setup
[11]: https://github.com/microsoft-foundry/foundry-samples/tree/main/infrastructure/infrastructure-setup-bicep/15a-priva
te-network-evaluation-only-setup
[12]: https://github.com/microsoft-foundry/foundry-samples/blob/main/infrastructure/infrastructure-setup-bicep/01-connec
tions/connection-storage-account.bicep
[13]: ../how-to/configure-private-link
[14]: observability
[15]: /en-us/azure/storage/blobs/assign-azure-role-data-access
[16]: #
[17]: https://aka.ms/yourcaliforniaprivacychoices
[18]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[19]: https://learn.microsoft.com/en-us/previous-versions/
[20]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[21]: https://learn.microsoft.com/en-us/contribute
[22]: https://go.microsoft.com/fwlink/?LinkId=521839
[23]: https://go.microsoft.com/fwlink/?linkid=2259814
[24]: https://learn.microsoft.com/en-us/legal/termsofuse
[25]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
