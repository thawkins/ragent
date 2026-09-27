# Web source

- URL: https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/port-dotnet-application.html
- Title: Porting a .NET application with Amazon Q Developer in Visual Studio - Amazon Q Developer
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T11:24:36.716513730+00:00
- Relevance: Medium — partial query match


```text
Amazon Q Developer in Visual Studio ports Windows-based .NET applications to Linux-compatible cross-platform .NET applications. Prerequisites require C#-only .NET projects, Microsoft-authored NuGet dependencies, UTF-8 characters (non-UTF-8 is still attempted), and only default IIS configurations if IIS-dependent; Amazon Q creates a code group from the selected project and dependencies and does not transform UI layer components such as Razor views or WebForms ASPX, performing partial transformations if detected. The workflow involves opening a C# solution/project, right-clicking in Solution Explorer, choosing “Port with Amazon Q Developer,” selecting the .NET target version, and confirming; users monitor progress in Transformation Hub, review diffs and a code transformation summary (downloadable as .md), download a Linux readiness report (.csv) for required manual updates, and can choose “Accept changes” to update files in place.
```
