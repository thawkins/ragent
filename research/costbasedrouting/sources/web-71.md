# Web source

- URL: https://learn.microsoft.com/en-ca/answers/questions/5896701/sweden-central-ai-foundry-doesnt-work
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:43:54.575641394+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Sweden Central AI Foundry doesn't work

[ Pepijn Fijt ][8] 5 Reputation points
2026-05-20T08:17:25.1466667+00:00

Hello Azure Team / Community,

We are experiencing severe performance degradation with our Anthropic Claude model deployments via Microsoft Foundry /
Azure AI Foundry.

**Region:** Sweden Central

**Problem:** Request response times are exceptionally slow, to the point where our application layer routinely hits
gateway or client timeouts before Claude can complete its response.

**Impact:** This is currently blocking our development and production workflows, as the model has become unusable under
standard API timeout thresholds.

Given that Claude models on Microsoft Foundry currently route inference via managed partner infrastructure, we suspect
there may be a regional peering bottleneck, severe capacity constraints, or routing latency between the Sweden Central
control plane and the underlying inference endpoints.

Could the engineering team look into whether there is an active service degradation, high token-per-minute (TPM) queuing
delays, or network issues specific to the Claude routing path in Sweden Central?

Thank you for your help.

Foundry Models
[ Foundry Models ][9]

A catalog of AI models in Microsoft Foundry that you can discover, compare, and deploy using Azure’s built‑in tools for
evaluation, fine‑tuning, and inference

Sign in to follow Follow
6 comments Hide comments for this question Report a concern
I have the same question (0)
1. [ Schönwald, Alexander ][10] • Follow 120 Reputation points
   2026-05-21T10:11:05.98+00:00
   
   Bump, we are experience the same for 2 days now!
   
   0 votes Report a concern
2. [ kagiyama yutaka ][11] • Follow 3,925 Reputation points
   2026-05-21T10:30:34.7466667+00:00
   
   @**[Pepijn Fijt][12]**— still seeing the slowdown? if it continues, a recent request‑id + timestamp usually helps
   Support check the region path.
   
   0 votes Report a concern
3. [ Pepijn Fijt ][13] • Follow 5 Reputation points
   2026-05-21T12:05:25.3666667+00:00
   
   [@kagiyama yutaka ][14]For now it seems to be working fine. The issues were temporary.. Let's hope it will more
   stable from now on.
   
   1 vote Report a concern
4. [ kagiyama yutaka ][15] • Follow 3,925 Reputation points
   2026-05-21T12:11:29.5166667+00:00
   
   [@Pepijn Fijt ][16]thx for the update, if u ever hear what caused it, mind sharing? only if u got time. might help
   folks hitting the same thing.
   
   0 votes Report a concern
5. [ Karnam Venkata Rajeswari ][17] • Follow 4,260 Reputation points • Microsoft External Staff • Moderator
   2026-06-01T10:06:47.1266667+00:00
   
   Hello [@Pepijn Fijt ][18] ,
   
   Following up to see if the below answer was helpful. If this answers your query, could you please take a moment to
   mark it as Accepted with an upvote? This helps others in the community with the same question find the solution more
   easily.
   
   Thank you
   
   0 votes Report a concern
6. [ Karnam Venkata Rajeswari ][19] • Follow 4,260 Reputation points • Microsoft External Staff • Moderator
   2026-06-02T11:09:35.63+00:00
   
   Hello [@Pepijn Fijt ][20] ,
   
   Just checking in to see if you have got a chance to see my response to resolve the issue.
   
   Looking forward to your response and appreciate your time on this.
   
   If the query has been resolved, please accept the answer by clicking the "Upvote" and "Accept Answer" on the post.
   
   Thank you!
   
   0 votes Report a concern
Show 1 more comment
[ Sign in to comment ][21]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 3 answers

