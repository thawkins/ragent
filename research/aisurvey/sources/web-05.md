# Web source

- URL: https://www.fiddler.ai/blog/opentelemetry-ai-observability-guide
- Title: OpenTelemetry for AI Observability: What It Covers and Where It Stops | Fiddler AI Blog
- Author(s): Fiddler Team, @fiddler_ai
- Language: English
- Published (UTC): 2026-10-30T23:58:05.456+00:00
- Captured (UTC): 2026-11-02T11:12:37.809132977+00:00
- Relevance: Medium - multiple title terms match query


```text
OpenTelemetry is presented as the vendor-neutral telemetry standard for AI observability, providing distributed tracing, metrics, and logs across LLM calls, agent workflows, and infrastructure; its GenAI semantic conventions—still at Development status—standardize attributes such as `gen_ai.provider.name`, model, input/output token usage, and finish reasons, but do not cover output evaluation, safety scoring, or content quality. Production AI observability therefore needs OpenTelemetry as the data plane plus an evaluation/control layer, which Fiddler provides by ingesting OTel telemetry and applying in-environment Centor Models for faithfulness, toxicity, and policy-compliance scoring without sampling, along with Guardrails checks under 80ms. The article also flags production pitfalls including lost multi-turn session context, prompt/completion data-governance risk, span explosion in multi-agent systems, and evaluation-latency tradeoffs, and cites a financial-services agent example where traditional APM showed green while compliance violations occurred.
```
