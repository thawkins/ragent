# Web source

- URL: https://blog.codeinside.eu/2024/03/07/upgrade-assistant
- Title: .NET Upgrade Assistant
- Author(s): Code Inside Team
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:29:32.556822586+00:00
- Relevance: High — title matches query


```text
Microsoft’s Upgrade Assistant, available as a Visual Studio extension (adding an “Upgrade project” option in Solution Explorer) or via CLI, is recommended for migrating .NET Framework WPF, WinForms, class libraries, and web apps to the newest .NET; it also supports paths such as UWP to WinUI 3 and older .NET Core to newer versions. Depending on project type, it offers In-Place Upgrade, Side-by-Side, or Side-by-Side Incremental, the last only for ASP.NET web apps, where a parallel .NET Core project and a “bridge” in the original .NET project are created—clever but risky. The author tested it for WPF and class libraries to .NET Core, finding it helps identify incompatible NuGet packages or old framework code; on complex codebases it can sometimes mess with code, but still helps give direction.
```
