# Web source

- URL: https://learn.microsoft.com/en-us/dotnet/core/porting/premigration-needed-changes
- Title: Prerequisites to port from .NET Framework - .NET Core
- Author(s): StephenBonikowsky
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:24:54.293823222+00:00
- Relevance: High — title + snippet match query


```text
Before porting, make premigration changes while the app still builds/runs on .NET Framework: upgrade MSBuild/Visual Studio to support the target .NET version; target .NET Framework 4.7.2 or higher (recommended because it provides the latest API alternatives where .NET Standard lacks existing APIs), set each project’s Target Framework to .NET Framework 4.7.2 and recompile; convert all references to PackageReference; convert projects to SDK-style format; and update dependencies to their latest versions, using .NET Standard where possible. The .NET Upgrade Assistant is deprecated; instead use the GitHub Copilot modernization chat agent, included with Visual Studio 2026 and Visual Studio 2022 17.14.16 or later, which analyzes projects/dependencies, produces a step-by-step migration plan with recommendations and automated code fixes, commits each change for validation or rollback, and automates updating project files, replacing deprecated APIs, and resolving build issues. Last updated 2025-09-16.
```
