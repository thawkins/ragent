# Web source

- URL: https://learn.microsoft.com/en-us/answers/questions/5863634/how-to-increase-azure-ai-foundry-throughput-for-de
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:44:04.931122515+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# How to increase Azure AI Foundry throughput for deployed LLM under high parallel load?

[ Vitalii Horbovyi ][8] 25 Reputation points
2026-04-19T02:45:13.61+00:00

Hi,

I am experiencing significant throughput degradation when handling parallel user requests to GPT-4.1-mini via Azure AI
Foundry, and I would like your guidance on the best architectural approach for our situation.

**Current situation:**

Each user session triggers approximately 10 sequential LLM calls, with each call consuming roughly 10,000 tokens. In
isolation (single user), the full flow takes approximately 50-70 seconds.

However, under parallel load the performance degrades significantly: 4 concurrent users ~200-300 seconds (40 parallel
requests). This is already unacceptable for our use case, and we are concerned about what will happen at 10–50
concurrent users.

**What I have already tried or considered:**
1. Multiple deployments within a single subscription (Pay-As-You-Go). I deployed several models of GPT-4.1-mini with
   deployment type *Global Standard* within the same region and same subscription in Azure AI Foundry, expecting that
   load balancing across deployments would increase overall throughput. However, after reading the [following Microsoft
   documentation][9], I understand that standard quota is subscription-scoped, not deployment-scoped. Therefore, adding
   more deployments within the same subscription does not increase throughput.
2. Batch API. We evaluated the Batch API but it does not fit our use case, as we require real-time responses from the
   model.
3. Provisioned Throughput Units (PTU). We have evaluated PTU but it is not financially viable for our business at this
   stage. Our margins do not support this option.

Since standard quota is subscription-bound, would deploying one Azure AI Foundry instance per Azure subscription - each
in the same region, each with one GPT-4.1-mini deployment - and routing requests across them via a gateway effectively
multiply available throughput?

For example:
* Subscription A → Azure AI Foundry instance: 1× GPT-4.1-mini (Poland Central)
* Subscription B → Azure AI Foundry instance: 1× GPT-4.1-mini (Poland Central)
* Subscription C → Azure AI Foundry instance: 1× GPT-4.1-mini (Poland Central)
* Gateway → load balances across all three

Would this approach actually increase throughput proportionally to the number of subscriptions? Are there any
limitations, compliance considerations, or technical blockers we should be aware of? Is this a good way to scale the
system?

**Additional questions:**
1. Is there any other approach - aside from PTU and multi-subscription load balancing - that could meaningfully increase
   throughput for Pay-As-You-Go standard deployments under high parallel load?
2. What is the recommended scalable architecture for a workload like ours?

Thanks in advance!

Azure OpenAI in Foundry Models
[ Azure OpenAI in Foundry Models ][10]

An Azure service that provides access to OpenAI’s GPT-3 models with enterprise capabilities.

Sign in to follow Follow
0 comments No comments Report a concern
I have the same question (0)
[ Sign in to comment ][11]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 3 answers

