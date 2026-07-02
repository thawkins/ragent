# Web source

- URL: https://learn.microsoft.com/en-us/answers/questions/5764725/does-kimi-k2-5-on-ai-foundry-has-prompt-caching
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:41:20.468480970+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Does Kimi K2.5 on AI foundry has prompt caching.

[ andrew ][8] 20 Reputation points
2026-02-06T13:40:06.3666667+00:00

Does Kimi K2.5 on AI foundry has prompt caching. I wanted to check if it has prompt caching enabled by default for it.

Foundry Tools
[ Foundry Tools ][9]

Formerly known as Azure AI Services or Azure Cognitive Services is a unified collection of prebuilt AI capabilities
within the Microsoft Foundry platform

Sign in to follow Follow
2 comments Hide comments for this question Report a concern
I have the same question (0)
1. Anonymous
   2026-02-09T08:38:29.4766667+00:00
   
   Hi **[andrew][10]**
   
   Did you get any chance to review the above response.
   
   Thank you!
   
   0 votes Report a concern
2. Anonymous
   2026-02-10T10:49:29.0166667+00:00
   
   Hi **[andrew][11]**
   
   We haven’t heard from you on the last response and was just checking back to see if you have a resolution yet. In
   case if you have any resolution please do share that same with the community as it can be helpful to others.
   Otherwise, will respond with more details and we will try to help.
   
   0 votes Report a concern
[ Sign in to comment ][12]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 2 answers

Sort by: Most helpful
[Most helpful][13] [Newest][14] [Oldest][15]
1. Anonymous
   2026-02-06T14:10:52.7433333+00:00
   
   Hi **[andrew][16]**
   
   Kimi K2.5 on Azure AI Foundry does not have prompt caching enabled by default, and there is **no customer-visible or
   configurable prompt caching feature** exposed for Kimi models in Azure AI Foundry.
   
   As of today, **Microsoft documentation for Azure AI Foundry does not list prompt caching as a supported or
   configurable capability for Kimi models**, including **Kimi K2.5**. Neither the Foundry portal experience nor the
   API/SDK references mention prompt caching for Kimi. In Azure AI, behaviors are considered supported only when they
   are explicitly documented.
   
   Azure does **not make contractual or behavioral guarantees** about internal execution optimizations (such as reuse of
   identical prompts) unless they are explicitly documented as a feature. For **Kimi K2.5**, there is **no documented
   guarantee** that repeated prompts benefit from caching, reuse, or reduced cost/latency.
   
   While Azure may apply **internal, opaque, transient optimizations** at the infrastructure level, these are **not
   exposed, configurable, or reliable** from a customer perspective. They should **not be assumed** for architecture,
   cost planning, or latency optimization, and they **must not be relied on** as prompt caching behavior.
   
   From an architectural standpoint, you should **treat Kimi K2.5 as processing every request independently**. Identical
   prompts will incur **full token processing and inference cost each time**, and latency will **not automatically
   improve** for repeated static system prompts.
   
   **(workarounds):**
   
   If you need caching behavior, it must be implemented at the **application layer**, not the model layer. Common
   patterns include:
   * Caching deterministic responses client-side for identical prompts
   * Reducing repeated static instructions by restructuring prompts
   * Using embeddings and retrieval for static context instead of re-sending large prompt blocks **Note:**Kimi K2.5 on
     Azure AI Foundry does not support prompt caching, is not enabled by default, and cannot be enabled by customers.
     Design your solution assuming **no prompt caching**, and plan cost and latency accordingly.
   
   ### References:
   
   [Azure OpenAI Service documentation - prompt caching][17]
   
   [Azure OpenAI Service pricing][18]
   
   [Prompt engineering best practices][19]
   
   Please let me know if there are any remaining questions or additional details, I can help with, I’ll be glad to
   provide further clarification or guidance.
   
   Thankyou!
   
   Was this answer helpful?
   
   Yes No
   0 comments No comments Report a concern
   [ Sign in to comment ][20]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
2. AI answer
   
   2026-02-06T13:40:13.73+00:00
   
   Yes, Kimi K2.5 on AI Foundry has prompt caching enabled by default. This feature allows for improved performance by
   reducing request latency and cost for longer prompts that have identical content at the beginning. Prompt caching is
   supported for all Azure OpenAI models, including Kimi K2.5, and it requires that requests be a minimum of 1,024
   tokens in length with the first 1,024 tokens being identical for it to take effect.
   
   AI-generated content may be incorrect. Read our [transparency notes][21] for more information.
   
   Was this answer helpful?
   
   Yes No
   Report a concern
[ Sign in to answer ][22]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-us][23]
[ Your Privacy Choices][24]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][25]
* [Previous Versions][26]
* [Blog][27]
* [Contribute][28]
* [Privacy][29]
* [Consumer Health Privacy][30]
* [Terms of Use][31]
* [Code of Conduct][32]
* [Trademarks][33]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5764
725%2Fdoes-kimi-k2-5-on-ai-foundry-has-prompt-caching 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2
F5764725%2Fdoes-kimi-k2-5-on-ai-foundry-has-prompt-caching 											&text=Does%20Kimi%20K2.5%20on%20AI%20foundry%20has%20prompt%2
0caching. 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5764725%2Fdoes-kimi-k2
-5-on-ai-foundry-has-prompt-caching
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5764725%2F
does-kimi-k2-5-on-ai-foundry-has-prompt-caching
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Does%20Kimi%20K2.5%20on%20AI%20foundry%20has%20prompt%20caching.&body=Do
es Kimi K2.5 on AI foundry has prompt caching.https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5764725%
2Fdoes-kimi-k2-5-on-ai-foundry-has-prompt-caching
[8]: /en-us/users/na/?userid=dd3cd3b5-5d4c-42c7-ae88-f2f42a9ddf25
[9]: /en-us/answers/tags/1580/foundry-tools/
[10]: https://learn.microsoft.com/en-us/users/na/?userid=dd3cd3b5-5d4c-42c7-ae88-f2f42a9ddf25
[11]: https://learn.microsoft.com/en-us/users/na/?userid=dd3cd3b5-5d4c-42c7-ae88-f2f42a9ddf25
[12]: #
[13]: ?orderby=helpful&page=1#answers
[14]: ?orderby=newest&page=1#answers
[15]: ?orderby=oldest&page=1#answers
[16]: https://learn.microsoft.com/en-us/users/na/?userid=dd3cd3b5-5d4c-42c7-ae88-f2f42a9ddf25
[17]: https://learn.microsoft.com/en-us/azure/ai-services/openai/how-to/prompt-caching
[18]: https://azure.microsoft.com/pricing/details/cognitive-services/openai-service/
[19]: https://learn.microsoft.com/azure/cognitive-services/openai/concepts/prompt-engineering
[20]: #
[21]: /answers/support/ai-first-overview
[22]: #
[23]: #
[24]: https://aka.ms/yourcaliforniaprivacychoices
[25]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[26]: https://learn.microsoft.com/en-us/previous-versions/
[27]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[28]: https://learn.microsoft.com/en-us/contribute
[29]: https://go.microsoft.com/fwlink/?LinkId=521839
[30]: https://go.microsoft.com/fwlink/?linkid=2259814
[31]: https://learn.microsoft.com/en-us/legal/termsofuse
[32]: https://aka.ms/msftqacodeconduct
[33]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
