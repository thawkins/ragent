# Web source

- URL: https://learn.microsoft.com/en-us/answers/questions/5900139/billing-error-token-cache-hit-rate-reporting-0-acr
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:41:21.564402222+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Billing Error — Token Cache Hit Rate Reporting 0% Across All Workloads; Requesting Review

[ Raymond Pitts ][8] 0 Reputation points
2026-05-23T12:24:34.5766667+00:00

Our Azure AI Foundry billing shows $10,586 in Foundry Models charges over the last 22 days, against $13,395 in total
spend since onboarding. Token cache hit rate is reporting **0%** across all workloads. This is incorrect.

We run three independent applications — all configured with prompt caching enabled. The probability that three separate
applications, operating independently, each produce a 0% cache hit rate is not credible. This indicates a **metering or
reporting defect**, not a product behavior issue.

**Financial Impact**

Cached input tokens are priced at approximately 10% of standard input token rates. If cache hits are occurring but not
being metered correctly, we are being billed at 10x the correct rate for cached tokens.

Our billing breakdown shows the following token categories actively accruing charges in the affected period:
* 5.4 longco inp Gl 1M Tokens: $3,124.22
* 5.4 inp Gl 1M Tokens: $1,763.58
* 5.5 ShortCo inp Gl 1M Tokens: $163.83
* 5.5 ShortCo cd inp Gl 1M Tokens: $115.42

Line items labeled "cd inp" confirm the platform is aware of cached input as a distinct billing dimension — yet cache
utilization is reporting zero. These are contradictory states.

**Request**
1. Confirm whether token cache hit metering is functioning correctly for Claude 5.4 and 5.5 model families in our
   subscription
2. Pull raw metering logs for our workloads from April 29 – May 21 and audit cache hit reporting
3. Issue billing credits for the delta between standard input token pricing and the correct cached token pricing for all
   cache hits that were not metered during this period

We started with $25,000 in credits. $10,586 has been consumed in 22 days due to what the data indicates is a metering
defect, not legitimate usage growthOur Azure AI Foundry billing shows $10,586 in Foundry Models charges over the last 22
days, against $13,395 in total spend since onboarding. Token cache hit rate is reporting **0%** across all workloads.
This is incorrect.

We run three independent applications - all configured with prompt caching enabled. The probability that three separate
applications, operating independently, each produce a 0% cache hit rate is not credible. This indicates what we believe
to be a **metering or reporting defect**.

**Financial Impact**

Cached input tokens are priced at approximately 10% of standard input token rates. If cache hits are occurring but not
being metered correctly, we are being billed at 10x the correct rate for cached tokens.

Our billing breakdown shows the following token categories actively accruing charges in the affected period:
* 5.4 longco inp Gl 1M Tokens: $3,124.22
* 5.4 inp Gl 1M Tokens: $1,763.58
* 5.5 ShortCo inp Gl 1M Tokens: $163.83
* 5.5 ShortCo cd inp Gl 1M Tokens: $115.42

Line items labeled "cd inp" confirm the platform is aware of cached input as a distinct billing dimension — yet cache
utilization is reporting zero. These are contradictory states.

**Ask**
1. 1. Is there a known metering issue with cache hit tracking for Claude 5.4/5.5 models in Foundry
2. What is the escalation path to get raw metering logs audited and billing credits issued?

Azure OpenAI in Foundry Models
[ Azure OpenAI in Foundry Models ][9]

An Azure service that provides access to OpenAI’s GPT-3 models with enterprise capabilities.

