# Web source

- URL: https://learn.microsoft.com/en-us/answers/a/12816634
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:43:58.213790638+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Severe Latency Degradation (~4 tokens/sec) Across Azure OpenAI Models in Sweden Central

[ Benedikt Hielscher ][8] 35 Reputation points
2026-06-08T13:10:56.4333333+00:00

Hello everyone,

We are currently experiencing a severe performance degradation across our **Azure OpenAI models** deployed in the
**Sweden Central** region.

### **Current Observations**

**Affected Region:** Sweden Central (`swedencentral`)

**Observed Throughput:** Throughput has dropped to roughly **4 tokens per second**, causing requests that normally take
seconds to either drag out or hit client-side timeouts.

**Models Impacted:** This appears to be affecting all of our deployed models uniformly, rather than being isolated to a
single deployment.

### **What We've Verified**

**No Workload Changes:** Our input prompt sizes, overall traffic volume (TPM/RPM), and application configurations have
not changed.

**Azure Service Health:** The main Azure Status dashboard isn't showing an active incident for OpenAI in this region,
but the behavior strongly behaves like a regional backend capacity constraint or a platform-level load-balancing issue.


Azure OpenAI in Foundry Models
[ Azure OpenAI in Foundry Models ][9]

An Azure service that provides access to OpenAI’s GPT-3 models with enterprise capabilities.

Sign in to follow Follow
10 comments Hide comments for this question Report a concern
I have the same question (3)
1.  [ Karnam Venkata Rajeswari ][10] • Follow 4,260 Reputation points • Microsoft External Staff • Moderator
    2026-06-08T13:39:55.6266667+00:00
    
    Hello [@Benedikt Hielscher ][11] ,
    
    Welcome to Microsoft Q&A .Thank you for reaching out to us.
    
    Based on the behavior described - consistently elevated latency across multiple models, reduced token throughput and
    no corresponding changes in workload patterns , this scenario aligns with a regional performance degradation
    affecting Azure OpenAI deployments in Sweden Central.
    
    We are seeing similar patterns reported in parallel threads, where:
    * Multiple models in the same region are impacted simultaneously
    * Throughput and response-start times are significantly higher than expected
    * Other regions show comparatively stable performance under the same test conditions
    
    In these cases, the behavior is typically associated with temporary backend capacity pressure or request queuing at
    the regional level.
    
    This behavior is currently under review and the engineering teams are actively working to stabilize capacity and
    restore expected latency behavior.
    
    To help reduce user impact please check if the following steps help-
    1. Testing in alternate regions if available - If latency is significantly lower elsewhere, traffic routing can help
       maintain performance continuity
    2. Enable streaming responses - Allows partial output to begin earlier, improving perceived responsiveness
    3. Separate workloads if applicable - Isolate short interactive calls from longer generations to minimize queue
       contention
    4. Continue monitoring key metrics Please focus on
       1. Time to First Token / Time to Response
       2. Time to Last Byte
       3. Tokens per second trends These help confirm recovery as backend conditions improve.
    
    The following references might be helpful , please check them out
    * [Monitoring data reference for Azure OpenAI - Microsoft Foundry | Microsoft Learn][12]
    * [Azure OpenAI in Microsoft Foundry Models performance & latency - Microsoft Foundry | Microsoft Learn][13]
    * [Azure Service Health documentation - Azure Service Health | Microsoft Learn][14]
    
    We appreciate your patience while we are working on this
    
    Thank you
    
    0 votes Report a concern
2.  [ Benedikt Hielscher ][15] • Follow 35 Reputation points
    2026-06-08T13:52:16.9166667+00:00
    
    Hello [@Karnam Venkata Rajeswari ][16]
    
    i just tested it again, and gpt-5-nano averaged about 60 t/s while 5.4 averaged around 15 t/s.
    
    Both Models are Deployed in sweden central.
    
    2 votes Report a concern
3.  [ Karnam Venkata Rajeswari ][17] • Follow 4,260 Reputation points • Microsoft External Staff • Moderator
    2026-06-09T02:59:32.45+00:00
    
    Hello [@Benedikt Hielscher ][18] ,
    
    Following up to see if the response was helpful
    
    Thank you
    
    0 votes Report a concern
4.  [ Karnam Venkata Rajeswari ][19] • Follow 4,260 Reputation points • Microsoft External Staff • Moderator
    2026-06-10T12:50:19.75+00:00
    
    Hello [@Benedikt Hielscher ][20] ,
    
    Checking to see if you had any chance to review the response
    
    Thank you
    
    0 votes Report a concern
