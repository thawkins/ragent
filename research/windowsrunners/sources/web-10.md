# Web source

- URL: https://notes.runtimeterror.dev/CICD/Set-up-Gitlab-Runner-for-Windows-containers.html
- Title: Set up GitLab Runner for Windows containers
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:10:01.715873904+00:00
- Relevance: High — title matches query


```text
These notes outline setting up GitLab Runner on Windows Server 2022 for Windows container workloads: install Git from git-scm.com/downloads/win with Explorer integration, file associations, and Scalar add-on deselected; Notepad as editor; main as default branch; recommended PATH handling; external OpenSSH; native Windows Secure Channel; checkout/commit as-is; Windows default console; fast-forward/merge pull; no Credential Manager; and file system caching; then install Docker CE by downloading and running Microsoft’s install-docker-ce.ps1 via Invoke-WebRequest, rebooting, and verifying with `docker run --rm hello-world` (Docker Desktop/Hyper-V is needed for Linux containers, but the Docker daemon is enough for Windows-on-Windows). For the runner, create `C:\GitLab-Runner`, download/rename the x64 binary to `gitlab-runner.exe`, create a Windows-tagged runner in GitLab, register with `.\gitlab-runner.exe register --url https://gitlab.example.com --token $token`, choose the `docker-windows` executor and e.g. `mcr.microsoft.com/windows/nanoserver:ltsc2022`, run `install`/`start`, and edit `C:\GitLab-Runner\config.toml` (restart to reapply), optionally replacing `helper_image` with an internally hosted one; the example config uses name “My Windows Runner,” id 69, shell `powershell` (default `pwsh` may not be available on WS2022), `tls_verify = true`, image `harbor.example.com/mcr/windows/nanoserver:ltsc2022`, `privileged = false`, and helper_image `harbor.example.com/dockerhub/gitlab/gitlab-runner-helper:x86_64-v${CI_RUNNER_VERSION}-servercore21H2`.
```
