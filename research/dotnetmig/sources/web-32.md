# Web source

- URL: https://www.telerik.com/blogs/meet-dotnet-upgrade-assistant-your-dotnet-5-moving-company
- Title: Meet the .NET Upgrade Assistant, Your .NET 5 Moving Company
- Author(s): @Telerik
- Language: English
- Published (UTC): 2021-04-15T15:02:02+00:00
- Captured (UTC): 2026-09-24T11:28:53.944215118+00:00
- Relevance: High — title matches query


```text
The .NET Upgrade Assistant is a prerelease global command-line tool, installed via `dotnet tool install -g upgrade-assistant` and run as `upgrade-assistant <MySolution.sln>`, that automates migrating .NET Framework apps—Windows Forms, WPF, ASP.NET MVC, console apps, and class libraries—to .NET 5, using `try-convert` version 0.7.212201+ and steps such as backup, SDK-style project conversion, TFM update (e.g., net472 to net5.0), NuGet package updates, template/config migration, and C# source fixes. In a walkthrough migrating Cesar de la Torre’s legacy ASP.NET MVC `eShopLegacyMVCSolution` targeting .NET Framework 4.7.2 (demo in `daveabrock/UpgradeAssistantDemo`), manual fixes were still needed—removing `Global.asax` and `App_Start` files, handling bundling, and using `IHttpContextAccessor` for session access—while extensibility is available via `ExtensionManifest.json`, `-e`, or `UpgradeAssistantExtensionPaths`. The post notes .NET 5 is a Current release supported three months after .NET 6 ships (ending February 2022), while .NET 6 (November release) is LTS for at least three years, with future ability to choose the target release.
```
