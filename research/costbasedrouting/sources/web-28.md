# Web source

- URL: https://learn.microsoft.com/en-nz/answers/questions/5817608/microsoft-foundry-pricing-and-reservation
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:42:08.772208500+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Microsoft Foundry Pricing and Reservation

[ Ananya ][8] 85 Reputation points
2026-03-11T20:20:47.98+00:00

I am currently trying to understand the pricing structure of **Microsoft Foundry** and how reservations apply to it. I
have a few questions:

The Azure pricing documentation provides pricing details for all Foundry models:
[https://azure.microsoft.com/en-gb/pricing/details/ai-foundry-models/aoai/][9]

However, in the Azure Pricing Calculator ([https://azure.microsoft.com/en-in/pricing/calculator/][10]), I can only find
**Azure OpenAI**, and not the other models. Are the other models listed under a different product name?

Based on my current understanding, only **OpenAI, Microsoft, and DeepSeek models** can currently be covered by
reservations. Are there any other models that are eligible for reservations?
1. Does **Azure Advisor** provide reservation recommendations for Microsoft Foundry? If so, can these reservations be
   retrieved using the Reservation Recommendations API?
   [https://learn.microsoft.com/en-us/rest/api/consumption/reservation-recommendations/list?view=rest-consumption-2024-0
   8-01&tabs=HTTP][11] At the moment, I do not see **Azure Foundry or Azure OpenAI** listed as a supported
   `resourceType` in the documentation.

Azure OpenAI in Foundry Models
[ Azure OpenAI in Foundry Models ][12]

An Azure service that provides access to OpenAI’s GPT-3 models with enterprise capabilities.

Sign in to follow Follow
3 comments Hide comments for this question Report a concern
I have the same question (0)
1. [ Sayali-MSFT ][13] • Follow 6,106 Reputation points • Microsoft External Staff • Moderator
   2026-03-13T05:13:55.2966667+00:00
   
   Hello **Ananya,**Thank you for bringing this issue to our attention. We will look into it and get back to you
   shortly. 
   
   0 votes Report a concern
2. [ Ananya ][14] • Follow 85 Reputation points
   2026-03-16T06:33:56.4166667+00:00
   
   Hi Sayali, any updates on this ?
   
   0 votes Report a concern
3. [ Ananya ][15] • Follow 85 Reputation points
   2026-03-24T06:49:40.0266667+00:00
   
   Reposting the question the comments below
   Why there is a mention of use recommendations api here
   
   [https://learn.microsoft.com/en-us/azure/advisor/advisor-reference-cost-recommendations#consider-openai-service-provi
   sioned-throughput-reservations-to-save-over-pay-as-you-go-costs][16]
   
   And this does not have more information about openai though.
   
   Please explain and let us know when it will get supported to fetch openai or foundry recommendations via the
   recommendations api.
   
   Thanks!
   
   0 votes Report a concern
[ Sign in to comment ][17]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

Answer accepted by question author

[ SRILAKSHMI C ][18] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
2026-03-16T08:42:59.2433333+00:00

Hello **[Ananya][19]**,

Thank you for your patience while we clarify how **pricing, reservations, and recommendations work for Azure AI Foundry
today**. I’ll address each of your questions below.
1. Why you only see “Azure OpenAI” in the Pricing Calculator

Currently, the **Azure Pricing Calculator** only provides a first-class entry for **Azure OpenAI Service**.

Many models available through **Azure AI Foundry** (including Microsoft-branded models or partner models such as
DeepSeek) are not yet surfaced as separate tiles in the calculator UI. This means they may appear on the **Foundry
pricing documentation page**, but not as individual selectable products in the calculator.

In practice:
* Pricing for these models is published on the **Azure AI Foundry pricing page** (token-based PAYG or PTU-based
  pricing).
* To estimate costs today, you can reference the **per-token or per-PTU rates** from the pricing page and:
  * Use the calculator’s **Custom cost entries**, or
  * Estimate usage manually until the models appear in the calculator.
* It is also common for **new or preview models to appear in documentation before they are added to the calculator UI**,
  so manual estimation is the typical workaround in the meantime.
2. Which Foundry models support reservations

Reservations apply only to **Provisioned Throughput Unit (PTU)** deployments, not to token-based pay-as-you-go usage.

Currently, reservation eligibility includes deployments such as:

**Azure OpenAI Service** models deployed using **PTU (provisioned throughput)**

**Microsoft Foundry provisioned throughput capacity** used for supported Foundry models or tools

Some partner model deployments (for example **DeepSeek**) when they expose a **PTU-based SKU**

Workloads that are **strictly PAYG/token-based** or **fixed-rate service SKUs** (for example transaction-based AI
services) are **not eligible for reservations**.

At this time, **OpenAI models, Microsoft-hosted models, and DeepSeek engines** are the primary model families that can
be covered by PTU reservations. There are currently **no additional model lines beyond these that support reservation
coverage**.
3. Azure Advisor and Reservation Recommendations API

Regarding reservation recommendations:

**Azure Advisor** does not currently generate reservation recommendations for **Azure OpenAI or Azure AI Foundry PTU
usage**.

Similarly, the **Reservation Recommendations REST API** does not list resource types such as:

`Microsoft.Foundry`

`  `Microsoft.CognitiveServices/accounts/openai`

`

Because these resource types are not currently supported, reservation recommendations for these services **cannot yet be
retrieved via the Advisor API**.

Support for **Advisor-driven PTU reservation guidance is planned but not yet available**.

In the meantime, the recommended approach is to:

Track usage through **Azure Cost Analysis**

Export usage data and estimate long-term PTU needs

Purchase reservations manually through the Azure portal under **All Services → Reservations → Microsoft Foundry
Provisioned Throughput**

The pricing calculator currently exposes Foundry usage primarily through **Azure OpenAI**, while other Foundry models
may not yet appear as separate calculator entries.

**Reservations only apply to PTU-based deployments**, not token-based PAYG workloads.

**Azure Advisor and the Reservation Recommendations API do not yet provide reservation guidance for Foundry/OpenAI
resources**, so reservations must currently be evaluated and purchased manually.

Please refer this

Buy Foundry PTU reservation:
[https://learn.microsoft.com/azure/cost-management-billing/reservations/microsoft-foundry][20]

I Hope this helps. Do let me know if you have any further queries.

If this answers your query, please do click `Accept Answer` and `Yes` for was this answer helpful.

Thank you!

Was this answer helpful?

Yes No
1 person found this answer helpful.
2 comments Show comments for this answer Report a concern
1. [ SRILAKSHMI C ][21] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
   2026-03-17T09:51:36.4866667+00:00
   
   Hi **[Ananya][22]**,
   
   Following up to see if the above answer was helpful. If this answers your query, do click `Accept Answer` and `Yes`
   for was this answer helpful. And, if you have any further query do let us know.
   
   Thank you!
   
   1 vote Report a concern
2. [ Sunil Sutar ][23] • Follow 0 Reputation points
   2026-03-19T00:26:44.32+00:00
   
   So [@Anonymous ][24]- why there is a mention of use recommendations api here
   
   [https://learn.microsoft.com/en-us/azure/advisor/advisor-reference-cost-recommendations#consider-openai-service-provi
   sioned-throughput-reservations-to-save-over-pay-as-you-go-costs][25]
   
   And this does not have more information about openai though.
   
   Please explain and let us know when it will get supported to fetch openai or foundry recommendations via the
   recommendations api.
   
   Thanks!
   
   0 votes Report a concern
[ Sign in to comment ][26]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 2 additional answers

Sort by: Most helpful
[Most helpful][27] [Newest][28] [Oldest][29]
1. [ Sayali-MSFT ][30] • Follow 6,106 Reputation points • Microsoft External Staff • Moderator
   2026-03-16T08:22:08.88+00:00
   
   Hello **[Ananya][31]** ,
   **Azure AI Foundry** does not appear as a separate item in the Azure Pricing Calculator because it functions as a
   control and orchestration layer rather than a billable service; costs are charged through the underlying Azure AI
   services it uses, such as **Azure OpenAI Service**, **Azure AI Search**, **Azure Vision**, **Azure Speech**, and
   **Azure Document Intelligence**. Model reservations in Foundry are available through **Provisioned Throughput Units
   (PTUs)** and currently support only OpenAI, Microsoft Phi, and DeepSeek models, while third-party models remain
   pay-as-you-go.
   Additionally, reservation recommendations for these services are not provided by **Azure Advisor** or the Reservation
   Recommendations API, meaning PTU reservations must be purchased and managed manually through the Azure Portal or
   Foundry interface and monitored using Azure Cost Management tools.
   
   Reference Document-
   1. [https://azure.microsoft.com/en-us/pricing/details/ai-foundry-models/microsoft/][32]
      2.[https://learn.microsoft.com/en-us/azure/cost-management-billing/reservations/reserved-instance-purchase-recomme
      ndations][33]
      3.[https://docs.azure.cn/en-us/cost-management-billing/dataset-schema/reservation-recommendations-mca][34]
   
   Was this answer helpful?
   
   Yes No
   0 comments No comments Report a concern
   [ Sign in to comment ][35]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
2. AI answer
   
   2026-03-11T20:21:11.44+00:00
   
   Microsoft Foundry pricing and reservations are tied to Azure AI services and Provisioned Throughput Units (PTUs) for
   models available in Microsoft Foundry Models.
   1. **Pricing visibility in the Azure Pricing Calculator**
      Foundry resources are billed through the underlying Azure AI services they use (for example, Azure OpenAI, Azure
      Speech in Foundry, Azure Vision in Foundry, etc.). Costs are calculated per service and feature, not as a single
      “Foundry” line item:
   * When a Foundry resource is created, charges accrue for the Azure services it uses, such as Azure OpenAI, Azure
     Speech in Foundry, Content Safety, Azure Vision in Foundry, Azure Document Intelligence, and Azure Language in
     Foundry.
   * Two billing models exist: pay‑as‑you‑go (serverless API) and commitment tiers.
   
   Because of this, services may appear in the pricing calculator under their individual product names (for example,
   Azure OpenAI) rather than a generic “Foundry” product. The context does not list alternative calculator product names
   for each Foundry model beyond the underlying services mentioned.
   2. **Which models can be covered by reservations**
      Reservations apply to **Microsoft Foundry Provisioned Throughput** for “models available in Microsoft Foundry
      Models.” Key points:
   * A Microsoft Foundry Provisioned Throughput reservation covers PTU usage for models available in Microsoft Foundry
     Models when deployed as **Global, Data Zone, or Regional Provisioned** deployments.
   * The reservation discount applies automatically to provisioned throughput pay‑as‑you‑go meters for those models,
     within the reservation’s scope and region.
   * Reservations for Global, Data Zone, and Regional deployments are not interchangeable; a separate reservation is
     required for each deployment type.
   
   The context explicitly calls out Azure OpenAI Service and DeepSeek as examples of “models available in Microsoft
   Foundry Models,” but does not enumerate all other eligible models. It states that the reservation mechanism “works on
   models available in Microsoft Foundry Models (e.g. Azure OpenAI Service, DeepSeek, etc.)” and refers to the Microsoft
   Foundry Models pricing page for the full list. No additional specific model families beyond these examples are listed
   in the provided material.
   3. **How Microsoft Foundry Provisioned Throughput reservations work**
   * Reservations are purchased for a specific **region**, **quantity of PTUs**, and **deployment type** (Global, Data
     Zone, or Regional).
   * The reservation discount is applied hourly by comparing reserved PTUs to deployed PTUs for eligible models in
     scope:
     * Deployed PTUs up to the reserved amount are covered by the reservation.
     * Any deployed PTUs above the reserved amount are billed at the hourly pay‑as‑you‑go rate.
     * Partial‑hour deployments are pro‑rated by minutes within the hour.
   * Reservations apply only to **provisioned deployments** and do not cover standard (serverless) deployments or
     fine‑tuning.
   * Reservations do **not** guarantee capacity; best practice is to create deployments first, then buy reservations
     sized to those deployments.
   * If deployments are deleted, the reservation is not automatically changed or canceled; it must be canceled or
     exchanged manually via Azure Reservations.
   4. **Buying and scoping reservations**
   * Reservations are purchased in the Azure portal under **Microsoft Foundry Provisioned Throughput**.
   * Supported subscription types include Enterprise Agreement, Microsoft Customer Agreement, and pay‑as‑you‑go.
   * Scopes can be:
     * Single resource group
     * Single subscription
     * Shared scope across eligible subscriptions in the same billing context
     * Management group (subscriptions in the management group and billing scope)
   * Reservation size should match the total PTUs deployed for each deployment type (Global, Data Zone, Regional).
     Reservations are purchased in one‑PTU increments.
   5. **Azure Advisor and Reservation Recommendations API**
      The provided context describes how reservations work for Azure AI Foundry Provisioned Throughput and how to
      purchase and scope them, but it does **not** state that Azure Advisor provides reservation recommendations
      specifically for Microsoft Foundry or Azure OpenAI, nor does it document support for these services in the
      Reservation Recommendations API. The context also does not list any `resourceType` values or mappings for
      Foundry/OpenAI in that API.
   
   Based on the available material, only the general reservation behavior for Azure AI Foundry Provisioned Throughput is
   documented; there is no explicit confirmation here that Azure Advisor or the Reservation Recommendations API
   currently surface recommendations for Microsoft Foundry or Azure OpenAI PTU reservations.
   
   References:
   * [Save costs with Microsoft Foundry Provisioned Throughput Reservations][36]
   * [Plan and manage costs for Microsoft Foundry][37]
   * [Understanding costs associated with provisioned throughput units (PTU)][38]
   
   AI-generated content may be incorrect. Read our [transparency notes][39] for more information.
   
   Was this answer helpful?
   
   Yes No
   Report a concern
[ Sign in to answer ][40]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-nz][41]
[ Your Privacy Choices][42]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][43]
* [Previous Versions][44]
* [Blog][45]
* [Contribute][46]
* [Privacy][47]
* [Consumer Health Privacy][48]
* [Terms of Use][49]
* [Code of Conduct][50]
* [Trademarks][51]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-nz%2Fanswers%2Fquestions%2F5817
608%2Fmicrosoft-foundry-pricing-and-reservation 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-nz%2Fanswers%2Fquestions%2
F5817608%2Fmicrosoft-foundry-pricing-and-reservation 											&text=Microsoft%20Foundry%20Pricing%20and%20Reservation 											&tw_p=tweet
button&url=https%3A%2F%2Flearn.microsoft.com%2Fen-nz%2Fanswers%2Fquestions%2F5817608%2Fmicrosoft-foundry-pricing-and-res
ervation
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-nz%2Fanswers%2Fquestions%2F5817608%2F
microsoft-foundry-pricing-and-reservation
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Microsoft%20Foundry%20Pricing%20and%20Reservation&body=Microsoft Foundry
 Pricing and Reservationhttps%3A%2F%2Flearn.microsoft.com%2Fen-nz%2Fanswers%2Fquestions%2F5817608%2Fmicrosoft-foundry-pr
icing-and-reservation
[8]: /en-nz/users/na/?userid=7162f684-ab4e-477b-81ab-9fb9f71a1ca6
[9]: https://azure.microsoft.com/en-gb/pricing/details/ai-foundry-models/aoai/
[10]: https://azure.microsoft.com/en-in/pricing/calculator/
[11]: https://learn.microsoft.com/en-us/rest/api/consumption/reservation-recommendations/list?view=rest-consumption-2024
-08-01&tabs=HTTP
[12]: /en-nz/answers/tags/387/azure-openai/
[13]: /en-nz/users/na/?userid=504f86fc-25a8-47de-a08a-442d8db377ab
[14]: /en-nz/users/na/?userid=7162f684-ab4e-477b-81ab-9fb9f71a1ca6
[15]: /en-nz/users/na/?userid=7162f684-ab4e-477b-81ab-9fb9f71a1ca6
[16]: https://learn.microsoft.com/en-us/azure/advisor/advisor-reference-cost-recommendations#consider-openai-service-pro
visioned-throughput-reservations-to-save-over-pay-as-you-go-costs
[17]: #
[18]: /en-nz/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[19]: https://learn.microsoft.com/en-us/users/na/?userid=7162f684-ab4e-477b-81ab-9fb9f71a1ca6
[20]: https://learn.microsoft.com/azure/cost-management-billing/reservations/microsoft-foundry
[21]: /en-nz/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[22]: https://learn.microsoft.com/en-us/users/na/?userid=7162f684-ab4e-477b-81ab-9fb9f71a1ca6
[23]: /en-nz/users/na/?userid=10a123b2-e479-4ce8-856c-19e0589eeb90
[24]: #
[25]: https://learn.microsoft.com/en-us/azure/advisor/advisor-reference-cost-recommendations#consider-openai-service-pro
visioned-throughput-reservations-to-save-over-pay-as-you-go-costs
[26]: #
[27]: ?orderby=helpful&page=1#answers
[28]: ?orderby=newest&page=1#answers
[29]: ?orderby=oldest&page=1#answers
[30]: /en-nz/users/na/?userid=504f86fc-25a8-47de-a08a-442d8db377ab
[31]: https://learn.microsoft.com/en-us/users/na/?userid=7162f684-ab4e-477b-81ab-9fb9f71a1ca6
[32]: https://azure.microsoft.com/en-us/pricing/details/ai-foundry-models/microsoft/
[33]: https://learn.microsoft.com/en-us/azure/cost-management-billing/reservations/reserved-instance-purchase-recommenda
tions
[34]: https://docs.azure.cn/en-us/cost-management-billing/dataset-schema/reservation-recommendations-mca
[35]: #
[36]: https://learn.microsoft.com/azure/cost-management-billing/reservations/microsoft-foundry
[37]: https://learn.microsoft.com/azure/foundry/concepts/manage-costs#costs-associated-with-foundry
[38]: https://learn.microsoft.com/azure/ai-foundry/openai/how-to/provisioned-throughput-onboarding#azure-reservations-fo
r-azure-ai-foundry-provisioned-throughput
[39]: /answers/support/ai-first-overview
[40]: #
[41]: #
[42]: https://aka.ms/yourcaliforniaprivacychoices
[43]: https://learn.microsoft.com/en-nz/principles-for-ai-generated-content
[44]: https://learn.microsoft.com/en-nz/previous-versions/
[45]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[46]: https://learn.microsoft.com/en-nz/contribute
[47]: https://go.microsoft.com/fwlink/?LinkId=521839
[48]: https://go.microsoft.com/fwlink/?linkid=2259814
[49]: https://learn.microsoft.com/en-nz/legal/termsofuse
[50]: https://aka.ms/msftqacodeconduct
[51]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
