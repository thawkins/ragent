# Web source

- URL: https://www.kognitos.com/blog/agent-observability
- Title: Agent Observability: What Traces Show and What Auditors Need
- Author(s): Kognitos
- Language: English
- Published (UTC): 2026-10-21T00:00:00+00:00
- Captured (UTC): 2026-11-02T11:12:13.529718100+00:00
- Relevance: Medium - multiple title terms match query


```text
Agent observability instruments AI agents using OpenTelemetry GenAI semantic conventions, with tool calls, model invocations, and retrieval steps as child spans, and is used for latency/cost monitoring, tracing, evaluation, and governance via tools such as Arize Phoenix, Langfuse, LangSmith, Braintrust, and Datadog LLM Observability. It is effective for debugging—answering what the system did—but not for audit evidence of why an action was authorized, because traces record model production rather than permission, evaluation is sampled, retention windows are short, and storing full prompt text in span attributes is an anti-pattern. Audit-grade records instead require the exact input, model determination/confidence and threshold, the rule and rule version that fired, the action taken in the system of record, reviewer identity/time, and replay capability, which requires deterministic execution. Kognitos is not an observability platform but exports every run over OpenTelemetry into Datadog, Splunk, or any SIEM; it uses a human-approved LLM-drafted procedure executed by a symbolic executor so every write comes from a versioned rule, and its plain-English business journal logs variables, rules, actions, timestamps, and policy citations, is accepted as SOX 404 evidence, and is replayable.
```