Sort by: Most helpful
[Most helpful][22] [Newest][23] [Oldest][24]
1. [ Karnam Venkata Rajeswari ][25] • Follow 4,260 Reputation points • Microsoft External Staff • Moderator
   2026-05-31T18:11:03.9566667+00:00
   
   Hello [@Pepijn Fijt ][26] ,
   
   Welcome to Microsoft Q&A .Thank you for reaching out to us.
   
   The observed behavior aligns with a temporary backend latency or routing-layer queuing condition in managed Claude
   deployments within Azure AI Foundry. Since performance has normalized and no active incident is currently reported,
   the issue is assessed as a transient, self-resolving platform event that has been successfully mitigated.
   
   Explanation of Observed Behavior
   
   Claude deployments in Azure AI Foundry rely on a multi-layer architecture that includes:
   * Regional routing and gateway layers
   * Managed inference infrastructure hosted through partner integration
   * Traffic distribution and load balancing systems
   
   During short periods of elevated demand or backend strain, this design may lead to:
   * Increased response latency without explicit errors
   * Gateway or client timeouts under standard thresholds
   * Temporary variability that resolves automatically once load stabilizes
   
   The observed recovery without configuration changes strongly aligns with a transient backend queuing or capacity
   condition that has since been resolved.Claude deployments in Azure AI Foundry rely on a multi-layer architecture that
   includes:
   * Regional routing and gateway layers
   * Managed inference infrastructure hosted through partner integration
   * Traffic distribution and load balancing systems
   
   During short periods of elevated demand or backend strain, this design may lead to:
   * Increased response latency without explicit errors
   * Gateway or client timeouts under standard thresholds
   * Temporary variability that resolves automatically once load stabilizes
   
   The observed recovery without configuration changes strongly aligns with a transient backend queuing or capacity
   condition that has since been resolved.
   
   The following validation steps might help if faced with the same behavior occurs later in time
   1. Monitoring Platform Metrics Use Azure Monitor to review:
      * Latency percentiles (P50 / P95 / P99)
      * Request volume and concurrency
      * Throttling indicators (HTTP 429)
      * Error and timeout patterns
      Sustained latency increases without error spikes often indicate backend queuing rather than application failure.
   2. Improving resilience to transient latency To reduce impact during intermittent slowdowns:
      * Enable streaming responses where supported
      * Increase client / gateway timeout thresholds
      * Implement retry policies with exponential backoff and jitter
      * Optimize prompt size and reduce redundant context
      * Avoid sudden high-concurrency bursts
   
   The following references might be helpful , please check them out
   * [Azure Monitor overview - Azure Monitor | Microsoft Learn][27]
   
   Please let us know if the response was helpful
   
    
   
   Thank you
   
    
   
   Please 'Upvote'(Thumbs-up) and 'Accept' as answer if the response was helpful. This will be benefitting other
   community members who face the same issue.
   
   Was this answer helpful?
   
   Yes No
   0 comments No comments Report a concern
   [ Sign in to comment ][28]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
2. [ kagiyama yutaka ][29] • Follow 3,925 Reputation points
   2026-05-20T09:33:29.8566667+00:00
   
   I think the safe step is to take one slow request‑id and timestamp and send them to Azure Support, as region‑side
   issues in Sweden Central are handled through Status and Support.
   
   Was this answer helpful?
   
   Yes No
   0 comments No comments Report a concern
   [ Sign in to comment ][30]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
