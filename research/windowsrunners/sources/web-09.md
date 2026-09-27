# Web source

- URL: https://aixxe.net/2024/04/windows-ci-docker
- Title: Windows containers with GitLab CI
- Author(s): aixxe
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:10:38.764099433+00:00
- Relevance: High — title matches query


```text
The author documents setting up Windows OS containers with the native Docker Engine on Windows Server 2025 preview (timed evaluations via the Windows Insider Program or uupdump.net) to replace a Windows GitLab Runner shell executor used since 2017 and avoid maintaining multiple host software versions. A VMware ESXi default Server 2025 VM failed to boot beyond the DVD prompt, so they installed as Server 2022 with ESXi 7.0 compatibility and upgraded. They enabled built-in OpenSSH (fixing a non-working TCP/22 firewall rule by recreating `OpenSSH-Server-In-TCP`), installed Docker via Microsoft’s `install-docker-ce.ps1`, and built a 12.1GB `mcr.microsoft.com/windows/servercore:ltsc2022`-based image with Visual Studio 2022 Build Tools/C++ (`cl 19.39.33521`), CMake, Meson, Ninja, PowerShell, and Git; they tested compilation and `docker cp`. GitLab Runner v16.9.1 was installed and registered with the `docker-windows` executor for x86/x64 runners using `VcVars.ps1`, but required patching the `supportedWindowsBuilds` map in source because it did not yet support Windows Server 2025.
```
