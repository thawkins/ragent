# Web source

- URL: https://docs.gitlab.com/runner/security
- Title: Security for self-managed runners | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:12:36.744956354+00:00
- Relevance: Medium — multiple title terms match query


```text
GitLab Runner executes CI/CD job code, so any user with the Developer role on a project repository can compromise the runner host/network, especially with non-ephemeral self-managed runners shared across projects; malicious jobs can steal secrets such as `CI_JOB_TOKEN` and access submodule contents via the parent repo’s reflog. Executor risks include Shell (high-risk, runs as Runner user), Docker (safer non-privileged, but privileged mode or `--pid=host` enables root/container breakout; Docker Machine should use `MaxBuilds = 1`), Docker credential helpers (execute on the runner manager; restrict with `allowed_docker_credential_helpers` and avoid `docker login` as runner user), SSH (MITM due to missing `StrictHostKeyChecking`), and Parallels (safest, full VM isolation). On mixed-access shared runners, avoid `if-not-present` for private Docker images, and use `GIT_STRATEGY: fetch` only when all shared-environment users are trusted. Hardening steps include running privileged jobs only on isolated ephemeral VMs or dedicated protected-branch runners, network segmentation, securing/removing host SSH keys, and enabling `FF_ENABLE_JOB_CLEANUP` to clean build directories after each build.
```
