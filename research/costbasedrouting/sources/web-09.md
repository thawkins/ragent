# Web source

- URL: https://vladiliescu.net/prompt-caching-with-azure-openai
- Title: [Vlad Iliescu][1]
- Captured (UTC): 2026-06-29T15:41:28.373317687+00:00

```text
[Vlad Iliescu][1]
* [ Search ][2]
* [ Talks 🎙 ][3]
* [ Apps ][4]
* [ Wiki 📚 ][5]
* [ Archive ][6]
[Home][7] » [Posts][8]

# Prompt Caching with Azure OpenAI

January 12, 2025 · 9 min
Table of Contents
* [Why this is a big deal][9]
* [Some caveats][10]
* [Trust, but verify][11]
* [Conclusion][12]

Microsoft has recently introduced the ability to **cache Azure OpenAI prompts**.

## Why this is a big deal[#][13]

When working with LLMs, most scenarios consist of sending and resending a lot of static information such as system
prompts, tools, images, and user/assistant messages. Processing them over and over and over again ends up costing quite
a bit, in both **response latency** and **actual money** paid. To give you an example, you currently have to pay **~2.6
EUR** per each million input tokens you send, and when we’re talking large batch jobs or multi-turn conversations + rich
RAG scenarios that quickly fill up the context, it’s not so cheap.

**Prompt caching** helps mitigate this by avoiding the cost on processing the **input** prompts. Not the outputs, mind
you, those are regenerated every time. But the inputs, including detailed system prompts, examples, rich tools, etc,
they’re cached and end up A) costing 50% less for non-provisioned deployments and B) being just a bit faster, since the
system doesn’t have to process the prompts all over again every time.

You will need to pay some extra **attention** to how you structure your prompts. Static content should be placed **at
the beginning**, and while this happens by default for system prompts and tools, it doesn’t necessarily happen for
images, user & assistant messages, or RAG information. It’s **a conscious decision**, and your design needs to reflect
it.

## Some caveats[#][14]
* You need to use one of the newer models, specifically one of `gpt-4o-2024-11-20`, `gpt-4o-2024-08-06`,
  `gpt-4o-mini-2024-07-18`, `o1-2024-12-17`, `o1-preview-2024-09-12`, `o1-mini-2024-09-12`.
* You **currently** *(12 Jan 2025)* need to use API version [2024-10-01-preview][15]. It’s not a stable feature yet – so
  using a newer, **stable** API such as [2024-10-21][16] **won’t work just yet**. You can verify this by looking for the
  `cached_tokens` property in the [Azure REST API Specs repository][17]. Simply compare [2024-10-21 inference.json][18]
  to [2024-10-01-preview inference.json][19] and see what’s available.
* Your input prompt needs to be of 1024 tokens minimum, and the first 1024 tokens need to be identical. This includes
  the system prompt, tool definitions, all **user & assistant** messages you send, but also images and structured
  outputs. The tokens are cached as follows: the initial 1024 tokens first, then in 128 token increments, assuming they
  stay the same.
* The prompts stay cached for about 5-10 minutes of inactivity (they give the same number in both the [Azure docs][20]
  the and [OpenAI docs][21]), but **might** stay cached for up to one hour, depending on their load.
* You won’t be able to check whether prompt caching is working unless you use one of the o1 models 😕. They’re the only
  ones returning the response [usage.prompt_tokens_details.cached_tokens][22] fields we need to know whether the input
  prompt was cached, and by how much. The GPT-4o models **will cache prompts**, but you’ll only be able to tell by
  checking the bill and the latency.
* You can’t disable prompt caching (but you can use API versions that don’t support it 😉)

## Trust, but verify[#][23]

I’ve ran some tests to check how this actually works in practice and it looks like, just as with llm responses, the way
prompt caching works in Azure OpenAI is not necessarily deterministic.

My test was simple (check out the code at the end of the post). I began by giving the system a GPT-generated description
of how to craft the bestest haiku, then requested one. Next, I appended the LLM’s reply to the conversation, asked for
another haiku, and repeated these steps several times, reporting the number of prompt tokens cached, speed, etc.

Here are the results I’ve got using a `o1-mini` deployment in `swedencentral`:

`1.
Elapsed time: 5.69 seconds
Prompt Tokens: 948
Response Tokens: 154 (Completion Tokens - Reasoning Tokens: 922 - 768)
Cached Tokens: 0
=====================
2.
Elapsed time: 2.36 seconds
Prompt Tokens: 1120
Response Tokens: 163 (Completion Tokens - Reasoning Tokens: 355 - 192)
Cached Tokens: 0
=====================
3.
Elapsed time: 2.15 seconds
Prompt Tokens: 1303
Response Tokens: 155 (Completion Tokens - Reasoning Tokens: 283 - 128)
Cached Tokens: 0
=====================
4.
Elapsed time: 3.17 seconds
Prompt Tokens: 1479
Response Tokens: 154 (Completion Tokens - Reasoning Tokens: 346 - 192)
Cached Tokens: 1152
=====================
5.
Elapsed time: 1.74 seconds
Prompt Tokens: 1654
Response Tokens: 140 (Completion Tokens - Reasoning Tokens: 204 - 64)
Cached Tokens: 0
=====================
6.
Elapsed time: 2.58 seconds
Prompt Tokens: 1815
Response Tokens: 151 (Completion Tokens - Reasoning Tokens: 407 - 256)
Cached Tokens: 1536
=====================
7.
Elapsed time: 2.55 seconds
Prompt Tokens: 1986
Response Tokens: 167 (Completion Tokens - Reasoning Tokens: 423 - 256)
Cached Tokens: 1664
=====================
8.
Elapsed time: 5.32 seconds
Prompt Tokens: 2172
Response Tokens: 0 (Completion Tokens - Reasoning Tokens: 1024 - 1024)
Cached Tokens: 1280
=====================
9.
Elapsed time: 1.86 seconds
Prompt Tokens: 2196
Response Tokens: 148 (Completion Tokens - Reasoning Tokens: 276 - 128)
Cached Tokens: 2048
=====================
10.
Elapsed time: 2.75 seconds
Prompt Tokens: 2364
Response Tokens: 161 (Completion Tokens - Reasoning Tokens: 417 - 256)
Cached Tokens: 2048
=====================
`

Some notes:
* I **haven’t noticed any big increase** in speed once prompt caching started to kick in, the generation time seems to
  revolve around 2.5-3.5 seconds, with some outliers. This is **to be expected**, since for smaller prompts such as
  mine, the prompt processing should be done quickly anyway.
* The **prompt caching doesn’t always work**? That’s what the numbers tell me anyway – just look at steps 3 and 5, both
  have **0 cached tokens** even though the preceding steps result in at least **1024 cacheable prompt tokens**. The
  entire Jupyter cell only took about **30 seconds** to execute all requests, and the docs mentioned **5-10 minutes of
  inactivity**, so I’m wondering what’s up with these results. I’m guessing that’s one of the reasons for this feature
  being available only in the preview API.
* One reason for disliking the o1 models for anything that’s not chat is that they can eat up all available context for
  reasoning, and consequently provide no response. Just take a look at **step 8**. You really need to pay attention not
  to set the `max_completion_tokens` too low, because it includes o1’s elusive `reasoning_tokens` as well, which you
  can’t really access.

Here’s my code if you want to try it out yourself:

`import os
import time
from openai import AzureOpenAI
from dotenv import load_dotenv

