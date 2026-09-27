# Web source

- URL: https://www.cockroachlabs.com/blog/ai-agent-identity-security
- Title: AI Agent Identity Security | CockroachDB
- Author(s): Quentin Packard
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-16T21:44:34.358943676+00:00
- Relevance: Medium — multiple title terms match query


```text
Cockroach Labs argues traditional IAM breaks for AI agents because they are autonomous software processes acting on behalf of humans at machine speed and scale—e.g., making 40 API calls in 30 seconds or accessing 500 customer records for one task—while often running under shared service accounts with broad privileges and no audit trail identifying which agent accessed which records. It cites four production failure patterns: shared service accounts, credentials passed through context windows and exposed to prompt injection (OWASP LLM01), delegation that enables privilege escalation/confused deputy (Norman Hardy, 1988), and audit logs that cannot capture structured execution records. The recommended architecture combines per-session/per-agent short-lived scoped credentials from a token service, least-privilege agent roles, database-layer RBAC and row-level security, explicit permission-narrowing across delegation chains, and structured audit artifacts; the post also cites The State of AI Infrastructure 2026, where 83% of engineering leaders expect AI-driven demand to push infrastructure to failure within two years.
```
