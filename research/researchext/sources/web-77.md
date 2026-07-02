# Web source

- URL: https://arxiv.org/html/2602.10122v1
- Title: 1. [1 Introduction][1]
- Captured (UTC): 2026-06-30T09:42:05.572998700+00:00

```text
1. [1 Introduction][1]
2. [2 Challenges in Agentic AI Transition][2]
   1. [2.1 Limited Recognition of Agentic AI Capabilities][3]
   2. [2.2 Limited Understanding of Agentic AI and Related Concepts][4]
   3. [2.3 Traditional Software Engineering Mindsets][5]
   4. [2.4 Misconception of Engineering as the Primary Bottleneck][6]
   5. [2.5 Insufficient Business-Domain Knowledge][7]
   6. [2.6 Challenges in Identifying High-Value Use Cases][8]
   7. [2.7 Lack of Collaboration Between Engineering and Business Teams][9]
3. [3 Guide for Agentic AI Transition][10]
   1. [3.1 Understanding Business Domains, Manual Processes, and Use Cases][11]
   2. [3.2 Delegating Manual Processes into AI Agents][12]
   3. [3.3 Keeping Humans as the Orchestrators of Agentic Workflows][13]
   4. [3.4 AI to Build Agentic AI Workflows][14]
   5. [3.5 Building Small, Autonomous Teams][15]
   6. [3.6 Deep Collaboration Between Engineering and Business Teams][16]
   7. [3.7 Staying Up to Date and Adapting to Change][17]
4. [4 Evaluation][18]
   1. [4.1 Evaluation of the Planning Workflow][19]
   2. [4.2 Evaluation of the Transport Management Workflow][20]
   3. [4.3 Discussion][21]
5. [5 Conclusion and Future Works][22]

# A Practical Guide to Agentic AI Transition in Organizations

Eranga Bandara [cmedawer@odu.edu][23] Ross Gore [rgore@odu.edu][24] Sachin Shetty [sshetty@odu.edu][25] Sachini
Rajapakse [sachini.rajapakse@iciclelabs.ai][26] Isurunima Kularathna [isurunima.kularathna@iciclelabs.ai][27] Pramoda
Karunarathna [pramodabhavani@sjp.ac.lk][28] Ravi Mukkamala [mukka@odu.edu][29] Peter Foytik [pfoytik@odu.edu][30] Safdar
H. Bouk [sbouk@odu.edu][31] Abdul Rahman [abdulrahman@deloitte.com][32] Xueping Liang [xuliang@fiu.edu][33] Amin Hass
[amin.hassanzadeh@accenture.com][34] Tharaka Hewa [tharaka.hewa@oulu.fi][35] Ng Wee Keong [awkng@ntu.edu.sg][36] Kasun
De Zoysa [kasun@ucsc.cmb.ac.lk][37] Aruna Withanage [aruna@effectz.ai][38] Nilaan Loganathan [nilaan@effectz.ai][39] Old
Dominion University, Norfolk, VA, USA Deloitte & Touche LLP, USA Florida International University, USA Nanyang
Technological University, Singapore University of Colombo, Sri Lanka Center for Wireless Communications, University of
Oulu, Finland IcicleLabs.AI University of Sri Jayewardenepura, Sri Lanka Accenture Technology Labs, Arlington, VA, USA
Effectz.AI

###### Abstract

Agentic AI represents a significant shift in how intelligence is applied within organizations, moving beyond AI-assisted
tools toward autonomous systems capable of reasoning, decision-making, and coordinated action across workflows. As these
systems mature, they have the potential to automate a substantial share of manual organizational processes,
fundamentally reshaping how work is designed, executed, and governed. Although many organizations have adopted AI to
improve productivity, most implementations remain limited to isolated use cases and human-centered, tool-driven
workflows. Despite increasing awareness of agentic AI’s strategic importance, engineering teams and organizational
leaders often lack clear guidance on how to operationalize it effectively. Key challenges include an overreliance on
traditional software engineering practices, limited integration of business-domain knowledge, unclear ownership of
AI-driven workflows, and the absence of sustainable human-AI collaboration models. Consequently, organizations struggle
to move beyond experimentation, scale agentic systems, and align them with tangible business value. Drawing on practical
experience in designing and deploying agentic AI workflows across multiple organizations and business domains, this
paper proposes a pragmatic framework for transitioning organizational functions from manual processes to automated
agentic AI systems. The framework emphasizes domain-driven use case identification, systematic delegation of tasks to AI
agents, AI-assisted construction of agentic workflows, and small, AI-augmented teams working closely with business
stakeholders. Central to the approach is a human-in-the-loop operating model in which individuals act as orchestrators
of multiple AI agents, enabling scalable automation while maintaining oversight, adaptability, and organizational
control.

###### keywords:

Agentic AI , Agentic AI Workflow , Responsible AI , Explainable AI , LLM , Model Context Protocol
^{†}^{†}journal: Journal Name

## 1 Introduction

Recent advances in Large Language Models (LLMs) and AI tooling have accelerated the adoption of artificial intelligence
across organizations [[4][40], [24][41]]. What began as experimentation with AI-assisted capabilities, such as code
generation, content drafting, and decision support, has evolved toward systems capable of autonomous reasoning and
coordinated action. These systems, commonly referred to as agentic AI, represent a fundamental shift in how intelligence
is embedded within organizational workflows. Rather than merely assisting humans, agentic AI systems can act on their
behalf within defined operational boundaries, enabling AI to participate directly in the execution of work [[1][42],
[43][43]].

As agentic AI systems mature, their impact extends beyond incremental productivity improvements toward the automation of
entire manual and semi-manual processes. Activities that previously required continuous human coordination, such as
information gathering, decision routing, operational monitoring, and exception handling, can increasingly be performed
by networks of AI agents operating across heterogeneous systems. This shift has the potential to redefine how work is
designed, how teams are organized, and how value is created within organizations [[9][44], [8][45]].

Despite this potential, most organizations remain in an intermediate transition phase rather than operating in a fully
agentic state. Although AI adoption has become widespread, its use is often limited to isolated tools embedded within
existing workflows [[15][46]]. In such settings, AI continues to function primarily as an assistant, with humans
retaining responsibility for orchestration, decision-making, and execution. As a result, the transformative capabilities
of agentic AI remain largely unrealized.

A central reason for this stagnation is that organizations approach agentic AI through the lens of traditional software
development. Engineering teams often emphasize code-centric architectures, rigid interfaces, and deterministic logic,
while business-domain knowledge, frequently encoded in informal processes, tacit expertise, and human judgment, remains
weakly integrated into AI-driven systems. This disconnect leads to solutions that are technically functional but
operationally misaligned with real organizational needs. Additional challenges further complicate the transition,
including unclear ownership of AI-driven workflows, underdefined human–AI collaboration models, and growing concerns
around trust, accountability, and control [[13][47]]. At the same time, the rapid pace of AI research and tooling
advancement creates a moving target, making it difficult for organizations to commit to long-term designs without fear
of rapid obsolescence.

These challenges are not primarily technological. In practice, the dominant barriers to the adoption of agentic AI are
organizational, cultural, and procedural. Successfully transitioning to agentic AI requires rethinking how work is
structured, how teams are composed, and how responsibilities are distributed between humans and machines. Organizations
must move beyond tool-centric adoption toward workflow-centric automation models that explicitly account for domain
knowledge, human oversight, and continuous adaptation [[5][48], [19][49], [34][50]].

While our previous work [[8][51]] focused on the engineering practices required to design, develop, and deploy
production-grade agentic AI workflows, this paper shifts attention to the organizational and operational changes
necessary for these systems to be effectively adopted and sustained in real-world settings. Drawing on practical
experience building and deploying agentic AI workflows across multiple organizations and business domains, we present a
pragmatic guide for transitioning from AI-assisted workflows to fully agentic AI systems.

The approach introduced in this paper emphasizes understanding business domains and manual processes, systematically
delegating these processes to specialized AI agents, and keeping humans in the loop as orchestrators of agentic
workflows. This transition is enabled through small, AI-augmented teams, deep collaboration between engineering and
business stakeholders, and AI-assisted development practices in which AI systems themselves contribute to the
construction and evolution of agentic workflows. By grounding agentic AI systems in human-centered operational models,
organizations can achieve scalable automation while maintaining adaptability, accountability, and control. The main
contributions of this work are summarized as follows.
1. 1.
   
   Framing agentic AI adoption as an organizational transition problem rather than a purely technical challenge. This
   work characterizes the ongoing global transition toward agentic AI and explains why conventional, engineering-centric
   approaches are insufficient for realizing its full organizational impact.
2. 2.
   
   Identification of key organizational and operational challenges in agentic AI transition. We analyze common obstacles
   observed in practice, including misalignment between engineering and business teams, insufficient integration of
   business-domain knowledge, unclear ownership of AI-driven workflows, and the absence of effective models for
   sustained human–AI collaboration.
3. 3.
   
   A practical, experience-driven framework for transitioning to agentic AI systems. Drawing on real-world deployments
   across multiple organizations and business domains, we propose a pragmatic approach that emphasizes domain-driven use
   case identification, AI-assisted construction of agentic workflows, small AI-augmented development teams, close
   collaboration between business and engineering stakeholders, and the systematic translation of manual processes into
   human-supervised agentic workflows.
4. 4.
   
   A human-centered operating model for scalable and sustainable agentic AI adoption. We introduce an operating model in
   which humans act as orchestrators of multiple AI agents, enabling scalable automation while preserving oversight,
   adaptability, and long-term organizational momentum.

The remainder of this paper is organized as follows. Section 2 examines the key challenges organizations face during the
transition to agentic AI, with an emphasis on organizational, procedural, and human-centered barriers that limit
adoption beyond isolated AI-assisted use cases. Section 3 presents a practical guide for agentic AI transition,
outlining principles and practices for identifying high-value use cases, delegating manual processes to AI agents,
designing human-in-the-loop orchestration models, and enabling effective team structures and collaboration. Section 4
evaluates the proposed transition guide through a real-world tourism SME use case, assessing the effectiveness of
agentic workflows in replacing manual operations and supporting human-supervised orchestration. Finally, Section 5
concludes the paper by summarizing key insights and contributions and outlining directions for future work, including
the study of additional real-world agentic AI transition deployments across diverse organizational contexts.

## 2 Challenges in Agentic AI Transition

Organizations across industries are currently in an intermediate stage of AI adoption. While AI-assisted tools have
become common in day-to-day work, most organizations have not yet transitioned to agentic AI systems capable of
autonomously executing end-to-end workflows [[32][52]]. In practice, AI is predominantly used as a productivity enhancer
for individuals rather than as an operational actor embedded within organizational processes.

A key reason for this stagnation is that many organizations have not fully realized the scope and implications of
agentic AI. Agentic systems are capable of reasoning across multiple steps, interacting with heterogeneous systems,
handling exceptions, and coordinating actions without continuous human intervention. These capabilities enable the
automation of a substantial portion of existing manual and semi-manual workflows. However, this potential remains
underappreciated, and agentic AI is often perceived as an incremental enhancement to existing tools rather than as a
fundamental shift in how work is designed and executed [[28][53]]. This misunderstanding gives rise to a set of
recurring organizational and operational challenges, which are discussed in the following subsections.

### 2.1 Limited Recognition of Agentic AI Capabilities

Despite rapid advances in agentic AI technologies, many organizations have yet to fully recognize the breadth of tasks
and workflows that can be automated using agentic systems. In practice, AI adoption is often confined to narrow use
cases such as chat-based assistants, document summarization, or isolated task automation. While these applications
provide incremental productivity benefits, they do not fundamentally change how work is executed within
organizations [[32][54]].

This limited recognition is partly driven by early exposure to large language models as conversational tools. As a
result, AI is commonly perceived as an interface for information retrieval or text generation rather than as an
autonomous actor capable of reasoning, planning, and executing complex workflows. Consequently, organizations
underestimate the extent to which agentic AI can automate multi-step processes that involve coordination across systems,
decision-making under uncertainty, and iterative refinement [[1][55], [35][56]].

The impact of this constrained perception extends beyond technical design to organizational decision-making. Investment
strategies, team structures, and success metrics are often aligned with low-risk, tool-centric deployments rather than
transformative workflow automation. This cautious approach limits experimentation with agentic systems and discourages
initiatives that could deliver substantial operational leverage.

Moreover, when agentic AI is framed primarily as an enhancement to individual productivity, its organizational value
becomes difficult to quantify and justify at scale. End-to-end workflow automation, by contrast, can deliver measurable
improvements in efficiency, consistency, and responsiveness. Failure to recognize this distinction leads organizations
to underinvest in agentic AI and to overlook opportunities for meaningful transformation [[33][57]].

Addressing this challenge requires expanding the organizational understanding of what agentic AI can do. Leaders and
practitioners must move beyond viewing AI as a collection of isolated tools and begin to conceptualize it as a workforce
of autonomous agents capable of executing and coordinating complex workflows. Without this shift in perspective,
organizations are unlikely to realize the full operational potential of agentic AI systems [[30][58], [13][59]].

### 2.2 Limited Understanding of Agentic AI and Related Concepts

Despite rapid progress in agentic AI technologies, many development teams still have a limited and fragmented
understanding of what agentic AI systems are and how they fundamentally differ from traditional LLM usage [[4][60],
[46][61]]. Because these concepts are relatively new, organizations often conflate agentic AI with simple prompt-based
interactions or chatbot-style systems, significantly underestimating their capabilities and design
implications [[16][62]].

Traditional LLM interactions follow a simple request–response pattern in which a human provides a prompt and the model
generates a corresponding output, as illustrated in the top half of Figure [1][63]. In this paradigm, humans remain
responsible for orchestration, decision-making, and follow-up actions. As a result, AI is treated primarily as a passive
assistant rather than as an active participant in workflow execution [[37][64]].

In contrast, an AI agent can autonomously perform this interaction loop. An agent can construct prompts, invoke models,
interpret responses, and trigger subsequent actions without direct human intervention, as illustrated in the bottom half
of Figure [1][65]. In essence, AI agents are software programs that leverage LLMs in combination with tools, APIs, and
external context to execute tasks automatically and iteratively [[11][66]].

When multiple such agents collaborate, each assigned specialized responsibilities such as searching, filtering,
scraping, reasoning, validation, or publishing, they form agentic AI workflows [[9][67]]. These workflows enable systems
that can reason over complex tasks, plan sequences of actions, interact with external systems, monitor outcomes, and
adapt their behavior through iterative feedback. Modern agentic workflows integrate LLMs with structured memory, search
functions, databases, Model Context Protocol (MCP) servers, cloud services, and API-driven environments [[12][68],
[23][69], [22][70]]. Rather than relying on a single monolithic prompt, responsibilities are distributed across
specialized agents, improving modularity, controllability, and maintainability.

The limited understanding of these concepts leads organizations to design agentic systems using inappropriate
abstractions, treating them as conventional software components or static AI services. This misunderstanding not only
constrains system capabilities but also reinforces engineering-centric approaches that fail to exploit the full
potential of agentic AI [[10][71]]. Addressing this gap in conceptual understanding is a prerequisite for effective
organizational transition to agentic AI workflows.

[Refer to caption] Figure 1: Human–LLM interaction versus autonomous AI agent–LLM interaction.

### 2.3 Traditional Software Engineering Mindsets

A significant barrier to effective agentic AI adoption is the tendency of engineering teams to approach agentic
workflows using traditional software engineering paradigms. These paradigms emphasize rigid specifications,
deterministic control flow, static interfaces, and extensive upfront design [[33][72]]. While such approaches have
proven effective for conventional software systems, they are poorly suited to agentic AI systems, which are inherently
probabilistic, adaptive, and prompt-driven.

When agentic AI workflows are treated as traditional software components, teams often engage in overengineering,
attempting to exhaustively define behavior, optimize prematurely, and impose strict structural constraints [[44][73]].
This results in slow iteration cycles, brittle implementations, and systems that fail to exploit the flexibility and
autonomy that agentic AI enables. Rather than simplifying work, these designs frequently reintroduce complexity in new
forms [[15][74]].

At the core of this issue is a misunderstanding of the primary goal of agentic AI. The objective is not to build another
class of software systems, but to delegate manual and cognitive tasks traditionally performed by humans to autonomous
agents that can reason, act, and adapt [[21][75]]. Agentic AI shifts the focus from precise control over execution to
effective delegation, supervision, and outcome management. Applying conventional software engineering assumptions
obscures this distinction and limits the potential impact of agentic systems.

This challenge is further amplified by a broader transformation in human-computer interaction. As agentic AI systems
mature, traditional interaction models such as explicit user interfaces, predefined workflows, and direct manipulation
are increasingly replaced by natural language–based and goal-oriented interactions. Accepting this shift requires a
fundamental change in how engineers conceptualize software, control, and user interaction. In practice, resistance to
abandoning familiar development models often slows adoption and reinforces legacy thinking.

Overcoming this challenge requires a deliberate mindset shift. Engineering teams must move away from treating agentic AI
as deterministic software artifacts and toward viewing them as autonomous collaborators embedded within workflows.
Embracing this shift is essential for designing systems that can effectively automate manual processes, adapt to
changing conditions, and deliver sustained organizational value [[36][76]].

### 2.4 Misconception of Engineering as the Primary Bottleneck

Many organizations continue to operate under the assumption that engineering capacity is the primary constraint in
building and deploying AI systems. This assumption is rooted in decades of conventional software development, where
progress was largely determined by the availability of skilled developers and the speed of implementation. In the
context of agentic AI, however, this assumption no longer holds [[8][77]].

Advances in AI-assisted development environments have significantly reduced the effort required to implement agentic
workflows. Tasks such as prompt construction, agent orchestration, tool integration, and even large portions of
application logic can now be generated, refined, and maintained with substantial assistance from AI systems. As a
result, the marginal cost of implementation has decreased, and engineering throughput has increased
dramatically [[9][78]].

Consequently, the primary bottleneck in agentic AI development shifts away from coding and toward problem framing,
workflow design, and domain understanding. Determining what should be automated, how tasks should be delegated to
agents, and where human oversight is required becomes far more critical than the mechanics of implementation.
Organizations that fail to recognize this shift often continue to structure teams, timelines, and success metrics around
engineering output, leading to misaligned incentives and suboptimal outcomes.

This misconception also manifests in excessive focus on optimization, performance tuning, and architectural refinement
at early stages of development. Such efforts provide limited value when the fundamental workflow design or problem
definition is incomplete or incorrect [[45][79]]. Over time, this misalignment slows progress, increases complexity, and
reinforces the false perception that agentic AI systems are inherently difficult to build, when in reality the challenge
lies elsewhere.

Addressing this issue requires organizations to redefine success metrics and team responsibilities. Emphasis must move
from engineering throughput to clarity of intent, quality of workflow design, and effectiveness of human-agent
collaboration. Recognizing that engineering is no longer the dominant constraint is a critical step toward enabling
scalable and sustainable agentic AI adoption [[17][80]].

### 2.5 Insufficient Business-Domain Knowledge

High-impact agentic AI workflows depend fundamentally on a deep understanding of business-domain processes. These
processes often include informal rules, context-dependent decisions, exception handling, and human judgment developed
through years of operational experience. Such knowledge is rarely captured in formal documentation and is frequently
embedded in day-to-day practices and interpersonal communication [[33][81]].

Engineering teams typically lack direct access to this tacit knowledge. When agentic AI systems are designed without
sufficient domain insight, they may automate surface-level tasks while failing to account for underlying constraints,
priorities, and edge cases. This results in agents that behave correctly in ideal scenarios but break down in real-world
conditions, undermining trust and limiting adoption.

The challenge is compounded by the fact that many valuable agentic AI use cases originate outside engineering
departments. Business functions such as operations, finance, compliance, and customer support often rely heavily on
manual workflows that involve nuanced reasoning and coordination across systems. Without deep engagement with these
domains, engineering-led initiatives tend to focus on technically convenient problems rather than those with the highest
organizational impact [[32][82]].

Bridging this gap requires sustained and direct collaboration with domain experts throughout the lifecycle of agentic AI
development. Rather than treating domain knowledge as a static input during requirements gathering, it must be
continuously integrated into workflow design, agent behavior, and evaluation criteria. Only through this close
collaboration can agentic systems effectively reason about real-world complexity and deliver meaningful automation
outcomes.

Ultimately, insufficient business-domain knowledge is not merely a knowledge gap but an organizational challenge.
Overcoming it requires changes in team composition, communication patterns, and development practices that prioritize
domain understanding as a first-class concern in the agentic AI transition.

### 2.6 Challenges in Identifying High-Value Use Cases

A persistent challenge in agentic AI adoption is the tendency for organizations to search for use cases primarily within
engineering departments. This approach reflects historical software development practices, where engineering teams were
responsible for identifying automation opportunities based on technical feasibility. In the context of agentic AI,
however, this strategy often leads to low-impact or misaligned deployments [[15][83]].

In practice, many of the most valuable opportunities for agentic AI reside within business functions such as operations,
finance, customer support, compliance, and supply chain management. These domains are characterized by complex manual
workflows that involve coordination across multiple systems, repeated decision-making, exception handling, and
information synthesis. Such workflows are particularly well-suited to agentic AI, which can reason across steps, adapt
to context, and automate end-to-end processes.

Limited visibility into these business domains prevents organizations from recognizing and prioritizing high-value use
cases. Engineering-led discovery efforts tend to focus on problems that are technically interesting or easy to
implement, rather than those that deliver meaningful organizational impact [[33][84]]. As a result, agentic AI
initiatives may succeed technically while failing to justify continued investment or broader adoption.

Identifying high-value agentic AI use cases, therefore, requires shifting ownership of use case discovery closer to
business teams. Engineers must work alongside domain experts to surface workflows that are currently manual, repetitive,
and decision-intensive. Without this shift, organizations risk underutilizing agentic AI and reinforcing the
misconception that its benefits are marginal or limited in scope [[21][85]].

### 2.7 Lack of Collaboration Between Engineering and Business Teams

Traditional handoff-based collaboration models, in which business stakeholders define requirements and engineering teams
implement solutions, are poorly suited to agentic AI development. Agentic AI workflows are inherently exploratory and
adaptive, requiring continuous refinement as agents interact with real-world processes and data. Static requirements and
one-time specifications are insufficient to capture this complexity [[43][86]].

Effective agentic AI development depends on sustained collaboration between engineering and business teams throughout
the lifecycle of a workflow. Domain experts must be actively involved in shaping agent behavior, defining success
criteria, and interpreting outcomes. Without ongoing feedback, agents may operate correctly from a technical perspective
while failing to align with operational realities.

A lack of close collaboration often results in repeated rework, misaligned assumptions, and low trust in deployed
systems. Business users may perceive agentic workflows as opaque or unreliable, while engineering teams struggle to
interpret vague or evolving requirements. Over time, these frictions reduce adoption and limit the scalability of
agentic AI initiatives.

Overcoming this challenge requires moving away from transactional interactions toward shared ownership of agentic
workflows. Engineering and business stakeholders must jointly design, monitor, and evolve agent behavior. Establishing
this collaborative model is essential for ensuring that agentic AI systems remain aligned with organizational goals and
are successfully integrated into everyday operations [[21][87]].

Taken together, these challenges indicate that the transition to agentic AI is not constrained by technological
capability, but by organizational readiness, mindset, and operating models. Overcoming these barriers requires
rethinking team composition, redefining the role of engineering, embedding business stakeholders directly into AI
development efforts, and shifting from tool-centric adoption toward workflow-centric automation. The following section
introduces a practical guide that synthesizes these insights and provides concrete principles for enabling a smooth and
sustainable transition to agentic AI systems.

## 3 Guide for Agentic AI Transition

Based on the challenges discussed in the previous section and our practical experience deploying agentic AI systems
across multiple organizations and business domains, we propose a pragmatic guide for transitioning from AI-assisted
workflows to fully agentic AI systems. Rather than prescribing a rigid, linear sequence of steps, this guide outlines a
set of principles that address both the technical and organizational dimensions of agentic AI adoption. The guide is
structured to first focus on problem understanding and workflow design, and then on the people, team structures, and
practices required to sustain the transition.

### 3.1 Understanding Business Domains, Manual Processes, and Use Cases

A successful agentic AI transition begins with a deep understanding of the business domain in which automation is
intended to operate. In practice, identifying the right problems to solve is more important than technical feasibility
alone. Agentic AI delivers the greatest value when applied to workflows that are manual, repetitive, decision-intensive,
and span multiple systems or stakeholders [[8][88]].

Many high-impact opportunities for agentic AI are rooted in informal business processes that rely heavily on human
judgment, contextual understanding, and tacit operational knowledge accumulated over time. These processes are rarely
documented in sufficient detail and are often invisible to engineering teams. Without direct and sustained engagement
with business domains, organizations risk applying agentic AI to low-impact tasks while overlooking workflows that offer
significant potential for end-to-end automation [[37][89]].

As part of our broader agentic AI transition efforts, we were directly involved in transforming operational workflows
for small and medium-sized tourism enterprises (SMEs) [[40][90]]. This initiative aimed to demonstrate how agentic
AI–based workflow automation can support efficient organizational scaling by automating core day-to-day operational
activities.

For this use case, the engineering effort consisted of a single engineer augmented by agentic AI–based development tools
such as Claude Code and Codex [[45][91], [17][92]]. These tools were used extensively for agent creation, workflow
composition, and iterative refinement. Despite the small team size, comprehensive domain studies were conducted through
close and continuous interaction with business stakeholders, enabling a deep understanding of business objectives,
operational constraints, seasonal variability, and existing manual workflows.

Through this sustained engagement, we identified agentic AI use cases that aligned closely with real operational needs
and delivered tangible organizational value. As an initial step, we identified a set of core manual processes common
across tourism SMEs that were suitable for delegation to AI agents, including invoicing, itinerary planning,
transportation management, customer inquiry handling, supplier coordination, and booking management [[29][93]]. Each use
case was then examined in detail to understand how work was performed in practice and how these manual processes could
be systematically translated into agentic workflows.

Figure [2][94] illustrates the core manual planning workflow used by administrative staff when generating daily planning
sheets. The process involves manually reading booking inquiries from multiple sources (primarily email), filtering and
reconciling updates and changes, checking the availability of activities and transportation resources, allocating
customers to appropriate activities and transport options, and finally producing a consolidated planning sheet stored in
a shared system.

This workflow is highly manual, coordination-intensive, and sensitive to timing and contextual constraints. It requires
continuous human judgment to interpret incomplete information, resolve conflicts, and synchronize multiple stakeholders.
Identifying such workflows is a critical step in agentic AI transition, as these characteristics, manual effort,
repeated decision-making, and cross-system coordination make them strong candidates for agentic AI–based automation with
meaningful operational impact [[39][95]].

[Refer to caption] Figure 2: Manual planning workflow used by tourism SME administrative staff to generate daily
planning sheets. The workflow relies on human coordination across booking inquiries, activity availability,
transportation resources, and final schedule consolidation.

### 3.2 Delegating Manual Processes into AI Agents

Once manual workflows are well understood, the next step is to delegate these processes to agentic AI workflows composed
of multiple autonomous agents [[1][96]]. This transition involves more than automating individual tasks; it requires
decomposing human-performed workflows into distinct reasoning steps, decision points, and actions that can be
meaningfully assigned to specialized AI agents. The objective is not to replicate existing user interfaces or software
flows, but to capture the underlying intent, logic, and decision-making patterns that guide human actions [[47][97],
[41][98]].

In practice, this process begins by identifying the cognitive responsibilities embedded within the manual workflow, such
as information extraction, validation, filtering, availability checking, allocation, and synthesis. Each responsibility
is then mapped to one or more AI agents with clearly defined roles, scopes, inputs, and outputs. Agents are equipped
with the contextual information required for effective reasoning, including access to external systems, structured data
sources, and intermediate workflow state.

Figure [3][99] illustrates how the previously manual planning workflow used by tourism SMEs was decomposed and
transitioned into a coordinated set of AI agents. In this agentic workflow, a dedicated email-reading agent extracts
booking information from incoming messages, followed by filtering agents that refine and normalize booking details.
Subsequent agents independently retrieve activity availability and transportation information from external sources,
combining these inputs to generate a coherent planning sheet. Finally, a publishing agent stores the generated planning
artifact in a shared system for human review and downstream use [[37][100]].

This agent-based decomposition enables modularity, parallelism, and iterative refinement. Individual agents can be
improved, replaced, or extended without redesigning the entire workflow, allowing the system to evolve incrementally as
business requirements change or as agent capabilities improve. By explicitly delegating reasoning and coordination tasks
to specialized agents, organizations can transform complex manual workflows into scalable, adaptable agentic AI systems.

[Refer to caption] Figure 3: Agentic AI planning workflow derived from a manual tourism SME planning process. The
workflow decomposes human planning tasks into specialized AI agents responsible for email ingestion, booking filtering,
activity availability retrieval, transportation coordination, planning sheet generation, and content publication.

### 3.3 Keeping Humans as the Orchestrators of Agentic Workflows

A central principle of the agentic AI transition is maintaining humans in the loop as high-level orchestrators of
autonomous workflows rather than as manual executors of individual tasks. The goal is to build multiple agentic AI
workflows that transform core business functions and expose these workflows through standardized interfaces, enabling
human coordination, supervision, and control [[9][101]].

In practice, each agentic AI workflow is exposed through an MCP server, allowing MCP-powered tools such as LM
Studio [[42][102], [14][103], [27][104]] to integrate with multiple workflows simultaneously. This design enables a
single human coordinator to interact with, invoke, and supervise diverse agentic workflows through a unified natural
language interface. Figure [4][105] illustrates this interaction model, where human intent expressed through an
MCP-powered tool is routed to appropriate agentic workflows, which in turn leverage different underlying language models
and tools.

[Refer to caption] Figure 4: Interaction model for agentic AI workflows exposed through MCP servers. A human coordinator
interacts with multiple agentic workflows via an MCP-powered tool (e.g., LM Studio), which routes requests to
appropriate workflows and underlying language models.

Rather than embedding business logic directly into user interfaces or applications, this approach positions agentic
workflows as modular services that can be orchestrated dynamically by humans [[44][106]]. The human coordinator does not
manage low-level execution details; instead, they specify goals, trigger workflows, review outputs, and intervene only
when necessary. This significantly reduces cognitive and operational load while preserving human agency and
accountability.

In our tourism SME use case, multiple agentic AI workflows were developed to automate end-to-end business functions,
including planning, transportation management, customer inquiry handling, supplier coordination, and booking
management [[25][107]]. As shown in Figure [5][108], these workflows operate as specialized agents surrounding a human
supervisor. Each workflow is independently exposed through an MCP server and integrated into LM Studio, enabling the
human coordinator to invoke specific capabilities on demand [[6][109]].

[Refer to caption] Figure 5: Human coordinator surrounded by multiple specialized agentic AI workflows. Each workflow
automates a distinct business function and is orchestrated by a human supervisor through MCP-enabled interfaces.

For example, when a daily itinerary needs to be generated, the human coordinator simply invokes the planning workflow
through the MCP interface. The agentic system performs the required reasoning and coordination steps and publishes the
generated planning sheet to a shared repository, such as SharePoint, for downstream use [[40][110]]. This interaction
model allows employees within tourism SMEs to focus on oversight and exception handling rather than manual coordination.

Equally important is the explicit definition of human-agent interaction boundaries. Not all decisions should be fully
automated. Certain actions may require human validation, escalation, or intervention based on risk, uncertainty, or
business impact. Designing these interaction points ensures that agentic workflows operate autonomously where
appropriate while remaining aligned with organizational control, trust, and accountability requirements.

### 3.4 AI to Build Agentic AI Workflows

A defining characteristic of the agentic AI transition is that AI systems increasingly perform a substantial portion of
the development work themselves [[45][111], [17][112]]. Tasks that traditionally required significant human engineering
effort, including agent creation, prompt design, workflow orchestration, external tool integration, and MCP server
implementation, can now be generated, refined, and maintained with extensive assistance from AI [[42][113]]. As a
result, the locus of development effort shifts away from manual coding toward supervision, validation, and iterative
refinement of AI-generated artifacts.

In our deployments, the full lifecycle of agentic AI workflows, including prompt construction, agent definition,
workflow composition, and continuous optimization, was carried out using AI-assisted development environments such as
Claude Code. These environments enabled rapid experimentation and short feedback loops, allowing agent behaviors,
prompts, and interaction patterns to be continuously improved based on observed outcomes. Rather than manually tuning
individual components, the development process evolved into one of guiding, evaluating, and constraining AI-generated
solutions in response to real operational needs [[13][114]].

This shift fundamentally challenges traditional software development methodologies. Conventional approaches based on
fixed requirements, long design phases, and deterministic implementations are poorly suited to environments in which
systems are adaptive, probabilistic, and continuously evolving [[9][115]]. Agentic AI workflows require development
practices that explicitly accommodate uncertainty, emphasize rapid feedback, and support ongoing co-evolution between
humans and AI systems.

To address this gap, we adopt and extend an AI-native development methodology referred to as *Agentsway*, designed
specifically for teams building systems in collaboration with AI. In this methodology, AI is treated not merely as a
development tool but as an active participant in the development process. Human contributors focus on defining goals,
constraints, evaluation criteria, and oversight mechanisms, while AI systems generate, modify, and evolve the underlying
agents and workflows [[9][116]].

By leveraging AI to build and evolve agentic AI workflows, organizations can significantly reduce development overhead,
accelerate experimentation, and respond more effectively to changing requirements. This approach reinforces the broader
transition described in this paper: engineering effort is no longer centered on writing code, but on shaping intelligent
systems that can autonomously reason, act, and improve over time under human supervision.

### 3.5 Building Small, Autonomous Teams

Agentic AI systems are most effectively developed by small, autonomous, and highly motivated teams. In our experience,
teams consisting of no more than three to four members were sufficient to design, build, and deploy production-ready
agentic AI workflows. This stands in contrast to traditional software development practices, which typically rely on
large, specialized teams and hierarchical coordination structures.

Large teams and legacy development models are poorly suited to agentic AI development due to the inherent uncertainty
and exploratory nature of the work. Agentic workflows evolve rapidly as understanding of the business problem deepens
and as agent behavior is refined through continuous iteration. Extensive upfront planning, rigid role separation, and
long optimization cycles introduce unnecessary friction and delay feedback, ultimately reducing the effectiveness of
agentic AI initiatives [[33][117]].

The effectiveness of small teams is further amplified by advances in AI-assisted development environments.
Development/Engineering is no longer the primary bottleneck; AI systems can generate prompts, agents, workflows,
integrations, and MCP server components with minimal human effort [[45][118]]. In our deployments, the majority of
implementation work was performed using AI-assisted tools such as Claude Code [[17][119]]. This allowed human
contributors to focus on higher-value activities, including problem framing, workflow design, validation, and
supervision of agent behavior.

Crucially, these small teams should not consist solely of engineers. Including business-domain representatives as core
members of the team is essential for ensuring alignment with real operational needs. By combining deep domain expertise
with AI-augmented engineering capability, small teams can operate with a high degree of autonomy, respond quickly to
changing requirements, and maintain close alignment between agentic workflows and business objectives [[15][120]].

This shift challenges the long-standing assumption that complex systems require large development teams. In the context
of agentic AI, smaller teams supported by AI tools are often more effective, more adaptable, and better positioned to
sustain momentum throughout the transition. Embracing this team model is a key enabler of scalable and practical agentic
AI adoption.

### 3.6 Deep Collaboration Between Engineering and Business Teams

Unlike traditional software development projects, agentic AI initiatives rarely benefit from predefined and
well-established roles such as business analysts or product managers. Agentic AI concepts, workflows, and operating
models are still emerging, and most organizations lack prior experience in designing and operating such systems. As a
result, both engineers and business-domain experts must often learn, experiment, and discover new possibilities together
throughout the transition process [[20][121]].

In this context, requirements cannot be fully specified upfront. Many critical insights, such as what should be
automated, how agents should reason and act, and where human oversight is necessary, only emerge through direct
experimentation and interaction with real workflows. This makes close, continuous collaboration between engineering and
business teams not merely advantageous, but essential for successful agentic AI adoption.

In our deployments, engineers worked directly with business stakeholders to jointly explore existing workflows, uncover
implicit assumptions, and iteratively refine agent behavior. Rather than relying on formal handoffs or static
documentation, teams engaged in frequent feedback cycles, using early agent prototypes as shared artifacts for
discussion, validation, and learning. This collaborative approach enabled rapid clarification of domain-specific
nuances, exception handling strategies, and operational constraints that would have been difficult to capture through
traditional requirement-gathering processes [[28][122]].

Sustained collaboration also played a critical role in building trust. Business stakeholders developed confidence in
agentic systems by observing how agents reasoned, took actions, and handled edge cases, while engineers gained a deeper
understanding of real operational priorities and constraints. This mutual understanding reduced friction, minimized
rework, and ensured that deployed agentic workflows aligned closely with day-to-day business needs.

Ultimately, successful agentic AI adoption requires a shift away from transactional, handoff-based interactions between
engineering and business teams toward shared ownership of workflows and outcomes. Deep collaboration enables
organizations to navigate uncertainty, adapt agent behavior as requirements evolve, and integrate agentic AI systems
into core operations in a sustainable and scalable manner [[32][123]].

### 3.7 Staying Up to Date and Adapting to Change

Agentic AI systems operate in an environment characterized by rapid and often unprecedented change. Advances in
foundation models, tooling, and architectural patterns continuously reshape what is possible, frequently outpacing
traditional organizational planning and development cycles. In this context, agentic AI workflows that are effective
today may require substantial adaptation in a relatively short period of time [[32][124]].

Successfully navigating this environment places new demands on the people involved in agentic AI initiatives. Engineers,
domain experts, and organizational leaders must collectively accept that stability is no longer the default state.
Staying effective requires deliberate and ongoing effort to remain informed about emerging research and industry
developments through sources such as arXiv, practitioner blogs, and real-time knowledge-sharing communities(e.g., in
X/Twitter) [[31][125]].

However, staying up to date is not merely a matter of consuming information. Teams must develop the capability to
evaluate new ideas critically, experiment with them in controlled settings, and selectively integrate them into existing
workflows. This requires individuals who are comfortable with uncertainty and organizations that explicitly allow time
and space for exploration without immediate pressure for production outcomes.

At the same time, a common failure mode in agentic AI initiatives is the loss of momentum after early prototypes or
pilot deployments. While initial results often demonstrate technical feasibility, they frequently fail to produce
sustained operational impact. This is especially problematic in agentic AI transitions, where value depends on
continuous refinement rather than one-time implementation [[32][126]]. Sustaining momentum is primarily an
organizational challenge. Successful organizations assign clear ownership, maintain feedback loops between users and
development teams, and treat learning and iteration as ongoing responsibilities, allowing agentic work

[Content truncated]
```