Sort by: Most helpful
[Most helpful][12] [Newest][13] [Oldest][14]
1. [ Ghulam Muhayyu Din ][15] • Follow 0 Reputation points
   2026-04-19T17:12:23.48+00:00
   
   **Hello Vitalii,**
   
   Your observation about throughput degradation is a common challenge when transitioning from isolated testing to high
   parallel load.
   
   To answer your primary question: **Yes, the multi-subscription gateway approach you proposed will technically
   multiply your throughput.** Because Azure OpenAI rate limits (TPM and RPM) are scoped per region, per subscription,
   and per model, routing through three subscriptions in Poland Central will grant you three distinct quota pools.
   
   However, **this is generally considered an anti-pattern.** Scaling via multiple subscriptions introduces unnecessary
   administrative overhead, complex billing, and fragmented security. Best practices dictate using separate
   subscriptions only for distinct environments (like Dev vs. Prod), not for bypassing regional quotas.
   
   ### The Recommended Scalable Architecture (Pay-As-You-Go)
   
   Instead of a multi-subscription architecture, the most effective Pay-As-You-Go strategy is **Multi-Region scaling
   within a single subscription.**
   
   **Leverage Regional Quota Pools:** Because your quota is allocated *per region* within a single subscription, you can
   easily multiply your total available TPM/RPM by deploying your GPT-4.1-mini model across multiple regions (e.g.,
   Poland Central, Sweden Central, and East US).
   
   **Implement Azure API Management (APIM):** Place Azure APIM in front of these regional deployments.
   
   **Use Smart Load Balancing & Circuit Breakers:** Configure APIM to distribute requests across your multiple regional
   endpoints. By implementing a circuit breaker policy, APIM will detect when a specific region is overwhelmed
   (returning 429 rate limit errors) and automatically reroute subsequent requests to the next available region. This
   prevents cascading failures and ensures high availability.
   
   **A Note on Global Standard:** You mentioned using "Global Standard" deployments. These are already designed to
   dynamically route your traffic to the datacenter with the best availability across Azure's global infrastructure. If
   you are still hitting rate limits on Global Standard, your immediate next step should be to submit a request for a
   quota increase through the Azure Foundry Service, as Global Standard typically offers the highest initial throughput
   limits. If that is insufficient, transition to the Multi-Region + APIM architecture described above.
   
   Was this answer helpful?
   
   Yes No
   1 person found this answer helpful.
   1 comment Show comments for this answer Report a concern
   1. [ Vitalii Horbovyi ][16] • Follow 25 Reputation points
      2026-04-19T20:26:11.3866667+00:00
      
      Hi Ghulam,
      
      Thank you for the detailed explanation. I went ahead and tried the multi-region approach within a single
      subscription - I created several Azure AI Foundry instances, each in a different region, and deployed one model
      per instance.
      
      Unfortunately, the throughput did not improve. It seems like the quota is actually enforced at the subscription
      level overall, not per region independently.
      
      At this point, it looks like the only viable option is actually creating separate subscriptions after all - which
      is exactly the anti-pattern you mentioned. Not ideal, but it seems like there may be no other way around it.
      
      0 votes Report a concern
   [ Sign in to comment ][17]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