3. AI answer
   
   2026-05-20T08:17:46.96+00:00
   
   Based on the available information, Claude models in Microsoft Foundry are only supported in the East US 2 and Sweden
   Central regions. For Sweden Central, there is no region-specific performance or incident detail in the provided
   context, but the following checks and mitigations are supported:
   1. Confirm region support and avoid cross-region deployment
      * Claude models must be deployed in East US 2 or Sweden Central. Ensure the Foundry resource and Claude
        deployments are actually in Sweden Central and that the application is not inadvertently calling a different
        region, which could add latency.
   2. Check for rate limiting and queuing (HTTP 429)
      * If requests are hitting subscription or deployment limits, the service can queue or throttle requests, which
        manifests as slow or timed-out responses.
      * For Claude Code and related tooling, the guidance is to check quotas in the Foundry portal under **Operate >
        Quotas** and adjust token limits or request a quota increase if needed. The same quota constraints can affect
        API calls from applications.
      * If HTTP 429 responses appear, implement exponential backoff and retry logic as recommended.
   3. Validate deployment health
      * If the deployment is not fully ready or is in a non-succeeded provisioning state, behavior can be degraded. For
        similar deployment issues, deleting and recreating the deployment has resolved problems once the platform side
        was healthy again.
   4. Rule out authentication and configuration issues
      * Ensure that authentication is not intermittently failing or re-authenticating mid-call (for example,
        invalid/expired tokens causing retries). For Claude in Foundry, incorrect scopes or keys typically surface as
        401/403, but misconfiguration can still add latency if the client repeatedly retries.
      * Verify:
        * API calls use the correct base URL pattern: `https://<resource-name>.services.ai.azure.com/anthropic`.
        * The deployment name used by the application matches the configured Claude deployment.
   5. Compare behavior across regions (if possible)
      * If organizational policy allows, test the same workload against an equivalent deployment in East US 2. If
        performance is normal there but degraded in Sweden Central, this strongly indicates a region-specific service or
        capacity issue.
   6. When to escalate
      * If:
        * Quotas are not being exceeded,
        * Deployments are healthy,
        * Network from the application to the Sweden Central endpoint is verified (no firewall/VNet/private endpoint
          issues), and
        * The same workload performs normally in another region,
      * Then this aligns with a service-side degradation scenario similar to other Azure services where the recommended
        path is to open an Azure Support case or work with Microsoft support/PG to investigate regional issues
        (capacity, routing, or service incidents).
   
   Because the context does not list any active incident or known degradation for Claude in Sweden Central, the
   supported actions are to validate configuration, check quotas and rate limiting, test a comparison region, and then
   escalate through Azure Support with correlation IDs and timestamps from affected requests so the engineering team can
   investigate the Sweden Central Claude routing path.
   
   References:
   * [Deploy and use Claude models in Microsoft Foundry (preview)][31]
   * [Configure Claude Code for Microsoft Foundry][32]
   * [Configure Claude Desktop for Microsoft Foundry][33]
   * [Region availability for Foundry Models sold by Azure (standard)][34]
   * [Region availability for Foundry Models sold by Azure (provisioned)][35]
   * [Enable priority processing for Microsoft Foundry models][36]
   * [Microsoft Foundry publish to Teams Bot service error - Microsoft Q&A][37]
   * [Deployed Claude and getting API deployment resource is not ready even though it has been an hour - Microsoft
     Q&A][38]
   * [Document Intelligence classification and extraction requests timing out - Microsoft Q&A][39]
   * [Trying to create an azure local cluster - Microsoft Q&A][40]
   * [Azure Deployment Takes 70 Minutes to Fail with Obscure Error - Microsoft Q&A][41]
   
   AI-generated content may be incorrect. Read our [transparency notes][42] for more information.
   
   Was this answer helpful?
   
   Yes No
   Report a concern
