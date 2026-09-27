# Web source

- URL: https://learn.microsoft.com/en-us/aspnet/core/migration/fx-to-core?view=aspnetcore-10.0
- Title: Migrate from ASP.NET Framework to ASP.NET Core
- Author(s): wadepickett
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:32:41.300369414+00:00
- Relevance: Medium — multiple title terms match query


```text
Microsoft's guide states that migrating most production apps from ASP.NET Framework to ASP.NET Core is non-trivial due to technical debt (e.g., pervasive System.Web/HttpContext dependencies, outdated packages, legacy build tools, deprecated APIs), cross-cutting concerns (session state, authentication/authorization, logging, caching, error handling, configuration, dependency injection), library dependency chains requiring postorder depth-first upgrades and multi-targeting, and architecture differences in hosting, middleware vs. HTTP modules/handlers, request processing, and performance. It recommends incremental migration via the Strangler Fig pattern for larger or production-continuous projects, while in-place migration may work for sufficiently small apps, and notes the .NET Generic Host can help bring modern .NET infrastructure to ASP.NET Framework applications. The page was last updated on 2026-07-08.
```
