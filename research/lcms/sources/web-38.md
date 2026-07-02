# Web source

- URL: https://www.digitalocean.com/community/tutorials/large-concept-models
- Title: * [Blog][1]
- Captured (UTC): 2026-06-29T16:30:44.083306904+00:00

```text
* [Blog][1]
* [Docs][2]
* [Careers][3]
* [Get Support][4]
* [Contact Sales][5]
[DigitalOcean
][6]
* Products
  
  Featured AI Products
  
  Compute
  
  Build, deploy, and scale cloud compute resources
  
  Containers and Images
  
  Safely store and manage containers and backups
  
  Managed Databases
  
  Fully managed resources running popular database engines
  
  Management and Dev Tools
  
  Control infrastructure and gather insights
  
  Networking
  
  Secure and control traffic to apps
  
  Security
  
  Help protect your account and resources with these security features
  
  Storage
  
  Store and access any amount of data reliably in the cloud
  
  [Browse all products][7]
  
* Solutions
  
  AI/ML
  
  CMS
  
  Data and IoT
  
  Developer Tools
  
  Gaming and Media
  
  Hosting
  
  Security and Networking
  
  Startups and SMBs
  
  Web and App Platforms
  
  [See all solutions][8]
  
* Developers
  
  Community
  
  Documentation
  
  Developer Tools
  
  Get Involved
  
  Utilities and Help
  
* Partners
  
  Become a Partner
  
  Marketplace
  
* [Pricing][9]
* Log in
  * Log in to:
  * [ Community][10]
  * [DigitalOcean][11]
* Sign up
  * Sign up for:
  * [ Community][12]
  * [DigitalOcean][13]
* Log in
  * Log in to:
  * [ Community][14]
  * [DigitalOcean][15]
* Sign up
  * Sign up for:
  * [ Community][16]
  * [DigitalOcean][17]
* [Tutorials][18]
* [Questions][19]
* [Product Docs][20]
* Search Community

## Report this

What is the reason for this report?
This undefined is spam
This undefined is offensive
This undefined is off-topic
This undefined is other
Submit

## Table of contents
1.  [Introduction][21]
2.  [Key Takeaways][22]
3.  [From Tokens to Concepts][23]
4.  [How Do LCMs Work][24]
5.  [Why is this approach better][25]
6.  [The SONAR embedding space][26]
7.  [BaseLCM A Naive Approach][27]
8.  [DiffusionBased LCMs Better Handling of Ambiguity][28]
9.  [Evaluation and Results][29]
10. [Installing and Running LCMs][30]
11. [Limitations][31]
12. [References and Resources][32]
1. [Tutorials][33]
2. [Data Science][34]
3. Large Concept Models Explained

[Tutorial][35]

# Large Concept Models Explained

Published on June 18, 2025
[Data Science][36]
[Machine Learning][37]
[Shaoni Mukherjee]

By [Shaoni Mukherjee][38]

AI Technical Writer

[Large Concept Models Explained]
Table of contents
Popular topics

## [**Introduction**][39]

[Large Language Models][40] (LLMs) have transformed AI by powering tasks like summarization, translation, and code
generation. These models rely on token-based processing—breaking down text into subwords or characters to understand and
generate content. While effective, this approach is limited in mimicking how humans reason and communicate through
abstract, high-level concepts.

Meta’s [paper][41], **“Large Concept Models: Language Modeling in a Sentence Representation Space”**, proposes a
fundamental shift. The paper suggests of processing concepts instead of tokens. Large Concept Models (LCMs) operate in
[**conceptual embedding spaces**][42], redefining how language models understand, represent, and generate meaning.

## [**Key Takeaways**][43]
* **Concept over Tokens**: LCMs move away from traditional token-by-token processing and instead focus on understanding
  and generating language at the *conceptual* level.
* **Better Long-Context Understanding**: By reasoning with broader ideas, LCMs handle longer text inputs more
  effectively than current models.
* **Improved Reasoning Abilities**: Their structure supports more advanced, layered thinking—helpful for tasks requiring
  logical steps and deep understanding.
* **Closer to Human-Like AI**: Meta’s research brings us closer to AI systems that think more like humans, making
  interactions smoother, smarter, and more intuitive.

## [**From Tokens to Concepts**][44]

### [**The Role of Tokenization in LLMs**][45]

In traditional LLMs like [GPT-4][46], tokenization is the first step. For instance, the sentence *“Will tokenization
eventually be dead?”* is split into subword tokens like `"Will"`, `"token"`, and `"ization"`. These tokens are then fed
into a Transformer model to generate a response.

### [**The Problem**][47]

However, this approach:
* Splits meaningful expressions into fragmented parts
* Operates on arbitrary units, ignoring semantic cohesion
* Struggles with long-context reasoning and hierarchical planning

### [**The Solution: Concepts**][48]

**Concepts** refer to higher-order representations of meaning. They’re not tied to language-specific tokens and can be
derived from **text, speech, or even multimodal signals**. LCMs process sentences as unified semantic
units—**concepts**—which enables:
* Better handling of **long contexts**
* More **abstract reasoning**
* Language and modality independence

### [**What Are Large Concept Models?**][49]

Traditional AI models like ChatGPT work by predicting the next word (called a “token”) based on the previous ones. But
humans don’t think like that—we think in full ideas or sentences and build up meaning in layers.

A **Large Concept Model (LCM)** is a new type of AI model that goes beyond traditional Large Language Models (LLMs),
which operate at the word or token level. Instead of focusing on predicting the next word, LCMs work with **higher-level
abstract ideas or “concepts”**, like full sentences or meanings, to understand and generate content.

This process is much inspired by how humans think and plan—first outlining ideas and then filling in details. LCMs use a
**hierarchical reasoning approach**. This means they first grasp the bigger picture and then focus on specifics,
allowing for more coherent and structured outputs. Unlike current LLMs, which are mostly English-based and data-heavy,
LCMs aim to be more efficient and language-independent by using sentence-level embeddings like SONAR.

## [**How Do LCMs Work?**][50]

LCMs follow a three-step pipeline:
* [**Concept Encoder (SONAR)**][51]**: Input** (text or speech) is broken into **sentences**. Each sentence is turned
  into a **“concept embedding”** using a tool called **SONAR** (which works in 200 languages and supports text +
  speech).
* **Large Concept Model (LCM):** A Transformer operating entirely in concept space. It reasons over the concept sequence
  to generate output concepts. The concept embeddings are processed by the **LCM** to perform a task, like summarizing
  or translating. The result is a new set of concept embeddings.
* **Concept Decoder (SONAR)**: Translates concept embeddings back into natural language (or speech) output.

The reasoning step happens *only* on concepts—not words or tokens—so the model doesn’t need to care about the
input/output language or whether it’s speech or text.

[image1]

This architecture resembles the [**Joint Embedding Predictive Architecture (JEPA)**][52], which also promotes abstract
reasoning across modalities.

## [**Why is this approach better?**][53]

This approach can **handle any language or speech/text** input without needing extra training. It’s much **faster and
less resource-intensive** than traditional LLMs because it works with fewer, more meaningful chunks (concepts instead of
tokens). Further, it also supports [**zero-shot generalization**][54]: you don’t have to retrain it for every language
or task.

## [**The SONAR embedding space**][55]

[image2]

The motivation behind this work is to enable reasoning at a higher, more meaningful level than individual words or
tokens. To achieve this, the authors use SONAR—a powerful embedding system that represents entire sentences as semantic
concepts. SONAR was trained using machine translation, denoising, and similarity objectives across 200 languages, and it
performs exceptionally well on semantic similarity tasks. It also extends to speech through a teacher-student approach,
supporting 76 languages for speech input and English for speech output. Because the Large Concept Model (LCM) operates
directly on these SONAR concept embeddings, it can perform reasoning across multiple languages and modalities, including
an experimental encoder for American Sign Language. This makes LCMs more inclusive, efficient, and scalable than
traditional LLMs.

## [**Base-LCM: A Naive Approach**][56]

[image3]

The **Base-LCM** is a model designed to generate **sentence-level embeddings** (representing full ideas or concepts),
instead of predicting one word at a time like current language models. It works by **learning how to predict the next
sentence embedding** given previous ones, like continuing a conversation or paragraph, but at the idea level.

To do this, the model uses **SONAR embeddings**, which represent full sentences in a continuous space. However, since
these embeddings are very different from the internal format used by the model, they are **normalized and mapped into
the model’s hidden space** using a “PreNet.” After generating new embeddings, they’re **converted back** using a
“PostNet.”

Training this model means teaching it to guess the next sentence’s embedding as accurately as possible—this is done by
minimizing the [**mean squared error (MSE)**][57] between what it predicted and the true next embedding. To help it know
when to stop generating during real use, the model is trained to recognize an **“End of text”** sentence, and it stops
generating if it sees that or if the new sentence embedding is too similar to the previous one.

Additionally, because a sentence can have many valid next ideas (just like how many pictures can match one prompt in
DALL·E), the authors are also exploring [**diffusion models**][58] **and [quantization][59]** to better handle the
uncertainty in next-sentence generation.

### [**Limitation**][60]

The Base-LCM assumes a single “correct” next concept, which is unrealistic since many plausible conceptual continuations
may exist in context.

## [**Diffusion-Based LCMs: Better Handling of Ambiguity**][61]

### [**Why Diffusion?**][62]

Inspired by diffusion models in image generation, LCMs use a similar approach to predict **a distribution over possible
next concepts**, rather than a single fixed one. Diffusion-based LCMs are advanced models that generate sentence-level
concepts (embeddings) by gradually transforming random noise into meaningful sentences, similar to how diffusion models
generate images.

They do this in two steps: first, they add noise to clean data (the “forward process”) using a schedule that controls
how much noise is added over time. Then, during generation (the “reverse process”), they learn to remove that noise step
by step to recover the original sentence embedding. This reverse process is guided by learned patterns in the data and
can be either conditional (based on the previous context) or unconditional. The model uses special noise schedules—like
cosine, quadratic, or sigmoid—to control how it learns at different noise levels. During training, it learns how to
reconstruct the original data from noisy versions using simple or weighted losses.

At inference, various techniques like classifier-free guidance, step selection, and error-scaling help improve the
quality and diversity of the generated sentences. Two architectures—One-Tower and Two-Tower—are proposed for
implementing this diffusion-based reasoning.

[image4]

[Image Source][63]

### [**Two Architectures**][64]

#### **One-Tower Diffusion LCM**

[image5]

The One-Tower diffusion LCM is a model designed to predict clean sentence embeddings (representations of sentences) from
their noisy versions, using a single Transformer. During training, it receives a mix of clean and noisy sentence
embeddings as input. These embeddings are interleaved (i.e., placed alternately) so that the model sees both types but
is instructed to only focus on the clean ones when making predictions. Each noisy embedding also includes information
about how much noise was added (the “diffusion timestep”), which is appended to the input. The model uses causal
attention—meaning it can only look at previous sentences, not future ones—to maintain the natural flow of text.
Sometimes, parts of the model are trained without context (unconditional) so that later, during inference, it can
balance between following context and being creative (using classifier-free guidance). This approach allows the model to
efficiently learn to denoise and generate entire sequences of sentence embeddings all at once.

#### **Two-Tower Diffusion LCM**

The **Two-Tower diffusion LCM** separates the job of understanding the context from the task of generating the next
sentence embedding.

[image6]

Here’s how it works in simple terms:
* **Tower 1: Contextualizer**: This is a Transformer that reads all the previous sentence embeddings (`x<n`) and encodes
  them using **causal attention**—meaning it reads them one by one in order, without peeking ahead. This creates a
  meaningful summary of the past context.
* **Tower 2: Denoiser**: This second Transformer takes in a **noisy version** of the next sentence embedding and tries
  to **recover the clean version** (`x₀ₙ`). It does this gradually through multiple steps of denoising. To help guide
  this process, it uses **cross-attention** to look at the context produced by the first tower.
* **Adaptive LayerNorm (AdaLN)**: The denoiser adapts its internal computations based on how noisy the input is at each
  step. It does this using a small network that learns how much to scale, shift, and blend each layer’s output,
  depending on the timestep of the diffusion process. This helps the model adjust its behavior at different noise
  levels.
* **Training setup**: To help the model learn, during training:
  * The context is slightly shifted so that each prediction only sees past sentences.
  * A zero-vector is used as a placeholder at the start of the sequence to predict the first sentence.
  * Some context rows are randomly dropped (with a probability `pcfg`) so the model also learns how to denoise without
    context—this supports **classifier-free guidance** during inference.

In short, the Two-Tower model first understands the context using one Transformer and then generates the next sentence
by denoising a noisy version using another Transformer that refers back to that context. These architectures allow
**multiple valid outputs**, enhancing diversity and flexibility in generation.

## [**Evaluation and Results**][65]

To assess the effectiveness of Large Concept Models (LCMs), the researchers conducted comprehensive evaluations,
particularly focusing on **summarization tasks**. The models were compared against several strong baselines, including
both **encoder-decoder models** and **decoder-only large language models (LLMs)**. The evaluations were done using both
automatic metrics and human-aligned analysis methods.

The evaluation compares multiple versions of LCMs:
* **Base-LCM** – the naive autoregressive version predicting the next concept deterministically.
* **Diffusion-Based LCMs** – including both **One-Tower** and **Two-Tower** architectures that generate concepts
  stochastically.
* **Quant LCMs** – a variant not covered in detail in the paper, but included for comparison.
* **smaLLaMA** – an instruction-tuned LLM used as an additional performance baseline.

The best-performing LCM model was then **scaled to 7 billion parameters** and compared against popular LLMs, including:
* **T5-3B** – an encoder-decoder Transformer fine-tuned for summarization.
* **Gemma-7B**, **LLaMA 3.1-8B**, and **Mistral-7B-v0.3** – decoder-only, instruction-finetuned LLMs.

### [**Metrics Used for Evaluation**][66]

The paper employed a range of automatic metrics commonly used for evaluating natural language generation models:
* [**ROUGE-L**][67]: Measures the longest common subsequence between generated and reference summaries.
* [**Coherence**][68]: A classifier-based metric evaluating logical flow and topic consistency.
* **OVL-3 (Overlap Score)**: Quantifies how extractive or abstractive a summary is.
* **Repetition Rate**: Measures the frequency of repeated words/phrases in the generated output.
* [**CoLA (Corpus of Linguistic Acceptability)**][69]: Assesses grammatical acceptability based on linguistic norms.
* **SH-4 (Source Attribution)**: Evaluates how well the generated summary attributes content to the original source.
* **SH-5 (Semantic Coverage)**: Measures the completeness of semantic content transfer from source to summary.

## [Installing and Running LCMs][70]

Meta’s Large Concept Models rely on [`fairseq2`][71] and support installation via [`uv`][72] or [`pip`][73].

### [Option 1: Using `uv` (Recommended)][74]

`# Set up environment and install CPU dependencies
uv sync --extra cpu --extra eval --extra data