2. [ SRILAKSHMI C ][18] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
   2026-04-25T11:34:07.87+00:00
   
   Hello [@Vitalii Horbovyi][19]
   
   Thank you for the detailed explanation. You’ve already identified the key constraint correctly: for **Standard
   (Pay-As-You-Go) deployments**, throughput is governed by **subscription-level quota** (Tokens Per Minute and Requests
   Per Minute) for a given model and region. As a result, deploying multiple instances of the same model within the same
   subscription does **not** increase total throughput, since all deployments share the same quota pool.
   
   **Why You’re Seeing Throughput Degradation**
   
   Your workload is highly parallel and token-intensive:
   * Approximately **10 sequential LLM calls per user session**
   * Roughly **10,000 tokens per call**
   * Around **100,000 tokens per user session**
   
   With just four concurrent users, this translates to:
   * Approximately **40 simultaneous requests**
   * Up to **400,000 tokens being processed concurrently**
   
   As your workload approaches your subscription’s TPM/RPM limits, Azure OpenAI begins queueing requests. Even before
   explicit throttling (`429`) occurs, this queuing can significantly increase latency—which is exactly the behavior
   you're observing.
   
   **Can Multiple Subscriptions Increase Throughput?**
   
   Yes. This is a valid and widely adopted scaling strategy for Standard deployments.
   
   Because quota is allocated **per subscription, per region, per model**, each additional subscription receives its own
   independent quota allocation. By deploying separate Azure OpenAI resources (or Azure AI Foundry hubs/projects) in
   multiple subscriptions, you can effectively scale throughput horizontally.
   
   For example:
   * **Subscription A** → Azure OpenAI resource → GPT-4.1-mini deployment
   * **Subscription B** → Azure OpenAI resource → GPT-4.1-mini deployment
   * **Subscription C** → Azure OpenAI resource → GPT-4.1-mini deployment
   
   A centralized gateway can then distribute traffic across these deployments, allowing aggregate throughput to scale
   approximately linearly with the number of subscriptions.
   
   This is often the most practical alternative when PTU is not yet financially viable.
   
   **Recommended Step: Request a Quota Increase**
   
   Before introducing architectural complexity, we strongly recommend submitting a quota increase request for your
   existing subscription.
   
   Even with Standard deployments, you can request higher TPM/RPM allocations for GPT-4.1-mini in your target region.
   This is the simplest and most cost-effective way to gain additional capacity.
   
   If approved, it may significantly improve throughput without requiring any changes to your application architecture.
   
   **Recommended Scalable Architecture**
   
   For sustained high concurrency, we recommend the following architecture:
   
   Multiple Azure subscriptions (or multiple Azure OpenAI resources)
   
   One GPT-4.1-mini deployment per resource
   
   Centralized routing layer using:
   * Azure Front Door
   * Azure API Management
   * Azure Traffic Manager
   * or a custom gateway
   
   Your gateway should:
   * Route requests to the least-utilized endpoint
   * Monitor token consumption and request rates
   * Respect `Retry-After` headers
   * Automatically retry or fail over on `429` responses
   * Implement exponential backoff and circuit breakers
   * Dynamically adjust routing based on endpoint health and latency
   
   This design provides:
   * Increased aggregate throughput
   * Better resilience and fault tolerance
   * Isolation from individual quota exhaustion
   * Improved operational flexibility
   
   Additional Optimization Opportunities
   
   **1. Reduce Token Consumption**
   
   Token optimization is often the fastest path to better throughput.
   * Minimize prompt size
   * Trim unnecessary conversation history
   * Use summarization between steps
   * Set `max_output_tokens` appropriately
   * Avoid over-allocating output tokens
   
   Even a 20–30% reduction in tokens can materially improve throughput and reduce costs.
   
   **2. Consolidate Sequential Calls**
   
   Ten sequential calls per session compounds latency.
   
   Consider:
   * Combining multiple tasks into a single prompt
   * Using structured outputs to perform multiple operations in one call
   * Parallelizing independent steps where possible
   
   Reducing the total number of model invocations can significantly improve end-to-end response times.
   
   **3. Use Smaller Models Strategically**
   
   Not every step may require GPT-4.1-mini.
   
   For lighter tasks, consider GPT-4.1-nano
   
   Smaller or specialized models for classification, extraction, or validation
   
   Reserve GPT-4.1-mini for the most reasoning-intensive stages.
   
   **4. Enable Streaming**
   
   Streaming does not reduce total compute time, but it substantially improves perceived responsiveness by returning
   tokens as they are generated.
   
   This can greatly enhance the user experience for interactive workloads.
   
   **Regional Scaling Option**
   
   If your compliance and data residency requirements permit, you can also distribute deployments across multiple Azure
   regions.
   
   This provides:
   * Additional quota pools
   * Better geographic resilience
   * Reduced risk of regional saturation
   
   However, be sure to consider:
   * Data residency requirements
   * Regulatory constraints
   * Cross-region latency
   * Governance and monitoring complexity
   
   **About PTU**
   
   You are correct that **Provisioned Throughput Units (PTU)** may not be cost-effective at your current scale.
   
   However, PTU remains the only option that provides:
   * Guaranteed throughput
   * Predictable latency
   * Reserved capacity
   * Performance consistency under sustained heavy load
   
   For business-critical or highly predictable workloads, it may be worth revisiting as your usage grows.
   
   **Key Considerations for Multi-Subscription Scaling**
   * Ensure each subscription has adequate quota approved
   * Centralize monitoring across all deployments
   * Standardize RBAC, networking, and security policies
   * Implement consolidated billing and cost tracking
   * Maintain consistent deployment configurations
   
   I Hope this helps. Do let me know if you have any further queries.
   
   If this answers your query, please do click Accept Answer and Yes for was this answer helpful.
   
   Thank you!
   
   Was this answer helpful?
   
   Yes No
   2 comments Show comments for this answer Report a concern
   1. [ SRILAKSHMI C ][20] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
      2026-04-27T12:18:45.5466667+00:00
      
       Hi [@Vitalii Horbovyi][21],
      
      Following up to see if the above answer was helpful. If this answers your query, please do click Accept Answer and
      Yes for was this answer helpful. And, if you have any further query do let us know.
      
      Thank you!
      
      0 votes Report a concern
   2. [ SRILAKSHMI C ][22] • Follow 19,550 Reputation points • Microsoft External Staff • Moderator
      2026-04-28T13:52:19.3466667+00:00
      
       Hi [@Vitalii Horbovyi][23],
      
      Just checking in to see if you have got a chance to see my response to your question in resolving the issue.
      
      If you are still facing any further issues, please don't hesitate to reach out to us. We are happy to assist you.
      
      Looking forward to your response and appreciate your time on this.
      
      If you feel that your quires have been resolved, please accept the answer by clicking the "Upvote" and **"Accept
      Answer"** on the post.
      
      Thank you!
      
      0 votes Report a concern
   [ Sign in to comment ][24]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
