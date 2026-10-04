# Web source

- URL: https://apitree.ai/migrate/agent
- Title: Migration Agent — Zero-Risk API Migration
- Author(s): apitree
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:28:03.723529019+00:00
- Relevance: Medium - multiple title terms match query


```text
Apitree’s migration agent (`npx @apitree/migrate-agent analyze .`) performs a free, read-only scan (no code changes, no credit card) across TypeScript, Python, Java, Go, Ruby, PHP, C#, and Rust to audit API infrastructure, map detected APIs to apitree equivalents with confidence scores, generate adapters, and verify calls. Example outputs include an audit score of 42/100 (Grade F, 3 security issues, 8 reliability risks) and a migration plan with 5 providers detected, 4 migratable at 100% confidence, and 1 pending; shadow mode verified 247 requests with a 100% match rate and +23ms average latency before canary rollout at 10% → 25% → 50% → 100%, with auto-fallback to the original API on any error. Existing code is never modified (only new adapter files are created), rollback is one command (`apitree-migrate rollback`, works without git), the agent runs locally and sends only provider names/occurrence counts to apitree, and it detects hardcoded keys, missing error handling/timeouts/retries/circuit breakers, hardcoded URLs, mixed auth, no caching/usage monitoring, and over-fetching.
```