# For GPU support (example: Torch 2.5.1 + CUDA 12.1)
uv pip install torch==2.5.1 --extra-index-url https://download.pytorch.org/whl/cu121 --upgrade
uv pip install fairseq2==v0.3.0rc1 --pre --extra-index-url https://fair.pkg.atmeta.com/fairseq2/whl/rc/pt2.5.1/cu121 --u
pgrade
`

### [Option 2: Using pip][75]

`# Install pip dependencies
pip install --upgrade pip
pip install fairseq2==v0.3.0rc1 --pre --extra-index-url https://fair.pkg.atmeta.com/fairseq2/whl/rc/pt2.5.1/cpu
pip install -e ".[data,eval]"
`

### [Preparing the Data][76]

LCMs use sentence-level embeddings from [SONAR][77] to train on textual data.

`# Prepare Wikipedia data with SONAR and SaT
uv run --extra data scripts/prepare_wikipedia.py /output/dir/for/the/data
`

### [Fitting a Normalizer][78]

`python scripts/fit_embedding_normalizer.py \
  --ds dataset1:4 dataset2:1 dataset3:10 \
  --save_path "path/to/new/normalizer.pt" \
  --max_nb_samples 1000000
`

### [Pre-training LCMs][79]

Option A: Train MSE LCM with SLURM (submitit)

`python -m lcm.train +pretrain=mse \
  ++trainer.output_dir="checkpoints/mse_lcm" \
  ++trainer.experiment_name=training_mse_lcm