load_dotenv()

o1_model_name = os.environ.get("AZURE_OPENAI_O1_MODEL") # o1-mini

openai_client = AzureOpenAI(
    api_key=os.environ.get("AZURE_OPENAI_API_KEY"),
    api_version=os.environ.get("AZURE_OPENAI_API_VERSION"),
    azure_endpoint=os.environ.get("AZURE_OPENAI_ENDPOINT")
)

system_message_content = """\
A beginner-friendly guide to crafting a quintessential haiku:

Understanding the Structure
1. Traditional Format:

A haiku traditionally consists of three lines with a 5-7-5 syllable pattern.
Line 1: Five syllables
Line 2: Seven syllables
Line 3: Five syllables
However, modern haikus sometimes deviate from this strict structure to capture the essence more effectively, so focus on
 succinctness and spontaneity.

Choosing the Subject
2. Themes and Inspiration:

Haikus often focus on nature, seasons, or a specific moment.
Explore themes like change (e.g., seasons shifting), contrast (e.g., something vibrant against a dull background), or em
otions invoked by natural settings.
Spend time observing your surroundings – a garden, a park, a stream.
3. Finding a 'Kigo':

A 'kigo' is a seasonal word used in traditional Japanese haikus.
Examples include “cherry blossoms” for spring or “snow” for winter.
These words anchor your poem in a specific time and evoke sensory associations.
Crafting Your Haiku
4. Capture a Moment:

Focus on a brief moment of beauty or insight.
Use concrete imagery to paint a picture in the reader's mind.
For example, instead of saying “a beautiful sunset,” describe it: "Crimson sky darkens."
5. Emotion and Mood:

Subtly infuse emotion by capturing how the scene influences you.
A haiku should not explicitly state an emotion but rather evoke it through the imagery.
6. First Draft:

Jot down your observations and feelings spontaneously.
Don’t worry about the syllable count initially; focus on getting your feelings on paper.
Refining the Haiku
7. Edit for Conciseness:

Pare down your draft to its essence while keeping the imagery rich.
Remove unnecessary adjectives or adverbs. Use nouns and verbs to convey the image.
8. Syllable Adjustment:

Now, refine your text to fit the syllable pattern of 5-7-5, if adhering to tradition.
Count syllables carefully, but remember that capturing the moment weightily is more crucial than strict adherence to for
m.
9. Evocative Language:

Employ powerful, evocative words that convey sensory details.
Avoid cliches or overly complex language that can distract from the purity of the image.
Final Touches
10. Read Aloud:

Read your haiku aloud to feel its rhythm and impact.
The cadence should reflect the haiku’s mood and should feel natural.
11. Seek Feedback:

Share your haiku with others to gain different perspectives.
Consider group discussions or haiku workshops for diverse opinions.
12. Embrace Imperfection:

Accept that perfection is subjective and that each haiku may resonate differently with different people.
A haiku’s simplicity is its strength; focus on the feelings and images it evokes.

Example Haiku Analysis #1
Haiku:
Golden leaves flutter
Whispering secrets to ground
Autumn's soft farewell

Imagery: The first line paints a vivid picture of movement ("Golden leaves flutter").
Emotion: The whisper metaphor and reference to a farewell evoke a sense of transience and nostalgia.
Kigo: The mention of "autumn" firmly places this moment in a specific season.

Example Haiku Analysis #2
Haiku:
Gentle spring breeze plays
Cherry blossoms swirl and dance
Petals kiss the ground

Imagery: The haiku paints a vivid picture of cherry blossom petals being carried by a soft spring breeze, creating a dyn
amic scene of movement and beauty.
Emotion: It evokes a sense of joy mixed with a touch of melancholy, as the falling petals symbolize both the peak of bea
uty and the transient nature of life.
Kigo: "Cherry blossoms" are a classic seasonal reference for spring in Japanese poetry, grounding the haiku in a specifi
c time of year.

Creating a haiku can be a reflective and rewarding process that sharpens both your observation and poetic skills. As you
 practice, you'll develop an intuitive feel for the balance between form and content, allowing your haikus to blossom wi
th authenticity and grace.

Go ahead write the perfect haiku, and add a short interpretation (1-2 paragraphs) for it
"""
o1_system_message = {"role": "user", "content": system_message_content}
messages=[o1_system_message]

