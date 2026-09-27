# Web source

- URL: https://www.ssh.com/blog/pam-ai-agents-ssh
- Title: PAM &amp; AI Agents | SSH
- Author(s): Miikka Sainio
- Language: English
- Published (UTC): 2026-04-30T08:30:00+00:00
- Captured (UTC): 2026-09-16T21:48:24.619047824+00:00
- Relevance: High — title + snippet match query


```text
The SSH blog argues AI agents are moving from content generation to execution, using existing protocols (HTTP, SSH/SFTP, file systems) but adding non-deterministic, model-agnostic autonomy that challenges PAM models built for humans and static service identities. It recommends sandboxing runtimes, ephemeral/attestable non-human identities (e.g., SPIFFE/SPIRE), PAM-mediated or vaulted authentication, scoped delegation, fine-grained authorization (ABAC/PBAC) at API endpoint/method level, DLP, UEBA, and session recording/audit logs streamed to SIEM. Remaining gaps include ephemeral machine identity in PAM, context-aware API authentication, MCP-layer governance, and safe user privilege delegation to AI agents. It presents PrivX as a central control point brokering agent-to-machine SSH/HTTP access with short-lived certificates, just-in-time scoped access, granular command/API enforcement, and SIEM streaming.
```
