# Web source

- URL: https://learn.microsoft.com/en-in/answers/questions/5616660/does-azure-openai-ever-retain-or-persist-hidden-ac
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:41:36.810318095+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Does Azure OpenAI ever retain or persist hidden activations, KV-cache data, or prompt cache contents after request
# processing?

[ Alexander Bonnot ][8] 0 Reputation points
2025-11-10T21:24:39.72+00:00

It's important that we have no logging of any prompt data entered. I understand that we need approval to disable logging
for review and we can add a cache buster to the first 1024 characters of our prompts if we don't want short term prompt
caching.

My concern is about the tensors internal to the AI system. Algorithms like SipIt show that it is theoretically possible
to reconstruct the input using the hidden layers. I want to make sure that these tensors are never logged or transmitted
for any reason, out of concern that any sensitive part of the input could be reconstructed.

Azure OpenAI in Foundry Models
[ Azure OpenAI in Foundry Models ][9]

An Azure service that provides access to OpenAI’s GPT-3 models with enterprise capabilities.

Sign in to follow Follow
1 comment Hide comments for this question Report a concern
I have the same question (0)
1. Anonymous
   2025-11-11T16:45:32.5566667+00:00
   
   Hi **[Alexander Bonnot][10]**
   
   Did you get any chance to review the above response. Thank you!
   
   0 votes Report a concern
[ Sign in to comment ][11]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 1 answer

Sort by: Most helpful
[Most helpful][12] [Newest][13] [Oldest][14]
1. Anonymous
   2025-11-10T21:45:17.4633333+00:00
   
   Hi **[Alexander Bonnot][15]**
   
   Welcome to Microsoft Q&A and Thank you for reaching out.
   
   Azure OpenAI does not persist or log internal model states such as hidden activations, attention KV-cache, or
   intermediate tensors. These elements are ephemeral and exist only in memory during the inference process. Once the
   request completes, they are discarded and never written to disk, logged, or transmitted outside the secure inference
   environment.
   
   Prompt caching is a performance optimization for long prompts (typically 1024 tokens or more). It stores processed
   token computations not raw text to reduce latency and cost. This cache is short-lived, cleared after 5–10 minutes of
   inactivity, and always removed within one hour. Cached data is numeric and not shared across subscriptions, making it
   unsuitable for reconstructing original text.
   
   The cached data consists of low-level numeric arrays, such as attention key/value vectors, which are meaningless
   without the full model context and token mapping. Microsoft explicitly states that caching does not involve storing
   prompts; it only involves token computations. Therefore, reconstructing sensitive input from these ephemeral tensors
   is not feasible under Azure’s architecture.
   
   **Logging and Zero Data Retention**
   
   Azure OpenAI retains prompts and completions for up to 30 days for abuse monitoring. However, enterprises can request
   zero data retention mode through a formal approval process. When approved, prompts and completions are not stored at
   all, and abuse monitoring is disabled. This option transfers compliance responsibility to the customer.
   
   All processing occurs within your Azure region, and any optional persisted features (such as Assistants API threads
   or fine-tuning data) are encrypted in transit and at rest. Internal states like hidden layers and KV-cache are never
   exposed outside the secure inference environment, ensuring that sensitive data remains protected.
   
   **Key Takeaways**
   * Hidden activations and KV-cache are transient and never logged.
   * Prompt caching is temporary and numeric-only, not text.
   * Zero data retention requires explicit approval.
   * No mechanism exists for reconstructing sensitive input from ephemeral tensors.
   
   I Hope this helps. Do let me know if you have any further queries.
   
   Thank you!
   
   Was this answer helpful?
   
   Yes No
   0 comments No comments Report a concern
   [ Sign in to comment ][16]
   Add comment
   Comment Use comments to ask for clarification, additional information, or improvements to the question.
   Discard draft Add comment
[ Sign in to answer ][17]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-in][18]
[ Your Privacy Choices][19]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][20]
* [Previous Versions][21]
* [Blog][22]
* [Contribute][23]
* [Privacy][24]
* [Consumer Health Privacy][25]
* [Terms of Use][26]
* [Code of Conduct][27]
* [Trademarks][28]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-in%2Fanswers%2Fquestions%2F5616
660%2Fdoes-azure-openai-ever-retain-or-persist-hidden-ac 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-in%2Fanswers%2Fquestions%2
F5616660%2Fdoes-azure-openai-ever-retain-or-persist-hidden-ac 											&text=Does%20Azure%20OpenAI%20ever%20retain%20or%20persist
%20hidden%20activations%2C%20KV-cache%20data%2C%20or%20prompt%20cache%20contents%20after%20request%20processing%3F 											&tw_p
=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2Fen-in%2Fanswers%2Fquestions%2F5616660%2Fdoes-azure-openai-ever-reta
in-or-persist-hidden-ac
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-in%2Fanswers%2Fquestions%2F5616660%2F
does-azure-openai-ever-retain-or-persist-hidden-ac
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Does%20Azure%20OpenAI%20ever%20retain%20or%20persist%20hidden%20activati
ons%2C%20KV-cache%20data%2C%20or%20prompt%20cache%20contents%20after%20request%20processing%3F&body=Does Azure OpenAI ev
er retain or persist hidden activations, KV-cache data, or prompt cache contents after request processing?https%3A%2F%2F
learn.microsoft.com%2Fen-in%2Fanswers%2Fquestions%2F5616660%2Fdoes-azure-openai-ever-retain-or-persist-hidden-ac
[8]: /en-in/users/na/?userid=0ee402ed-830b-418d-a21a-6c4944af36a4
[9]: /en-in/answers/tags/387/azure-openai/
[10]: https://learn.microsoft.com/en-us/users/na/?userid=0ee402ed-830b-418d-a21a-6c4944af36a4
[11]: #
[12]: ?orderby=helpful&page=1#answers
[13]: ?orderby=newest&page=1#answers
[14]: ?orderby=oldest&page=1#answers
[15]: https://learn.microsoft.com/en-us/users/na/?userid=0ee402ed-830b-418d-a21a-6c4944af36a4
[16]: #
[17]: #
[18]: #
[19]: https://aka.ms/yourcaliforniaprivacychoices
[20]: https://learn.microsoft.com/en-in/principles-for-ai-generated-content
[21]: https://learn.microsoft.com/en-in/previous-versions/
[22]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[23]: https://learn.microsoft.com/en-in/contribute
[24]: https://go.microsoft.com/fwlink/?LinkId=521839
[25]: https://go.microsoft.com/fwlink/?linkid=2259814
[26]: https://learn.microsoft.com/en-in/legal/termsofuse
[27]: https://aka.ms/msftqacodeconduct
[28]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
