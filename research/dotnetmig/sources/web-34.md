# Web source

- URL: https://atalupadhyay.wordpress.com/2025/11/13/from-monolith-to-modern-a-net-developers-practical-roadmap-to-the-cloud
- Title: From Monolith to Modern: A .NET Developer&#8217;s Practical Roadmap to the Cloud
- Author(s): —
- Language: English
- Published (UTC): 2025-11-13T11:35:59+00:00
- Captured (UTC): 2026-09-24T11:29:47.751931289+00:00
- Relevance: High — title + snippet match query


```text
The article outlines a phased roadmap for migrating a .NET Framework 4.x monolith to cloud-native .NET 8: assess with Microsoft’s .NET Upgrade Assistant, re-platform SQL Server to managed Azure SQL Database via Azure Data Migration Service and move the app as-is to Azure App Service, refactor using the Strangler Fig Pattern, then re-architect with Docker containers, Linux App Service, microservices, and Azure Kubernetes Service (AKS). Its hands-on lab migrates a “LegacyWeather” .NET Framework 4.8 MVC app to a .NET 8 container on App Service using `upgrade-assistant analyze/upgrade`, code changes such as `Global.asax` to `Program.cs` and `web.config` to `appsettings.json`, Azure SQL scripting, Docker, Azure Container Registry, and Azure CLI deployment. It warns against big-bang rewrites and cites porting gotchas including removal of `System.Web`/`HttpContext`/`HttpModules`/`HttpHandlers`, migration to ASP.NET Core Identity or JWTs, and moving in-memory session state to a distributed cache like Redis; it also contrasts re-hosting with re-platforming and notes App Service’s 99.95% SLA and automated scaling/backups.
```
