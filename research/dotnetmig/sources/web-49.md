# Web source

- URL: https://learn.microsoft.com/en-us/dotnet/core/porting/porting-approaches
- Title: Porting approaches - .NET Core
- Author(s): StephenBonikowsky
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:32:06.823211348+00:00
- Relevance: High — title + snippet match query


```text
Microsoft’s .NET porting guidance states that API Port is deprecated in favor of binary analysis with .NET Upgrade Assistant, whose backend is shut down so it must be used offline; .NET Upgrade Assistant is also deprecated in favor of the GitHub Copilot modernization chat agent in Visual Studio 2026 and VS 2022 17.14.16+, which analyzes projects/dependencies, creates a migration plan with recommendations and automated fixes, and commits changes for validation or rollback. It presents porting approaches including compiler-first for small/simple projects, staying on .NET Framework until portability issues are resolved, comprehensive planning for larger/complex codebases, and mixed per-project use, plus porting tests and a recommended base-outward, layer-by-layer method that selects a .NET Standard version. Last updated 2025-10-08.
```
