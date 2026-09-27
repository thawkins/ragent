# Web source

- URL: https://dev.to/zenika/gitlab-runners-which-topology-for-fastest-job-execution-5bma
- Title: 🦊 GitLab Runners: Which Topology for Fastest Job Execution?
- Author(s): @
- Language: English
- Published (UTC): 2026-01-30T17:28:43+00:00
- Captured (UTC): 2026-09-24T14:13:07.931736866+00:00
- Relevance: High — title matches query


```text
This article compares GitLab Runner topologies by job-execution overhead, concluding that Shell and Docker executors on single servers deliver the fastest jobs: Shell is the absolute fastest (no containers or image pulls, local git/cache reuse, no isolation), while Docker adds isolation with fast local cache access, cached image layers, and quick warm container startup. GitLab Shared SaaS runners are slowest due to multi-tenancy and shared resources; fixed Kubernetes offers balanced warm performance but has queue/capacity limits and remote-cache latency, while autoscaling Kubernetes and Docker Autoscaler/Fleeting provide unlimited capacity but suffer cold starts (Kubernetes node provisioning 30s–2min; VM provisioning 15+ seconds) plus full image pulls and git clones. The article recommends vertical scaling of well-provisioned single servers (SSD, dozens of concurrent jobs, predictable costs, low maintenance) for fastest execution, and references a follow-up on achieving 3-second jobs on million-line codebases.
```