3. [ Vinodh247-1375 ][25] • Follow 43,346 Reputation points • Volunteer Moderator
   2026-04-19T06:38:06.88+00:00
   
   Hi ,
   
   Thanks for reaching out to Microsoft Q&A.
   
   your current design is token-heavy and chatty, which is why it collapses under parallel load. Fixing that will give
   you far more gain than just adding more subscriptions
   
   Short answer: Yes, your multi-subs approach will increase throughput, but it is a workaround, not the recommended
   long-term architecture.
   
   Paragraph answer: In Azure AI Foundry standard (pay-as-you-go) deployments, throughput is primarily constrained by
   subscription-level quotas (tokens per minute and requests per minute). Because of this, adding multiple deployments
   inside the same subscription does not help, but spreading deployments across multiple subscriptions does effectively
   multiply available throughput, provided each subscription has its own quota allocation. So your design (A/B/C
   subscriptions + gateway load balancing) will scale linearly in practice. However, this comes with operational
   overhead (quota management, auth, monitoring, cost tracking) and potential soft limits if Microsoft detects
   coordinated scaling patterns or applies regional capacity constraints.
   
   A better architecture for your case (high parallel, multi-step LLM workflows) is to reduce pressure on the model
   rather than only scaling horizontally. The biggest issue in your design is not just concurrency, but token volume (10
   calls × 10k tokens per user).
   
   You should aggressively optimize here:
   
   collapse multi-step chains into fewer prompts (prompt engineering or tool-calling)
   
   cache deterministic or semi-deterministic responses (semantic cache layer)
   
   use smaller or mixed models where possible (route some steps away from GPT-4.1-mini)
   
   implement request queuing + rate shaping instead of pure parallel fan-out
   
   introduce async pipelines where user experience allows partial streaming instead of blocking full flows
   
   If you still need raw throughput scaling without PTU, combine:
   * multi-subscription sharding (what you proposed)
   * multi-region deployments (if latency allows)
   * intelligent gateway (token-aware routing + backpressure)
   
   Please 'Upvote'(Thumbs-up) and 'Accept' as answer if the reply was helpful. This will be benefitting other community
   members who face the same issue.
   
   Was this answer helpful?
   
   Yes No
   0 comments No comments Report a concern
   [ Sign in to comment ][26]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
[ Sign in to answer ][27]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-us][28]
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
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5863
634%2Fhow-to-increase-azure-ai-foundry-throughput-for-de 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2
F5863634%2Fhow-to-increase-azure-ai-foundry-throughput-for-de 											&text=How%20to%20increase%20Azure%20AI%20Foundry%20through
put%20for%20deployed%20LLM%20under%20high%20parallel%20load%3F 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2
Fen-us%2Fanswers%2Fquestions%2F5863634%2Fhow-to-increase-azure-ai-foundry-throughput-for-de
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5863634%2F
how-to-increase-azure-ai-foundry-throughput-for-de
[7]: mailto:?subject= 												%5BShared%20Question%5D%20How%20to%20increase%20Azure%20AI%20Foundry%20throughput%20for%20deployed
%20LLM%20under%20high%20parallel%20load%3F&body=How to increase Azure AI Foundry throughput for deployed LLM under high 
parallel load?https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5863634%2Fhow-to-increase-azure-ai-found
ry-throughput-for-de
[8]: /en-us/users/na/?userid=24c4e435-2d5b-460f-91d6-63040cb9012b
[9]: https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/azure-openai-gateway-multi-backend
[10]: /en-us/answers/tags/387/azure-openai/
[11]: #
[12]: ?orderby=helpful&page=1#answers
[13]: ?orderby=newest&page=1#answers
[14]: ?orderby=oldest&page=1#answers
[15]: /en-us/users/na/?userid=80e7652f-3444-478f-8263-eedeee161cd7
[16]: /en-us/users/na/?userid=24c4e435-2d5b-460f-91d6-63040cb9012b
[17]: #
[18]: /en-us/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[19]: https://learn.microsoft.com/en-us/users/na/?userid=24c4e435-2d5b-460f-91d6-63040cb9012b
[20]: /en-us/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[21]: https://learn.microsoft.com/en-us/users/na/?userid=24c4e435-2d5b-460f-91d6-63040cb9012b
[22]: /en-us/users/na/?userid=d551a661-9581-42ac-9cfd-2bf53cccf48b
[23]: https://learn.microsoft.com/en-us/users/na/?userid=24c4e435-2d5b-460f-91d6-63040cb9012b
[24]: #
[25]: /en-us/users/na/?userid=a9730184-bffd-0003-0000-000000000000
[26]: #
[27]: #
[28]: #
[29]: https://aka.ms/yourcaliforniaprivacychoices
[30]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[31]: https://learn.microsoft.com/en-us/previous-versions/
[32]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[33]: https://learn.microsoft.com/en-us/contribute
[34]: https://go.microsoft.com/fwlink/?LinkId=521839
[35]: https://go.microsoft.com/fwlink/?linkid=2259814
[36]: https://learn.microsoft.com/en-us/legal/termsofuse
[37]: https://aka.ms/msftqacodeconduct
[38]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