for i in range(0, 10):
    start = time.perf_counter()
    o1_result = openai_client.chat.completions.create(
        model=o1_model_name,
        temperature=1,
        # we don't want the reasoning to take up a lot of time, so limit it at 1024 tokens (512 is way to small)
        max_completion_tokens=1024,
        messages=messages,
        stream=False,
    )
    end = time.perf_counter()
    response = o1_result.choices[0].message.content

    # simulate a conversation by continually appending the messages to our list
    messages += [{"role": "assistant", "content": response}, {"role": "user", "content": "ANOTHER!"}]

    completion_tokens = o1_result.usage.completion_tokens
    reasoning_tokens = o1_result.usage.completion_tokens_details.reasoning_tokens
    response_tokens = o1_result.usage.completion_tokens - o1_result.usage.completion_tokens_details.reasoning_tokens

    print(f"{i+1}.")
    # print(f"Response: {response}")
    print(f"Elapsed time: {end - start:.2f} seconds")
    print(f"Prompt Tokens: {o1_result.usage.prompt_tokens}")
    
print(f"Response Tokens: {response_tokens} (Completion Tokens - Reasoning Tokens: {completion_tokens} - {reasoning_token
s})")
    print(f"Cached Tokens: {o1_result.usage.prompt_tokens_details.cached_tokens}")
    print("="*21)