5.  [ Martin Günther ][21] • Follow 11 Reputation points
    2026-06-10T18:58:34.62+00:00
    
    We are also experiencing higher latency since 29. Mai and very high latency since 05. June with GPT 5.4 and 5.1 in
    Sweden Central.
    
    The same prompts now take up to 120 sec. to first response and > 180 for full response which then hits client
    timeouts.
    
    0 votes Report a concern
6.  [ Benedikt Hielscher ][22] • Follow 35 Reputation points
    2026-06-10T19:01:23.7+00:00
    
    [@Karnam Venkata Rajeswari ][23]
    
    i saw the answer and i am aware of that. The workflows didn't change, but latency got worse, that's what i was
    reporting.
    
    1 vote Report a concern
7.  [ Martin Günther ][24] • Follow 11 Reputation points
    2026-06-10T19:14:36.4633333+00:00
    
    I can absolutely confirm [@Benedikt Hielscher ][25]observation. It is getting worse. We've deployed GPT-5.4 as "Data
    Zone Standard"
    
    0 votes Report a concern
8.  [ Karnam Venkata Rajeswari ][26] • Follow 4,260 Reputation points • Microsoft External Staff • Moderator
    2026-06-11T03:16:23.0233333+00:00
    
    Hello [@Benedikt Hielscher ][27] and [Martin Günther][28],
    
    Thank you for letting us know the update. Please allow us some time while the respective teams are working on the
    issue
    
    Thank you
    
    0 votes Report a concern
9.  [ Martin Günther ][29] • Follow 11 Reputation points
    2026-06-12T06:53:31.9833333+00:00
    
    Hello [@Karnam Venkata Rajeswari ][30],
    
    Thanks for the update. Could you please provide further details? I need to justify to my customer why this is not
    working and how long it is expected to take to resolve the issue.
    
    Thank you,
    Martin
    
    0 votes Report a concern
10. [ P, Rohith Krishnan (FT D AA IN DTS SES AIN) ][31] • Follow 5 Reputation points
    2026-06-12T16:17:26.8866667+00:00
    
    I am also facing the same issue for last one week , the agents created in this region is responding extremely slow
    or gets timedout . Kindly provide an ETA as its impacting our delivery
    
    1 vote Report a concern
Show 5 more comments
[ Sign in to comment ][32]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 1 answer

Sort by: Most helpful
[Most helpful][33] [Newest][34] [Oldest][35]
1. AI answer
   
   2026-06-08T13:11:14.6433333+00:00
   
   The symptoms described (sudden, severe slowdown across all Azure OpenAI deployments in a single region, with no app
   or workload changes) are consistent with a regional service-side issue or capacity constraint rather than a model- or
   application-specific problem.
   
   From the available information, the following points are relevant:
   1. Latency for Azure OpenAI is primarily driven by:
      * Model type
      * Number of tokens in the prompt
      * Number of tokens generated
      * Overall load on the deployment and system
      The total time to last token (TTLT) follows: `TTLT = TTFT + (TBT × Tokens Generated)` where TTFT is time to first
      token and TBT is time between tokens. A uniform drop to ~4 tokens/sec across models, without changes in prompt or
      output size, strongly suggests an increase in TBT due to backend load rather than anything in the client or prompt
      design.
   2. When latency suddenly degrades without workload changes, and especially when it affects all models in a region,
      the recommended actions are:
      * Check Azure Status and Service Health for the region to confirm whether there is an incident affecting Azure
        OpenAI.
      * If nothing is reported, test the same workloads in another region where the same models are available (for
        example, another region listed as supporting `gpt-4o` or `gpt-4o-mini` in the model availability table) to
        confirm the issue is region-specific.
   3. If the issue is confirmed to be regional and persists while other regions behave normally, this is indicative of a
      platform-side problem. In similar service-side timeout or performance issues (for example, with Document
      Intelligence or regional deployment problems), the guidance is:
      * Verify there are no network/firewall/VNet issues on the client side.
      * Collect request details (including correlation IDs from logs/SDK diagnostics) for affected calls.
      * Escalate via Azure Support so the product group can investigate the regional backend.
   4. To mitigate impact while the regional issue is investigated:
      * If possible, temporarily route traffic to another region where the same models are available, based on the model
        summary and region availability table.
      * Reduce `max_tokens` and overall generated tokens where feasible, as fewer output tokens directly reduce latency.
      * Enable streaming for user-facing chat or interactive scenarios so users see partial responses earlier, improving
        perceived latency even if total TTLT remains high.
      * Avoid mixing very long and short workloads on the same deployment; separate deployments per workload can help
        reduce queuing and batching delays.
   
   Given the described behavior, the next concrete steps are:
   1. Confirm via Service Health whether there is an active incident in Sweden Central for Azure OpenAI.
   2. Run the same prompts against a deployment of the same model in another supported region to validate that
      throughput is normal elsewhere.
   3. If the problem is isolated to Sweden Central and persists, open a support case with timestamps, deployment names,
      and correlation IDs so the Azure OpenAI team can investigate regional capacity or load-balancing issues.
   4. As a temporary workaround, fail over latency-sensitive traffic to a healthy region and apply token/streaming
      optimizations to reduce user-visible impact.
   
   References:
   * [Performance and latency][36]
   * [Azure OpenAI in Azure AI Foundry Models][37]
   * [Document Intelligence classification and extraction requests timing out - Microsoft Q&A][38]
   * [Azure Deployment Takes 70 Minutes to Fail with Obscure Error - Microsoft Q&A][39]
   
   AI-generated content may be incorrect. Read our [transparency notes][40] for more information.
   
   Was this answer helpful?
   
   Yes No
   Report a concern
