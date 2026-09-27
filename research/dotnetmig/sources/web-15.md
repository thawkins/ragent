# Web source

- URL: https://learn.microsoft.com/en-in/answers/questions/5938516/migrating-c-visual-studio-2022-17-8-from-net-4-7-t
- Title: Migrating C# Visual Studio 2022 (17.8) from .NET 4.7 to .NET 8 - Microsoft Q&amp;A
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:25:28.371275406+00:00
- Relevance: Medium — multiple title terms match query


```text
For migrating a C# project from .NET Framework 4.7 to .NET 8 in Visual Studio 2022 17.8, the accepted answer says .NET Upgrade Assistant is not in the VS Installer components list but can be installed via Extensions → Manage Extensions → Browse/Marketplace or as a global CLI tool using `dotnet tool install -g upgrade-assistant` (add `--ignore-failed-sources` for custom NuGet feed failures); then right-click the project in Solution Explorer, choose Upgrade, and select .NET 8, though manual code changes may still be needed for older APIs/packages. A 2026-07-05 update states the .NET Upgrade Assistant is deprecated and no longer actively developed, recommending the GitHub Copilot app modernization agent (for VS 2022 17.14.16+ or VS 2026), which analyzes projects/dependencies, produces a migration plan and automated fixes, and handles project file updates, deprecated API replacement, and build fixes. If that agent is unavailable, the legacy Upgrade Assistant can be enabled via Tools > Options > All Settings > Projects and Solutions > Modernization > Enable legacy Upgrade Assistant (then restart), or used as the global tool `upgrade-assistant`, with specific guidance for project types such as ASP.NET, WPF, Windows Forms, and UWP and possible remaining manual changes.
```
