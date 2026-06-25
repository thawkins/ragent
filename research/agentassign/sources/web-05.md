# Web source

- URL: https://medium.com/@kyeg/introducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-25T05:52:55.030537776+00:00

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
Press enter or click to view image in full size
swarms v10 update

# Introducing Swarms v10: Async Sub-Agents, SkillOrchestra, and More

[
[Kye Gomez]
][7]
[Kye Gomez][8]
8 min read
·
Mar 19, 2026
[
][9]

--

[
][10]
[][11]
[

Listen

][12]

Share

Over the past six weeks, the Swarms team focused on delivering Swarms v10, one of the largest updates to the framework.
The release rebuilds core systems, adds powerful new orchestration primitives, and improves performance, reliability,
and developer experience.

Major upgrades include a fully overhauled sub-agent system with autonomous task spawning, real-time monitoring, and
failure-resilient async task management. The HierarchicalSwarm now supports parallel worker execution and introduces a
judge agent that evaluates outputs and provides feedback to improve swarm performance over time.

We also introduced SkillOrchestra, a new routing primitive that automatically delegates tasks to the most qualified
agent based on defined skills. Alongside these features, the CLI was modernized, long-standing bugs were fixed, and
internal improvements made the framework more stable and predictable.

## Installation

pip3 install -U swarms

uv pip install swarms