`

## Conclusion[#][24]

Prompt caching can reduce cost and offer slight speedups when used properly. Especially with GPT-4o models 😉. However,
it remains somewhat unpredictable, so you may want to wait just a bit until it hits GA status.
* [Lessons-Learned][25]
* [Llms][26]
* [Openai][27]
* [Azure-Openai][28]

[ « Prev
Building a simple agent with smolagents and Azure OpenAI ][29] [ Next »
Clipit (previously Grabit), the Web Page Downloader ][30] © 2026 Vlad Iliescu

[1]: https://vladiliescu.net/
[2]: https://vladiliescu.net/search/
[3]: https://vladiliescu.net/talks/
[4]: https://vladiliescu.net/price-snoop/
[5]: https://vladiliescu.net/wiki/
[6]: https://vladiliescu.net/archives/
[7]: https://vladiliescu.net/
[8]: https://vladiliescu.net/posts/
[9]: #why-this-is-a-big-deal
[10]: #some-caveats
[11]: #trust-but-verify
[12]: #conclusion
[13]: #why-this-is-a-big-deal
[14]: #some-caveats
[15]: https://github.com/Azure/azure-rest-api-specs/tree/main/specification/cognitiveservices/data-plane/AzureOpenAI/inf
erence/preview/2024-10-01-preview
[16]: https://github.com/Azure/azure-rest-api-specs/tree/main/specification/cognitiveservices/data-plane/AzureOpenAI/inf
erence/stable/2024-10-21
[17]: https://github.com/Azure/azure-rest-api-specs
[18]: https://github.com/Azure/azure-rest-api-specs/blob/main/specification/cognitiveservices/data-plane/AzureOpenAI/inf
erence/stable/2024-10-21/inference.json
[19]: https://github.com/Azure/azure-rest-api-specs/blob/main/specification/cognitiveservices/data-plane/AzureOpenAI/inf
erence/preview/2024-10-01-preview/inference.json
[20]: https://learn.microsoft.com/en-us/azure/ai-services/openai/how-to/prompt-caching
[21]: https://platform.openai.com/docs/guides/prompt-caching
[22]: https://learn.microsoft.com/en-us/azure/ai-services/openai/reference-preview#cached_tokens
[23]: #trust-but-verify
[24]: #conclusion
[25]: https://vladiliescu.net/tags/lessons-learned/
[26]: https://vladiliescu.net/tags/llms/
[27]: https://vladiliescu.net/tags/openai/
[28]: https://vladiliescu.net/tags/azure-openai/
[29]: https://vladiliescu.net/smolagents-with-azure-openai/
[30]: https://vladiliescu.net/clipit-web-downloader/
```
