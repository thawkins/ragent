# Web source

- URL: https://github.com/sshaaf/migIQ
- Title: GitHub - sshaaf/migIQ: An experimental project showcasing code migrations using agents, harness, skills and more
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:26:20.172758299+00:00
- Relevance: Medium - multiple title terms match query


```text
MigIQ is an AI-powered application migration orchestrator for modernizing legacy apps to cloud-native platforms; it requires `rgctl` (installed via `rgctl install --skill`, checking `~/.local/bin/rgctl` even off PATH), Claude Code with agent skills, and Node.js 14+ for `npx @sshaaf/migiq`, installed globally with `npx @sshaaf/migiq -g` (recommended) or locally, then restarted in Claude Code and run via interactive `/migiq` (best for learning/<1-hour migrations) or autonomous Migrator Agent (`AGENT.md`, best for >1-hour/background/parallel work). Its 5-phase workflow uses core skills mig-rgctl (analysis; run `rgctl discover .` from the app repo root, with `rgctl --no-daemon discover .` for reproducible CI artifacts), mig-prompt-builder (requirements/migration-prompt.md), mig-plan (tasks.md/UserStory.md), mig-execute (code changes, tests, containers, OpenShift manifests), and reporting (summary, metrics, next steps), with mig-test-gen, mig-containerize, and mig-deploy also named. It claims support for 31 languages and migrations such as Java EE → Spring Boot → Quarkus, .NET Framework → .NET 8, Node.js 12 → 20, monolith → microservices, and on-prem → OpenShift/Kubernetes/Docker/Podman, producing artifacts like graph.json/graph.html, tests/containers/deployments, and MIGRATION_REPORT.md, with an example completed Spring Boot-to-Quarkus migration deployed to OpenShift in `examples/spring-boot-to-quarkus.tar.gz`.
```
