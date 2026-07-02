# Web source

- URL: https://www.linkedin.com/pulse/large-concept-models-thinking-beyond-tokens-amita-kapoor-ymq9c
- Title: Agree & Join LinkedIn
- Captured (UTC): 2026-06-29T16:30:53.298009985+00:00

```text
Agree & Join LinkedIn

By clicking Continue to join or sign in, you agree to LinkedIn’s [User Agreement][1], [Privacy Policy][2], and [Cookie
Policy][3].

`` `` `` `` `` `` ``

## Sign in to view more content

Create your free account or sign in to continue your search

`` `` `` `` `` `` `` `` `` ``
Email or phone
Password
Show
[Forgot password?][4] Sign in
Sign in with Email

or

New to LinkedIn? [Join now][5]

By clicking Continue to join or sign in, you agree to LinkedIn’s [User Agreement][6], [Privacy Policy][7], and [Cookie
Policy][8].

`` `` `` `` `` `` `` [ Skip to main content ][9] [ LinkedIn ][10]
* [ Top Content ][11]
* [ People ][12]
* [ Learning ][13]
* [ Jobs ][14]
* [ Games ][15]
[ Join now ][16] [ Sign in ][17]
`` `` `` ``
[Large Concept Models: Thinking Beyond Tokens]

# Large Concept Models: Thinking Beyond Tokens
* [ Report this article ][18]

[ Amita Kapoor ][19]

### Amita Kapoor

Published Jan 26, 2025
[ + Follow ][20]

Hello, Gen AI Simplified readers! If you have ever wondered why current AI language models sometimes feel a bit linear
or stuck on the surface of words, today’s topic may spark your imagination. We are diving into a new approach from Meta
called Large Concept Models (LCMs), which step away from individual tokens and focus on broader, more meaningful chunks
called concepts.

Take a moment and imagine you are reading a research paper—perhaps it’s about astrophysics or a complicated legal text -
Do you typically pay attention to each word as you go, or do you jump around, scanning sentences and paragraphs for the
“big ideas”?

If you are like most people, you probably do the latter. It’s not that individual words are unimportant; it’s that we
can’t function if we remain stuck at that granular level. We chunk ideas into higher-level notions (concepts) and
reference them in our minds.

That, in a nutshell, is the big idea behind Large Concept Models. Instead of focusing on tokens—the individual pieces of
text—LCMs jump up a level to sentences. They aim to mirror our own ability to handle large volumes of information
conceptually, without getting lost in the weeds of token-level detail.

> If LCMs think more like humans, do they perform better at certain tasks than traditional Large Language Models? Let’s
> find out.

## Motivation: Why Concepts Instead of Tokens?

If you look at how humans reason, you will notice that we think in hierarchies of meaning—often referred to as
conceptual thinking. In daily life, we do not usually parse the world in single-word increments. Why should AI do so?

1. Efficiency of Information Processing: When you read a lengthy document, you do not commit every single word to
   memory. Instead, you group words together, identify the essential ideas, and then recall which section of the text
   pertains to a particular concept. This method of “chunking” helps you absorb information faster and recall it more
   efficiently.
2. Planning and Composition: Think about the last time you wrote a research paper, gave a presentation, or even wrote a
   short story. You probably started with a rough structure—an outline of key ideas or chapter headings. Only after
   fleshing out the conceptual framework did you fill in the details with words and sentences. This top-down composition
   process allows for high-level planning before diving into specifics. We think in hierarchies: main idea → supporting
   points → details.
3. Language and Modality Agnosticism: Whether you say “Hello” in English, “Bonjour” in French, or use sign language, the
   underlying concept is the same: a greeting. Our thoughts aren’t tethered to the exact tokens used. We can switch
   mediums (or modalities- text, speech, images etc) or languages without losing the meaning.
4. Abstraction and Generalization: Each piece of knowledge in our minds is connected to others at an abstract level.
   That’s why we can read about an experiment in the “Methods” section of a paper, and remember to find the results for
   that experiment in the “Results” section. Concepts hold these dependencies in a web of meaning.
5. Explicit Reasoning and Planning: Lastly, the reasoning chain in our minds is not purely linear; it weaves through
   multiple levels of abstraction. We might manipulate ideas in a hierarchical, top-down manner, or pivot mid-way to
   reevaluate at a lower level of detail. This capacity for fluid multi-level thinking pushes us to explore new AI
   architectures that embody this flexibility—hence the appeal of LCMs.

## LCMs: A High-Level Overview

Large Concept Models differ from Large Language Models (LLMs) primarily in the unit of analysis. While LLMs predict the
next token (words) in a sequence, LCMs predict the next concept. Importantly, these concepts are meant to be language-
and modality-agnostic, capturing the abstract core of what is being communicated or reasoned about.

In Meta’s work, each sentence in a text is treated as a single concept. The input text is segmented into sentences, each
of which becomes a fixed-size embedding via a robust encoder-decoder framework called SONAR. The LCM thus processes a
sequence of these sentence embeddings, and then produces new sentence embeddings that are finally decoded back into text
(or speech).

> SONAR is an encoder-decoder architecture that transforms any given sentence into a fixed-size embedding. No matter if
> the sentence has 6 words or 20 words, SONAR compresses it into a single vector.

Why choose sentences? Sentences are a sufficiently large chunk of meaning, so they capture more context than an
individual token. At the same time, they are typically the smallest complete unit of communication that makes sense in
everyday use. Of course, the authors also hint that future work may move beyond sentences toward more flexible or larger
conceptual groupings like paragraph summaries.

At a high level, an LCM is typically a Transformer-based model that operates on sequences of sentence embeddings rather
than word tokens. A straightforward version of LCM is basically a decoder-only Transformer with two additional modules:

1. PreNet: This layer normalizes and projects the SONAR embeddings into the hidden dimension of the Transformer.
2. PostNet: After the Transformer processes these embeddings, PostNet maps them back into SONAR’s embedding dimension.

This model predicts the next concept (the next sentence embedding) by minimizing MSE loss between the predicted
embedding and the actual embedding of the next sentence in the training data. It is simple but effective, showing that
LCMs can indeed learn to chain sentences together.

In continuous spaces, a popular trend is to use diffusion models, which gradually transform a noise vector into a data
vector through a learned iterative process. Meta’s research explored applying diffusion to sentence embeddings. This is
known as the Diffusion-LCM and comes in two variants:

1. One-Tower Diffusion LCM: A single Transformer backbone predicts a “clean” version of the next sentence embedding from
   a noisy version.
2. Two-Tower Diffusion LCM: Uses two Transformers—one for contextualizing the existing sequence and another for
   “denoising” the next sentence embedding.

Diffusion-based LCMs can yield smoother and often more accurate embeddings. Early results indicate they may outperform
the simple MSE-based approach in predicting the next sentence embedding.

## Core Benefits of LCMs

By now, you might be wondering: “Okay, so LCMs are new. But what do they actually do better than token-based models?”
Let’s break it down.

1. Abstract Reasoning: LCMs are built to capture the underlying “reasoning” that happens between sentences, rather than
   focusing on the micro-level details of how each token is spelled out. Ideally, this fosters deeper logical or topical
   coherence over longer texts.
2. Multilingual & Multimodal: Since SONAR has already been trained for 200 languages and for both speech and text, LCMs
   inherit these capabilities automatically. No additional fine-tuning is necessary to switch between languages or from
   text to speech.
3. Hierarchical Structure: Outputs from LCMs are more chunked. Each sentence can act like a lego piece. That might make
   it easier for humans to read, edit, or reorder.
4. Long Context Handling: Because each sentence is a single unit, the model’s “context window” can cover a huge text
   while dealing with fewer total units. Traditional LLMs might struggle with context windows of tens of thousands of
   tokens.

## Performance and Benchmarks

Researchers have tested LCMs on tasks such as summarization, summary expansion, and next-sentence prediction.

1. Summarization: A 7B-parameter LCM was instruction-fine-tuned and then compared to other public models like Gemma,
   Mistral, and Llama (also around 7B-8B parameters).
2. Long-Context Summarization: LCMs outperformed or matched other large models such as Mistral in tasks that required
   summarizing very long documents. Handling fewer “concepts” than a sea of tokens helps preserve coherence across big
   contexts.
3. Summary Expansion: LCMs tended to produce more controlled lengths, whereas LLMs sometimes went off on lengthy
   tangents. However, some scoring metrics favored LLMs because they measure factors like word overlap that align more
   with the token-level approach.
4. Zero-Shot Language Transfer: LCMs displayed robust performance across multiple languages, sometimes surpassing
   standard LLMs. This underscores the strength of conceptual embeddings for bridging language differences.

## Limitations and Challenges

No model is perfect, and LCMs still face several hurdles:

1. Defining Concepts: Right now, concepts are defined at the sentence level, which is still fairly large. This makes it
   difficult for the model to correctly guess the “next sentence” due to a huge space of possibilities.
2. Embedding Fragility: SONAR embeddings can be very sensitive. Slight shifts in the embedding space might lead to
   significantly different outputs, especially for specialized or precise text.
3. Fluency and Evaluation Metrics: Many existing automated evaluation metrics (e.g., certain BERT-based or model-based
   coherence measures) are inadvertently tailored to token-level text. LCMs sometimes score lower, even though their
   actual text might be conceptually coherent.
4. Data Sparsity: The sentence-level approach means that each sample is a single concept-sentence pair. Also, the
   training corpora (bitext for machine translation) can differ from the typical large-scale text corpora used by
   standard LLMs.

## Recommended by LinkedIn

[
Our Favorite LLMs of 2025 (and Why Feel Still Matters)
David Norris 5 months ago
][21]
[
Coconut: The Next Leap in AI Reasoning
Brylan Donaldson 1 year ago
][22]
[
When AI Learns to Think: The Game-Changing Leap of LINC
Neurog 2 years ago
][23]

## Going Beyond Sentences: Concepts at Other Scales

Interestingly, while using a sentence as a concept is convenient, it is not the only possibility:

* Paragraph Summaries: A concept could be a short summary of a paragraph, capturing a bigger idea with multiple
  sentences. This approach might reduce the difficulty of predicting the next concept because each concept is at a
  slightly higher level of abstraction.
* Sub-Sentence Chunks: For texts that require finer-grained analysis, shorter conceptual chunks might be beneficial,
  though that challenges the idea of a universal representation.
* Hierarchical Approaches: One can imagine an architecture where a top-level model deals with large conceptual blocks
  (e.g., paragraph or section summaries), and a lower-level model deals with sentence-level concepts.

Research on the Large Planning Concept Model (LPCM) is already hinting at an approach where the model predicts a
conceptual plan before generating the next set of sentences, echoing how humans outline a text before writing.

## My Proposal: “Association Activation for Machines”

Now, let me introduce my personal idea—a concept I call “Association Activation for Machines”

Have you ever noticed how the word “apple” can trigger an entire chain of related thoughts—like “fruit,” “orchard,”
“pie,” and “Newton” (gravity)? In our human minds, mentioning one concept often activates a web of interconnected ideas.
That’s the inspiration behind Association Activation: I want to bring this spontaneous chain-linking of concepts into
Large Concept Models (LCMs).

My vision is that, rather than viewing each new sentence as an isolated idea, the model would maintain a graph of all
previously activated concepts.

* When a new concept appears, the LCM would activate any closely related nodes in that graph—just like our own mental
  process of recalling “gravity” upon thinking about “apple.”
* This dynamic linking keeps the flow of ideas context-rich and responsive to newly introduced material.

I also propose exploring energy-based frameworks—for example, Hopfield Networks or another associative memory mechanism.
Why? Because these systems excel at preserving or pruning the associations that matter most.

By weaving these cross-connections into the generation process, the LCM would do more than just spit out the next
sentence concept. It would harness the “cloud” of associations around that concept.

* The result: more dynamic, context-rich text that feels truly human-like in how it jumps between ideas.
* Rather than a simple linear chain—concept → concept → concept—my approach would let the system explore a small network
  of related concepts each time.

I believe this Association Activation layer could provide an extra dimension of human-like cognition for LCMs. It might
unlock greater creativity, better recall of details mentioned earlier, and a much stronger sense of narrative and
thematic continuity. Ultimately, my goal is to make AI outputs not just accurate, but also enriched with the same
spontaneous connections that make human thought so dynamic.

## Other Models Taking a Similar Route

LCMs aren’t the sole approach exploring bigger-than-token perspectives. Let’s do a quick “name-drop” of a few models or
ideas in the same vein:

* Jepa (Joint Embedding Predictive Architecture): Predicts representations of the next observation in an embedding
  space, used for images and video. Similar concept to LCM, but with a broader scope and less direct emphasis on text.
* INSET (Sentence Infilling): Uses a denoising autoencoder for entire sentences rather than tokens.
* Diffusion in Language: Projects like PLANNER, TEncDM, or discrete diffusion methods all attempt to adapt diffusion (so
  popular in image generation) for text generation at various chunk sizes.
* SentenceVAE: A Sentence-level Variational Autoencoder for language modeling.

In short, LCM is among the first to fully commit to a reconstructable, fully generative architecture entirely in a
sentence-level representation space.

> The future of AI might be about bridging token-level and concept-level approaches into a single pipeline.

## Concluding Thoughts & Future Directions

So, is the era of token-based models coming to an end? Maybe not—tokens are still integral to how we physically encode
text. But LCMs demonstrate a compelling alternative: they unify languages, handle long contexts more intuitively, and
focus on next-idea generation.

Looking ahead, it is clear that LCMs are not limited to short tasks alone. They are well-suited to long-form discourse,
where managing a coherent narrative is paramount. Association Activation could further enhance this by enabling truly
creative and context-rich concept generation, encouraging the model to recall or explore ideas in a more flexible,
human-like manner.

However, as these models evolve, evaluation will also need to evolve. Traditional token- or word-level metrics may not
capture the value of novel connections or conceptual depth. New metrics that reward conceptual innovation and cohesion
could become standard. In tandem, hierarchical or graph-based expansions of LCMs are likely on the horizon. These might
combine plan-based modeling at a larger scale with sentence-level or paragraph-level concept modeling, further bridging
abstract, high-level structures with the detailed flow of ideas.

### Stay Curious!

If you found this newsletter enlightening, don’t keep it to yourself—share the concepts and spread the word! Subscribe
to stay in the loop on all things GenAI, where we explore the latest ideas shaping tomorrow’s AI breakthroughs.

Now go forth, spark your own associations, and help us build the future of conceptual thinking—one concept (not token)
at a time!



[ Gen AI Simplified ][24]

### Gen AI Simplified

#### 2,903 followers

[ + Subscribe ][25]
`` `` `` `` ``
``
[
Like
][26]
[ Comment ][27]
`` ``
* Copy
* LinkedIn
* Facebook
* X
Share
`` ``
[ 19 ][28] `` `` `` `` `` `` `` [ 11 Comments ][29]
[ Rizwan Qureshi ][30] 11mo
* [ Report this comment ][31]

Please have a read at our new paper. [https://arxiv.org/abs/2507.00951][32]

[
Like
][33] [
Reply
][34] [ 1 Reaction ][35] 2 Reactions
[ Manjari S. ][36] 1y
* [ Report this comment ][37]

Very informative Ma'am

[
Like
][38] [
Reply
][39] [ 1 Reaction ][40] 2 Reactions
[ Ramesh Chandra Panda ][41] 1y
* [ Report this comment ][42]

Wishing you a happy day You are invited as Guest Speaker at National Workshop on Indian Innovation and Problem
Statements at Bakliwal Foundation College of Arts , Commerce and Science on coming 11th July 2025, Guest Speaker, Indian
Innovation and Problem Statements on coming 12th July 2025 at Reena Mehta College of Arts , Science, Commerce and
Management Studies and Guest of Honour at International Intellectual Property Right Conference 2025 at Hotel Taj on
coming 13th July 2025 Kindly find the photographs and videos of International Innovation Conclave 2024
[https://drive.google.com/drive/folders/1mYgBESiB4XMNCEfh-4qPcE4YKpciKPUR?usp=sharing][43] Love to invite you as guest
of honour for International Intellectual Property Right Conference 2025 on coming 13th July 2025. Venue - Hotel Taj
Palace,Mumbai
[https://www.linkedin.com/posts/dr-ramesh-chandra-highest-research-patent-holder-in-world-16485919b_dear-all-greetings-w
e-are-going-to-activity-7287916114028769280-WetY?utm_source=share&utm_medium=member_android][44]

[
Like
][45] [
Reply
][46] [ 1 Reaction ][47] 2 Reactions
[ John DuCrest ][48] 1y
* [ Report this comment ][49]

Amitā, thank you for sharing your insights on Large Concept Models. This made me think on how LCMs might benefit from
modeling the best aspects of human cognition. Not as emotions, but as behaviors like empathy, creativity, and
adaptability. These traits seem to align naturally with LCMs' ability to process concepts rather than tokens, offering a
way to make their reasoning more dynamic and intuitive. For instance, empathy-inspired modules could help LCMs interpret
emotional context, while creativity could emerge from associative thinking to link diverse concepts and generate fresh
ideas. Adaptability, taking cues from ADHD, could let LCMs shift focus dynamically while maintaining coherence, and
precision, inspired by autism, could ensure accuracy and reliability. Practically, modular and hierarchical
architectures could integrate trait-specific modules (like a graph-based memory for creativity or an emotional context
layer) into LCM frameworks. Ethical filters and human feedback loops could refine their development and avoid risks.
This focus on simulation rather than emotion seems a possible way to enhance adaptability and contextual understanding.
Do you think this approach could work? Thank you for the amazing read!

[
Like
][50] [
Reply
][51] [ 1 Reaction ][52] 2 Reactions
[ See more comments ][53]

To view or add a comment, [sign in][54]

## More articles by Amita Kapoor
* [ The Subsidised Lunch: Why the Price of AI Isn't the Cost of AI ][55]
  Jun 29, 2026
  
  ### The Subsidised Lunch: Why the Price of AI Isn't the Cost of AI
  
  Two things are true about the large language model (LLM) market in 2026, and they refuse to sit comfortably together…
  
  `` ``
  6
  `` `` `` `` `` `` ``
* [ The Model That Got Banned to Protect Us (From Itself) ][56]
  Jun 22, 2026
  
  ### The Model That Got Banned to Protect Us (From Itself)
  
  Loki, Fable, and the Myth of Containment In Norse mythology, Loki was the gods' most indispensable problem-solver.
  When…
  
  `` ``
  9
  `` `` `` `` `` `` ``
  1 Comment
* [ The Cloud Is Heavy: Why AI's Next Bottleneck Wears a Hard Hat ][57]
  Jun 14, 2026
  
  ### The Cloud Is Heavy: Why AI's Next Bottleneck Wears a Hard Hat
  
  In his 1984 cyberpunk novel Neuromancer, William Gibson described cyberspace as a "consensual hallucination." For two…
  
  `` ``
  13
  `` `` `` `` `` `` ``
  6 Comments
* [ The Illusion of the Digital Soul: Why AGI Is Just a Calculator With a Markdown Persona ][58]
  Jun 8, 2026
  
  ### The Illusion of the Digital Soul: Why AGI Is Just a Calculator With a Markdown Persona
  
  "The question of whether machines can think is about as relevant as the question of whether submarines can swim." —…
  
  `` ``
  19
  `` `` `` `` `` `` ``
  9 Comments
* [ The Six-Dollar Freelancer: What Happens When an Honest AI Agent Enters the Real Economy ][59]
  Jun 3, 2026
  
  ### The Six-Dollar Freelancer: What Happens When an Honest AI Agent Enters the Real Economy
  
  There is a line in Stanisław Lem's Solaris that has stayed with me for years: "We don't want to conquer the cosmos,
  we…
  
  `` ``
  14
  `` `` `` `` `` `` ``
* [ When Machines Prove What Mathematicians Cannot: The Real Shift in AI-Powered Discovery ][60]
  May 21, 2026
  
  ### When Machines Prove What Mathematicians Cannot: The Real Shift in AI-Powered Discovery
  
  On May 20, 2026, an OpenAI model solved the planar unit distance problem — a conjecture that had resisted the best…
  
  `` ``
  30
  `` `` `` `` `` `` ``
  4 Comments
* [ The Architecture of the Mind: Why I’m Trading Neural Networks for Freud (Temporarily) ][61]
  May 10, 2026
  
  ### The Architecture of the Mind: Why I’m Trading Neural Networks for Freud (Temporarily)
  
  The Brain — is wider than the Sky — For — put them side by side — The one the other will contain With ease — and you
  —…
  
  `` ``
  26
  `` `` `` `` `` `` ``
  2 Comments
* [ The Momo Dilemma and the Illusion of Free Time ][62]
  May 3, 2026
  
  ### The Momo Dilemma and the Illusion of Free Time
  
  "Time is life itself, and life resides in the human heart. And the more people saved, the less they had.
  
  `` ``
  16
  `` `` `` `` `` `` ``
  5 Comments
* [ AI's Workforce Disruption: Orchestration Over Obsolescence ][63]
  Apr 26, 2026
  
  ### AI's Workforce Disruption: Orchestration Over Obsolescence
  
  “Before you become too entranced with gorgeous gadgets and mesmerizing video displays, let me remind you that…
  
  `` ``
  15
  `` `` `` `` `` `` ``
  3 Comments
* [ The "Vibes" Are Over — Case for Neurosymbolic AI ][64]
  Apr 19, 2026
  
  ### The "Vibes" Are Over — Case for Neurosymbolic AI
  
  "The single biggest problem in communication is the illusion that it has taken place." George Bernard Shaw’s timeless…
  
  `` ``
  45
  `` `` `` `` `` `` ``
  18 Comments

Show more
[ See all articles ][65]

## Others also viewed
* [
  
  ### Useful AI things with Nat — Vol XV [EN] [ES] You’re not building AI systems. You’re connecting pieces (P.1)
  
  Natalie Gil 3mo
  ][66]
* [
  
  ### How AI Actually "Sees" the World (And Why That Matters)
  
  Robert Viren 9mo
  ][67]
* [
  
  ### The Next Token: Where AI Meets Imagination
  
  Aniket kumar Singh 1y
  ][68]
* [
  
  ### What the AI Model Market Quietly Reveals About How to Build with It
  
  Dinand Tinholt 1y
  ][69]
* [
  
  ### Battery Industry Deep-dive: How I Fell Down a Well of AI Prompts to Attempt the Perfect Infographic
  
  Diana Frentzos Knorr 1mo
  ][70]
* [
  
  ### Titans: AI's New Frontier
  
  Jyoti Vasudev 1y
  ][71]
* [
  
  ### Google’s AI Agents & DeepSeek Explained in 5 Mins
  
  Snigdha Gupta 1y
  ][72]
* [
  
  ### Gen AI in 2025: The Year of Quantity
  
  Dmitri Tcherevik 1y
  ][73]
* [
  
  ### Demystifying AI Fundamentals: A Beginner’s Guide
  
  Cedric Strickland 2y
  ][74]
* [
  
  ### Leveling up in the AI Era
  
  Kunjan Shah 1y
  ][75]

Show more Show less

## Similar topics
* [
  
  ### How Large Language Models Represent Concepts and Behaviors
  
  10 Posts
  2,028
  `` `` `` `` `` `` ``
  ][76]
* [
  
  ### How Large Language Models Create Conceptual Coherence
  
  5 Posts
  2,199
  `` `` `` `` `` `` ``
  ][77]
* [
  
  ### How Large Language Models Process Contextual Information
  
  10 Posts
  1,818
  `` `` `` `` `` `` ``
  ][78]
* [
  
  ### How Large Language Models Solve Problems Without Introspection
  
  10 Posts
  1,545
  `` `` `` `` `` `` ``
  ][79]
* [
  
  ### How to Optimize Large Language Models
  
  10 Posts
  2,107
  `` `` `` `` `` `` ``
  ][80]
* [
  
  ### How to Understand Large Language Model Fundamentals
  
  10 Posts
  2,760
  `` `` `` `` `` `` ``
  ][81]
* [
  
  ### How Large Language Models Process Big Data Sets
  
  6 Posts
  992
  `` `` `` `` `` `` ``
  ][82]
* [
  
  ### Guide to Meta Llama Large Language Models
  
  9 Posts
  3,818
  `` `` `` `` `` `` ``
  ][83]
* [
  
  ### How Large Language Models Reshape Data Patterns
  
  5 Posts
  802
  `` `` `` `` `` `` ``
  ][84]
* [
  
  ### How Llms Process Language
  
  10 Posts
  3,318
  `` `` `` `` `` `` ``
  ][85]

Show more Show less

## Explore content categories
* [Career][86]
* [Productivity][87]
* [Finance][88]
* [Soft Skills & Emotional Intelligence][89]
* [Project Management][90]
* [Education][91]
* [Technology][92]
* [Leadership][93]
* [Ecommerce][94]
* [User Experience][95]
* [Recruitment & HR][96]
* [Customer Experience][97]
* [Real Estate][98]
* [Marketing][99]
* [Sales][100]
* [Retail & Merchandising][101]
* [Science][102]
* [Supply Chain Management][103]
* [Future Of Work][104]
* [Consulting][105]
* [Writing][106]
* [Economics][107]
* [Artificial Intelligence][108]
* [Employee Experience][109]
* [Workplace Trends][110]
* [Fundraising][111]
* [Networking][112]
* [Corporate Social Responsibility][113]
* [Negotiation][114]
* [Communication][115]
* [Engineering][116]
* [Hospitality & Tourism][117]
* [Business Strategy][118]
* [Change Management][119]
* [Organizational Culture][120]
* [Design][121]
* [Innovation][122]
* [Event Planning][123]
* [Training & Development][124]

Show more Show less
* LinkedIn © 2026
* [ About ][125]
* [ Accessibility ][126]
* [ User Agreement ][127]
* [ Privacy Policy ][128]
* [ Cookie Policy ][129]
* [ Copyright Policy ][130]
* [ Brand Policy ][131]
* [ Guest Controls ][132]
* [ Community Guidelines ][133]
* * العربية (Arabic)
  * বাংলা (Bangla)
  * Čeština (Czech)
  * Dansk (Danish)
  * Deutsch (German)
  * Ελληνικά (Greek)
  * **English (English)**
  * Español (Spanish)
  * فارسی (Persian)
  * Suomi (Finnish)
  * Français (French)
  * हिंदी (Hindi)
  * Magyar (Hungarian)
  * Bahasa Indonesia (Indonesian)
  * Italiano (Italian)
  * עברית (Hebrew)
  * 日本語 (Japanese)
  * 한국어 (Korean)
  * मराठी (Marathi)
  * Bahasa Malaysia (Malay)
  * Nederlands (Dutch)
  * Norsk (Norwegian)
  * ਪੰਜਾਬੀ (Punjabi)
  * Polski (Polish)
  * Português (Portuguese)
  * Română (Romanian)
  * Русский (Russian)
  * Svenska (Swedish)
  * తెలుగు (Telugu)
  * ภาษาไทย (Thai)
  * Tagalog (Tagalog)
  * Türkçe (Turkish)
  * Українська (Ukrainian)
  * Tiếng Việt (Vietnamese)
  * 简体中文 (Chinese (Simplified))
  * 正體中文 (Chinese (Traditional))
  Language

[1]: /legal/user-agreement?trk=linkedin-tc_auth-button_user-agreement
[2]: /legal/privacy-policy?trk=linkedin-tc_auth-button_privacy-policy
[3]: /legal/cookie-policy?trk=linkedin-tc_auth-button_cookie-policy
[4]: https://www.linkedin.com/uas/request-password-reset?trk=csm-v2_forgot_password
[5]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-a
mita-kapoor-ymq9c&trk=pulse-article_contextual-sign-in-modal_join-link
[6]: /legal/user-agreement?trk=linkedin-tc_auth-button_user-agreement
[7]: /legal/privacy-policy?trk=linkedin-tc_auth-button_privacy-policy
[8]: /legal/cookie-policy?trk=linkedin-tc_auth-button_cookie-policy
[9]: #main-content
[10]: /?trk=article-ssr-frontend-pulse_nav-header-logo
[11]: https://www.linkedin.com/top-content?trk=article-ssr-frontend-pulse_guest_nav_menu_topContent
[12]: https://www.linkedin.com/pub/dir/+/+?trk=article-ssr-frontend-pulse_guest_nav_menu_people
[13]: https://www.linkedin.com/learning/search?trk=article-ssr-frontend-pulse_guest_nav_menu_learning
[14]: https://www.linkedin.com/jobs/search?trk=article-ssr-frontend-pulse_guest_nav_menu_jobs
[15]: https://www.linkedin.com/games?trk=article-ssr-frontend-pulse_guest_nav_menu_games
[16]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_nav-header-join
[17]: https://www.linkedin.com/uas/login?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-amita-k
apoor-ymq9c&fromSignIn=true&trk=article-ssr-frontend-pulse_nav-header-signin
[18]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-a
mita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_ellipsis-menu-semaphore-sign-in-redirect&guestReportContentType=PONCHO_
ARTICLE&_f=guest-reporting
[19]: https://in.linkedin.com/in/amitakapoor
[20]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_publisher-author-card
[21]: https://www.linkedin.com/pulse/our-favorite-llms-2025-why-feel-still-matters-david-norris-bttne
[22]: https://www.linkedin.com/pulse/coconut-next-leap-ai-reasoning-brylan-donaldson-brx1c
[23]: https://www.linkedin.com/pulse/when-ai-learns-think-game-changing-leap-linc-aineurog-myrve
[24]: https://www.linkedin.com/newsletters/gen-ai-simplified-7205492822492291072
[25]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c
[26]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_like-toggle_like-cta
[27]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_comment-cta
[28]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_likes-count_social-actions-reactions
[29]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_likes-count_social-actions-comments
[30]: https://pk.linkedin.com/in/rizwan-qureshi-224b99114?trk=article-ssr-frontend-pulse_x-social-details_comments-actio
n_comment_actor-name
[31]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-a
mita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_ellipsis-menu-semaphore-sign-i
n-redirect&guestReportContentType=COMMENT&_f=guest-reporting
[32]: https://arxiv.org/abs/2507.00951?trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment-text
[33]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_like
[34]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reply
[35]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reactions
[36]: https://ae.linkedin.com/in/manjarisharma01?trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment
_actor-name
[37]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-a
mita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_ellipsis-menu-semaphore-sign-i
n-redirect&guestReportContentType=COMMENT&_f=guest-reporting
[38]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_like
[39]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reply
[40]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reactions
[41]: https://in.linkedin.com/in/ramesh-chandra-panda-16485919b?trk=article-ssr-frontend-pulse_x-social-details_comments
-action_comment_actor-name
[42]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-a
mita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_ellipsis-menu-semaphore-sign-i
n-redirect&guestReportContentType=COMMENT&_f=guest-reporting
[43]: https://drive.google.com/drive/folders/1mYgBESiB4XMNCEfh-4qPcE4YKpciKPUR?usp=sharing&trk=article-ssr-frontend-puls
e_x-social-details_comments-action_comment-text
[44]: https://www.linkedin.com/posts/dr-ramesh-chandra-highest-research-patent-holder-in-world-16485919b_dear-all-greeti
ngs-we-are-going-to-activity-7287916114028769280-WetY?utm_source=share&utm_medium=member_android&trk=article-ssr-fronten
d-pulse_x-social-details_comments-action_comment-text
[45]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_like
[46]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reply
[47]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reactions
[48]: https://www.linkedin.com/in/john-ducrest-5a4b3528?trk=article-ssr-frontend-pulse_x-social-details_comments-action_
comment_actor-name
[49]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-a
mita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_ellipsis-menu-semaphore-sign-i
n-redirect&guestReportContentType=COMMENT&_f=guest-reporting
[50]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_like
[51]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reply
[52]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments-action_comment_reactions
[53]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_comments_comment-see-more
[54]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-thinking-beyond-tokens-
amita-kapoor-ymq9c&trk=article-ssr-frontend-pulse_x-social-details_feed-cta-banner-cta
[55]: https://www.linkedin.com/pulse/subsidised-lunch-why-price-ai-isnt-cost-amita-kapoor-zdzkf
[56]: https://www.linkedin.com/pulse/model-got-banned-protect-us-from-itself-amita-kapoor-ijrqf
[57]: https://www.linkedin.com/pulse/cloud-heavy-why-ais-next-bottleneck-wears-hard-hat-amita-kapoor-kbn5e
[58]: https://www.linkedin.com/pulse/illusion-digital-soul-why-agi-just-calculator-markdown-amita-kapoor-ebwre
[59]: https://www.linkedin.com/pulse/six-dollar-freelancer-what-happens-when-honest-ai-agent-amita-kapoor-7ulte
[60]: https://www.linkedin.com/pulse/when-machines-prove-what-mathematicians-cannot-real-shift-kapoor-urazf
[61]: https://www.linkedin.com/pulse/architecture-mind-why-im-trading-neural-networks-freud-amita-kapoor-d5r7c
[62]: https://www.linkedin.com/pulse/momo-dilemma-illusion-free-time-amita-kapoor-und9f
[63]: https://www.linkedin.com/pulse/ais-workforce-disruption-orchestration-over-amita-kapoor-rxtbf
[64]: https://www.linkedin.com/pulse/vibes-over-case-neurosymbolic-ai-amita-kapoor-kw6hf
[65]: https://in.linkedin.com/in/amitakapoor/recent-activity/articles/
[66]: https://www.linkedin.com/pulse/useful-ai-things-nat-vol-xv-en-youre-building-systems-natalie-gil-xnzhf
[67]: https://www.linkedin.com/pulse/how-ai-actually-sees-world-why-matters-robert-viren-v0lbc
[68]: https://www.linkedin.com/pulse/next-token-where-ai-meets-imagination-aniket-kumar-singh-cxwef
[69]: https://www.linkedin.com/pulse/what-ai-model-market-quietly-reveals-how-build-dinand-tinholt-cbqvc
[70]: https://www.linkedin.com/pulse/battery-industry-deep-dive-how-i-fell-down-well-ai-frentzos-knorr-97pyf
[71]: https://www.linkedin.com/pulse/titans-ais-new-frontier-jyoti-vasudev-vpwqc
[72]: https://www.linkedin.com/pulse/googles-ai-agents-deepseek-explained-5-mins-snigdha-gupta-hvqcc
[73]: https://www.linkedin.com/pulse/gen-ai-2025-year-quantity-dmitri-tcherevik-enyze
[74]: https://www.linkedin.com/pulse/demystifying-ai-complete-beginners-guide-cedric-strickland-oiunc
[75]: https://www.linkedin.com/pulse/leveling-up-ai-era-kunjan-shah-yexbc
[76]: https://www.linkedin.com/top-content/artificial-intelligence/understanding-ai-systems/how-large-language-models-re
present-concepts-and-behaviors/
[77]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-create-conceptual
-coherence/
[78]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-process-contextua
l-information/
[79]: https://www.linkedin.com/top-content/artificial-intelligence/understanding-ai-systems/how-large-language-models-so
lve-problems-without-introspection/
[80]: https://www.linkedin.com/top-content/artificial-intelligence/large-language-models-insights/how-to-optimize-large-
language-models/
[81]: https://www.linkedin.com/top-content/artificial-intelligence/large-language-models-insights/how-to-understand-larg
e-language-model-fundamentals/
[82]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-process-big-data-
sets/
[83]: https://www.linkedin.com/top-content/technology/ai-language-processing/guide-to-meta-llama-large-language-models/
[84]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-reshape-data-patt
erns/
[85]: https://www.linkedin.com/top-content/artificial-intelligence/large-language-models-insights/how-llms-process-langu
age/
[86]: https://www.linkedin.com/top-content/career/
[87]: https://www.linkedin.com/top-content/productivity/
[88]: https://www.linkedin.com/top-content/finance/
[89]: https://www.linkedin.com/top-content/soft-skills-emotional-intelligence/
[90]: https://www.linkedin.com/top-content/project-management/
[91]: https://www.linkedin.com/top-content/education/
[92]: https://www.linkedin.com/top-content/technology/
[93]: https://www.linkedin.com/top-content/leadership/
[94]: https://www.linkedin.com/top-content/ecommerce/
[95]: https://www.linkedin.com/top-content/user-experience/
[96]: https://www.linkedin.com/top-content/recruitment-hr/
[97]: https://www.linkedin.com/top-content/customer-experience/
[98]: https://www.linkedin.com/top-content/real-estate/
[99]: https://www.linkedin.com/top-content/marketing/
[100]: https://www.linkedin.com/top-content/sales/
[101]: https://www.linkedin.com/top-content/retail-merchandising/
[102]: https://www.linkedin.com/top-content/science/
[103]: https://www.linkedin.com/top-content/supply-chain-management/
[104]: https://www.linkedin.com/top-content/future-of-work/
[105]: https://www.linkedin.com/top-content/consulting/
[106]: https://www.linkedin.com/top-content/writing/
[107]: https://www.linkedin.com/top-content/economics/
[108]: https://www.linkedin.com/top-content/artificial-intelligence/
[109]: https://www.linkedin.com/top-content/employee-experience/
[110]: https://www.linkedin.com/top-content/workplace-trends/
[111]: https://www.linkedin.com/top-content/fundraising/
[112]: https://www.linkedin.com/top-content/networking/
[113]: https://www.linkedin.com/top-content/corporate-social-responsibility/
[114]: https://www.linkedin.com/top-content/negotiation/
[115]: https://www.linkedin.com/top-content/communication/
[116]: https://www.linkedin.com/top-content/engineering/
[117]: https://www.linkedin.com/top-content/hospitality-tourism/
[118]: https://www.linkedin.com/top-content/business-strategy/
[119]: https://www.linkedin.com/top-content/change-management/
[120]: https://www.linkedin.com/top-content/organizational-culture/
[121]: https://www.linkedin.com/top-content/design/
[122]: https://www.linkedin.com/top-content/innovation/
[123]: https://www.linkedin.com/top-content/event-planning/
[124]: https://www.linkedin.com/top-content/training-development/
[125]: https://about.linkedin.com?trk=d_flagship2_pulse_read_footer-about
[126]: https://www.linkedin.com/accessibility?trk=d_flagship2_pulse_read_footer-accessibility
[127]: https://www.linkedin.com/legal/user-agreement?trk=d_flagship2_pulse_read_footer-user-agreement
[128]: https://www.linkedin.com/legal/privacy-policy?trk=d_flagship2_pulse_read_footer-privacy-policy
[129]: https://www.linkedin.com/legal/cookie-policy?trk=d_flagship2_pulse_read_footer-cookie-policy
[130]: https://www.linkedin.com/legal/copyright-policy?trk=d_flagship2_pulse_read_footer-copyright-policy
[131]: https://brand.linkedin.com/policies?trk=d_flagship2_pulse_read_footer-brand-policy
[132]: https://www.linkedin.com/psettings/guest-controls?trk=d_flagship2_pulse_read_footer-guest-controls
[133]: https://www.linkedin.com/legal/professional-community-policies?trk=d_flagship2_pulse_read_footer-community-guide
```
