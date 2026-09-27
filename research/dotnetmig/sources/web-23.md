# Web source

- URL: https://developersde.azurewebsites.net/2018/01/09/migrating-to-net-core
- Title: Few things about migrating to .NET Core
- Author(s): @ddobric
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:27:37.230750540+00:00
- Relevance: High — title + snippet match query


```text
An article on migrating .NET Framework projects to .NET Core recommends .NET Standard 2.0 compatibility mode for temporarily referencing existing .NET Framework binaries, warning that such references may compile but fail at runtime—for example, .NET Core referencing System.DirectoryServices or System.Configuration compiles but fails because those assemblies are not in .NET Core. It describes the API Port tool (aka.ms/apiport), which scans application and third-party binaries to produce a portability report and table of unavailable or must-migrate APIs (example: `apiport analyze -f C:\src\fabrikam\bin\Fabrikam.Shared.dll -t ".NET Standard + Platform Extensions"`), and the Microsoft.Windows.Compatibility NuGet meta-package containing about 40 Windows-related components such as System.Configuration.ConfigurationManager and System.Drawing, advising OS checks via `RuntimeInformation.IsOSPlatform(OSPlatform.Windows)` to avoid a PlatformNotSupportedException for Registry. It also notes the Roslyn-based API Analyzer NuGet package detects deprecated APIs and cross-platform issues.
```
