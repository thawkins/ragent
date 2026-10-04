# Web source

- URL: https://github.com/awslabs/startups/tree/main/migrate
- Title: startups/migrate at main · awslabs/startups
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:27:12.143499818+00:00
- Relevance: Medium - partial query match


```text
The `migration-to-aws` GitHub plugin (awslabs/startups) has been folded into `aws-startup-advisor` in `aws/agent-toolkit-for-aws` and is no longer maintained. It provides AI agent skills for migrating workloads to AWS for Claude Code, Codex, and Cursor, supporting Azure→AWS (App Service, AKS, VMs, Functions, PostgreSQL/MySQL Flexible Server, Cosmos DB, Redis, Blob, Service Bus, Event Hubs, VNet, Azure OpenAI), GCP→AWS (Cloud Run, Cloud SQL, GKE, Cloud Functions, Pub/Sub, Cloud Storage, VPC), and Heroku→AWS (Dynos default to Elastic Beanstalk, Postgres, Redis, Kafka, Private Spaces, Pipelines, 13+ add-ons). It runs phased assessments, generates production-ready Terraform (vpc.tf, compute.tf, database.tf, security.tf, baseline.tf with GuardDuty, CloudTrail, IMDSv2, ECR scanning), selects database migration tools (pg_dump, pgcopydb, AWS DMS), and estimates monthly costs across Premium/Balanced/Optimized tiers using live AWS Pricing API. Live Azure discovery is planned; live GCP/Heroku discovery uses read-only CLI commands with consent and drift detection against Terraform. The related `agent-advisor` skill recommends runtimes (AgentCore, ECS/EKS, Lambda, AWS Batch, Lambda MicroVMs) via deterministic scoring and generates POCs. Requirements include Claude Code ≥2.1.29, Codex, or Cursor ≥2.5, AWS CLI, and an authenticated gcloud/heroku CLI.
```
