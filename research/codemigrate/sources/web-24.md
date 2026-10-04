# Web source

- URL: https://stanleycyang.com/writing/coding-agent-framework-migration
- Title: Migrate Coding-Agent Frameworks Without Losing Behavioral Guarantees
- Author(s): https://stanleycyang.com/about
- Language: English
- Published (UTC): 2026-07-18T00:00:00+00:00
- Captured (UTC): 2026-10-02T21:26:31.730803005+00:00
- Relevance: Medium - multiple title terms match query


```text
To migrate a coding agent, preserve observable contracts rather than translating framework classes one-for-one: inventory inputs, tools, state, approvals, stop conditions, telemetry, and artifacts; freeze a representative evaluation set; build a thin adapter; then dual-run, canary, and roll back through a versioned boundary, keeping mechanical rewrites separate from behavioral changes. This applies to vendor SDKs, open-source agent frameworks, or internal orchestrators; the article cites the OpenHands platform paper’s explicit agents, tools, sandboxing, and benchmark integration as useful migration surfaces without assuming it is the source or destination. Migration should be justified by measurable constraints (e.g., unsupported protocol, missing cancellation/durable state, latency/cost, security, observability, maintenance risk) against a recorded baseline and minimum acceptance threshold, with a neutral `CodingAgentRuntime` interface, compatibility matrix, pinned migration corpus, deterministic fake adapters, and end-to-end real-model evaluation—e.g., recording whether a 30 s external tool timeout is preserved or changed to an adapter default. Treat state migration as a data migration via drain, pin, translate, or restart; dual-run without duplicating side effects; canary with a working rollback and versioned events/artifacts; and diagnose regressions by first divergent layer, using git bisect with deterministic tests where possible.
```
