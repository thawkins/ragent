# Web source

- URL: https://fullscale.io/blog/dotnet-framework-to-dotnet-migration
- Title: .NET Framework to .NET Core Migration: A Decision Guide From a Team That’s Done It - Full Scale
- Author(s): Matt Watson
- Language: English
- Published (UTC): 2026-09-06T00:00:00+00:00
- Captured (UTC): 2026-09-24T11:31:05.456331541+00:00
- Relevance: Medium — multiple title terms match query


```text
Full Scale’s guide argues that migrating from Windows-only .NET Framework to modern cross-platform .NET is worthwhile because .NET Framework 4.8.1 is the final version—still supported via Windows but limited to security and reliability fixes—while active development goes into modern .NET (renamed from .NET Core at .NET 5; current LTS is .NET 10). The main blockers are parts missing from modern .NET—WCF, Web Forms, and Windows-only APIs/dependencies—which must be rewritten rather than ported and therefore drive timelines. The author’s Stackify migration replaced WCF services ingesting billions of data points per day on Azure with .NET Core Web APIs running on Linux in Kubernetes, using an incremental Strangler Fig approach instead of a big-bang rewrite. Tools such as Microsoft’s .NET Upgrade Assistant and AWS Transform automate mechanical conversion, and AI can help map dependencies, document legacy services, and draft equivalents, but AI-generated code still carries bug and security risk and requires senior-engineer review.
```
