# Web source

- URL: https://www.splunk.com/en_us/blog/learn/ai-agent-observability.html
- Title: What Is AI Agent Observability? A Practitioner Guide | Splunk
- Author(s): Jackson Wells
- Language: English
- Published (UTC): 2026-10-22T00:00:00+00:00
- Captured (UTC): 2026-11-03T21:05:24.610852300+00:00
- Relevance: Medium - multiple title terms match query


```text
AI agent observability captures, connects, and evaluates every production step—LLM calls, tool calls, retrievals, and handoffs—using sessions, traces, and spans plus four signals: traces, evals, cost, and guardrail decisions; it asks “was the agent right?” rather than only “did it run?” The article cites MAST’s analysis of more than 1,600 traces finding 41%–86.7% failure rates across seven open-source multi-agent systems and 14 failure modes, Claw-Eval’s finding that trajectory-blind evaluation missed 44% of safety violations, Anthropic’s estimate that agents use about 4× chat tokens and multi-agent systems about 15×, and Gartner’s prediction that inference costs per agentic workflow will rise more than fivefold through 2028. It notes that frontier-judge sampling typically grades only 1%–5% of traffic, while purpose-built small evaluator models can cost up to 98% less than LLM-as-judge, making full-traffic grading more affordable; Splunk Agent Observability combines tracing, Splunk Luna evaluators, Signals pattern detection, and runtime guardrails, with the article recommending starting with one agent, instrumenting spans, grouping by session, evaluating the highest-risk failures, reviewing patterns weekly, and promoting trusted evals to guardrails.
```
