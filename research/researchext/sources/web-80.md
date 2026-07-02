# Web source

- URL: https://www.oakslab.com/story/a-practical-guide-to-building-agentic-ai-products
- Title: [
- Captured (UTC): 2026-06-30T09:42:11.043474048+00:00

```text
[
][1]
Company
[

About

Learn more about our company

][2][

Careers

See all open positions

][3]
[Case studies][4]
Services
[

Our services

][5][

Product discovery & design

][6][

Software development

][7][

AI & agentic systems

][8]
Insights
[

Blog

Read our latest articles

][9][

Newsletter

View quarterly updates

][10][

**Knowledge Hub**

Explore practical resources

][11]
[
Get in touch
][12]
[
][13]

Technology

Product Development

January 9, 2025

# A Practical Guide to Building Agentic AI Products

## Agentic AI systems transform workflows by using specialized agents to achieve customized, efficient, and scalable
## solutions. At OAK'S LAB, we design these systems to help businesses adapt and thrive in an AI-driven world.

[A Practical Guide to Building Agentic AI Products]

Andy Powell

Partner & COO

*It’s the buzzword on everyone’s lips in the tech ecosystem. Startups, enterprises, and investors are all clamouring to
understand what “Agentic AI” is, and what it means for the future of business. *

*But for those not in the technical detail, the perception of these systems might be as simple as making an API call to
a magical "intelligence" that handles everything. In reality, AI-driven applications are far more intricate, often
involving multiple specialized models and systems working in tandem to achieve specific goals. In this article, we’ll
break down what Agentic AI, or a “Multi-Agent System”  is, and how we think about designing and building these types of
products.*

### **Why Agentic Systems Are Now a Core Focus at OAK'S LAB**

At OAK'S LAB, we’ve had the opportunity to work on AI-enabled products for startups and enterprises across **HRTech**,
**HealthTech**, **FinTech**, **EdTech**, and other industries. Many of the products we’ve built rely on multi-agent
workflows to solve a business problem. These systems can be particularly valuable for companies looking to optimize any
kind of multi-step process such as analyzing documents, automating customer support, or scheduling appointments. 

As businesses evolve, we see that there may be a growing shift from traditional SaaS platforms to custom-built agentic
systems. Klarna, for instance, has built an agentic AI system for their customer service operations which is doing the
equivalent work of 700 full-time customer service staff and driving $40M in profit improvement. While SaaS platforms
remain valuable for standardizing processes, as agentic systems improve they will allow businesses to create highly
customized workflows that adapt to their specific business processes. This can reduce overreliance on third-party SaaS
vendors, and create more cost-effective, business-specific tools. 

### **What Is a Multi-Agent System?**

[Multi-agent system architecture — interacting autonomous agents]

A **multi-agent system **consists of multiple autonomous entities (agents) that interact and collaborate to achieve
individual or shared objectives. Unlike single-agent systems, where one model handles everything, they distribute
responsibilities across specialized agents, each designed to handle a specific subtask.

As with most functions or processes within an application, you can think of these systems as taking inputs, processing
them, and generating some output which ultimately provides some value to the user. 

### **Example of a Multi-Agent System**

[Example: recruitment multi-agent workflow diagram]

Imagine a recruitment app that conducts and evaluates candidate interviews. With a multi-agent system, the product
doesn’t just follow a static sequence of steps—it actively works together to achieve the desired requirements. Here's
how it might work: 

**1. Voice Agent**:
* Converts spoken responses into text using a speech-to-text model.
* Communicates with the Language Detection Agent if unclear audio is detected.**‍**
* **Model Example**: [**Deepgram**][14]

**2. Language Detection Agent**:
* Identifies the language of the candidate’s speech and ensures the Voice Agent uses the appropriate model.**‍**
* **Model Example**: [**XLM-RoBERTa**][15]

**3. Sentiment Analysis Agent**:
* Evaluates tone, confidence, and emotional context of the candidate’s responses.
* Shares its findings with the Summarization Agent, e.g., “Candidate seemed confident about leadership experience.”**‍**
* **Model Example**: [**roBERTa-based Sentiment**][16]

**4. Summarization Agent**:
* Summarizes the candidate’s responses into concise, structured notes for recruiters.
* Asks the Voice Agent for clarification if it detects incomplete or ambiguous text.**‍**
* **Model Example**: [**Llama 3.3**][17]

**5. Recommendation Agent**:
* Matches the candidate’s skills and experience to relevant job openings.
* Dynamically adjusts recommendations based on recruiter preferences provided by the Coordination Agent.**‍**
* **Model Example**: [**Llama 3.3**][18]

**6. Coordination or Supervisor Agent**:
* Oversees the entire process, ensuring agents collaborate efficiently.
* For example, if the Sentiment Analysis Agent detects hesitation about a specific skill, the Coordination Agent prompts
  the Summarization Agent to highlight this in its output.**‍**
* **Model Example: **[**Llama 3.3**][19]

### **The Challenges of Agentic Systems**

While agentic systems hold immense potential, they come with significant challenges: 

#### **1. Non-Determinism**:

LLMs are inherently non-deterministic rather than probabilistic, meaning their behavior can vary based on inputs and
internal states. This unpredictability can cause cascading errors, where a mistake in one agent leads to compounding
issues across the workflow. To mitigate this, these products often require either human oversight or deterministic
fallback mechanisms, especially as workflows scale in complexity.

#### **2. Coordination Complexity**:

As more agents are added to a system, ensuring seamless collaboration becomes increasingly difficult. The orchestrator
plays a vital role here, but the design must minimize bottlenecks and handle failures dynamically.

#### **3. Data and Model Dependency**:

Errors stemming from poor data or model misbehavior can propagate throughout the system. For example, if the Sentiment
Analysis Agent misinterprets a response, the Summarization and Recommendation Agents may produce flawed outputs.

#### **4. Trust and Transparency**:

For businesses to trust agentic systems, they need visibility into how decisions are made. This requires robust
monitoring, logging, and explanation mechanisms to ensure accountability.

### **The Importance of Data & Discovery**

Before building an agentic system, two foundational steps are essential: ensuring high-quality data and conducting
proper business discovery.

#### **1. Business Discovery**:

A clear understanding of business processes is crucial. At OAK'S LAB, we emphasize deep discovery to identify key user
problems and determine whether a multi-agent system is the right solution. This stage avoids "building AI Agents just
because we can." Instead, we ensure that the system aligns with genuine business needs and provides tangible value. Not
every problem is suited for an agentic approach, particularly those requiring absolute determinism or straightforward,
repeatable processes.

#### **2. Data Quality**:

High-quality data is the backbone of any agentic system. Poor data leads to cascading errors as agents rely on one
another’s outputs. For example, a sentiment analysis agent’s inaccurate result might skew the behavior of a
recommendation agent, leading to flawed outcomes. Ensuring data accuracy, relevance, and structure minimizes such risks
and makes the system more reliable. 

### **The Importance of Orchestration**

Orchestration is at the heart of any effective multi-agent system. Orchestration acts as the "manager" of the system,
making sure all the individual agents work together smoothly, tasks are handled in the right order, and any errors are
caught before they cause bigger problems. A strong orchestrator relies on clear, rules-based workflows to keep things
structured and predictable while still allowing agents to adapt to new inputs as they come in. Tools like **LangChain**
and **LangGraph** make setting up these workflows easier, helping to define task sequences, manage dependencies, and
create backup plans when something goes wrong. With a rules-based approach, you get better control over the system,
ensuring that even as things get more complex, the process stays organized and aligned with what your business needs.

### **Building a Multi-Agent System: A General Framework**

[Framework for designing and implementing agentic systems]

Designing and implementing a multi-agent system is a structured, iterative process that ensures the solution addresses
real business needs. Below is a generalized approach applicable across industries and workflows.

#### **Step 1: Conduct As-Is Process Analysis**

Begin by thoroughly mapping out the current state of the workflow or process you intend to automate or enhance. It’s
crucial to understand how a *human* would complete the task before automating with an agent.
* **What to do**:
  * Identify all tasks in the process, their dependencies, and key inputs and outputs.
  * Understand existing inefficiencies, pain points, and bottlenecks.
  * Document all stakeholders and their roles in the process.
* **Key outcomes**:
  * A clear understanding of the process’s limitations (e.g., time-intensive tasks, inconsistent outcomes, or
    scalability issues).
  * Insight into whether an agentic workflows is a viable solution, as not all processes require this solution.

#### **Step 2: Define the To-Be Process**

Using the insights from the as-is analysis, design the desired future state of the workflow where multi-agent system
plays a role. 
* **What to do**:
  * Define the outcomes you want the workflow to achieve (e.g., faster processing, higher accuracy, improved user
    experience).
  * Break down the workflow into discrete tasks that agents will handle. For example:some text
    * Data collection
    * Analysis and decision-making
    * Communication or reporting
  * Identify where human oversight or intervention may still be necessary.
* **Key outcomes**:
  * A detailed map of the "to-be" process, including which tasks are automated and which remain manual.
  * Clear success metrics (e.g., speed, cost savings, accuracy improvements).

#### **Step 3: Design the Orchestration Framework**

The orchestrator, the system’s “conductor,” ensures all agents collaborate effectively and that workflows remain
coherent even as complexity grows.
* **What to do**:
  * Sequence workflows by defining the flow of tasks between agents and identifying dependencies.
  * Implement rules for conflict resolution, such as how the system will handle ambiguous inputs or agent failures.
  * Design fallback mechanisms and recovery protocols to prevent cascading errors.
  * Enable adaptability by allowing the orchestrator to make decisions dynamically, such as re-routing tasks or
    triggering additional agents as needed.
* **Key outcomes**:
  * A robust orchestration framework outlining agent interactions, triggers, and responses.
  * Flexibility to add, remove, or modify agents without disrupting the workflow.

#### **Step 4: Select and Configure Agents**

Agents are the building blocks of an an agentic system, each designed to handle a specific task. At this stage,
determine what types of agents are needed and define their roles.
* **What to do**:
  * Categorize tasks by complexity and determine whether they require rule-based automation, simple AI models, or
    advanced decision-making agents.
  * Configure agents to perform specific functions, such as:some text
    * Data ingestion or preprocessing.
    * Task-specific analyses (e.g., sentiment analysis, summarization, categorization).
    * Output generation or recommendations.
  * Ensure each agent has clear input and output expectations to integrate seamlessly into the workflow.
* **Key outcomes**:
  * A comprehensive list of agents with defined roles and responsibilities.
  * Clear communication protocols between agents.

#### **Step 5: Test the System**

Testing ensures that the product performs as expected and identifies areas for improvement before deployment.
* **What to do**:
  * Conduct **unit testing** to verify that individual agents function correctly.
  * Perform **integration testing** to ensure agents communicate and collaborate as designed.
  * Simulate real-world scenarios, including edge cases, to evaluate system performance under different conditions.
  * Stress-test the system to assess scalability and identify potential bottlenecks.
* **Key outcomes**:
  * A validated product ready for deployment with known performance benchmarks.
  * Documentation of potential weaknesses and areas for further optimization.

#### **Step 6: Implement Feedback Loops**

Once the system is live, continuous improvement is essential for maintaining efficiency and relevance.
* **What to do**:
  * Collect feedback from end-users to identify issues or suggest enhancements.
  * Monitor system performance metrics, such as processing speed, error rates, and task completion accuracy.
  * Optimize agent behavior through:some text
    * **Prompt refinement**: Adjust prompts or configurations for improved outputs.
    * **Rule updates**: Modify orchestrator logic to address recurring issues.
    * **Agent retraining**: Update models based on new data or changing requirements.
* **Key outcomes**:
  * A dynamic system that evolves and improves over time.
  * A feedback-driven process for refining workflows, agents, and orchestration.

### **Conclusion**

*The ecosystem of Agentic AI tools is evolving rapidly, with new frameworks, tools, and methodologies emerging almost
daily. At OAK'S LAB, we remain committed to refining our processes, deepening our expertise, and staying at the
forefront of this exciting field.*

*The future of agentic AI is already taking shape. Businesses that embrace these systems today will be equipped to lead
and innovate in an increasingly AI-driven world.*

***Ready to explore how agentic AI and multi-agent systems can transform your business? Let’s start the conversation.***

### Subscribe to our newsletter and receive the latest updates from our CEO.

Thank you! Your submission has been received!
Oops! Something went wrong while submitting the form.

## Top articles

[[Monolith to modular transformation — engineering teams scaling structure illustration]][20]
[

### From Monoliths to Modular: How to Structure Engineering Teams as You Scale

][21]

July 23, 2025

Scaling tech companies often hit bottlenecks as old engineering structures slow progress. This guide covers when to
restructure, proven team models, and key principles to scale effectively.

[[Practical guide to building agentic AI systems illustration]][22]
[

### A Practical Guide to Building Agentic AI Products

][23]

January 9, 2025

Agentic AI systems transform workflows by using specialized agents to achieve customized, efficient, and scalable
solutions. At OAK'S LAB, we design these systems to help businesses adapt and thrive in an AI-driven world.

[[Building AI-Powered Products Without ML Teams]][24]
[

### Building AI-Powered Products Without ML Teams

][25]

October 15, 2024

Learn how to build AI-powered products without a dedicated machine learning team. This article outlines practical steps
for integrating AI using pre-built models, improving your product's personalization, automation, and efficiency with
minimal investment.

[[The OAK’S LAB WAY: An Introduction to Our Product Development Methodology]][26]
[

### The OAK’S LAB WAY: An Introduction to Our Product Development Methodology

][27]

January 23, 2023

Our product development methodology designed to take an early-stage startup from pre-seed to series A and beyond.

[[How to Set Your Startup’s Mission & Vision]][28]
[

### How to Set Your Startup’s Mission & Vision

][29]

March 14, 2023

The twin north stars navigating startup founders and our teams on the journey to build a product that makes a big impact
in the world.

[[Building Products with LLMs | 7 Tips for Success]][30]
[

### Building Products with LLMs | 7 Tips for Success

][31]

August 1, 2023

Over the last few months, we’ve been fortunate enough to work with some startups that have LLM-driven products. Here are
the best practices that we’ve learned along the way.

[[The Ultimate 7-Slide Formula for a Winning Startup Pitch Deck]][32]
[

### The Ultimate 7-Slide Formula for a Winning Startup Pitch Deck

][33]

May 24, 2023

Securing a seed investment is great validation and a big boost for early-stage startups; however, for first-time
founders navigating the venture capital world isn’t easy. A well-crafted pitch deck can open investor doors and become
the starting point from which an investment is made.

[
][34][
][35]

## Product, engineering and AI partner for high-growth companies.

[Get in touch][36]

OAK'S LAB

[About us][37][Careers][38]

BUILD

[Our services][39][Case studies][40]

Learn

[Blog][41][Newsletter][42][Knowledge hub][43]

Follow us

[
][44][
][45][
][46][
][47]

Contact us

[hello@oakslab.com][48]

ISO 27001 Certified

[

CLUTCH Average RATING

5 / 5

][49]

## Subscribe to our quarterly newsletter and receive our latest insights.

Thank you! Your submission has been received!
Oops! Something went wrong while submitting the form.

© 2026 OAK'S LAB

[Cookies][50][Privacy Policy][51][ISMS Policy][52]

[1]: /
[2]: /about
[3]: /careers
[4]: /case-studies
[5]: /services
[6]: /product-discovery-design
[7]: /custom-software-development
[8]: /ai-agentic-systems
[9]: /blog
[10]: /newsletter
[11]: /knowledge-hub
[12]: /book-a-call
[13]: /blog
[14]: https://deepgram.com/
[15]: https://huggingface.co/docs/transformers/en/model_doc/xlm-roberta
[16]: https://huggingface.co/cardiffnlp/twitter-roberta-base-sentiment
[17]: https://www.llama.com/docs/model-cards-and-prompt-formats/llama3_3/
[18]: https://www.llama.com/docs/model-cards-and-prompt-formats/llama3_3/
[19]: https://www.llama.com/docs/model-cards-and-prompt-formats/llama3_3/
[20]: /story/from-monoliths-to-modular-how-to-structure-engineering-teams-as-you-scale
[21]: /story/from-monoliths-to-modular-how-to-structure-engineering-teams-as-you-scale
[22]: /story/a-practical-guide-to-building-agentic-ai-products
[23]: /story/a-practical-guide-to-building-agentic-ai-products
[24]: /story/building-ai-powered-products-without-ml-teams
[25]: /story/building-ai-powered-products-without-ml-teams
[26]: /story/product-development-methodology
[27]: /story/product-development-methodology
[28]: /story/how-to-set-your-startups-mission-and-vision
[29]: /story/how-to-set-your-startups-mission-and-vision
[30]: /story/building-products-with-llms-7-tips-for-success
[31]: /story/building-products-with-llms-7-tips-for-success
[32]: /story/7-slide-formula-for-a-winning-startup-pitch-deck
[33]: /story/7-slide-formula-for-a-winning-startup-pitch-deck
[34]: #
[35]: #
[36]: /book-a-call
[37]: /about
[38]: /careers
[39]: /services
[40]: /case-studies
[41]: /blog
[42]: /newsletter
[43]: /knowledge-hub
[44]: https://www.instagram.com/oakslab/
[45]: https://www.linkedin.com/company/oakslab/
[46]: https://www.facebook.com/oakslab
[47]: https://twitter.com/theoakslab
[48]: mailto:hello@oakslab.com
[49]: https://clutch.co/profile/oaks-lab#highlights
[50]: /cookies
[51]: /privacy-policy
[52]: /isms-policy
```
