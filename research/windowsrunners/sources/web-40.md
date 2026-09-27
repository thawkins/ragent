# Web source

- URL: https://archives.docs.gitlab.com/16.1/runner/executors/shell.html
- Title: The Shell executor | GitLab
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:13:22.888893387+00:00
- Relevance: High — title matches query


```text
The Shell executor runs builds locally on the GitLab Runner host, supports Bash, PowerShell Core, Windows PowerShell, and deprecated Windows Batch, with shell selection via `config.toml` (e.g., `shell="powershell"`); unprivileged execution via `--user` is Bash-only. Builds are checked out to `<working-directory>/builds/<short-token>/<concurrent-id>/<namespace>/<project-name>` and cached in `<working-directory>/cache/<namespace>/<project-name>`, overridable with `builds_dir` and `cache_dir` under `[[runners]]`; `<short-token>` is the first 8 letters of the Runner token, and `<concurrent-id>` is available via `CI_CONCURRENT_PROJECT_ID`. On Linux `.deb`/`.rpm` installs, the installer uses `gitlab_ci_multi_runner` if found, otherwise creates `gitlab-runner`, which must be added to groups like `docker` or `vboxusers` for privileged resources. The executor is considered unsafe because jobs run with `gitlab-runner` permissions and can steal code from other projects. It starts each job in a new process and on UNIX uses a process group: GitLab 13.0 and earlier sends `SIGKILL` on UNIX and `taskkill /F /T` on Windows, while 13.1+ sends `SIGTERM` then `SIGKILL` after 10 minutes; Windows sends the kill signal twice, the second after 10 minutes.
```