`

Option B: Train Locally (Torchrun)

`CUDA_VISIBLE_DEVICES=0,1 torchrun --standalone --nnodes=1 --nproc-per-node=2 \
  -m lcm.train launcher=standalone \
  +pretrain=mse \
  ++trainer.data_loading_config.max_tokens=1000 \
  ++trainer.output_dir="checkpoints/mse_lcm" \
  +trainer.use_submitit=false
`

### [Finetuning the Two-Tower Diffusion LCM][80]

`CUDA_VISIBLE_DEVICES=0,1 torchrun --standalone --nnodes=1 --nproc-per-node=2 \
  -m lcm.train launcher=standalone \
  +finetune=two_tower \
  ++trainer.output_dir="checkpoints/finetune_two_tower_lcm" \
  ++trainer.data_loading_config.max_tokens=1000 \
  +trainer.use_submitit=false \
  ++trainer.model_config_or_name=my_pretrained_two_tower
`

### [Evaluating LCMs][81]

`python -m nltk.downloader punkt_tab

torchrun --standalone --nnodes=1 --nproc-per-node=1 -m lcm.evaluation \
  --predictor two_tower_diffusion_lcm \
  --model_card ./checkpoints/finetune_two_tower_lcm/checkpoints/step_1000/model_card.yaml \
  --data_loading.max_samples 100 \
  --data_loading.batch_size 4 \
  --generator_batch_size 4 \
  --dump_dir evaluation_outputs/two_tower \
  --inference_timesteps 40 \
  --initial_noise_scale 0.6 \
  --guidance_scale 3 \
  --guidance_rescale 0.7 \
  --tasks finetuning_data_lcm.validation \
  --task_args '{"max_gen_len": 10, "eos_config": {"text": "End of text."}}'
