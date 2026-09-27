# Web source

- URL: https://blog.autodesk.io/migrating-from-net-48-to-net-core-8
- Title: Autodesk Developer Blog : Migrating from .NET 4.8 to .NET Core 8
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:27:18.334994038+00:00
- Relevance: Medium — partial query match


```text
Revit 2025 API is built on .NET 8, a major jump from .NET 4.8 used by Revit 2024 and earlier; it is .NET 8-only, so Revit add-ins must be recompiled. Autodesk published a 1.25-hour January webinar recording and slide deck on the Autodesk Desktop API .NET Core 8.0 migration, plus Madhukar Moogala’s AutoCAD API migration guides and a Revit migration guide in the Revit 2025 SDK’s RevitAPI.chm (also PDF migrating_to_net_core_8.pdf). The guide covers C#/C++/CLI upgrade steps (e.g., CLRSupport true→NetCore, TargetFrameworkVersion→net8.0-windows, WindowsDesktop FrameworkReference), recommends CefSharp versions 119.4.3, 119.4.30, and 119.1.20 and Newtonsoft.Json 13.0.1, notes multi-targeting with older .NET 4.8 Revit releases, and lists common issues such as MSB3277, CA1416, assembly loading/runtimeconfig.json/deps.json changes, double ToString formatting (“G15”), Process.Start UseShellExecute default change, UTF-8 Encoding.Default, List<T>.Sort ordering, CoreWCF for System.ServiceModel, and nullable types.
```