Sign in to follow Follow
3 comments Hide comments for this question Report a concern
I have the same question (0)
1. [ Santhosh Kumar Machukuri ][10] • Follow 0 Reputation points • Microsoft External Staff • Moderator
   2026-05-23T21:38:55.1+00:00
   
   Hey **[Raymond][11]**, thanks for flagging this—getting charged full‐price for what should be cached tokens is
   definitely concerning. From the documentation we have on Foundry billing and prompt caching, there’s no known
   widespread defect reporting a 0% hit rate for Claude 5.4/5.5, so let’s dig in and make sure everything’s configured
   correctly and then get this escalated to billing.
   
   Here’s what we can do next:
   1. Validate your prompt caching setup
      * Prompt caching in Azure OpenAI (and by extension Foundry) only kicks in when the first 1,024 tokens are
        identical and then every additional 128 tokens match exactly.
      * Double-check that each of your three workloads is meeting those thresholds (otherwise you won’t see any cache
        hits by design).
   2. Gather the details we’ll need to investigate and escalate • Subscription ID and invoice or billing period (Apr
      29–May 21) • Resource names and deployment IDs for your Claude 5.4 and 5.5 endpoints • Exact meter IDs showing “cd
      inp” entries in Cost Management (so we know which line items to audit) • Sample usage patterns (token counts per
      request) to verify they should have been cache-eligible
   3. Open a billing support request in the Azure portal
      * Go to Help + support > New support request
      * Issue type: “Billing” > Service: “Azure AI Foundry Models”
      * In the problem details, reference the period and meter IDs, and request raw metering logs for April 29–May 21
        plus an audit of cache hit reporting
      * Ask explicitly for credits to cover the delta between standard input pricing and cached pricing for any
        un-metered hits
   
   Once you’ve filed that case, the Azure billing team can pull the raw logs, confirm whether cache hits occurred behind
   the scenes, and apply credits for any mis-billed tokens. In parallel, share the details above here so we can monitor
   the case and assist with any technical follow-up.
   
   Hope this helps! Let me know if you need anything else.
   
   Reference list
   * Plan and manage costs for Microsoft Foundry
     [https://learn.microsoft.com/azure/foundry/concepts/manage-costs#understand-the-billing-model-for-foundry-models][1
     2]
   * Plan to manage costs for Azure OpenAI in Azure AI Foundry Models
     [https://learn.microsoft.com/azure/ai-foundry/openai/how-to/manage-costs#understand-the-azure-openai-full-billing-m
     odel][13]
   * Effective Management of Token Billing in Azure OpenAI (Prompt Caching Guide)
     [https://learn.microsoft.com/azure/ai-services/openai/how-to/prompt-caching][14]
   * Azure Cost Management meter data overview
     [https://learn.microsoft.com/azure/cost-management-billing/costs/understand-cost-meters][15].
   
   0 votes Report a concern
2. [ Raymond Pitts ][16] • Follow 0 Reputation points
   2026-06-02T01:51:13.05+00:00
   
   [@Santhosh Kumar Machukuri ][17]I investigated more closely, it appears caching is working but the billing charges do
   not match the token usage within Azure AI Foundry. How can we be charged for tokens that do appear not in our usage?
   Please advise, this is a P1 for our costs at the moment.
   
   0 votes Report a concern
3. [ Raymond Pitts ][18] • Follow 0 Reputation points
   2026-06-03T02:19:13.46+00:00
   
   [@Santhosh Kumar Machukuri ][19][@Santhosh Kumar Machukuri ][20]We created a support ticket with full details
   2606030040000464
   
   0 votes Report a concern
[ Sign in to comment ][21]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 1 answer

Sort by: Most helpful
[Most helpful][22] [Newest][23] [Oldest][24]
1. [ kagiyama yutaka ][25] • Follow 3,925 Reputation points
   2026-05-24T03:33:10.58+00:00
   
   I think cache‑hit billing for Foundry Models is validated through Cost Management meter IDs, and any investigation or
   correction is handled by the Azure Billing support team. As of now, no Microsoft documentation reports any known
   cache‑metering issue for Claude 5.4 or 5.5.
   
   Was this answer helpful?
   
   Yes No
   5 comments Show comments for this answer Report a concern
   1. [ Manas Mohanty ][26] • Follow 17,265 Reputation points • Microsoft External Staff • Moderator
      2026-05-25T19:31:06.0433333+00:00
      
      Hi **[Raymond Pitts][27]**
      
      Could you please create support ticket on Billing side for investigation with above insights.
      
      [User's image]
      
      We are not actually handling billing queries on technical support side.
      
      Thank you for understanding our limitations.
      
      1 vote Report a concern
   2. [ Raymond Pitts ][28] • Follow 0 Reputation points
      2026-06-02T01:49:34.97+00:00
      
      [@Manas Mohanty ][29]Everytime I try to make a billing ticket, it says "request a refund", then that redirects me
      back to the general support page. Please advise.
      
      0 votes Report a concern
   3. [ Raymond Pitts ][30] • Follow 0 Reputation points
      2026-06-03T01:47:44.4466667+00:00
      
      We are also working from an Azure sponsorship subscription that has given us startup credits ". When we click
      "request a refund" it says:
      
      Your account is not eligible for expedited refunds
      
      Submit a support request to get a refund.
      
      0 votes Report a concern
   4. [ Raymond Pitts ][31] • Follow 0 Reputation points
      2026-06-03T02:19:28.7033333+00:00
      
      [@kagiyama yutaka ][32]We created a support ticket with full details 2606030040000464
      
      0 votes Report a concern
   5. [ Raymond Pitts ][33] • Follow 0 Reputation points
      2026-06-07T17:05:46.9233333+00:00
      
      Hello [@Manas Mohanty ][34]has there been any updates on this? I've filed a support ticket 12 days ago that has
      gone un-answered
      
      0 votes Report a concern
   [ Sign in to comment ][35]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
[ Sign in to answer ][36]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-us][37]
[ Your Privacy Choices][38]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][39]
* [Previous Versions][40]
* [Blog][41]
* [Contribute][42]
* [Privacy][43]
* [Consumer Health Privacy][44]
* [Terms of Use][45]
* [Code of Conduct][46]
* [Trademarks][47]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5900
139%2Fbilling-error-token-cache-hit-rate-reporting-0-acr 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2
F5900139%2Fbilling-error-token-cache-hit-rate-reporting-0-acr 											&text=Billing%20Error%20%E2%80%94%20Token%20Cache%20Hit%20
Rate%20Reporting%200%25%20Across%20All%20Workloads%3B%20Requesting%20Review 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.mi
crosoft.com%2Fen-us%2Fanswers%2Fquestions%2F5900139%2Fbilling-error-token-cache-hit-rate-reporting-0-acr
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5900139%2F
billing-error-token-cache-hit-rate-reporting-0-acr
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Billing%20Error%20%E2%80%94%20Token%20Cache%20Hit%20Rate%20Reporting%200
%25%20Across%20All%20Workloads%3B%20Requesting%20Review&body=Billing Error — Token Cache Hit Rate Reporting 0% Across Al
l Workloads; Requesting Reviewhttps%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5900139%2Fbilling-error-
token-cache-hit-rate-reporting-0-acr
[8]: /en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[9]: /en-us/answers/tags/387/azure-openai/
[10]: /en-us/users/na/?userid=ca583741-24d3-477b-aaf8-4664d8e317bd
[11]: https://learn.microsoft.com/en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[12]: https://learn.microsoft.com/azure/foundry/concepts/manage-costs#understand-the-billing-model-for-foundry-models
[13]: https://learn.microsoft.com/azure/ai-foundry/openai/how-to/manage-costs#understand-the-azure-openai-full-billing-m
odel
[14]: https://learn.microsoft.com/azure/ai-services/openai/how-to/prompt-caching
[15]: https://learn.microsoft.com/azure/cost-management-billing/costs/understand-cost-meters
[16]: /en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[17]: /users/na/?userid=ca583741-24d3-477b-aaf8-4664d8e317bd
[18]: /en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[19]: /users/na/?userid=384f81ef-8716-4d03-9049-d6fc961c24c7
[20]: /users/na/?userid=384f81ef-8716-4d03-9049-d6fc961c24c7
[21]: #
[22]: ?orderby=helpful&page=1#answers
[23]: ?orderby=newest&page=1#answers
[24]: ?orderby=oldest&page=1#answers
[25]: /en-us/users/na/?userid=343ea57f-96c7-4fdc-b807-0ef49c154584
[26]: /en-us/users/na/?userid=6ed415a7-b41e-41ad-bd5b-ede8461a8b16
[27]: https://learn.microsoft.com/en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[28]: /en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[29]: /users/na/?userid=6ed415a7-b41e-41ad-bd5b-ede8461a8b16
[30]: /en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[31]: /en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[32]: /users/na/?userid=343ea57f-96c7-4fdc-b807-0ef49c154584
[33]: /en-us/users/na/?userid=afd29b86-7748-4cb3-9bc3-26a1ec60bd32
[34]: /users/na/?userid=6ed415a7-b41e-41ad-bd5b-ede8461a8b16
[35]: #
[36]: #
[37]: #
[38]: https://aka.ms/yourcaliforniaprivacychoices
[39]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[40]: https://learn.microsoft.com/en-us/previous-versions/
[41]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[42]: https://learn.microsoft.com/en-us/contribute
[43]: https://go.microsoft.com/fwlink/?LinkId=521839
[44]: https://go.microsoft.com/fwlink/?linkid=2259814
[45]: https://learn.microsoft.com/en-us/legal/termsofuse
[46]: https://aka.ms/msftqacodeconduct
[47]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