`

Evaluation outputs (including ROUGE metrics and predictions) will be saved in `./evaluation_outputs/two_tower`.

### [**Key Findings**][82]

**a. Summarization Quality** **Diffusion-based LCMs** significantly **outperformed Base-LCM** in both ROUGE-L and
Coherence metrics. Compared to **instruction-finetuned LLMs**, the **Two-Tower LCM** achieved **comparable or superior
results**, especially in tasks requiring semantic abstraction. The **7B parameter LCM** was competitive with **T5-3B**,
a model specifically designed and tuned for summarization tasks.

**b. Abstractive vs. Extractive Summarization** LCMs tend to produce more abstractive summaries than traditional LLMs.
This was evident from lower OVL-3 scores, suggesting that LCMs aren’t simply copying phrases from the input but are
rephrasing or summarizing content at a conceptual level.

**c. Repetition Handling** LCMs demonstrated lower repetition rates, an important advantage for generating fluent and
natural outputs. The repetition rates of LCMs were **closer to human-written summaries**, indicating better semantic
planning during generation.

**d. Fluency and Linguistic Quality** Interestingly, while LCMs fared well in semantic metrics, they performed slightly
lower in fluency, as measured by CoLA scores. This suggests that while the models generate meaningful summaries, they
occasionally produce **grammatical or stylistic inconsistencies**—a possible side effect of operating in an embedding
space without tight token-level control.

**e. Source Attribution and Semantic Coverage** On source attribution (SH-4) and semantic coverage (SH-5), LCMs
performed reasonably well, but some model-based evaluation biases may have favored token-level generation from LLMs.
Nevertheless, LCMs maintained **core semantic fidelity**, showing promise in aligning with source content at a
conceptual level.

## [Limitations][83]

While Large Concept Models (LCMs) show a lot of promise, there are a few key limitations that stand out.

First, the model relies on the SONAR embedding space, which was trained on short, well-aligned translation data. This
makes it great for capturing local meaning but less reliable for understanding loosely related sentences or more complex
inputs like links, numbers, or code.

Another challenge is the use of a frozen encoder. While this makes training more efficient, it limits the model’s
ability to adapt to the specific needs of concept-level modeling. An end-to-end trained encoder might offer better
semantic understanding but would require more data and compute.

The way LCMs currently treat whole sentences as single “concepts” can also be problematic. Long sentences may contain
multiple ideas, and breaking them down isn’t always straightforward. This makes it harder to represent them accurately
with fixed-size embeddings.

There’s also a data sparsity issue. Most sentences in a training corpus are unique, making it difficult for the model to
learn general patterns. Higher-level semantic representations could help, but they come with trade-offs like losing
important details.

Finally, modeling text with diffusion techniques is still tricky. Text may be represented as continuous vectors, but
it’s ultimately a discrete structure. The Quant-LCM attempts to handle this, but SONAR wasn’t optimized for
quantization, leading to inefficiencies.

### [**Conclusion**][84]

Large Concept Models (LCMs) represent an important step forward in how [AI][85] understands and works with language.
Instead of focusing on small parts of text called [tokens][86], LCMs look at the bigger picture—concepts and meaning.
This helps them better understand longer conversations, think more logically, and support multiple languages and
different types of dat, like text and images.

Meta’s work on LCMs shows what the future of AI could look like: smarter, more human-like systems that truly understand
context and meaning. These models have the potential to improve how we interact with AI, making it more natural,
accurate, and helpful. As this technology grows, it could lead to more powerful tools for communication, creativity, and
problem-solving across many fields.

## [References and Resources][87]
* [Large Concept Models (LCMs) by Meta: The Era of AI After LLMs?][88]
* [Official GitHub repo][89]
* [Original paper][90]
* [Meta Large Concept Models (LCM): End of LLMs?][91]

Thanks for learning with the DigitalOcean Community. Check out our offerings for compute, storage, networking, and
managed databases.

[Learn more about our products][92]

### About the author

[Shaoni Mukherjee]
Shaoni Mukherjee
Author
AI Technical Writer
[See author profile][93]

With a strong background in data science and over six years of experience, I am passionate about creating in-depth
content on technologies. Currently focused on AI, machine learning, and GPU computing, working on topics ranging from
deep learning frameworks to optimizing GPU-based workloads.

[See author profile][94]
Category:
[Tutorial][95]
Tags:
[Data Science][96]
[Machine Learning][97]

#### Still looking for an answer?

[Ask a question][98][Search for more help][99]
Was this helpful?
YesNo
Comments(0)Follow-up questions(0)
﻿


This textbox defaults to using Markdown to format your answer.

You can type !ref in this text area to quickly search our full set of tutorials, documentation & marketplace offerings
and insert the link!

[Sign in/up to comment][100]
[[Creative Commons]][101]This work is licensed under a Creative Commons Attribution-NonCommercial- ShareAlike 4.0
International License.

## Deploy on DigitalOcean

Click below to sign up for DigitalOcean's virtual machines, Databases, and AIML products.
[Sign up][102]

## Popular Topics
1.  [AI/ML][103]
2.  [Ubuntu][104]
3.  [Linux Basics][105]
4.  [JavaScript][106]
5.  [Python][107]
6.  [MySQL][108]
7.  [Docker][109]
8.  [Kubernetes][110]
9.  [All tutorials][111]
10. [Talk to an expert][112]

## Featured tutorials
1. [SOLID Design Principles Explained: Building Better Software Architecture][113]
2. [How To Remove Docker Images, Containers, and Volumes][114]
3. [How to Create a MySQL User and Grant Privileges (Step-by-Step)][115]
* [All tutorials][116]
* [All topic tags][117]

##### Join the Tech Talk

**Success!** Thank you! Please check your email for further details.

Please complete your information!
* Table of contents
* [**Introduction**][118]
* [**Key Takeaways**][119]
* [**From Tokens to Concepts**][120]
* [**How Do LCMs Work?**][121]
* [**Why is this approach better?**][122]
* [**The SONAR embedding space**][123]
* [**Base-LCM: A Naive Approach**][124]
* [**Diffusion-Based LCMs: Better Handling of Ambiguity**][125]
* [**Evaluation and Results**][126]
* [Installing and Running LCMs][127]
* [Limitations][128]
* [References and Resources][129]
* ## Deploy on DigitalOcean
  
  Click below to sign up for DigitalOcean's virtual machines, Databases, and AIML products.
  [Sign up][130]
  
  ## Popular Topics
  1.  [AI/ML][131]
  2.  [Ubuntu][132]
  3.  [Linux Basics][133]
  4.  [JavaScript][134]
  5.  [Python][135]
  6.  [MySQL][136]
  7.  [Docker][137]
  8.  [Kubernetes][138]
  9.  [All tutorials][139]
  10. [Talk to an expert][140]
  
  ## Featured tutorials
  1. [SOLID Design Principles Explained: Building Better Software Architecture][141]
  2. [How To Remove Docker Images, Containers, and Volumes][142]
  3. [How to Create a MySQL User and Grant Privileges (Step-by-Step)][143]
  * [All tutorials][144]
  * [All topic tags][145]

## Become a contributor for community

Get paid to write technical tutorials and select a tech-focused charity to receive a matching donation.

[Sign Up][146]

## DigitalOcean Documentation

Full documentation for every DigitalOcean product.

[Learn more][147]

## Resources for startups and AI-native businesses

The Wave has everything you need to know about building a business, from raising funding to marketing your product.

[Learn more][148]

## The developer cloud

Scale up as you grow — whether you're running one virtual machine or ten thousand.

[View all products][149]

## Start building today

From GPU-powered inference and Kubernetes to managed databases and storage, get everything you need to build, scale, and
deploy intelligent applications.

[Sign up][150]

## Company
* [About][151]
* [Leadership][152]
* [Blog][153]
* [Careers][154]
* [Customers][155]
* [Partners][156]
* [Referral Program][157]
* [Affiliate Program][158]
* [Press][159]
* [Legal][160]
* [Privacy Policy][161]
* [Security][162]
* [Investor Relations][163]

## Products
* [GPU Droplets][164]
* [Bare Metal GPUs][165]
* [Inference Engine][166]
* [Data & Learning][167]
* [Model Library][168]
* [Droplets][169]
* [Kubernetes][170]
* [Functions][171]
* [App Platform][172]
* [Load Balancers][173]
* [Managed Databases][174]
* [Spaces][175]
* [Block Storage][176]
* [Network File Storage][177]
* [API][178]
* [Uptime][179]
* [Cloud Security Posture Management (CSPM)][180]
* [Identity and Access Management (IAM)][181]
* [Cloudways][182]
* [View all Products][183]

## Resources
* [Community Tutorials][184]
* [Community Q&A][185]
* [CSS-Tricks][186]
* [Write for DOnations][187]
* [Currents Research][188]
* [DigitalOcean Startups][189]
* [Wavemakers Program][190]
* [Compass Council][191]
* [Open Source][192]
* [Newsletter Signup][193]
* [Marketplace][194]
* [Pricing][195]
* [Pricing Calculator][196]
* [Documentation][197]
* [Release Notes][198]
* [Code of Conduct][199]
* [Shop Swag][200]

## Solutions
* [AI Training GPU][201]
* [GPU Inference][202]
* [VPS Hosting][203]
* [Website Hosting][204]
* [VPN][205]
* [Docker Hosting][206]
* [Node.js Hosting][207]
* [Web Mobile Apps][208]
* [WordPress Hosting][209]
* [Virtual Machines][210]
* [View all Solutions][211]

## Contact
* [Support][212]
* [Sales][213]
* [Report Abuse][214]
* [System Status][215]
* [Share your ideas][216]

## Company
* [About][217]
* [Leadership][218]
* [Blog][219]
* [Careers][220]
* [Customers][221]
* [Partners][222]
* [Referral Program][223]
* [Affiliate Program][224]
* [Press][225]
* [Legal][226]
* [Privacy Policy][227]
* [Security][228]
* [Investor Relations][229]

## Products
* [GPU Droplets][230]
* [Bare Metal GPUs][231]
* [Inference Engine][232]
* [Data & Learning][233]
* [Model Library][234]
* [Droplets][235]
* [Kubernetes][236]
* [Functions][237]
* [App Platform][238]
* [Load Balancers][239]
* [Managed Databases][240]
* [Spaces][241]
* [Block Storage][242]
* [Network File Storage][243]
* [API][244]
* [Uptime][245]
* [Cloud Security Posture Management (CSPM)][246]
* [Identity and Access Management (IAM)][247]
* [Cloudways][248]
* [View all Products][249]

## Resources
* [Community Tutorials][250]
* [Community Q&A][251]
* [CSS-Tricks][252]
* [Write for DOnations][253]
* [Currents Research][254]
* [DigitalOcean Startups][255]
* [Wavemakers Program][256]
* [Compass Council][257]
* [Open Source][258]
* [Newsletter Signup][259]
* [Marketplace][260]
* [Pricing][261]
* [Pricing Calculator][262]
* [Documentation][263]
* [Release Notes][264]
* [Code of Conduct][265]
* [Shop Swag][266]

## Solutions
* [AI Training GPU][267]
* [GPU Inference][268]
* [VPS Hosting][269]
* [Website Hosting][270]
* [VPN][271]
* [Docker Hosting][272]
* [Node.js Hosting][273]
* [Web Mobile Apps][274]
* [WordPress Hosting][275]
* [Virtual Machines][276]
* [View all Solutions][277]

## Contact
* [Support][278]
* [Sales][279]
* [Report Abuse][280]
* [System Status][281]
* [Share your ideas][282]

© 2026 DigitalOcean, LLC.[Sitemap][283].

[1]: /blog
[2]: https://docs.digitalocean.com/products
[3]: /careers
[4]: /support
[5]: /company/contact/sales?referrer=tophat
[6]: /
[7]: /products
[8]: /solutions
[9]: /pricing
[10]: https://www.digitalocean.com/api/dynamic-content/v1/login?success_redirect=https%3A%2F%2Fwww.digitalocean.com&erro
r_redirect=https%3A%2F%2Fwww.digitalocean.com%2Fauth-error&type=login
[11]: https://cloud.digitalocean.com/login
[12]: https://www.digitalocean.com/api/dynamic-content/v1/login?success_redirect=https%3A%2F%2Fwww.digitalocean.com&erro
r_redirect=https%3A%2F%2Fwww.digitalocean.com%2Fauth-error&type=register
[13]: https://cloud.digitalocean.com/registrations/new
[14]: https://www.digitalocean.com/api/dynamic-content/v1/login?success_redirect=https%3A%2F%2Fwww.digitalocean.com&erro
r_redirect=https%3A%2F%2Fwww.digitalocean.com%2Fauth-error&type=login
[15]: https://cloud.digitalocean.com/login
[16]: https://www.digitalocean.com/api/dynamic-content/v1/login?success_redirect=https%3A%2F%2Fwww.digitalocean.com&erro
r_redirect=https%3A%2F%2Fwww.digitalocean.com%2Fauth-error&type=register
[17]: https://cloud.digitalocean.com/registrations/new
[18]: /community/tutorials
[19]: /community/questions
[20]: https://docs.digitalocean.com
[21]: /community/tutorials/large-concept-models#introduction
[22]: /community/tutorials/large-concept-models#key-takeaways
[23]: /community/tutorials/large-concept-models#from-tokens-to-concepts
[24]: /community/tutorials/large-concept-models#how-do-lcms-work
[25]: /community/tutorials/large-concept-models#why-is-this-approach-better
[26]: /community/tutorials/large-concept-models#the-sonar-embedding-space
[27]: /community/tutorials/large-concept-models#base-lcm-a-naive-approach
[28]: /community/tutorials/large-concept-models#diffusion-based-lcms-better-handling-of-ambiguity
[29]: /community/tutorials/large-concept-models#evaluation-and-results
[30]: /community/tutorials/large-concept-models#installing-and-running-lcms
[31]: /community/tutorials/large-concept-models#limitations
[32]: /community/tutorials/large-concept-models#references-and-resources
[33]: /community/tutorials?subtype=tutorial
[34]: /community/tags/data-science
[35]: /community/tutorials?subtype=tutorial
[36]: /community/tags/data-science
[37]: /community/tags/machine-learning
[38]: /community/users/smukherjee
[39]: #introduction
[40]: /resources/articles/large-language-models
[41]: https://arxiv.org/pdf/2412.08821
[42]: https://arxiv.org/abs/2209.00445
[43]: #key-takeaways
[44]: #from-tokens-to-concepts
[45]: #the-role-of-tokenization-in-llms
[46]: https://chat.chatbot.app/?model=gpt-4o
[47]: #the-problem
[48]: #the-solution-concepts
[49]: #what-are-large-concept-models
[50]: #how-do-lcms-work
[51]: https://arxiv.org/abs/2308.11466
[52]: https://www.turingpost.com/p/jepa
[53]: #why-is-this-approach-better
[54]: https://arxiv.org/abs/2402.14095
[55]: #the-sonar-embedding-space
[56]: #base-lcm-a-naive-approach
[57]: /community/tutorials/loss-functions-in-python#1-mean-square-error-mse
[58]: /community/tutorials/denoising-via-diffusion-model
[59]: /community/tutorials/model-quantization-large-language-models
[60]: #limitation
[61]: #diffusion-based-lcms-better-handling-of-ambiguity
[62]: #why-diffusion
[63]: https://developer.nvidia.com/blog/improving-diffusion-models-as-an-alternative-to-gans-part-1/
[64]: #two-architectures
[65]: #evaluation-and-results
[66]: #metrics-used-for-evaluation
[67]: https://dev.to/aws-builders/mastering-rouge-matrix-your-guide-to-large-language-model-evaluation-for-summarization
-with-examples-jjg
[68]: https://arxiv.org/pdf/2305.14587
[69]: https://paperswithcode.com/dataset/cola
[70]: #installing-and-running-lcms
[71]: https://github.com/facebookresearch/fairseq2
[72]: /community/conceptual-articles/uv-python-package-manager
[73]: /community/tutorials/common-python-tools-using-virtualenv-installing-with-pip-and-managing-packages
[74]: #option-1-using-uv-recommended
[75]: #option-2-using-pip
[76]: #preparing-the-data
[77]: https://github.com/facebookresearch/SONAR
[78]: #fitting-a-normalizer
[79]: #pre-training-lcms
[80]: #finetuning-the-two-tower-diffusion-lcm
[81]: #evaluating-lcms
[82]: #key-findings
[83]: #limitations
[84]: #conclusion
[85]: /community/tutorials/local-ai-agents-with-langgraph-and-ollama
[86]: /community/tutorials/run-tokenizer-on-gpu-for-faster-nlp#what-is-a-tokenizer
[87]: #references-and-resources
[88]: https://aipapersacademy.com/large-concept-models/
[89]: https://github.com/facebookresearch/large_concept_model
[90]: https://arxiv.org/pdf/2412.08821
[91]: https://medium.com/data-science-in-your-pocket/meta-large-concept-models-lcm-end-of-llms-68cb0c5cd5cf
[92]: /products
[93]: /community/users/smukherjee
[94]: /community/users/smukherjee
[95]: /community/tutorials?subtype=tutorial
[96]: /community/tags/data-science
[97]: /community/tags/machine-learning
[98]: /community/questions
[99]: /community
[100]: https://www.digitalocean.com/api/dynamic-content/v1/login?success_redirect=https%3A%2F%2Fwww.digitalocean.com&err
or_redirect=https%3A%2F%2Fwww.digitalocean.com%2Fauth-error&type=register
[101]: https://creativecommons.org/licenses/by-nc-sa/4.0/
[102]: https://cloud.digitalocean.com/registrations/new?refcode=f6fcd01aaffb
[103]: /community/tags/ai-ml
[104]: /community/tags/ubuntu
[105]: /community/tags/linux-basics
[106]: /community/tags/javascript
[107]: /community/tags/python
[108]: /community/tags/mysql
[109]: /community/tags/docker
[110]: /community/tags/kubernetes
[111]: /community/tutorials
[112]: /company/contact/sales?referrer=tutorials
[113]: /community/tutorials/s-o-l-i-d-the-first-five-principles-of-object-oriented-design
[114]: /community/tutorials/how-to-remove-docker-images-containers-and-volumes
[115]: /community/tutorials/how-to-create-a-new-user-and-grant-permissions-in-mysql
[116]: /community/tutorials
[117]: /community/tags
[118]: /community/tutorials/large-concept-models#introduction
[119]: /community/tutorials/large-concept-models#key-takeaways
[120]: /community/tutorials/large-concept-models#from-tokens-to-concepts
[121]: /community/tutorials/large-concept-models#how-do-lcms-work
[122]: /community/tutorials/large-concept-models#why-is-this-approach-better
[123]: /community/tutorials/large-concept-models#the-sonar-embedding-space
[124]: /community/tutorials/large-concept-models#base-lcm-a-naive-approach
[125]: /community/tutorials/large-concept-models#diffusion-based-lcms-better-handling-of-ambiguity
[126]: /community/tutorials/large-concept-models#evaluation-and-results
[127]: /community/tutorials/large-concept-models#installing-and-running-lcms
[128]: /community/tutorials/large-concept-models#limitations
[129]: /community/tutorials/large-concept-models#references-and-resources
[130]: https://cloud.digitalocean.com/registrations/new?refcode=f6fcd01aaffb
[131]: /community/tags/ai-ml
[132]: /community/tags/ubuntu
[133]: /community/tags/linux-basics
[134]: /community/tags/javascript
[135]: /community/tags/python
[136]: /community/tags/mysql
[137]: /community/tags/docker
[138]: /community/tags/kubernetes
[139]: /community/tutorials
[140]: /company/contact/sales?referrer=tutorials
[141]: /community/tutorials/s-o-l-i-d-the-first-five-principles-of-object-oriented-design
[142]: /community/tutorials/how-to-remove-docker-images-containers-and-volumes
[143]: /community/tutorials/how-to-create-a-new-user-and-grant-permissions-in-mysql
[144]: /community/tutorials
[145]: /community/tags
[146]: /community/pages/write-for-digitalocean
[147]: https://docs.digitalocean.com
[148]: /resources
[149]: /products
[150]: https://cloud.digitalocean.com/registrations/new
[151]: /about
[152]: /leadership/executive-management
[153]: /blog
[154]: /careers
[155]: /customers
[156]: /partners
[157]: /referral-program
[158]: /affiliates
[159]: /press
[160]: /legal
[161]: /legal/privacy-policy
[162]: /security
[163]: https://investors.digitalocean.com/
[164]: /products/gpu-droplets
[165]: /products/bare-metal-gpus
[166]: /products/inference-engine
[167]: /data-learning
[168]: /products/model-library
[169]: /products/droplets
[170]: /products/kubernetes
[171]: /products/functions
[172]: /products/app-platform
[173]: /products/load-balancers
[174]: /products/managed-databases
[175]: /products/spaces
[176]: /products/block-storage
[177]: /products/storage/network-file-storage
[178]: https://docs.digitalocean.com/reference/api
[179]: /products/uptime-monitoring
[180]: /products/cloud-security-posture-management
[181]: /products/identity-access-management
[182]: /products/cloudways
[183]: /products
[184]: /community/tutorials
[185]: /community/questions
[186]: https://css-tricks.com/
[187]: /community/pages/write-for-digitalocean
[188]: /currents
[189]: /startups
[190]: /wavemakers
[191]: /research
[192]: /open-source
[193]: /community#iaan
[194]: /products/marketplace
[195]: /pricing
[196]: /pricing/calculator
[197]: https://docs.digitalocean.com/
[198]: https://docs.digitalocean.com/release-notes
[199]: /community/pages/code-of-conduct
[200]: https://store.digitalocean.com/
[201]: /solutions/ai-training-gpu
[202]: /solutions/gpu-inference
[203]: /solutions/vps-hosting
[204]: /solutions/website-hosting
[205]: /solutions/vpn
[206]: /solutions/docker-hosting
[207]: /solutions/nodejs-hosting
[208]: /solutions/web-mobile-apps
[209]: /solutions/wordpress-hosting
[210]: /solutions/virtual-machines
[211]: /solutions
[212]: /support
[213]: /company/contact/sales?referrer=footer
[214]: /company/contact/abuse
[215]: https://status.digitalocean.com/
[216]: https://ideas.digitalocean.com/
[217]: /about
[218]: /leadership/executive-management
[219]: /blog
[220]: /careers
[221]: /customers
[222]: /partners
[223]: /referral-program
[224]: /affiliates
[225]: /press
[226]: /legal
[227]: /legal/privacy-policy
[228]: /security
[229]: https://investors.digitalocean.com/
[230]: /products/gpu-droplets
[231]: /products/bare-metal-gpus
[232]: /products/inference-engine
[233]: /data-learning
[234]: /products/model-library
[235]: /products/droplets
[236]: /products/kubernetes
[237]: /products/functions
[238]: /products/app-platform
[239]: /products/load-balancers
[240]: /products/managed-databases
[241]: /products/spaces
[242]: /products/block-storage
[243]: /products/storage/network-file-storage
[244]: https://docs.digitalocean.com/reference/api
[245]: /products/uptime-monitoring
[246]: /products/cloud-security-posture-management
[247]: /products/identity-access-management
[248]: /products/cloudways
[249]: /products
[250]: /community/tutorials
[251]: /community/questions
[252]: https://css-tricks.com/
[253]: /community/pages/write-for-digitalocean
[254]: /currents
[255]: /startups
[256]: /wavemakers
[257]: /research
[258]: /open-source
[259]: /community#iaan
[260]: /products/marketplace
[261]: /pricing
[262]: /pricing/calculator
[263]: https://docs.digitalocean.com/
[264]: https://docs.digitalocean.com/release-notes
[265]: /community/pages/code-of-conduct
[266]: https://store.digitalocean.com/
[267]: /solutions/ai-training-gpu
[268]: /solutions/gpu-inference
[269]: /solutions/vps-hosting
[270]: /solutions/website-hosting
[271]: /solutions/vpn
[272]: /solutions/docker-hosting
[273]: /solutions/nodejs-hosting
[274]: /solutions/web-mobile-apps
[275]: /solutions/wordpress-hosting
[276]: /solutions/virtual-machines
[277]: /solutions
[278]: /support
[279]: /company/contact/sales?referrer=footer
[280]: /company/contact/abuse
[281]: https://status.digitalocean.com/
[282]: https://ideas.digitalocean.com/
[283]: /sitemap
```
