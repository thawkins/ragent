# Web source

- URL: https://learn.microsoft.com/en-us/dotnet/core/porting/framework-overview
- Title: Port from .NET Framework to .NET - .NET Core
- Author(s): adegeo
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:26:33.024926119+00:00
- Relevance: Medium-high — snippet matches query


```text
Microsoft’s .NET Framework-to-.NET porting overview says many projects port relatively easily—libraries, console apps, and desktop apps whose app model exists in .NET need little change, while adopting a new app model such as ASP.NET Core from ASP.NET requires more work. WinForms and WPF are available in .NET but remain Windows-only; porting must account for SDK-style project files, unavailable APIs, unported third-party controls, and retired technologies, while the Windows Compatibility Pack (Microsoft.Windows.Compatibility NuGet) supplies much of the .NET Framework API surface. .NET Framework compatibility mode, introduced in .NET Standard 2.0 and extended to WinForms/WPF in .NET Core 3.0, lets projects reference some .NET Framework libraries, though .NET Standard 2.0 was the last version to support .NET Framework. Unsupported technologies include application domains, remoting (BeginInvoke/EndInvoke throw PlatformNotSupportedException), CAS, security transparency, System.EnterpriseServices, and WF (alternative CoreWF); SDK-style projects use `<TargetFramework>` (e.g., `net472`) instead of `<TargetFrameworkVersion>v4.7.2`. Recommended tools include GitHub Copilot modernization, Azure Migrate application and code assessment, .NET Upgrade Assistant, try-convert, and Platform compatibility analyzer, with guidance to examine dependencies, move to PackageReference and SDK-style projects, retarget to at least .NET Framework 4.7.2, and target .NET 8 LTS (or .NET 8+ for WinForms/WPF).
```
