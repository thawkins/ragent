# Web source

- URL: https://www.ssw.com.au/rules/migration-plans
- Title: Do you create a migration plan? | SSW.Rules
- Author(s): @SSW_TV
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:32:51.912788022+00:00
- Relevance: High — title + snippet match query


```text
SSW’s .NET Framework 4.x migration plan stresses preparation: audit architecture and technical debt, investigate blurred N-tier dependencies such as System.Web in app/data layers and third-party integrations, check on-prem infrastructure for .NET 10 runtimes, and identify obsolete APIs including AppDomains, .NET Remoting, and CAS. It recommends converting csproj files to SDK-style via the `try-convert` tool, multi-targeting TFMs (e.g., `net48;net8.0`), working bottom-up in N-tier or inside-out in Onion architecture, and treating SYSLIB warnings like SYSLIB0011 (BinaryFormatter) as blockers with `<WarningsAsErrors>SYSLIB*</WarningsAsErrors>`. The resulting migration backlog should include PBIs for architectural concerns and breaking changes, with incremental fixes committed as projects compile.
```
