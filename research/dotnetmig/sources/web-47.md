# Web source

- URL: https://learn.microsoft.com/en-us/dotnet/core/install/upgrade
- Title: Upgrade to a new .NET version - .NET
- Author(s): adegeo
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:31:57.251038445+00:00
- Relevance: Medium — multiple title terms match query


```text
Microsoft's .NET upgrade guidance recommends installing the new .NET SDK (via installers/archives, OS package managers, or automatically by upgrading Visual Studio), noting the only required source change is updating the `TargetFramework` property (e.g., net6.0 → net8.0) in .csproj/.vbproj/.fsproj files, rebuilding, and possibly running `dotnet workload restore`; common upgrade drivers are unsupported versions, new OS support, and important API/performance/security features. To control when changes land, it recommends version pinning: a `global.json` file (e.g., `dotnet new globaljson --sdk-version 9.0.100 --roll-forward latestFeature`), with `rollForward: disable` combined with `<RestorePackagesWithLockFile>` and `<RestoreLockedMode>` to lock both SDK and package dependency graph, plus the `AnalysisLevel` property (e.g., 9.0) to keep analyzer rules from a prior .NET version even on a newer SDK. Other mechanisms include package lock files (packages.lock.json) committed to source control, central package management via Directory.Packages.props with `ManagePackageVersionsCentrally`, package source mapping in nuget.config, and MSBuild version control (VS 2022 17.8 ships MSBuild 17.8; versions can be selected via pinned SDK, Developer Command Prompt, or direct MSBuild invocation). CI pipelines typically require only version value changes, while hosting updates require installing the new runtime, changing container `FROM` statements (e.g., mcr.microsoft.com/dotnet/aspnet:9.0), or a configuration change in services like Azure App Service; the page was last updated 2026-08-13.
```
