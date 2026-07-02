# Web source

- URL: https://learn.microsoft.com/en-ie/answers/questions/5869953/degrading-performance-of-ai-foundry-models-overtim
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:44:05.332029522+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Degrading performance of AI Foundry Models Overtime

[ Animesh Aditha ][8] 20 Reputation points
2026-04-24T06:59:09.8066667+00:00

We had deployed gpt-5-mini on azure AI foundry whose token per second has degraded overtime.

For instance the same job on gpt-5-mini is taking 90-120 seconds on average which used to 60 seconds at the worst case
while the newer models like gpt-5.4-mini is completing the same job in under 20 seconds.

This looks like an effort to push customers to use newer models but would like to confirm if this is the case and such
degrading in peformance will continue to occur overtime even of the newer models

Azure OpenAI in Foundry Models
[ Azure OpenAI in Foundry Models ][9]

An Azure service that provides access to OpenAI’s GPT-3 models with enterprise capabilities.

Sign in to follow Follow
1 comment Hide comments for this question Report a concern
I have the same question (4)
1. [ SRILAKSHMI C ][10] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
   2026-04-30T12:25:19.74+00:00
   
   Hi [@Animesh Aditha][11]
   
   Following up to see if the below answer was helpful. If this answers your query, do click Accept Answer and Yes for
   was this answer helpful. And, if you have any further query do let us know.
   
   Thank you!
   
   0 votes Report a concern
[ Sign in to comment ][12]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

Answer accepted by question author

[ SRILAKSHMI C ][13] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
2026-04-25T10:53:00.3933333+00:00

Hello [@Animesh Aditha][14],

Thank you for sharing your observations. We understand how important consistent latency is for production workloads.

To address your primary concern directly: **Microsoft does not intentionally degrade the performance of older models to
encourage customers to migrate to newer ones.** There is no deliberate throttling or artificial slowdown applied to
older models.

What you are seeing is typically the result of a combination of normal platform dynamics, shared-capacity behavior, and
the significant efficiency improvements built into newer model generations.

**Why GPT-5 Mini May Appear Slower Over Time**

If your `gpt-5-mini` deployment is now taking 90–120 seconds for workloads that previously completed in around 60
seconds, several factors may be contributing:

**Shared infrastructure behavior**: Standard (Pay-As-You-Go) deployments run on shared compute capacity. As overall
demand for a model increases, queueing and reduced tokens-per-second can occur.

**Regional demand fluctuations**: Latency can vary based on aggregate usage in your deployment region.

**Workload concurrency**: Higher parallel request volumes can increase waiting time.

**Prompt and output characteristics**: Token count, reasoning depth, structured outputs, and tool usage all affect
response time.

This is expected behavior for shared-capacity deployments.

**Why GPT-5.4 Mini Is Significantly Faster**

Newer models such as `gpt-5.4-mini` are designed with substantial improvements, including:
* More efficient inference architecture
* Higher token throughput
* Lower latency under concurrent workloads
* Better optimization for tool use and reasoning workflows

This is why the same workload may complete in under 20 seconds on `gpt-5.4-mini` while taking significantly longer on
earlier mini models.

These gains reflect normal platform evolution not degradation of older models.

**How to Investigate Current Performance**

We recommend reviewing your Azure AI Foundry metrics and logs:

Navigate to **Azure Portal → Monitor → Metrics / Logs**

Review:
* Request volume
* Throttling events (HTTP 429)
* Time to first token
* Tokens per second
* End-to-end latency
* Error rates

This can help identify whether increased latency correlates with higher demand or quota constraints.

**Recommendations to Improve and Stabilize Performance**

**1. Consider Provisioned Throughput Units (PTU)**

For workloads requiring predictable latency and consistent throughput, PTU is the recommended option.

Benefits include:
* Reserved dedicated capacity
* Stable token generation rates
* Reduced latency variability
* Better performance under sustained load

Standard PAYG deployments do not provide latency guarantees.

**2. Implement Load Balancing Across Deployments or Regions**

To reduce the impact of localized capacity constraints:

Deploy across multiple regions and/or subscriptions

Us Azure Front Door, Azure Traffic Manager, Azure API Management

This helps distribute load and improve resiliency.

**3. Verify API Version**

Ensure you are using the latest supported API version, as newer versions often include performance improvements,
reliability enhancements, and bug fixes.

Using the most current API version is strongly recommended.

**4. Review Deployment Update Policy**

For your deployment, consider the available update settings:
* **Auto-update to default** – Automatically receives the latest default model improvements
* **Upgrade when expired** – Remains on the current version until retirement

This allows you to balance stability with access to performance enhancements.

**Will Newer Models Also Slow Down Over Time?**

Not inherently.

While latency can fluctuate in shared-capacity environments due to demand, **systematic long-term degradation is not
expected**. In fact:
* Platform optimizations continue over time
* Newer runtime improvements are regularly introduced
* Provisioned deployments offer highly consistent performance

Future model generations will likely continue to improve in latency, throughput, and efficiency.

Microsoft does **not** intentionally slow older models.

The observed latency increase is most likely due to shared-capacity dynamics and growing demand.

`gpt-5.4-mini` is significantly faster because of architectural and runtime improvements.

