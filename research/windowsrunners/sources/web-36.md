# Web source

- URL: https://docs.gitlab.com/runner/executors
- Title: Executors | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:12:46.129654441+00:00
- Relevance: Medium — multiple title terms match query


```text
GitLab Runner (Free/Premium/Ultimate; GitLab.com, Self-Managed, GitLab Dedicated) implements executors including Docker, Docker Autoscaler, Instance, Kubernetes, SSH, Shell, VirtualBox, Parallels, and Custom. Docker Autoscaler and Instance use Fleeting plugins for autoscaling and are recommended for full capabilities; Kubernetes creates a new Pod per CI/CD job, Docker provides container-based clean environments with Podman and services like MySQL, and Instance can give jobs full host/OS/device access. SSH, Shell, VirtualBox, Parallels, and Custom are in maintenance mode, receiving only critical security updates and no new features; SSH is least supported and Shell is suggested for local shell builds. Feature support varies: all executors support secure variables, cache, artifacts, and artifact passing; image is supported by Docker, Docker Autoscaler, Kubernetes, VirtualBox, Parallels, and Custom; services by Docker, Docker Autoscaler, Kubernetes, and Custom; interactive web terminal by Docker, Kubernetes, and Shell; and non-Docker executors require Git in PATH, use Git LFS if installed, and authenticate Git interactions with CI_JOB_TOKEN.
```