[ Sign in to answer ][43]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-ca][44]
[ Your Privacy Choices][45]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][46]
* [Previous Versions][47]
* [Blog][48]
* [Contribute][49]
* [Privacy][50]
* [Consumer Health Privacy][51]
* [Terms of Use][52]
* [Code of Conduct][53]
* [Trademarks][54]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F5896
701%2Fsweden-central-ai-foundry-doesnt-work 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2
F5896701%2Fsweden-central-ai-foundry-doesnt-work 											&text=Sweden%20Central%20AI%20Foundry%20doesn't%20work 											&tw_p=tweetbutto
n&url=https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F5896701%2Fsweden-central-ai-foundry-doesnt-work
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F5896701%2F
sweden-central-ai-foundry-doesnt-work
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Sweden%20Central%20AI%20Foundry%20doesn't%20work&body=Sweden Central AI 
Foundry doesn't workhttps%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F5896701%2Fsweden-central-ai-foundr
y-doesnt-work
[8]: /en-ca/users/na/?userid=0e326642-52d7-450b-8b63-1ca13adad24e
[9]: /en-ca/answers/tags/1584/foundry-models/
[10]: /en-ca/users/na/?userid=efe109f2-1d19-482c-9770-996aa5092dc4
[11]: /en-ca/users/na/?userid=343ea57f-96c7-4fdc-b807-0ef49c154584
[12]: https://learn.microsoft.com/en-us/users/na/?userid=0e326642-52d7-450b-8b63-1ca13adad24e
[13]: /en-ca/users/na/?userid=0e326642-52d7-450b-8b63-1ca13adad24e
[14]: /users/na/?userid=343ea57f-96c7-4fdc-b807-0ef49c154584
[15]: /en-ca/users/na/?userid=343ea57f-96c7-4fdc-b807-0ef49c154584
[16]: /users/na/?userid=0e326642-52d7-450b-8b63-1ca13adad24e
[17]: /en-ca/users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[18]: https://learn.microsoft.com/en-us/users/na/?userid=0e326642-52d7-450b-8b63-1ca13adad24e
[19]: /en-ca/users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[20]: https://learn.microsoft.com/en-us/users/na/?userid=0e326642-52d7-450b-8b63-1ca13adad24e
[21]: #
[22]: ?orderby=helpful&page=1#answers
[23]: ?orderby=newest&page=1#answers
[24]: ?orderby=oldest&page=1#answers
[25]: /en-ca/users/na/?userid=c02e51dd-6f8b-4c05-baa1-b18f5388b4bc
[26]: https://learn.microsoft.com/en-us/users/na/?userid=0e326642-52d7-450b-8b63-1ca13adad24e
[27]: https://learn.microsoft.com/en-us/azure/azure-monitor/fundamentals/overview
[28]: #
[29]: /en-ca/users/na/?userid=343ea57f-96c7-4fdc-b807-0ef49c154584
[30]: #
[31]: https://learn.microsoft.com/azure/foundry/foundry-models/how-to/use-foundry-models-claude#troubleshooting
[32]: https://learn.microsoft.com/azure/foundry/foundry-models/how-to/configure-claude-code#troubleshooting
[33]: https://learn.microsoft.com/azure/foundry/foundry-models/how-to/configure-claude-desktop
[34]: https://learn.microsoft.com/azure/foundry/foundry-models/concepts/models-sold-directly-by-azure-region-availabilit
y#global-standard
[35]: https://learn.microsoft.com/azure/foundry/foundry-models/concepts/models-sold-directly-by-azure-region-availabilit
y#regional-provisioned-managed
[36]: https://learn.microsoft.com/azure/foundry/openai/concepts/priority-processing#priority-processing-availability-by-
deployment-type
[37]: https://learn.microsoft.com/answers/a/12641989
[38]: https://learn.microsoft.com/answers/a/12385413
[39]: https://learn.microsoft.com/answers/a/12244048
[40]: https://learn.microsoft.com/answers/a/12283506
[41]: https://learn.microsoft.com/answers/a/12358264
[42]: /answers/support/ai-first-overview
[43]: #
[44]: #
[45]: https://aka.ms/yourcaliforniaprivacychoices
[46]: https://learn.microsoft.com/en-ca/principles-for-ai-generated-content
[47]: https://learn.microsoft.com/en-ca/previous-versions/
[48]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[49]: https://learn.microsoft.com/en-ca/contribute
[50]: https://go.microsoft.com/fwlink/?LinkId=521839
[51]: https://go.microsoft.com/fwlink/?linkid=2259814
[52]: https://learn.microsoft.com/en-ca/legal/termsofuse
[53]: https://aka.ms/msftqacodeconduct
[54]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