To improve consistency, consider:
* Migrating to `gpt-5.4-mini`
* Using PTU for guaranteed performance
* Load balancing across regions or deployments
* Monitoring throughput and throttling metrics
* Using the latest API version

Please refer this

Provisioned Throughput Units for Azure AI Foundry:
[https://learn.microsoft.com/azure/ai-foundry/openai/concepts/provisioned-throughput?tabs=global-ptum][15]

Azure OpenAI Service Model Version Deprecation & Auto-update Policies:
[https://learn.microsoft.com/azure/ai-services/openai/how-to/working-with-models?tabs=powershell#model-updates][16]

I Hope this helps. Do let me know if you have any further queries.

If this answers your query, please do click Accept Answer and Yes for was this answer helpful.

Thank you!

Was this answer helpful?

Yes No
1 person found this answer helpful.
0 comments No comments Report a concern
[ Sign in to comment ][17]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 1 additional answer

Sort by: Most helpful
[Most helpful][18] [Newest][19] [Oldest][20]
1. AI answer
   
   2026-04-24T06:59:22.67+00:00
   
   The provided information only covers model availability, retirement timelines, and high-level guidance on choosing
   models and reasoning levels. It does not contain any data or statements about intentional performance degradation of
   specific models over time, nor about service-level guarantees or policies related to throttling older models.
   
   References:
   * [Azure OpenAI in Microsoft Foundry model deprecations and retirements][21]
   * [GPT-5 vs GPT-4.1: choosing the right model for your use case][22]
   * [Retired Azure OpenAI models in Microsoft Foundry][23]
   * [Azure OpenAI in Azure AI Foundry Models model deprecations and retirements][24]
   * [Azure OpenAI in Azure AI Foundry Models][25]
   
   AI-generated content may be incorrect. Read our [transparency notes][26] for more information.
   
   Was this answer helpful?
   
   Yes No
   Report a concern
[ Sign in to answer ][27]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-ie][28]
[ Your Privacy Choices][29]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][30]
* [Previous Versions][31]
* [Blog][32]
* [Contribute][33]
* [Privacy][34]
* [Consumer Health Privacy][35]
* [Terms of Use][36]
* [Code of Conduct][37]
* [Trademarks][38]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-ie%2Fanswers%2Fquestions%2F5869
953%2Fdegrading-performance-of-ai-foundry-models-overtim 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-ie%2Fanswers%2Fquestions%2
F5869953%2Fdegrading-performance-of-ai-foundry-models-overtim 											&text=Degrading%20performance%20of%20AI%20Foundry%20Models
%20Overtime 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2Fen-ie%2Fanswers%2Fquestions%2F5869953%2Fdegrading-
performance-of-ai-foundry-models-overtim
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-ie%2Fanswers%2Fquestions%2F5869953%2F
degrading-performance-of-ai-foundry-models-overtim
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Degrading%20performance%20of%20AI%20Foundry%20Models%20Overtime&body=Deg
rading performance of AI Foundry Models Overtimehttps%3A%2F%2Flearn.microsoft.com%2Fen-ie%2Fanswers%2Fquestions%2F586995
3%2Fdegrading-performance-of-ai-foundry-models-overtim
[8]: /en-ie/users/na/?userid=1d863a1f-32b8-4b4a-8144-8761166b5f8d
[9]: /en-ie/answers/tags/387/azure-openai/
[10]: /en-ie/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[11]: https://learn.microsoft.com/en-us/users/na/?userid=1d863a1f-32b8-4b4a-8144-8761166b5f8d
[12]: #
[13]: /en-ie/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[14]: https://learn.microsoft.com/en-us/users/na/?userid=1d863a1f-32b8-4b4a-8144-8761166b5f8d
[15]: https://learn.microsoft.com/azure/ai-foundry/openai/concepts/provisioned-throughput?tabs=global-ptum
[16]: https://learn.microsoft.com/azure/ai-services/openai/how-to/working-with-models?tabs=powershell#model-updates
[17]: #
[18]: ?orderby=helpful&page=1#answers
[19]: ?orderby=newest&page=1#answers
[20]: ?orderby=oldest&page=1#answers
[21]: https://learn.microsoft.com/azure/foundry/openai/concepts/model-retirements#current-models
[22]: https://learn.microsoft.com/azure/foundry/foundry-models/how-to/model-choice-guide#gpt-5-thinking-levels-trade-off
s
[23]: https://learn.microsoft.com/azure/foundry/openai/concepts/legacy-models#retired-models
[24]: https://learn.microsoft.com/azure/ai-foundry/openai/concepts/model-retirements#current-models
[25]: https://learn.microsoft.com/azure/ai-foundry/openai/concepts/models
[26]: /answers/support/ai-first-overview
[27]: #
[28]: #
[29]: https://aka.ms/yourcaliforniaprivacychoices
[30]: https://learn.microsoft.com/en-ie/principles-for-ai-generated-content
[31]: https://learn.microsoft.com/en-ie/previous-versions/
[32]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[33]: https://learn.microsoft.com/en-ie/contribute
[34]: https://go.microsoft.com/fwlink/?LinkId=521839
[35]: https://go.microsoft.com/fwlink/?linkid=2259814
[36]: https://learn.microsoft.com/en-ie/legal/termsofuse
[37]: https://aka.ms/msftqacodeconduct
[38]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
