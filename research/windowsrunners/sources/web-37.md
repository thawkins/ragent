# Web source

- URL: https://docs.gitlab.com/runner/executors/shell
- Title: The Shell executor | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:13:01.011529730+00:00
- Relevance: Medium — multiple title terms match query


```text
The GitLab Runner Shell executor is in maintenance mode (critical security updates only, no new features; new projects should consider actively developed executors) and is the simplest executor: it runs builds locally on the Runner host, requires dependencies on that machine, supports Bash, PowerShell Core, Windows PowerShell, and deprecated Windows Batch, and provides limited isolation. Builds check out to `<working-directory>/builds/<short-token>/<concurrent-id>/<namespace>/<project-name>` and cache to `<working-directory>/cache/<namespace>/<project-name>`, overridable with `builds_dir`/`cache_dir` in `config.toml`; scripts can run as an unprivileged user only with `--user` on `gitlab-runner run` (Bash only), Linux .deb/.rpm installers use `gitlab_ci_multi_runner` if present or create `gitlab-runner`, and that user can be added to groups like `docker`/`vboxusers`. Security is generally unsafe—jobs run with the `gitlab-runner` user's permissions and may steal code from other projects or execute arbitrary privileged commands, so use only with trusted users/servers—and on termination UNIX sends SIGTERM then SIGKILL after 10 minutes, while Windows sends the kill signal twice, the second after 10 minutes.
```
