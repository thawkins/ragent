# Web source

- URL: https://medium.com/google-cloud/a-developers-guide-to-model-routing-1f21ecc34d60
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T15:44:18.908699734+00:00

```text
[Sitemap][1]
[Open in app][2]

Sign up

[Sign in][3]

Get app
[
Write
][4]
[
Search
][5]

Sign up

[Sign in][6]

[Unknown user]
[

## Google Cloud - Community

][7]
·
[
[Google Cloud - Community]
][8]

A collection of technical articles and blogs published or curated by Google Cloud Developer Advocates. The views
expressed are those of the authors and don't necessarily reflect those of Google.

# A Developer’s Guide to Model Routing

[
[Karl Weinmeister]
][9]
[Karl Weinmeister][10]
12 min read
·
Aug 25, 2025
[
][11]

--

1

[
][12]
[][13]
[

Listen

][14]

Share

Press enter or click to view image in full size

Not long ago, building with LLMs meant picking one general-purpose model and sticking with it. Today, the landscape is
flooded with thousands of options: large and small, open and closed-source, generalist and specialist, each with unique
capabilities and costs.

This explosion of choice has fundamentally changed how we build AI applications. The one-size-fits-all approach is over.

Instead, we architect systems that select the best model for each task. This is the idea behind model routing. This
architectural pattern can be implemented today, and has the potential to change the economics of model inference. Let’s
get into it!

## Understanding Model Routing

As a developer building with LLMs, you’re constantly juggling three competing priorities: performance, cost, and
latency.
1. **Performance (Quality):** For complex reasoning and creative generation, you might reach for state-of-the-art models
   like Google’s [Gemini 2.5 Pro][15]. These models deliver high-quality, accurate responses.
2. **Cost:** While premium models deliver state-of-the-art performance, they represent a significant investment. The key
   to a sustainable AI strategy is to reserve these powerful models for tasks where their advanced capabilities provide
   a clear return on investment. For more routine queries, smaller, highly efficient models can deliver excellent
   results at a fraction of the cost. Recent studies show this approach can yield [cost savings][16] without
   significantly degrading performance.
3. **Latency:** In interactive applications like chatbots, a fast response time is critical for a positive user
   experience. Smaller, specialized models can deliver near-instantaneous responses, making them ideal for real-time,
   conversational AI. By routing interactive queries to these faster models, you can create a more engaging and
   responsive application.

Relying on a single model forces an unnecessary compromise. Use a top-tier model for everything, and you pay a premium
for power you don’t always need. Use a smaller model for everything, and you sacrifice quality on complex queries. So
why are we still forcing ourselves to choose just one?

Model routing is an architectural pattern designed to solve this optimization problem. It involves maintaining a pool of
candidate LLMs and routing each incoming prompt to the most suitable model. That’s often the smallest, fastest, and most
cost-effective model that can successfully complete the task.

## Common Routing Patterns

Implementing a model router involves choosing an architectural pattern that determines how routing decisions are made.
These patterns exist on a spectrum of complexity and intelligence, from simple, predefined rules to sophisticated,
AI-driven classification. We will focus on dynamic routing patterns that assess the content, intent, and complexity of
the prompt to select the optimal model.

## Rule-Based Routing

This is the simplest form of dynamic routing. It uses hard-coded logic, typically a series of if/else statements, to
make routing decisions based on simple characteristics of the prompt.

The rules are based on easily measurable attributes of the prompt, such as the presence of certain keywords, its overall
length, or matches against regular expressions. For instance, a system might check for specific terms to identify a task
category or measure the prompt’s length to estimate its complexity.
* **Pros:** This approach is predictable, transparent, and fast to execute. It’s an excellent choice for well-defined,
  simple workflows where task categories can be reliably distinguished by straightforward heuristics.
* **Cons:** Rule-based systems are brittle and inflexible because they lack a true understanding of language. They can
  be easily confused by semantic nuance, such as negation or context. The system also becomes difficult to maintain and
  scale as the number of rules grows.

## LLM-Based Routing

This pattern leverages the intelligence of an LLM to perform the routing task itself. A dedicated, often smaller and
faster, “router LLM” acts as a classification engine.

The user’s prompt is fed into the router LLM. The router LLM is given a prompt that instructs it to analyze the query
and classify it into predefined categories. To ensure the output is machine-readable, the router LLM is instructed to
respond in a structured format like JSON. The application then parses this JSON output to determine which model to call
next.
* **Pros:** This is a powerful and flexible approach. The router LLM can understand complex, ambiguous, and nuanced
  language. It can handle multi-intent queries and can be adapted to new routing tasks simply by updating its system
  prompt.
* **Cons:** The primary drawback is significant overhead. This method introduces an additional, full LLM API call into
  the critical path of every request. This adds both cost and latency, which can undermine the goals of optimization the
  router was intended to achieve.

## Semantic Routing

Semantic routing offers a powerful compromise, combining the speed of rule-based systems with the intelligence of
LLM-based approaches. It operates on the principle of semantic similarity in vector space and is the core mechanism
we’ll implement.

The process involves four steps. First, routes are defined, each with a name and a list of representative example
phrases, or utterances. Next, a text embedding model converts all of these utterances into high-dimensional numerical
vectors that capture their semantic meaning, which are then stored in an efficient index. When a new user query arrives,
the same embedding model converts it into a vector. Finally, a vector similarity search is performed between the query’s
vector and all the utterance vectors in the index, and the route whose utterances are most similar to the query is
selected as the winner.
* **Pros:** This method is fast, with decision times often in the milliseconds, because it relies on optimized vector
  math rather than a slow, generative LLM call. It’s highly scalable to thousands of potential routes and is more robust
  than simple keyword matching because it understands meaning and context. Modern libraries often allow this
  configuration to be externalized into declarative files like YAML, separating the routing logic from the application
  code for better maintainability.
* **Cons:** The effectiveness of a semantic router is highly dependent on the quality and comprehensiveness of the
  example utterances provided for each route. It can also struggle with contextual, multi-turn conversational queries
  where the user’s intent is not explicitly stated in their most recent message.

The choice of routing architecture is governed by the “Router Latency Paradox”: a component designed to reduce overall
application latency must itself be exceptionally low-latency. An LLM-based router introduces a full inference step to
every request, increasing both latency and cost. For this approach to be a net positive, the downstream savings must
consistently outweigh its operational overhead, which is a high bar for most interactive applications. Semantic routing,
in contrast, replaces this slow inference with a near-instantaneous vector search. This performance difference
establishes semantic routing as the default architectural best practice for dynamic, real-time model routing. LLM-based
routing is thus reserved for cases where the routing logic is too complex to be captured by semantic similarity alone
and the added latency is an acceptable trade-off.

## The Gemini 2.5 Model Family

To build an effective router, you need a solid grasp of the candidate models in your pool. For our implementation, we’ll
use Google’s Gemini 2.5 family, a suite of models with a tiered structure of capability and cost that’s perfect for a
routing architecture.

A key innovation across the Gemini 2.5 family is their capability as “thinking models.” This means they can be
configured to perform internal reasoning steps, akin to a chain of thought, before generating a final response. This
feature, controllable via an API parameter known as the “thinking budget,” can significantly improve performance and
accuracy on complex tasks. This controllable reasoning becomes another powerful dimension for our routing logic to
consider.

## Gemini 2.5 Pro
* **Capabilities:** [Gemini 2.5 Pro][17] is Google’s flagship model, engineered for maximum performance and
  state-of-the-art accuracy. It’s optimized for the most complex and demanding tasks, including deep logical reasoning,
  advanced code generation, and sophisticated multimodal understanding across text, images, audio, and video.
* **Router Use Case:** This is our designated **“strong” model**. We’ll route only the most challenging queries here:
  prompts that involve complex problem-solving, novel algorithm design, in-depth analysis of dense technical documents,
  or multi-step logical puzzles.
* **Thinking:** For this model, the “thinking” capability is on by default, as it’s integral to its high-end
  performance.

## Gemini 2.5 Flash
* **Capabilities:** [Gemini 2.5 Flash][18] is designed to be the best model in the family in terms of its
  price-to-performance ratio. It offers well-rounded, powerful capabilities that approach those of Pro but at a
  significantly lower operational cost. It also features a controllable thinking budget.
* **Router Use Case:** This is our **“default” or “go-to” model**. It’s the workhorse that will handle the majority of
  general-purpose queries. These are tasks that are more complex than simple classification but don’t require the full
  power (and expense) of Pro. Ideal use cases include general conversation, creative writing, drafting emails, and
  performing detailed summarizations.

## Gemini 2.5 Flash-Lite
* **Capabilities:** As its name suggests, [Gemini 2.5 Flash-Lite][19] is the fastest and most cost-efficient model in
  the 2.5 family. It’s highly optimized for low latency and high-throughput scenarios, making it a cost-effective
  upgrade from previous generations of Flash models.
* **Router Use Case:** This is our **fastest model**. We’ll route simple, high-volume, and latency-sensitive tasks here.
  It’s perfect for text classification (e.g., sentiment analysis), simple data extraction (e.g., pulling names and dates
  from text), translation, and answering straightforward factual questions.
* **Thinking:** To maximize its speed and cost-efficiency, “thinking” is turned off by default for Flash-Lite. However,
  it can be optionally enabled, providing granular control for tasks that might need a small boost in reasoning without
  escalating to the full Flash model.

## Implementing a Semantic Router

With the theory covered, let’s get to the code. This section walks through the [gemini-model-router][20] project, which
builds a semantic router to intelligently distribute queries among the Gemini 2.5 Pro, Flash, and Flash-Lite models. It
uses the open-source [semantic-router][21] library as its engine and serves it all up with [FastAPI][22].

Press enter or click to view image in full size
Embeddings are created upfront for each route, and then matched to queries at runtime

## Project Setup

To get started, clone the repository and follow the setup instructions in the `[README.md][23]` file, which covers
creating the `.env` file and installing the required dependencies from `requirements.txt`.

## Centralizing Configuration

A key architectural decision in the `gemini-model-router` project is the separation of configuration from code. All
routing logic, including the routes, their representative utterances, and the specific LLM assigned to each route, is
defined in a single `[router.yaml][24]` file. This makes the system highly maintainable and easy to modify without
changing the application’s Python code.

The `router.yaml` file has two main sections:
1. **encoder**: Specifies the embedding model to use for converting text to vectors. In this case, it uses Google’s
   `[gemini-embedding-001][25]` via the semantic-router’s `[GoogleEncoder][26]`.
2. **routes**: A list of route definitions. Each route has:
* `name`: A unique identifier that maps directly to a Gemini model.
* `description`: A human-readable explanation of the route’s purpose.
* `utterances`: A list of example phrases that define the semantic space of the route.
* `llm`: An object specifying the custom class (`GoogleLLM`), the Python module where it’s defined (`main`), and the
  target model ID (e.g., `gemini-2.5-pro`).

Here is a snippet from the router.yaml file, defining the route for complex queries. A key parameter in the full
configuration is the `score_threshold`. When the router compares a query to its routes, it calculates a similarity
score. By setting the threshold to 0.0, we ensure that the router always selects the route with the highest similarity,
effectively guaranteeing that a decision is always made.

# router.yaml
encoder_name: gemini-embedding-001
encoder_type: google
routes:
- name: gemini-2.5-pro
  description: For complex, multi-step tasks requiring deep reasoning, code generation, and analysis of large documents.
  utterances:
  - Develop a comprehensive, multi-year business plan for a direct-to-consumer sustainable
    fashion brand, including financial projections and marketing strategies.
  - Write a Python script to perform sentiment analysis on a large CSV of customer
    reviews, generate visualizations, and create a summary report.
  - Compare and contrast the philosophical implications of determinism and free will
    in the context of advanced artificial intelligence, citing relevant academic sources.
  llm:
    module: main
    class: GoogleLLM
    model: gemini-2.5-pro
#... other routes for flash and flash-lite follow...

## Routing Logic

The `main.py` file contains the FastAPI application that serves the router. It includes several key components that work
together to bring the YAML configuration to life.

### The GoogleLLM Wrapper

The semantic-router library requires a compatible LLM object for each route. To integrate with Google’s GenAI SDK, the
project defines a custom `GoogleLLM` class that inherits from `[semantic_router.llms.BaseLLM][27]`. This class acts as a
bridge, translating the semantic-router’s call signature into an asynchronous request to the Vertex AI Gemini API.

# main.py (simplified)
from semantic_router.llms import BaseLLM
from google import genai

class GoogleLLM(BaseLLM):
    _client: ClassVar[Optional[genai.Client]] = None

    @classmethod
    def get_client(cls) -> genai.Client:
        if cls._client is None:
            project_id = os.getenv("GOOGLE_CLOUD_PROJECT")
            cls._client = genai.Client(vertexai=True, project=project_id)
        return cls._client

    async def __acall__(self, messages: List[Message], **kwargs) -> Optional[str]:
        contents = kwargs.get("multimodal_contents", messages[0].content)
        config = kwargs.get("config", self.kwargs.get("config", {}))

        response = await self.get_client().aio.models.generate_content(
            model=self.name,
            contents=contents,
            **config,
        )
        return response.text if response else ""

### The /query Endpoint

The main API endpoint uses a series of helper functions to route and execute the query. The `handle_query` function
orchestrates the process: it extracts text for routing, determines the best route, and executes the LLM call.

# main.py (simplified)
@app.post("/query", response_model=RouterResponse)
async def handle_query(request: QueryRequest, fastapi_request: Request):
    router = fastapi_request.app.state.router
    default_route = fastapi_request.app.state.default_route_name

    # 1. Extract text and determine the route
    text_for_routing = _get_text_for_routing(request.contents)
    route_choice = _determine_route(router, text_for_routing, default_route)
    chosen_route = router.get(route_choice.name)

    # 2. Execute the call using the LLM from the chosen route
    model_response = await _execute_llm_call(
        chosen_route, request.contents, request.config, text_for_routing
    )

    return RouterResponse(
        route_name=chosen_route.name, model_response=model_response
    )

## Deploying to Production

While FastAPI’s web server `[uvicorn][28]` is perfect for local development, a production deployment requires a robust,
scalable hosting environment. [Cloud Run][29] is an ideal choice for this service because it’s a fully managed,
serverless platform that takes your containerized application (including the Uvicorn server) and handles all the
underlying infrastructure, scaling, and request management.

To deploy the router, you first need to have the Google Cloud SDK installed and configured. Then, you can deploy the
service with a single command:

gcloud run deploy gemini-model-router \
  --source . \
  --region us-central1

This command builds a container from your source code, pushes it to the Artifact Registry, and deploys it as a
public-facing service. Cloud Run handles all the infrastructure, so you can focus on the application logic.

## Production Best Practices

Deploying a model router to production requires building an observable and resilient system. An API management platform
like Google Cloud’s [Apigee][30] can serve as a unified and secure gateway to your model routing service. It can provide
essential capabilities like enforcing security policies, managing traffic with rate limiting and quotas, and offering
deep visibility through analytics and monitoring. Let’s review the key principles needed to move beyond a
proof-of-concept.

First, treat the router as a mission-critical, standalone service. Because it can be a single point of failure and a
performance bottleneck, it must be independently scalable and fault-tolerant. Containerize the router and deploy it on a
platform like [Cloud Run][31] to ensure high availability, allowing it to scale independently of the applications that
consume it.

Second, you cannot optimize what you cannot measure. Implement comprehensive logging and monitoring for every routing
decision. For each request, log the chosen route, similarity score, final model, latency, and estimated cost. This data
can be fed into [Google Cloud’s observability suite][32] to create dashboards for tracking key performance indicators
like route distribution, cost per query, and P99 latency. This allows you to set up alerts for anomalies, such as a
sudden shift in routing patterns or an increase in fallback rates.

Third, the initial configuration is just a starting point. True optimization requires a data-driven feedback loop.
Collect and review production queries to identify misrouted requests, and use this analysis to refine your route
utterances. A/B testing frameworks are invaluable for comparing different routing strategies or model configurations in
a live environment to validate improvements.

Finally, enterprise-grade reliability requires planning for failure. Implement a chain of fallbacks that goes beyond a
simple default route. For instance, if a request to `gemini-2.5-pro` fails, the system should automatically retry with
exponential backoff. If that also fails, it should fall back to the next best model, `gemini-2.5-flash`.

## The Future of Model Routing

There is a broader trend towards more modular and dynamic AI architectures, and model routing is no exception. The
future of model routing could include:
* **Multimodal Routing:** The next logical step is routing on more than just text. The current router simplifies the
  problem by extracting the text from a multimodal prompt, but the concept of vector similarity works for any modality
  you can embed.
* **Hierarchical Routing:** The concept of system-level model routing is a macro-scale analog of what
  [Mixture-of-Experts][33] or MoE architectures do within a single neural network. In an MoE model, an internal “router”
  network dynamically selects which “expert” sub-networks should process each token of an input sequence. Our external
  router does the same thing, but its “experts” are entire, independent LLMs. Future systems may employ hierarchical
  routing, where a top-level semantic router first selects the best specialized MoE model for a task, which then
  performs its own fine-grained, internal routing to process the request.

Ultimately, model routing is a foundational building block for the next generation of complex, multi-agent AI systems.
As we’ve shown, the combination of a powerful model family like [Google’s Gemini 2.5][34], a serverless platform like
[Cloud Run][35], and the open-source [gemini-model-router][36] project makes this advanced architecture an achievable
engineering task. The tools are here. The patterns are clear.

It’s time to start building. Share what you’ve built with me on [LinkedIn][37], [X][38], or [Bluesky][39]!

[
Routing
][40]
[
Large Language Models
][41]
[
Gemini
][42]
[
Google Cloud Run
][43]
[
Machine Learning
][44]
[
][45]

--

[
][46]

--

1

[
][47]
[][48]
[
[Google Cloud - Community]
][49]
[
[Google Cloud - Community]
][50]
[

## Published in Google Cloud - Community

][51]
[76K followers][52]
·[Last published 5 hours ago][53]

A collection of technical articles and blogs published or curated by Google Cloud Developer Advocates. The views
expressed are those of the authors and don't necessarily reflect those of Google.

[
[Karl Weinmeister]
][54]
[
[Karl Weinmeister]
][55]
[

## Written by Karl Weinmeister

][56]
[1.2K followers][57]
·[52 following][58]

Developer Relations @ Google Cloud

[

Help

][59]
[

Status

][60]
[

About

][61]
[

Careers

][62]
[

Press

][63]
[

Blog

][64]
[

Store

][65]
[

Privacy

][66]
[

Rules

][67]
[

Terms

][68]
[

Text to speech

][69]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f
21ecc34d60&source=post_page---top_nav_layout_nav-----------------------global_nav------------------
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f
21ecc34d60&source=post_page---top_nav_layout_nav-----------------------global_nav------------------
[7]: https://medium.com/google-cloud?source=post_page---publication_nav-e52cf94d98af-1f21ecc34d60-----------------------
----------------
[8]: https://medium.com/google-cloud?source=post_page---post_publication_sidebar-e52cf94d98af-1f21ecc34d60--------------
-------------------------
[9]: /@kweinmeister?source=post_page---byline--1f21ecc34d60---------------------------------------
[10]: /@kweinmeister?source=post_page---byline--1f21ecc34d60---------------------------------------
[11]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fgoogle-cloud%2F1f21ecc34d60&operation=register&redirect=
https%3A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&user=Karl+Weinmeister&userId=
56d387899b8b&source=---header_actions--1f21ecc34d60---------------------clap_footer------------------
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F1f21ecc34d60&operation=register&redirect=https%3A%
2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&user=Karl+Weinmeister&userId=56d387899
b8b&source=---header_actions--1f21ecc34d60---------------------repost_header------------------
[13]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F1f21ecc34d60&operation=register&redirect=https%3
A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&source=---header_actions--1f21ecc34d
60---------------------bookmark_footer------------------
[14]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D1f21ecc34d60&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&source
=---header_actions--1f21ecc34d60---------------------post_audio_button------------------
[15]: https://cloud.google.com/vertex-ai/generative-ai/docs/models/gemini/2-5-pro?utm_campaign=CDR_0x2b6f3004_user-journ
ey_b440933914&utm_medium=external&utm_source=blog
[16]: https://lmsys.org/blog/2024-07-01-routellm/
[17]: https://cloud.google.com/vertex-ai/generative-ai/docs/models/gemini/2-5-pro?utm_campaign=CDR_0x2b6f3004_user-journ
ey_b440933914&utm_medium=external&utm_source=blog
[18]: https://cloud.google.com/vertex-ai/generative-ai/docs/models/gemini/2-5-flash?utm_campaign=CDR_0x2b6f3004_user-jou
rney_b440933914&utm_medium=external&utm_source=blog
[19]: https://cloud.google.com/vertex-ai/generative-ai/docs/models/gemini/2-5-flash-lite?utm_campaign=CDR_0x2b6f3004_use
r-journey_b440933914&utm_medium=external&utm_source=blog
[20]: https://github.com/kweinmeister/gemini-model-router
[21]: https://github.com/aurelio-labs/semantic-router
[22]: https://fastapi.tiangolo.com/
[23]: http://readme.md
[24]: https://github.com/kweinmeister/gemini-model-router/blob/main/router.yaml
[25]: https://console.cloud.google.com/vertex-ai/publishers/google/model-garden/gemini-embedding-001?utm_campaign=CDR_0x
2b6f3004_user-journey_b440933914&utm_medium=external&utm_source=blog
[26]: https://docs.aurelio.ai/semantic-router/client-reference/encoders/google
[27]: https://docs.aurelio.ai/semantic-router/client-reference/llms/base
[28]: https://www.uvicorn.org/
[29]: https://cloud.google.com/run?utm_campaign=CDR_0x2b6f3004_user-journey_b440933914&utm_medium=external&utm_source=bl
og
[30]: https://cloud.google.com/apigee/api-management?utm_campaign=CDR_0x2b6f3004_user-journey_b440933914&utm_medium=exte
rnal&utm_source=blog
[31]: https://cloud.google.com/run?utm_campaign=CDR_0x2b6f3004_user-journey_b440933914&utm_medium=external&utm_source=bl
og
[32]: https://cloud.google.com/observability?utm_campaign=CDR_0x2b6f3004_user-journey_b440933914&utm_medium=external&utm
_source=blog
[33]: https://huggingface.co/blog/moe
[34]: https://cloud.google.com/vertex-ai/generative-ai/docs/models?utm_campaign=CDR_0x2b6f3004_user-journey_b440933914&u
tm_medium=external&utm_source=blog
[35]: https://cloud.google.com/run?utm_campaign=CDR_0x2b6f3004_user-journey_b440933914&utm_medium=external&utm_source=bl
og
[36]: https://github.com/kweinmeister/gemini-model-router
[37]: https://www.linkedin.com/in/karlweinmeister/
[38]: https://x.com/kweinmeister
[39]: https://bsky.app/profile/kweinmeister.bsky.social
[40]: /tag/routing?source=post_page-----1f21ecc34d60---------------------------------------
[41]: /tag/large-language-models?source=post_page-----1f21ecc34d60---------------------------------------
[42]: /tag/gemini?source=post_page-----1f21ecc34d60---------------------------------------
[43]: /tag/google-cloud-run?source=post_page-----1f21ecc34d60---------------------------------------
[44]: /tag/machine-learning?source=post_page-----1f21ecc34d60---------------------------------------
[45]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fgoogle-cloud%2F1f21ecc34d60&operation=register&redirect=
https%3A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&user=Karl+Weinmeister&userId=
56d387899b8b&source=---footer_actions--1f21ecc34d60---------------------clap_footer------------------
[46]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fgoogle-cloud%2F1f21ecc34d60&operation=register&redirect=
https%3A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&user=Karl+Weinmeister&userId=
56d387899b8b&source=---footer_actions--1f21ecc34d60---------------------clap_footer------------------
[47]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F1f21ecc34d60&operation=register&redirect=https%3A%
2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&user=Karl+Weinmeister&userId=56d387899
b8b&source=---footer_actions--1f21ecc34d60---------------------repost_footer------------------
[48]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F1f21ecc34d60&operation=register&redirect=https%3
A%2F%2Fmedium.com%2Fgoogle-cloud%2Fa-developers-guide-to-model-routing-1f21ecc34d60&source=---footer_actions--1f21ecc34d
60---------------------bookmark_footer------------------
[49]: https://medium.com/google-cloud?source=post_page---post_publication_info--1f21ecc34d60----------------------------
-----------
[50]: https://medium.com/google-cloud?source=post_page---post_publication_info--1f21ecc34d60----------------------------
-----------
[51]: https://medium.com/google-cloud?source=post_page---post_publication_info--1f21ecc34d60----------------------------
-----------
[52]: /google-cloud/followers?source=post_page---post_publication_info--1f21ecc34d60------------------------------------
---
[53]: /google-cloud/securing-data-in-transit-enforcing-tls-ssl-the-baseline-defense-3df64a759853?source=post_page---post
_publication_info--1f21ecc34d60---------------------------------------
[54]: /@kweinmeister?source=post_page---post_author_info--1f21ecc34d60---------------------------------------
[55]: /@kweinmeister?source=post_page---post_author_info--1f21ecc34d60---------------------------------------
[56]: /@kweinmeister?source=post_page---post_author_info--1f21ecc34d60---------------------------------------
[57]: /@kweinmeister/followers?source=post_page---post_author_info--1f21ecc34d60---------------------------------------
[58]: /@kweinmeister/following?source=post_page---post_author_info--1f21ecc34d60---------------------------------------
[59]: https://help.medium.com/hc/en-us?source=post_page-----1f21ecc34d60---------------------------------------
[60]: https://status.medium.com/?source=post_page-----1f21ecc34d60---------------------------------------
[61]: /about?autoplay=1&source=post_page-----1f21ecc34d60---------------------------------------
[62]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----1f21ecc34d60-------------------------------------
--
[63]: mailto:pressinquiries@medium.com
[64]: https://blog.medium.com/?source=post_page-----1f21ecc34d60---------------------------------------
[65]: https://medium.com/store
[66]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----1f21ecc34d60--------------------
-------------------
[67]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----1f21ecc34d60-----------------------------
----------
[68]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----1f21ecc34d60------------------
---------------------
[69]: https://speechify.com/medium?source=post_page-----1f21ecc34d60---------------------------------------
```
