# Web source

- URL: https://tipsmake.com/it-is-possible-to-export-code-from-net-framework-to-net-core
- Title: It Is Possible to Export Code from. NET Framework to. NET Core
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:26:10.303227011+00:00
- Relevance: High — title + snippet match query


```text
The article states that code can be exported from .NET Framework to .NET Core, but the conversion depends on how tied developers are to Windows. Microsoft released a beta Windows Compatibility Pack that exposes more than 20,000 APIs previously available only in .NET Framework, downloadable via the NuGet package Microsoft.Windows.Compatibility; .NET Framework remains Windows-only, while open-source .NET Core is optimized for web apps across Windows, Linux, and macOS, though developers needing WinForms, WPF, or ASP.NET must stay on .NET Framework. Migration should be step-by-step—for example, moving an ASP.NET MVC app on Windows Server to ASP.NET Core on Linux via Azure—with Microsoft recommending ASP.NET Core while still targeting .NET Framework, then switching to .NET Core on Windows, and finally moving to Linux/Azure, though the order can vary. Microsoft also provides porting instructions and the API Portability Analyzer for identifying third-party dependencies.
```
