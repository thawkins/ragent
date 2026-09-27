# Web source

- URL: https://workos.com/blog/agent-identity-authorization-audit
- Title: Agents need identity, authorization, and audit in the same place — WorkOS
- Author(s): WorkOS
- Language: English
- Published (UTC): 2026-09-01T00:00:00+00:00
- Captured (UTC): 2026-09-16T21:45:17.961495976+00:00
- Relevance: Medium — multiple title terms match query


```text
WorkOS’s blog compares FusionAuth 1.69 (published Aug. 25) and WorkOS Agent Registration (announced Aug. 4) for agent identity, authorization, and audit. FusionAuth 1.69 adds an AI Agent entity type and entity lifecycle webhooks, plus RFC 9207 issuer identification on by default and signature-verification key selection (DPoP server-side support dates to 1.63, with React/Angular/Vue SDK support on Aug. 5), but its 50+ authentication lifecycle events, admin audit log, and system event log cover only events inside FusionAuth; application-level agent actions, such as which agent read a document, must be recorded and joined by the app, a problem it links to FusionAuth’s 2026 report finding 88% of technology leaders say AI deployment outpaces identity/security infrastructure. WorkOS’s AuthKit uses auth.md and RFC 9728 discovery for agent-driven registration with anonymous, service_auth, and refresh types, issues short-lived scoped credentials carrying sub (agent registration ID) and act (delegated user, per RFC 8693), and provides Audit Logs filterable by agent registration ID plus Log Streams to Datadog, Splunk, AWS S3, Google Cloud Storage, Microsoft Sentinel, Snowflake, or generic HTTPS. It notes the tradeoff that WorkOS is a hosted dependency—if unreachable, agents cannot register or rotate credentials—while FusionAuth fits self-hosted teams that build their own application-event pipeline.
```
