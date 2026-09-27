# Web source

- URL: https://learn.microsoft.com/en-us/dotnet/fundamentals/apicompat/overview
- Title: API compatibility tools - .NET
- Author(s): dotnet-bot
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:32:25.556902291+00:00
- Relevance: High — title + snippet match query


```text
The .NET SDK’s API compatibility tooling helps library authors validate multi-targeted assemblies and packages for unintentional breaking changes, such as ensuring code compiled against a .NET Standard 2.0 binary can run against a .NET 6 binary. It supports comparing different target-framework versions or validating a newer version against a baseline via MSBuild tasks at compile/pack time, the `Microsoft.DotNet.ApiCompat.Tool` global tool, package validation, or assembly validation for non-packable apps; MSBuild assembly validation requires a reference to `Microsoft.DotNet.ApiCompat.Task`. By default it checks compatibility, while strict mode performs equality checks (no API additions or assembly changes, even compatible ones), supports servicing and API-change tracking, records differences in a suppression file when `ApiCompatGenerateSuppressionFile` is true, and is enabled with `--strict-mode`/`--enable-strict*` or MSBuild properties. Source-compatible changes such as adding an optional parameter or changing a constant’s value can still cause runtime problems if consumers are not recompiled.
```
