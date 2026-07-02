# Web source

- URL: https://learn.microsoft.com/en-gb/answers/questions/5535653/low-cache-hit-rate-for-large-fixed-system-prompt-i
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:41:28.926382837+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Low cache hit rate for large fixed System Prompt in Azure OpenAI (Foundry Models)

[ WayChou ][8] 0 Reputation points
2025-08-27T02:14:50.6066667+00:00

I am using Azure OpenAI in Foundry Models via the API, and I noticed that the cache hit rate for my System Prompt is une
xpectedly low.

• My System Prompt is very large (~100k+ tokens) and always identical for every request.

•
However, when I check usage.input_tokens_details.cached_tokens, the cache hit probability is very low — averaging only a
bout 3.77% (see attached chart).

[QQ20250827-100806]

• I expected the cache hit rate to be much higher since the System Prompt never changes.

Here is a simplified code snippet I’m using:

`import { AzureOpenAI } from 'openai';

const client = new AzureOpenAI({
  apiKey: process.env.AZURE_OPENAI_API_KEY,
  apiVersion: '2025-03-01-preview',
  endpoint: process.env.AZURE_OPENAI_ENDPOINT,
});

const stream = await this.client.responses.create({
  model: 'gpt-5',
  stream: true,
  max_output_tokens: 32768,
  input: [
    { "role": "system", "content": static_system_prompt },
    { "role": "user", "content": user_input },
  ],
});
`

My questions are:
1. Why is the cache hit rate for the System Prompt so low, even though it is fixed and identical across all requests?
2. Is there any recommended way to increase the cache hit rate for large System Prompts?

Thanks in advance for any guidance!

Azure OpenAI in Foundry Models
[ Azure OpenAI in Foundry Models ][9]

An Azure service that provides access to OpenAI’s GPT-3 models with enterprise capabilities.

Sign in to follow Follow
0 comments No comments Report a concern
I have the same question (0)
[ Sign in to comment ][10]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 1 answer

Sort by: Most helpful
[Most helpful][11] [Newest][12] [Oldest][13]
1. [ Jerald Felix ][14] • Follow 14,965 Reputation points • Volunteer Moderator
   2025-08-28T05:53:48.33+00:00
   
   Hello [WayChou][15],
   
   Greetings,
   
   Azure OpenAI’s prompt-caching is prefix-based and instance-local, so a large, static system prompt does not guarantee
   high cache hits on its own. Two common causes explain the 3.77% hit rate you’re seeing:
   
   `1.     Routing overflow
   
   •       
   The cache lives only on the specific model-server that first tokenized your prompt. If your traffic exceeds roughly 1
   5 identical requests/minute, Azure load-balances the overflow to other servers, each starting with an empty cache. Th
   ose extra requests become misses, lowering the average hit rate.
   
   2.      Prefix mismatch or user hash variation
   
   •       
   Only the first ~256 tokens of the prompt (model-dependent) form the cache key. Any difference—extra whitespace, newli
   nes, or a different user parameter—creates a new hash and a separate cache entry, again reducing hits.
   `
   
   How to raise the hit rate
   
   `•      
   Keep static content in the first 256 tokens Trim leading whitespace and ensure the opening block of your 100 k-token 
   system prompt is identical byte-for-byte across calls.
   
   •       
   Set a stable `user` field Use the same `user:` value (or omit it altogether) for requests that should share a cache e
   ntry. Different user IDs create parallel caches.
   
   •       
   Throttle burst traffic Aim for ≤15 identical requests/minute per deployment, or add a small jitter/delay so you repea
   tedly hit the same warmed-up instance.
   
   •       
   Reuse a single persistent connection If feasible, multiplex requests through one long-lived client that Azure may kee
   p “sticky” to the same backend host, boosting reuse of its local cache.
   
   •       
   Consider a warm-up job Send one priming request after each deployment restart; subsequent calls within the next 5–10 
   minutes will benefit while the entry remains resident.
   `
   
   Those adjustments typically push cache hits for an unchanged system prompt well above 50% in production tests, saving
   both cost and latency.
   
   Best regards,
   
   Jerald Felix
   
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
[en-gb][18]
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
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-gb%2Fanswers%2Fquestions%2F5535
653%2Flow-cache-hit-rate-for-large-fixed-system-prompt-i 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-gb%2Fanswers%2Fquestions%2
F5535653%2Flow-cache-hit-rate-for-large-fixed-system-prompt-i 											&text=Low%20cache%20hit%20rate%20for%20large%20fixed%20Sys
tem%20Prompt%20in%20Azure%20OpenAI%20(Foundry%20Models) 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2Fen-gb%
2Fanswers%2Fquestions%2F5535653%2Flow-cache-hit-rate-for-large-fixed-system-prompt-i
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-gb%2Fanswers%2Fquestions%2F5535653%2F
low-cache-hit-rate-for-large-fixed-system-prompt-i
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Low%20cache%20hit%20rate%20for%20large%20fixed%20System%20Prompt%20in%20
Azure%20OpenAI%20(Foundry%20Models)&body=Low cache hit rate for large fixed System Prompt in Azure OpenAI (Foundry Model
s)https%3A%2F%2Flearn.microsoft.com%2Fen-gb%2Fanswers%2Fquestions%2F5535653%2Flow-cache-hit-rate-for-large-fixed-system-
prompt-i
[8]: /en-gb/users/na/?userid=d82652c1-c5eb-4512-93d0-29ca16c5c9a9
[9]: /en-gb/answers/tags/387/azure-openai/
[10]: #
[11]: ?orderby=helpful&page=1#answers
[12]: ?orderby=newest&page=1#answers
[13]: ?orderby=oldest&page=1#answers
[14]: /en-gb/users/na/?userid=f5bf268b-7ffe-0006-0000-000000000000
[15]: https://learn.microsoft.com/en-us/users/na/?userid=d82652c1-c5eb-4512-93d0-29ca16c5c9a9
[16]: #
[17]: #
[18]: #
[19]: https://aka.ms/yourcaliforniaprivacychoices
[20]: https://learn.microsoft.com/en-gb/principles-for-ai-generated-content
[21]: https://learn.microsoft.com/en-gb/previous-versions/
[22]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[23]: https://learn.microsoft.com/en-gb/contribute
[24]: https://go.microsoft.com/fwlink/?LinkId=521839
[25]: https://go.microsoft.com/fwlink/?linkid=2259814
[26]: https://learn.microsoft.com/en-gb/legal/termsofuse
[27]: https://aka.ms/msftqacodeconduct
[28]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
