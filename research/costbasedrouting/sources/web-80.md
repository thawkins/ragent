# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/deployment-types
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:44:03.984817622+00:00

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

# Deployment types for Microsoft Foundry Models

Feedback
Summarize this article for me

## In this article

When you deploy a model in Microsoft Foundry, you choose a deployment type that determines:
* **Where your data is processed** (global, data zone, or single region)
* **How you pay** (pay-per-token or reserved capacity)
* **Performance characteristics** (latency variance, throughput limits)

The service offers two main categories: *standard* (pay-per-token) and *provisioned* (reserved capacity). Within each
category, you can choose global, data zone, or regional processing based on your compliance requirements.

Tip

You don't always need to create a deployment. With [instant access (preview)][8], you call supported models by name and
start running inference immediately — no deployment required.

[ [Screenshot of the Foundry portal deployment dialog showing the deployment type selection box with Global Standard
selected.] ][9]

Important

**Data residency for all deployment types**: Data stored at rest remains in the designated Azure geography. However,
inferencing data is processed as follows:
* **Global** types: May be processed in any Azure region
* **DataZone** types: Processed only within the Microsoft-specified data zone (US or EU)
* **Standard/Regional** types: Processed in the deployment region

[Learn more about data residency][10].

## Deployment type comparison

─────────────────────┬───────────────────────┬─────────────┬───────────────────────┬────────────────────────────────────
Deployment type      │SKU code               │Data         │Billing                │Best for                            
                     │                       │processing   │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Instant             │N/A — no deployment    │Any Azure    │Pay-per-token (global  │Getting started, prototyping, trying
(preview)][11]       │needed                 │region       │quota)                 │new models                          
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Global Standard][12]│`GlobalStandard`       │Any Azure    │Pay-per-token          │General workloads, highest quota    
                     │                       │region       │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Global              │`GlobalProvisionedManag│Any Azure    │Reserved PTU           │Predictable high-throughput         
Provisioned][13]     │ed`                    │region       │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Global Batch][14]   │`GlobalBatch`          │Any Azure    │50% discount, 24-hr    │Large async jobs                    
                     │                       │region       │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Data Zone           │`DataZoneStandard`     │Within data  │Pay-per-token          │EU/US data zone compliance          
Standard][15]        │                       │zone         │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Data Zone           │`DataZoneProvisionedMan│Within data  │Reserved PTU           │Data zone + predictable throughput  
Provisioned][16]     │aged`                  │zone         │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Data Zone Batch][17]│`DataZoneBatch`        │Within data  │50% discount           │Large async jobs with data zone     
                     │                       │zone         │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Standard][18]       │`Standard`             │Single region│Pay-per-token          │Regional compliance, low volume     
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Regional            │`ProvisionedManaged`   │Single region│Reserved PTU           │Regional compliance + throughput    
Provisioned][19]     │                       │             │                       │                                    
─────────────────────┼───────────────────────┼─────────────┼───────────────────────┼────────────────────────────────────
[Developer][20]      │`DeveloperTier`        │Any Azure    │Pay-per-token          │Fine-tuned model evaluation only    
                     │                       │region       │                       │                                    
─────────────────────┴───────────────────────┴─────────────┴───────────────────────┴────────────────────────────────────

Note

Not all models support all deployment types. Check [Foundry Models sold by Azure][21] for model availability by
deployment type and region.

Note

SLA guarantees vary by deployment type. Provisioned types provide guaranteed throughput and lower latency variance.
Standard types offer best-effort service. Developer deployments don't include an SLA. For details, see the [Azure SLA
for Azure OpenAI Service][22].

Tip

For detailed pricing, see [Azure OpenAI Service pricing][23].

## Choose the right deployment type

Use the following criteria to select a deployment type:

### By data residency requirement
* **No restrictions**: Use Global Standard or Global Provisioned
* **EU data zone**: Use DataZone Standard or DataZone Provisioned in an EU region
* **US data zone**: Use DataZone Standard or DataZone Provisioned in a US region
* **Single region only**: Use Standard or Regional Provisioned