[ Sign in to answer ][41]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-us][42]
[ Your Privacy Choices][43]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][44]
* [Previous Versions][45]
* [Blog][46]
* [Contribute][47]
* [Privacy][48]
* [Consumer Health Privacy][49]
* [Terms of Use][50]
* [Code of Conduct][51]
* [Trademarks][52]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5914
470%2Fsevere-latency-degradation-(-4-tokens-sec)-across 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2
F5914470%2Fsevere-latency-degradation-(-4-tokens-sec)-across 											&text=Severe%20Latency%20Degradation%20(~4%20tokens%2Fsec)%
20Across%20Azure%20OpenAI%20Models%20in%20Sweden%20Central 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2Fen-
us%2Fanswers%2Fquestions%2F5914470%2Fsevere-latency-degradation-(-4-tokens-sec)-across
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5914470%2F
severe-latency-degradation-(-4-tokens-sec)-across
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Severe%20Latency%20Degradation%20(~4%20tokens%2Fsec)%20Across%20Azure%20
OpenAI%20Models%20in%20Sweden%20Central&body=Severe Latency Degradation (~4 tokens/sec) Across Azure OpenAI Models in Sw
eden Centralhttps%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5914470%2Fsevere-latency-degradation-(-4-t
okens-sec)-across
[8]: /en-us/users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[9]: /en-us/answers/tags/387/azure-openai/
[10]: /en-us/users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[11]: https://learn.microsoft.com/en-us/users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[12]: https://learn.microsoft.com/en-us/azure/foundry/openai/monitor-openai-reference
[13]: https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/latency
[14]: https://learn.microsoft.com/en-us/azure/service-health/
[15]: /en-us/users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[16]: /users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[17]: /en-us/users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[18]: https://learn.microsoft.com/en-us/users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[19]: /en-us/users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[20]: https://learn.microsoft.com/en-us/users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[21]: /en-us/users/na/?userid=13a74038-064d-46a6-b054-ff9b4f7cbb6e
[22]: /en-us/users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[23]: /users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[24]: /en-us/users/na/?userid=13a74038-064d-46a6-b054-ff9b4f7cbb6e
[25]: /users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[26]: /en-us/users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[27]: https://learn.microsoft.com/en-us/users/na/?userid=7df021a9-0692-4c05-a7d6-095ef2a5c4b6
[28]: https://learn.microsoft.com/en-us/users/na/?userid=13a74038-064d-46a6-b054-ff9b4f7cbb6e
[29]: /en-us/users/na/?userid=13a74038-064d-46a6-b054-ff9b4f7cbb6e
[30]: /users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[31]: /en-us/users/na/?userid=2058ca33-d540-48cd-8052-1079bcac169f
[32]: #
[33]: ?orderby=helpful&page=1#answers
[34]: ?orderby=newest&page=1#answers
[35]: ?orderby=oldest&page=1#answers
[36]: https://learn.microsoft.com/azure/foundry/openai/how-to/latency#understanding-throughput-vs-latency
[37]: https://learn.microsoft.com/azure/ai-foundry/openai/concepts/models#model-summary-table-and-region-availability
[38]: https://learn.microsoft.com/answers/a/12244048
[39]: https://learn.microsoft.com/answers/a/12358264
[40]: /answers/support/ai-first-overview
[41]: #
[42]: #
[43]: https://aka.ms/yourcaliforniaprivacychoices
[44]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[45]: https://learn.microsoft.com/en-us/previous-versions/
[46]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[47]: https://learn.microsoft.com/en-us/contribute
[48]: https://go.microsoft.com/fwlink/?LinkId=521839
[49]: https://go.microsoft.com/fwlink/?linkid=2259814
[50]: https://learn.microsoft.com/en-us/legal/termsofuse
[51]: https://aka.ms/msftqacodeconduct
[52]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
