# Web source

- URL: https://www.linkedin.com/posts/sukhmeet-k-bhatia_dotnet8-migration-csharp-activity-7387117933715582977-cL_e
- Title: Migrating Legacy .NET Apps to .NET 8 — A Developer’s Roadmap | Sukhmeet Kour Bhatia
- Author(s): Sukhmeet Kour Bhatia
- Language: English
- Published (UTC): 2025-10-23T13:29:21.383+00:00
- Captured (UTC): 2026-09-24T11:31:45.075842338+00:00
- Relevance: Medium — multiple title terms match query


```text
In a LinkedIn post, Sukhmeet K Bhatia outlines a phased roadmap for migrating legacy .NET Framework apps to .NET 8: use Microsoft Upgrade Assistant (`dotnet tool install -g upgrade-assistant`; `upgrade-assistant upgrade MyApp.sln`), target .NET Standard 2.0 for old class libraries, replace WCF with gRPC/REST, WebForms with Razor Pages/Blazor, EF6 with EF Core 8, and Global.asax with the Program.cs minimal hosting model; containerize (`docker build -t myapp:latest .`, `docker run -d -p 8080:80 myapp`) and deploy to Azure App Service, AKS, or another container platform; add Health Checks and Application Insights. The post cites .NET 8 benefits as AOT startup speed, a unified API/Web/Cloud platform, better memory efficiency, and LTS, and advises modernizing in phases rather than rewriting everything at once.
```