### By workload pattern
* **Quick start, prototyping, or trying a new model**: Use [instant access (preview)][24] (no deployment needed)
* **Variable, bursty traffic**: Use Standard or Global Standard (pay-per-token)
* **Consistent high volume**: Use Provisioned types (reserved capacity)
* **Large batch jobs (not time-sensitive)**: Use Global Batch or DataZone Batch (50% cost savings)
* **Fine-tuned model evaluation**: Use Developer (no SLA, lowest cost)

### By latency requirement
* **Low latency variance required**: Use Provisioned types
* **Latency variance acceptable**: Use Standard types

## Data processing locations

For standard deployments, there are three options: global, data zone, and Azure geography. For provisioned deployments,
there are two options: global and Azure geography. Global Standard is a common starting point for most workloads.

### Global deployments

Global deployments use Azure's global infrastructure to dynamically route traffic to available datacenters. Global
deployments offer the highest initial throughput limits and broadest model availability.

For high-volume workloads, you might experience increased latency variation. If you require lower latency variance at
scale, use provisioned deployment types.

Global deployments receive new models and features first.

### Data Zone deployments

For **Global** deployment types, prompts and responses might be processed in any geography where the model is deployed.
For **DataZone** deployment types, prompts and responses are processed only within the specified data zone:
* **United States**: Data processed anywhere within the US
* **European Union**: Data processed within the [EU Data Boundary][25]

The EU Data Zone processes data within regions located in countries covered by the [Azure EU Data Boundary][26]. As of
May 2026, this includes regions in: France, Germany, Italy, Netherlands, Norway, Poland, Spain, Sweden, and Switzerland.
Additional regions within the EU Data Boundary may be added without prior notice to improve capacity and availability.

Learn more in the "Model region availability by deployment type" section of [Foundry Models sold by Azure][27].

Note

With Global Standard and Data Zone Standard deployment types, if the primary region experiences an interruption in
service, all traffic initially routed to this region is affected. To learn more, see the [high availability and disaster
recovery guide][28].

## Global Standard
* SKU name in code: `GlobalStandard`

Global Standard deployments use Azure's global infrastructure to dynamically route traffic to available datacenters.
This deployment type provides the highest default quota and eliminates the need to load balance across multiple
resources.

Customers with high consistent volume might experience greater latency variability. The threshold is set per model. To
learn more, see the [Quotas page][29]. For applications that require lower latency variance at large workload usage,
consider provisioned throughput.

Global Standard supports priority processing (preview) for faster response times on a pay-as-you-go basis. To learn
more, see [Priority processing for Foundry models (preview)][30].

## Global Provisioned
* SKU name in code: `GlobalProvisionedManaged`

Global Provisioned deployments use Azure's global infrastructure to dynamically route traffic to available datacenters.
This deployment type provides reserved model processing capacity for predictable throughput, combining global routing
with guaranteed capacity.

With provisioned throughput, you purchase a fixed number of provisioned throughput units (PTUs) that guarantee a
specific level of processing capacity. This deployment type provides lower and more consistent latency than Global
Standard. To learn more, see [Provisioned throughput concepts][31].

## Global Batch
* SKU name in code: `GlobalBatch`

[Global Batch][32] handles large-scale and high-volume processing tasks. You can process asynchronous groups of requests
with separate quota and a 24-hour target turnaround, at [50% less cost than Global Standard][33]. With batch processing,
rather than sending one request at a time, you send a large number of requests in a single file. Global Batch requests
have a separate enqueued token quota, which avoids any disruption of your online workloads.

Common use cases:
* **Large-scale data processing**: Analyze datasets in parallel.
* **Content generation**: Create large volumes of text, such as product descriptions or articles.
* **Document review and summarization**: Process and summarize lengthy documents.
* **Customer support automation**: Handle numerous queries simultaneously.
* **Data extraction and analysis**: Extract and analyze information from large amounts of unstructured data.
* **Natural language processing (NLP) tasks**: Perform sentiment analysis or translation on large datasets.

Note

Batch deployments trade real-time responsiveness for cost savings. Batch requests don't have a real-time SLA — they
target completion within 24 hours but might take longer.

