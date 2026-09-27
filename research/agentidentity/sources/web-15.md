# Web source

- URL: https://docs.cloud.google.com/iam/docs/agent-identity-overview
- Title: Agent Identity overview &nbsp;|&nbsp; Identity and Access Management (IAM) &nbsp;|&nbsp; Google Cloud Documentation
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-16T21:44:57.685134050+00:00
- Relevance: Medium — multiple title terms match query


```text
Google Cloud's Agent Identity provides each agent a strongly attested SPIFFE-based cryptographic identity—formatted as `spiffe://TRUST_DOMAIN/resources/SERVICE/RESOURCE_PATH` and, in IAM, `principal://TRUST_DOMAIN/resources/SERVICE/RESOURCE_PATH`—so it can authenticate to MCP servers, cloud resources, endpoints, and other agents either for itself or for an end user. Unlike service accounts, agent identities are not shared by default, cannot be impersonated, do not allow long-lived service-account keys, and Google Cloud access tokens are cryptographically bound to unique auto-provisioned X.509 certificates valid for 24 hours, using mTLS for Google Cloud APIs and DPoP across Agent Gateway. Agent Identity auth manager manages API keys, OAuth client IDs/secrets, and delegated end-user OAuth tokens; supports 3-legged OAuth, 2-legged OAuth, API key, agent's own cloud identity, and HTTP basic (not recommended); and integrates with Agent Registry, Agent Gateway, IAM, Principal Access Boundary, VPC Service Controls, Context-Aware Access, and audit logging. Deleting an agent does not remove IAM bindings for its principal, a replacement agent gets a new resource ID/principal, and legacy Cloud Storage bucket roles such as `storage.legacyBucketReader` cannot be granted to agent identities.
```
