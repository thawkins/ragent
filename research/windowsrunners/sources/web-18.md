# Web source

- URL: https://docs.gitlab.com/runner/install/docker
- Title: Run GitLab Runner in a container | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:11:28.049582411+00:00
- Relevance: Medium — multiple title terms match query


```text
GitLab Runner can run in a Docker container to execute CI/CD jobs, using Ubuntu- or Alpine-based images that wrap the standard `gitlab-runner` command; Docker Engine and Runner image versions need not match and are backward/forward compatible, though isolation guarantees break if the Runner container shares a Docker daemon with other payloads. Install via `docker pull gitlab/gitlab-runner:<version-tag>` and `docker run -d` with a permanent config volume (e.g. `/srv/gitlab-runner/config:/etc/gitlab-runner` or named volume `gitlab-runner-config`), optional `-p 8093:8093` for `session_server`, and `/root/.docker/machine` for Docker Machine autoscaling; register the runner, restart the container after config changes, upgrade by pulling latest then stop/rm/rerun with the same volume, view Docker-service logs with `docker logs`, and place trusted SSL certs at `/etc/gitlab-runner/certs/ca.crt` or use `CA_CERTIFICATES_PATH`. Images include `gitlab/gitlab-runner:latest` (Ubuntu, approx. 470 MB) and `:alpine` (Alpine 3.21 in GitLab Runner 18.8.0, approx. 270 MB), with notes on custom Alpine upgrade builds and SELinux using `:Z` volumes and `selinux-dockersock`.
```
