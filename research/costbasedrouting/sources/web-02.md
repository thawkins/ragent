# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/prompt-caching
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:41:15.338369267+00:00

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

# Prompt caching

Feedback
Summarize this article for me

## In this article

Prompt caching reduces overall request latency and cost for longer prompts that have identical content at the beginning
of the prompt. In this context, *"prompt"* refers to the input you send to the model as part of your chat completions or
response creation requests. Rather than reprocessing the same input tokens over and over again, the service retains a
temporary cache of processed input token computations to improve overall performance. Prompt caching has no impact on
the output content returned in the model response beyond a reduction in latency and cost.

For supported models, cached tokens are billed at a [discount on input token pricing][8] for Standard deployment types
and up to [100% discount on input tokens][9] for Provisioned deployment types. Prompt cache pricing is the same for both
retention policies.

## Prompt cache retention

Prompt caching can use either in-memory or extended retention policies. When available, extended prompt caching aims to
retain the cache for longer, so that subsequent requests are more likely to match the cache.

To configure the prompt cache retention policy, set the `prompt_cache_retention` parameter on the Responses or Chat
Completions API.

### In-memory prompt cache retention

The system typically clears caches within 5 to 10 minutes of inactivity and always removes them within one hour of the
cache's last use. The system doesn't share prompt caches between Azure subscriptions.

All Azure OpenAI models GPT-4o or newer support in-memory prompt cache retention. It applies to models that have
chat-completion, completion, responses, or real-time operations. For models that don't have these operations, this
feature isn't available.

### Extended prompt cache retention

Extended prompt cache retention keeps cached prefixes active for longer, up to a maximum of 24 hours. Extended prompt
caching works by offloading the key/value tensors to GPU-local storage when memory is full, which significantly
increases the storage capacity available for caching.

Extended prompt cache retention is available for the following models:
* `gpt-5.4`
* `gpt-5.3-codex`
* `gpt-5.2`
* `gp5-5.1-codex-max`
* `gpt-5.1`
* `gpt-5.1-codex`
* `gpt-5.1-codex-mini`
* `gpt-5.1-chat`
* `gpt-5`
* `gpt-5-codex`
* `gpt-4.1`

### Configure per request

For `gpt-5.4` and older models, if you don't specify a retention policy, the default is `in_memory`. Allowed values are
`in_memory` and `24h`. For all newer models, the default is `24h` and `in_memory` isn't supported.

`{
  "model": "gpt-5.4",
  "input": "Your prompt goes here...",
  "prompt_cache_retention": "24h"
}
`

## Getting started

To take advantage of prompt caching, a request must meet both of these requirements:
* A minimum of 1,024 tokens in length.
* The first 1,024 tokens in the prompt must be identical.

Requests are routed based on a hash of the initial prefix of a prompt. The hash typically uses the first 256 tokens,
though the exact length varies depending on the model.

When a match is found between the token computations in a prompt and the current content of the prompt cache, it's
referred to as a cache hit. Cache hits show up as [`cached_tokens`][10] under [`prompt_tokens_details`][11] in the chat
completions response.

`{
  "created": 1729227448,
  "model": "o1-2024-12-17",
  "object": "chat.completion",
  "service_tier": null,
  "system_fingerprint": "fp_50cdd5dc04",
  "usage": {
    "completion_tokens": 1518,
    "prompt_tokens": 1566,
    "total_tokens": 3084,
    "completion_tokens_details": {
      "audio_tokens": null,
      "reasoning_tokens": 576
    },
    "prompt_tokens_details": {
      "audio_tokens": null,
      "cached_tokens": 1408
    }
  }
}
`

After the first 1,024 tokens, cache hits occur for every 128 additional identical tokens.

A single character difference in the first 1,024 tokens results in a cache miss, which is characterized by a
`cached_tokens` value of 0. Prompt caching is enabled by default with no additional configuration needed for supported
models.

If you provide the `prompt_cache_key` parameter, it's combined with the prefix hash, so you can influence routing and
improve cache hit rates. This benefit is especially beneficial when many requests share long, common prefixes. If
requests for the same prefix and `prompt_cache_key` combination exceed a certain rate (approximately 15 requests per
minute), some requests overflow and get routed to extra machines, reducing cache effectiveness.

## Frequently asked questions

### What is cached?

Feature support for o1-series models varies by model. For more information, see the dedicated [reasoning models
guide][12].

Prompt caching supports:

─────────────────┬──────────────────────────────────────────────────────────────────────────────────────────────────────
**Caching        │**Description**                                                                                       
supported**      │                                                                                                      
─────────────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────
**Messages**     │The complete messages array: system, developer, user, and assistant content                           
─────────────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────
**Images**       │Images included in user messages, both as links or as base64-encoded data. The detail parameter must  
                 │be set the same across requests.                                                                      
─────────────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────
**Tool use**     │Both the messages array and tool definitions.                                                         
─────────────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────
**Structured     │Structured output schema is appended as a prefix to the system message.                               
outputs**        │                                                                                                      
─────────────────┴──────────────────────────────────────────────────────────────────────────────────────────────────────

To improve the likelihood of cache hits, structure your requests so that repetitive content occurs at the beginning of
the messages array.

### Can I disable prompt caching?

Prompt caching is enabled by default for all supported models. There's no opt-out support for prompt caching.

### Does prompt caching work with data residency?

In-memory prompt caching is compatible with all data residency regions. Extended prompt caching temporarily stores data
on GPU machines and keeps data in-region only when using Regional Standard or Regional Provisioned deployment types.

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-05-13

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][13]
[ Your Privacy Choices][14]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][15]
* [Previous Versions][16]
* [Blog][17]
* [Contribute][18]
* [Privacy][19]
* [Consumer Health Privacy][20]
* [Terms of Use][21]
* [Trademarks][22]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/how-to/prompt-caching.md
[7]: #
[8]: https://azure.microsoft.com/pricing/details/cognitive-services/openai-service/
[9]: /en-us/azure/ai-foundry/openai/concepts/provisioned-throughput
[10]: /en-us/rest/api/microsoft-foundry/azureopenai/chat?view=rest-microsoft-foundry-2025-04-01-preview&preserve-view=tr
ue
[11]: /en-us/rest/api/microsoft-foundry/azureopenai/chat?view=rest-microsoft-foundry-2025-04-01-preview&preserve-view=tr
ue
[12]: reasoning
[13]: #
[14]: https://aka.ms/yourcaliforniaprivacychoices
[15]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[16]: https://learn.microsoft.com/en-us/previous-versions/
[17]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[18]: https://learn.microsoft.com/en-us/contribute
[19]: https://go.microsoft.com/fwlink/?LinkId=521839
[20]: https://go.microsoft.com/fwlink/?linkid=2259814
[21]: https://learn.microsoft.com/en-us/legal/termsofuse
[22]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