## Data Zone Standard
* SKU name in code: `DataZoneStandard`

Data Zone Standard deployments dynamically route traffic to datacenters within the Microsoft-defined data zone (US or
EU). This deployment type provides higher default quotas than geography-based deployment types while keeping data within
the specified zone.

Customers with high consistent volume might experience greater latency variability. The threshold is set per model. To
learn more, see the [quotas and limits page][34]. For workloads that require low latency variance at large volume,
consider provisioned deployment types.

Data Zone Standard supports priority processing (preview) for faster response times on a pay-as-you-go basis. To learn
more, see [Priority processing for Foundry models (preview)][35].

## Data Zone Provisioned
* SKU name in code: `DataZoneProvisionedManaged`

Data Zone Provisioned deployments dynamically route traffic within the Microsoft-specified data zone (US or EU) while
providing reserved model processing capacity. This deployment type combines data zone compliance with high and
predictable throughput.

## Data Zone Batch
* SKU name in code: `DataZoneBatch`

Data Zone Batch deployments provide the same functionality as [Global Batch][36], including 50% cost savings and 24-hour
turnaround. Traffic is routed only to datacenters within the Microsoft-defined data zone (US or EU).

## Standard
* SKU name in code: `Standard`

Standard deployments use pay-per-token billing. You pay only for what you consume. Models available in each region and
throughput might be limited.

Standard deployments are suited for low-to-medium volume workloads with high burstiness. Customers with high consistent
volume might experience greater latency variability.

## Regional Provisioned
* SKU name in code: `ProvisionedManaged`

Regional Provisioned deployments allow you to specify the amount of throughput you require in a deployment. The service
then allocates the necessary model processing capacity and ensures it's ready for you. Throughput is defined in terms of
provisioned throughput units (PTUs), which is a normalized way of representing the throughput for your deployment. Each
model-version pair requires different amounts of PTUs to deploy, and provides different amounts of throughput per PTU.
Minimum PTU requirements vary by model. For current minimums and available capacity, see [Provisioned throughput
concepts][37].

## Developer (for fine-tuned models)
* SKU name in code: `DeveloperTier`

The Developer deployment type is designed for fine-tuned model evaluation only. It provides cost-efficient testing of
custom models but doesn't include data residency guarantees or an SLA. Developer deployments have a fixed 24-hour
lifetime and are automatically deleted after expiration. To learn more about using the Developer deployment type, see
the [fine-tuning guide][38].

## Troubleshooting deployment issues

Common issues when creating or using deployments:

─────────────────────────┬─────────────────────────────────────┬────────────────────────────────────────────────────────
Issue                    │Cause                                │Resolution                                              
─────────────────────────┼─────────────────────────────────────┼────────────────────────────────────────────────────────
Deployment type          │Model doesn't support the selected   │Check [model availability by deployment type][39]       
unavailable              │type                                 │                                                        
─────────────────────────┼─────────────────────────────────────┼────────────────────────────────────────────────────────
Quota exceeded           │Subscription limit reached for tokens│Request quota increase in Azure portal or use a         
                         │per minute                           │different region                                        
─────────────────────────┼─────────────────────────────────────┼────────────────────────────────────────────────────────
Region unavailable       │Model not deployed in selected region│Select a region from the model's availability list      
─────────────────────────┼─────────────────────────────────────┼────────────────────────────────────────────────────────
Provisioned capacity     │No PTU capacity in region            │Try a different region or use Global Provisioned for    
unavailable              │                                     │broader availability                                    
─────────────────────────┴─────────────────────────────────────┴────────────────────────────────────────────────────────

For quota limits by deployment type, see [Foundry Models quotas and limits][40].

## Restrict deployment types with Azure Policy

Azure Policy helps enforce organizational standards and assess compliance at scale. Through its compliance dashboard,
you can evaluate the overall state of the environment and drill down to per-resource, per-policy granularity. Azure
Policy also supports bulk remediation for existing resources and automatic remediation for new resources. [Learn more
about Azure Policy and specific built-in controls for Foundry Tools][41].