Github: [https://github.com/kyegomez/swarms][13]

## Async Sub-Agent Orchestration

The sub-agent system received a major overhaul, making it significantly more powerful and easier to rely on for complex,
multi-step workflows. Agents can now create and assign tasks to other agents autonomously — meaning your orchestrator no
longer needs manual wiring to delegate work.

Async execution was introduced so sub-agents run in the background without blocking your main agent, and a full task
registry backs every spawned sub-agent with automatic tracking, retries, and cancellation if something goes wrong.

You can now check the status of any running sub-agent or cancel its tasks on demand, giving you full visibility and
control over long-running workflows.

Sub-agent output is also now printed in real time so you can see exactly what each agent is doing as it works. The
entire system is cleaner and more stable under the hood, so you get faster, more reliable multi-agent pipelines with
less setup and fewer surprises.

from swarms import Agent

# Initialize the agent
agent = Agent(
    agent_name="Quantitative-Trading-Agent",
    agent_description="Advanced quantitative trading and algorithmic analysis agent",
    
system_prompt="You are a helpful assistant that can answer questions and help with tasks and your name is Quantitative-T
rading-Agent",
    model_name="gpt-5.4",
    max_loops="auto",
    dynamic_context_window=True,
    reasoning_effort=None,
)
out = agent.run(
    
"Create 2 sub agents that are conducting research on the top energy and datacenter etfs to invest in and then create a r
eport on the best ones to invest in"
)
print(out)

> ***Docs:**** *[*Sub-Agent Tutorial*][14]
> 
> ***Github:**** *[*github.com/kyegomez/swarms*][15]

## Hierarchical Swarm Improvements

The `HierarchicalSwarm` now brings two powerful new capabilities to make your multi-agent pipelines faster and smarter:

**Parallel Agent Execution** — Enabling `parallel_execution=True` dispatches all worker agents at once using a thread
pool, so instead of waiting on each agent one by one, they all run simultaneously — dramatically cutting down total
execution time on complex tasks.

**Judge Agent** — Setting `agent_as_judge=True` introduces a dedicated judge agent that scores every worker's output
after each cycle across five dimensions: task adherence, accuracy, depth, clarity, and swarm contribution. It produces a
structured report with per-agent scores and tailored improvement suggestions, so you can quickly identify which agents
are performing well and which need better instructions.

`judge_agent_model_name` lets you choose exactly which model powers the judge — use a lightweight model like
`gpt-4o-mini` for fast, economical evaluations, or swap in a more capable model when precision matters most.

from swarms import Agent, HierarchicalSwarm

# Create specialized agents
research_agent = Agent(
    agent_name="Research-Analyst",
    agent_description="Specialized in comprehensive research and data gathering",
    model_name="gpt-4o-mini",
    max_loops=1,
    verbose=False,
)

analysis_agent = Agent(
    agent_name="Data-Analyst",
    agent_description="Expert in data analysis and pattern recognition",
    model_name="gpt-4o-mini",
    max_loops=1,
    verbose=False,
)

strategy_agent = Agent(
    agent_name="Strategy-Consultant",
    agent_description="Specialized in strategic planning and recommendations",
    model_name="gpt-4o-mini",
    max_loops=1,
    verbose=False,
)

# Create hierarchical swarm with interactive dashboard
swarm = HierarchicalSwarm(
    name="Swarms Corporation Operations",
    description="Enterprise-grade hierarchical swarm for complex task execution",
    agents=[research_agent, analysis_agent, strategy_agent],
    max_loops=1,
    interactive=False,
    director_model_name="claude-haiku-4-5",
    director_temperature=0.7,
    director_top_p=None,
    parallel_execution=True,
    agent_as_judge=True,
    judge_agent_model_name="gpt-5.4",
)

print(swarm.display_hierarchy())

out = swarm.run(
    "Conduct a research analysis on water stocks and etfs"
)

print(out)

> ***Docs:**** *[*Hierarchical Swarm*][16]* ****Github:**** *[*github.com/kyegomez/swarms*][17]

## SkillOrchestra Implementation

SkillOrchestra is a new agent routing system, inspired by the research paper [SkillOrchestra: Learning to Route Agents
via Skill Transfer][18], that automatically sends each task to the most qualified agent based on skills rather than
manual configuration.

When a task comes in, an agent infers which skills it requires and how important each one is. Every agent in your pool
has a competence score for each skill, and a weighted scoring formula ranks agents against the task’s requirements in
real time — no agent calls needed for the scoring itself, just fast math. The top-scoring agent (or agents, if you set
`top_k_agents > 1`) is selected and executes the task concurrently via a thread pool.

Optionally, a learning mode evaluates output quality after execution and updates each agent’s competence scores over
time using exponential moving average, so the routing gets more accurate the more you use it. The result is a system
where you define what your agents are good at, and `SkillOrchestra` handles the rest — no hardcoded routing logic, no
guesswork.

from swarms import Agent, SkillOrchestra

code_agent = Agent(
    agent_name="CodeExpert",
    agent_description="Expert Python developer who writes clean, efficient, production-ready code",
    system_prompt=(
        "You are an expert Python developer. Write clean, well-documented, "
        "production-ready code with proper error handling and type hints."
    ),
    model_name="gpt-4o-mini",
    max_loops=1,
)

writer_agent = Agent(
    agent_name="TechWriter",
    agent_description="Technical writing specialist who creates clear documentation and tutorials",
    system_prompt=(
        "You are a technical writing specialist. Write clear, comprehensive "
        "documentation with examples, explanations, and proper formatting."
    ),
    model_name="gpt-4o-mini",
    max_loops=1,
)

researcher_agent = Agent(
    agent_name="Researcher",
    agent_description="Research analyst who gathers, synthesizes, and compares information",
    system_prompt=(
        "You are a research analyst. Provide thorough, well-structured analysis "
        "with comparisons, trade-offs, and actionable recommendations."
    ),
    model_name="gpt-4o-mini",
    max_loops=1,
)

# Create SkillOrchestra (auto-generates skill handbook from agent descriptions)
orchestra = SkillOrchestra(
    name="DevTeamOrchestra",
    agents=[code_agent, writer_agent, researcher_agent],
    model="gpt-4o-mini",
    top_k_agents=1,
    learning_enabled=False,
    output_type="final",
)

# The handbook was auto-generated. Inspect it:
print("Generated Skill Handbook:")
for skill in orchestra.skill_handbook.skills:
    print(f"  - {skill.name}: {skill.description}")
print()

# This should route to CodeExpert
result = orchestra.run(
    "Write a Python function to parse and validate JSON config files"
)
print(result)

> ***Github:**** *[*github.com/kyegomez/swarms*][19]* ****Docs:**** *[*SkillOrchestra*][20]

## Swarms CLI Overhaul

The CLI received its most substantial overhaul to date: the old ASCII art banner has been replaced with a clean, compact
header that displays your active model provider at a glance along with rotating startup tips and a simplified
red-and-white color scheme.

Help output has also been reworked — rather than rich-formatted tables, you now see a plain-text argparse-style
reference that lists every command, option, and example in a format that is easier to scan and copy from.

Throughout the CLI, unnecessary elements have been removed — including the deprecated `features` command, redundant
`help` subcommand, and unused CLI table utilities — yielding a leaner, more predictable interface.

The `swarms chat` feature was fixed, and the CLI now runs with fewer required parameters, accelerating setup. Under the
hood, the auto chat loop is now a persistent while-loop with exit-command handling, agent prompts receive a dynamically
injected timestamp at runtime, and new bash command security guardrails block dangerous shell patterns — including
recursive deletion, fork bombs, and privilege escalation — before any command can reach your system.

swarms help
swarms chat

> ***Docs:**** *[*CLI Reference*][21]

## Improvements
* **PDF Utility Removal** — Removed `pdf_to_text` as a core utility from `utils/__init__.py` and `data_to_text.py`;
  inlined a local implementation directly in the mem0 example script. *(2026-03-12)*
* **Agent Class Cleanup** — Removed duplicated async sub-agent methods from the Agent class body, now superseded by
  registry integration. *(2026–03–12)*
* **top_p Default Fix** — Removed hardcoded `top_p=None` from `auto_chat_agent` and set `Agent.top_p` default to `None`
  to avoid unintended sampling parameter injection. *(2026-03-12)*
* **Model & Version Update** — Bumped swarms to 9.0.3 and updated default model references from
  `gpt-4.1`/`claude-sonnet-4-5` to `gpt-5.4`/`gpt-5.1` across example files and CLI. *(2026-03-12)*
* **Async Sub-Agents Code Cleanup** — Code cleanup and logic refactoring for async sub-agent execution. *(2026–03–09)*
* **Hierarchical Swarm Docs Overhaul** — Overhauled `hierarchical_swarm.md` with updated Mermaid diagram, new
  constructor parameter table, dedicated sections for all new features, `arun()` docs with FastAPI integration example,
  and updated best practices. *(2026-03-05)*
* **CLI Cleanup** — Removed deprecated `features` command, `show_features` function, redundant `help` subcommand, and
  dead CLI table utilities. *(2026-03-03)*
* **Agent Defaults** — Agent now defaults to `gpt-4.1`, uses styled formatter console input prompts in interactive mode,
  and always returns the final summary from the autonomous loop. *(2026-03-03)*
* **CLI Documentation** — Updated CLI documentation to reflect recent CLI overhaul. *(2026–03–03)*
* **CLI Parameters** — Improved CLI with fewer parameters and fixed the `swarms chat` feature. *(2026-02-02)*
* **Sub-Agent Tutorial Docs** — Improved sub-agent tutorial documentation. *(2026–02–02)*
* **Examples Section** — Improved examples section with updated references. *(2026–02–05)*
* **Bash Tool for **`**agent.auto**` — Added bash tool support for agents running in auto mode. *(2026-02-02)*
* **Auto-Save Examples** — Added examples for auto-saving structures. *(2026–02–02)*
* **Bash Command Security Guardrails** — Added security validation to block dangerous shell patterns including recursive
  deletion, pipe-to-shell, disk writes, fork bombs, and privilege escalation; enforces a 512-character command length
  limit. *(2026–03–03)*

## Bug Fixes
* Fixed README and auto-chat issue on CLI. *(2026–03–11)*
* **Welcome Workflow** — Fixed unexpected inputs in welcome workflow. *(2026–02–28)*
* **LLMCouncil** — Fixed incompatibility between LLMCouncil, SwarmRouter, and the API server. *(2026–02–24)*
* **Agent Identity** — Fixed system prompt injection for agent identity when no system prompt is provided.
  *(2026–02–17)*
* **DebateWithJudge** — Fixed API field for DebateWithJudge to work correctly with the API server. *(2026–02–15)*
* **RoundRobin** — Fixed RoundRobin swarm; removed useless parameter. *(2026–02–15)*
* **LiteLLM Empty System Prompt Fix** — Fixed Anthropic API rejections by stripping whitespace-only system prompt blocks
  and normalizing orchestrator-style `System:`/`Human:` prompts before message construction. *(2026-03-12)*
* Fixed typo: `seperatedly` → `separately`. *(2026-02-10)*

## Conclusion

This release represents one of the most significant leaps forward in the Swarms framework to date.

Over the past six weeks, nearly every major system — sub-agent orchestration, the hierarchical swarm, the CLI, and core
agent infrastructure — has been meaningfully improved. Multi-agent workflows are faster with parallel execution, smarter
with the judge agent, and more autonomous than ever with a fully async sub-agent registry that handles task tracking,
retries, and cancellation out of the box.

The CLI is cleaner, the defaults are more sensible, and a wide range of bugs that affected real-world usage have been
resolved. Whether you are building simple pipelines or complex multi-layered multi-agent systems, the framework is more
capable, more reliable, and easier to work with than it has ever been.

We are grateful for every contributor, bug reporter, and community member who helped shape this release — and this is
only the beginning of what is planned for v10.

## Get Started with Swarms

Build, deploy, and scale your agents with Swarms now:
* **Github:** [github.com/kyegomez/swarms][22]
* **Docs:** [docs.swarms.world][23]
* **Discord:** [Join the community][24]

[
Artificial Intelligence
][25]
[
Machine Learning
][26]
[
Startup
][27]
[
Data Science
][28]
[
Aiagents
][29]
[
][30]

--

[
][31]

--

[
][32]
[][33]
[
[Kye Gomez]
][34]
[
[Kye Gomez]
][35]
[

## Written by Kye Gomez

][36]
[892 followers][37]
·[386 following][38]

Swarms Website: [https://swarms.ai][39] My Personal: [kyegomez.com][40]

[

Help

][41]
[

Status

][42]
[

About

][43]
[

Careers

][44]
[

Press

][45]
[

Blog

][46]
[

Store

][47]
[

Privacy

][48]
[

Rules

][49]
[

Terms

][50]
[

Text to speech

][51]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-ski
llorchestra-and-more-6f0754734677&source=post_page---top_nav_layout_nav-----------------------global_nav----------------
--
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-ski
llorchestra-and-more-6f0754734677&source=post_page---top_nav_layout_nav-----------------------global_nav----------------
--
[7]: /@kyeg?source=post_page---byline--6f0754734677---------------------------------------
[8]: /@kyeg?source=post_page---byline--6f0754734677---------------------------------------
[9]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F6f0754734677&operation=register&redirect=https%3A%2F%
2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677&user=Kye+Gomez&use
rId=65d61d27a2c8&source=---header_actions--6f0754734677---------------------clap_footer------------------
[10]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F6f0754734677&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677&user=Kye+Gomez&
userId=65d61d27a2c8&source=---header_actions--6f0754734677---------------------repost_header------------------
[11]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F6f0754734677&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677&source=---hea
der_actions--6f0754734677---------------------bookmark_footer------------------
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D6f0754734677&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-m
ore-6f0754734677&source=---header_actions--6f0754734677---------------------post_audio_button------------------
[13]: https://github.com/kyegomez/swarms
[14]: https://docs.swarms.world/en/latest/swarms/examples/sub_agent_tutorial/
[15]: https://github.com/kyegomez/swarms
[16]: https://docs.swarms.world/en/latest/swarms/structs/hierarchical_swarm/
[17]: https://github.com/kyegomez/swarms
[18]: https://arxiv.org/abs/2602.19672
[19]: https://github.com/kyegomez/swarms
[20]: https://docs.swarms.world/en/latest/swarms/structs/skill_orchestra/
[21]: https://docs.swarms.world/en/latest/swarms/cli/cli_reference/
[22]: https://github.com/kyegomez/swarms
[23]: https://docs.swarms.world
[24]: https://discord.gg/2bZ37UmP2a
[25]: /tag/artificial-intelligence?source=post_page-----6f0754734677---------------------------------------
[26]: /tag/machine-learning?source=post_page-----6f0754734677---------------------------------------
[27]: /tag/startup?source=post_page-----6f0754734677---------------------------------------
[28]: /tag/data-science?source=post_page-----6f0754734677---------------------------------------
[29]: /tag/ai-agent?source=post_page-----6f0754734677---------------------------------------
[30]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F6f0754734677&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677&user=Kye+Gomez&us
erId=65d61d27a2c8&source=---footer_actions--6f0754734677---------------------clap_footer------------------
[31]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F6f0754734677&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677&user=Kye+Gomez&us
erId=65d61d27a2c8&source=---footer_actions--6f0754734677---------------------clap_footer------------------
[32]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F6f0754734677&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677&user=Kye+Gomez&
userId=65d61d27a2c8&source=---footer_actions--6f0754734677---------------------repost_footer------------------
[33]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F6f0754734677&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40kyeg%2Fintroducing-swarms-v10-async-sub-agents-skillorchestra-and-more-6f0754734677&source=---foo
ter_actions--6f0754734677---------------------bookmark_footer------------------
[34]: /@kyeg?source=post_page---post_author_info--6f0754734677---------------------------------------
[35]: /@kyeg?source=post_page---post_author_info--6f0754734677---------------------------------------
[36]: /@kyeg?source=post_page---post_author_info--6f0754734677---------------------------------------
[37]: /@kyeg/followers?source=post_page---post_author_info--6f0754734677---------------------------------------
[38]: /@kyeg/following?source=post_page---post_author_info--6f0754734677---------------------------------------
[39]: https://swarms.ai
[40]: http://kyegomez.com
[41]: https://help.medium.com/hc/en-us?source=post_page-----6f0754734677---------------------------------------
[42]: https://status.medium.com/?source=post_page-----6f0754734677---------------------------------------
[43]: /about?autoplay=1&source=post_page-----6f0754734677---------------------------------------
[44]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----6f0754734677-------------------------------------
--
[45]: mailto:pressinquiries@medium.com
[46]: https://blog.medium.com/?source=post_page-----6f0754734677---------------------------------------
[47]: https://medium.com/store
[48]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----6f0754734677--------------------
-------------------
[49]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----6f0754734677-----------------------------
----------
[50]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----6f0754734677------------------
---------------------
[51]: https://speechify.com/medium?source=post_page-----6f0754734677---------------------------------------
```
