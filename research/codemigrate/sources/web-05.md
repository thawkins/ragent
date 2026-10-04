# Web source

- URL: https://developers.redhat.com/articles/2025/12/09/your-ai-agents-evolved-modernize-llama-stack-agents-migrating-responses-api
- Title: Your AI agents, evolved: Modernize Llama Stack agents by migrating to the Responses API | Red Hat Developer
- Author(s): J William Murdock
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:24:01.529495603+00:00
- Relevance: High - title matches query


```text
Red Hat explains that Llama Stack’s original server-side Agent APIs are deprecated and will be removed in favor of the OpenAI-compatible Responses API, though client-side Agent APIs still function so teams have time to plan migration. The Responses API consolidates tool discovery, execution planning, response synthesis, multi-step reasoning, and automatic tool chaining into one call, with follow-up context via a previous response ID; the Llama Stack Python client’s `Agent` class now uses updated server APIs as a rough approximation. Migration options covered include simple reuse of Python client Agent structures, direct rewrite with Responses API, a `LegacyAgent` emulation layer translating legacy turn calls, and simplified agent classes like `SimpleExampleAgent`; advanced patterns include human-in-the-loop tool approval (`SimpleExampleAgentWithApproval`), guardrail/model safety using Meta Llama Guard, NVIDIA NeMo Guardrails, and Guardrails AI, ReAct/`ReactAgent` alternatives, authored multi-step workflows, LangGraph and CrewAI integrations, and multi-process architecture challenges because Responses API configuration is client-managed rather than server-stored. Complete runnable examples are in a companion Python notebook.
```
