# Web source

- URL: https://archives.docs.gitlab.com/17.2/runner/executors/shell.html
- Title: The Shell executor | GitLab
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:13:33.521781558+00:00
- Relevance: High — title matches query


```text
The Shell executor runs builds locally on the machine where GitLab Runner is installed, supports all systems on which Runner can be installed, and can execute Bash, PowerShell Core, Windows PowerShell, and deprecated Windows Batch scripts. Project checkout is at `<working-directory>/builds/<short-token>/<concurrent-id>/<namespace>/<project-name>` and cache at `<working-directory>/cache/<namespace>/<project-name>`, overridable via `builds_dir` and `cache_dir` in `config.toml`; `<working-directory>` comes from `--working-directory` or the Runner’s current directory. Linux installs use `gitlab_ci_multi_runner` if found, otherwise create and use `gitlab-runner`; that user may need adding to groups like `docker` or `vboxusers`, and `--user` supports unprivileged runs only with Bash. GitLab warns Shell executor jobs are generally unsafe because they run with the user’s permissions, can steal other projects’ code, and may execute arbitrary commands as a highly privileged user. On UNIX, Runner terminates job process groups with SIGTERM then SIGKILL after 10 minutes; Windows sends the kill signal twice, with the second after 10 minutes.
```
