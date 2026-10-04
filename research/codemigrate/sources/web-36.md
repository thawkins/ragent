# Web source

- URL: https://aws.amazon.com/blogs/containers/ai-powered-eks-migration-assessment-with-amazon-bedrock-agentcore
- Title: AI-powered EKS migration assessment with Amazon Bedrock AgentCore | Amazon Web Services
- Author(s): -
- Language: English
- Published (UTC): 2026-09-30T15:34:19+00:00
- Captured (UTC): 2026-10-02T21:28:20.686222345+00:00
- Relevance: Medium - multiple title terms match query


```text
An AWS blog post describes an AI-powered Amazon EKS migration assessment agent built with Amazon Bedrock AgentCore and the open-source Strands Agents SDK. The agent analyzes source code and container artifacts from OpenShift, Azure, on-premises/self-managed Kubernetes, WebSphere/JBoss/WebLogic, and Docker Swarm to produce a 0–100 readiness score, severity-ranked blockers, and an EKS migration plan with effort estimates; it complements AWS Migration Hub, Application Discovery Service, AWS Transform for Containers, and Konveyor through code-level reasoning. Its workflow uses Terraform, an ECS Fargate UI behind ALB/WAF with optional Cognito, S3, DynamoDB, and AgentCore tools (clone_repository, assess_current_state, analyze_source_code, scan_dependencies, check_eks_compatibility, generate_migration_plan), with AgentCore Memory improving accuracy over time and AgentCore Runtime scaling concurrent assessments. Estimated cost is $0.18–$0.57 per assessment and about $96/month for always-on infrastructure, with claimed assessment time reduced from days to minutes.
```
