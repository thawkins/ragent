# Web source

- URL: https://archives.docs.gitlab.com/17.2/runner/install/windows.html
- Title: Install GitLab Runner on Windows | GitLab
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:10:32.323560357+00:00
- Relevance: High — title matches query


```text
GitLab Runner on Windows requires Git and, if run under a user account rather than the Built-in System Account, a valid user password; since GitLab Runner 10 the executable is `gitlab-runner`, installed by placing a 32/64-bit binary in a folder such as `C:\GitLab-Runner`, restricting Write permissions to prevent privilege escalation, registering the runner, and installing/starting it as a service via `gitlab-runner install`/`start` (optionally with `--user` and `--password`). It is recommended to use the Built-in System Account, concurrent jobs can be set in `C:\GitLab-Runner\config.toml`, logs appear in Windows Event Log under provider `gitlab-runner`, upgrades require stopping the service and replacing the binary, and uninstalling uses `stop`, `uninstall`, and `rmdir /s`. Troubleshooting covers logon failures requiring `SeServiceLogonRight`, `PathTooLongException` fixes via `git config --system core.longpaths true` or NTFSSecurity’s `Remove-Item2`, Batch scripts needing `call`, ANSI color output, robocopy exit-code handling, unsupported Windows versions in Docker/Kubernetes executors (e.g., Docker 17.06.2, Kubernetes `node.kubernetes.io/windows-build` nodeSelector), mapped drives needing UNC paths, Docker-Windows permission errors for `C:\ProgramData\Docker`, and WSL blank STDOUT lines fixed with `WSL_UTF8=1`.
```
