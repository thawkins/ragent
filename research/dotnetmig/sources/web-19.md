# Web source

- URL: https://talkthinkdo.com/guides/legacy-modernisation/dotnet-framework-migration-claude-code
- Title: .NET Framework to .NET 10 Migration with Claude Code | Talk Think Do
- Author(s): Matt Hammond
- Language: English
- Published (UTC): 2026-04-19T00:00:00+00:00
- Captured (UTC): 2026-09-24T11:26:49.058698102+00:00
- Relevance: High — title matches query


```text
The guide outlines an enterprise .NET Framework 4.x-to-.NET 10 migration using a harness-first Claude Code workflow: configure Directory.Build.props, pre-approved permission sets, and PostToolUse hooks (dotnet format/build after edits) before prompting; use the .NET Upgrade Assistant for analysis and project-file conversion; migrate bottom-up project-by-project starting with shared libraries; and use parallel `--worktree` agents so one developer can supervise three or four migrations, while budgeting 30-50% of time for human review. Preconditions include .NET 10 SDK alongside .NET Framework, a green baseline build/test, clean source control, dependency inventory, and a chosen LTS target; the guide says Claude Code cannot fully automate migration (expect 50-70% AI-generated changes with human review), warns about EF6 lazy loading, string comparison, WCF (no direct modern .NET equivalent; gRPC/REST/CoreWCF options), and Windows APIs, and notes Managed Agents cloud environments can run unattended overnight with the `managed-agents-2026-04-01` beta header. It reports three cases: a 150k-line .NET Framework 4.7.2 MVC/EF6/Autofac app migrated in seven weeks with ~65% Claude Code changes and 98.5% test pass (23 EF6 lazy-loading failures), an 85k-line 4.8 Web API in four weeks with ~70% AI-authored code and four hook-caught type mismatches, and a 220k-line 18-project 4.6/4.7.2 migration in eleven weeks with ~70% AI-authored code and 40-50% faster delivery; a Q2 2026 AI Velocity Report claims 91.6% of production code AI-authored (100% senior-reviewed) and 40-50% faster delivery.
```