Use the following policy to disable access to a specific Foundry deployment type. Replace `GlobalStandard` with the SKU
name for the deployment type you want to restrict.

`{
    "mode": "All",
    "policyRule": {
        "if": {
            "allOf": [
                {
                    "field": "type",
                    "equals": "Microsoft.CognitiveServices/accounts/deployments"
                },
                {
                    "field": "Microsoft.CognitiveServices/accounts/deployments/sku.name",
                    "equals": "GlobalStandard"
                }
            ]
        }
    }
}
`

## Related content
* [Deploy Microsoft Foundry Models in the Foundry portal][42]
* [Create and deploy an Azure OpenAI in Microsoft Foundry Models resource][43]
* [Foundry Models sold by Azure][44]
* [Model region availability by deployment type][45]
* [Microsoft Foundry Models quotas and limits][46]
* [Provisioned throughput concepts][47]
* [Global Batch processing][48]
* [Azure OpenAI Service pricing][49]
* [Data privacy and security for Foundry Models][50]
* [High availability and disaster recovery][51]

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-02-27

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][52]
[ Your Privacy Choices][53]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][54]
* [Previous Versions][55]
* [Blog][56]
* [Contribute][57]
* [Privacy][58]
* [Consumer Health Privacy][59]
* [Terms of Use][60]
* [Trademarks][61]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/foundry-models/concepts/deployment-types.
md
[7]: #
[8]: ../../concepts/instant-models
[9]: ../media/add-model-deployments/models-deploy-deployment-type.png#lightbox
[10]: https://azure.microsoft.com/explore/global-infrastructure/data-residency/
[11]: ../../concepts/instant-models
[12]: #global-standard
[13]: #global-provisioned
[14]: #global-batch
[15]: #data-zone-standard
[16]: #data-zone-provisioned
[17]: #data-zone-batch
[18]: #standard
[19]: #regional-provisioned
[20]: #developer-for-fine-tuned-models
[21]: models-sold-directly-by-azure
[22]: https://www.microsoft.com/licensing/docs/view/Service-Level-Agreements-SLA-for-Online-Services
[23]: https://azure.microsoft.com/pricing/details/cognitive-services/openai-service/
[24]: ../../concepts/instant-models
[25]: /en-us/privacy/eudb/eu-data-boundary-learn
[26]: /en-us/privacy/eudb/eu-data-boundary-learn
[27]: models-sold-directly-by-azure
[28]: ../../../foundry-classic/how-to/high-availability-resiliency
[29]: ../quotas-limits
[30]: ../../openai/concepts/priority-processing
[31]: ../../openai/concepts/provisioned-throughput
[32]: ../../openai/how-to/batch
[33]: https://azure.microsoft.com/pricing/details/cognitive-services/openai-service/
[34]: ../quotas-limits
[35]: ../../openai/concepts/priority-processing
[36]: ../../openai/how-to/batch
[37]: ../../openai/concepts/provisioned-throughput
[38]: ../../../foundry-classic/openai/how-to/fine-tune-test
[39]: models-sold-directly-by-azure
[40]: ../quotas-limits
[41]: ../../../ai-services/security-controls-policy
[42]: ../how-to/deploy-foundry-models
[43]: ../../../foundry-classic/openai/how-to/create-resource
[44]: models-sold-directly-by-azure
[45]: models-sold-directly-by-azure
[46]: ../quotas-limits
[47]: ../../openai/concepts/provisioned-throughput
[48]: ../../openai/how-to/batch
[49]: https://azure.microsoft.com/pricing/details/cognitive-services/openai-service/
[50]: ../../../foundry-classic/how-to/concept-data-privacy
[51]: ../../../foundry-classic/how-to/high-availability-resiliency
[52]: #
[53]: https://aka.ms/yourcaliforniaprivacychoices
[54]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[55]: https://learn.microsoft.com/en-us/previous-versions/
[56]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[57]: https://learn.microsoft.com/en-us/contribute
[58]: https://go.microsoft.com/fwlink/?LinkId=521839
[59]: https://go.microsoft.com/fwlink/?linkid=2259814
[60]: https://learn.microsoft.com/en-us/legal/termsofuse
[61]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
