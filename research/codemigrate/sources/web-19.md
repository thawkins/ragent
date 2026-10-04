# Web source

- URL: https://blog.inedo.com/dotnet/dotnet-migration
- Title: What is .NET? What You Need to Know Before Migrating from .NET Framework
- Author(s): -
- Language: English
- Published (UTC): 2025-07-10T06:59:00+00:00
- Captured (UTC): 2026-10-02T21:25:47.864868125+00:00
- Relevance: High - title matches query


```text
Microsoft released .NET 5 on 10 Nov 2020 and .NET 10 in Nov 2025, with major versions now shipping every November and only even-numbered versions after 5 (6, 8, etc.) receiving LTS; .NET Framework is deprecated and tied to OS support, while .NET Core support ended around Dec 2022. Key deprecated Framework features include ASP.NET Web Forms, WCF (Microsoft recommends gRPC; CoreWCF is a partial community alternative), and Windows Workflow Foundation (no official replacement; CoreWF is partial), so affected apps may require rewrites or community ports. Migration costs time/people rather than Microsoft fees, shifts CI builds from MSBuild to .NET CLI, requires an IIS module for web apps, and the Microsoft .NET Upgrade Assistant is still in preview; recommended prep is to inventory apps/versions, assess deprecated components, prioritize with stakeholders, draft a rough schedule, and increase release velocity.
```
