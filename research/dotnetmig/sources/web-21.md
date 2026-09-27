# Web source

- URL: https://github.com/dotnet/try-convert
- Title: GitHub - dotnet/try-convert: Helping .NET developers port their projects to .NET Core!
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:26:24.881370783+00:00
- Relevance: High — title matches query


```text
`dotnet try-convert` is an unsupported, open-source global tool from the .NET team that helps migrate .NET Framework projects to .NET Core/.NET SDK-style projects; it is installed or updated with `dotnet tool install -g try-convert` or `dotnet tool update -g try-convert`. It runs only on Windows, should not be used from the Visual Studio developer command prompt due to MSBuild resolution incompatibilities, and is conservative rather than guaranteed to produce a fully working project, with explicit gaps for complex custom builds, .NET Core-incompatible APIs, and unsupported project types such as Xamarin, WebForms, and WCF; source control is recommended. Internally, it evaluates a project, replaces it in memory with a simple SDK template, re-evaluates in the same folder, applies rules to known properties/items, and produces a diff identifying properties/items to remove, keep, or change to `Update` syntax. It is based on Srivatsn Narayanan’s ProjectSimplifier project, and when built locally the tool lives under `/artifacts/bin/try-convert/Debug/net6.0/try-convert.exe`.
```
