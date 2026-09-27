# Web source

- URL: https://docs.gitlab.com/runner/development/add-windows-version
- Title: Add Docker executor support for a Windows version | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:10:25.586484786+00:00
- Relevance: High — title matches query


```text
To add Windows version support for the Docker executor, GitLab must release a matching helper image, built with GitLab Runner on that Windows version because host and container OS versions must match. Base images are published by runner-tools/base-images and configured via the windows target in dockerfiles/runner-helper/docker-bake.hcl; windows-containers builds GCP host VM images provisioned by autoscaler, and backward compatibility may allow reuse (Windows Server 2025 reused 2022 helper images), though Windows Server 2022 needed a new image because the 2019 helper was incompatible with process isolation. The workflow includes dev testing, manual publish, registering two project-specific runner managers, updating Ansible/autoscaler and the liveness image, and updating GitLab Runner code (supportedWindowsBuilds, ltsc map, docker-bake.hcl, helper-images.json, knownWinVersions, CI jobs, docs). For Windows Server 2025/LTSC2025, including arm64, MR 88 added ltsc2025/ltsc2025-arm64/servercore/nanoserver base images, MR 6033 built servercore:ltsc2025 and -arm64 helper images, MR 6697 added a native ARM64 helper build, MR 6716 bundled the native ARM64 binary, and MR 6717 built nanoserver:ltsc2025 and -arm64 images.
```
