# Web source

- URL: https://github.com/libguestfs/nbdkit/tree/380c559696b88349200e37e7e9a10107c84e4abc/ci
- Title: nbdkit/ci at 380c559696b88349200e37e7e9a10107c84e4abc · libguestfs/nbdkit
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:13:18.833382351+00:00
- Relevance: Medium — partial query match


```text
nbdkit's CI runs its main checks via `ci/build.sh` using GitLab CI Docker containers to broaden Linux coverage. It uses `lcitool` from `libvirt-ci` to generate Dockerfiles under `ci/containers` (update with `lcitool manifest ci/manifest.yml`), builds and caches container images, and supports local reproduction with podman/docker, e.g. Fedora rawhide, while recommending clean or VPATH builds. Because GitLab CI lacks shared runners for FreeBSD and macOS, it triggers Cirrus CI jobs through `cirrus-run`; enabling this requires a GitHub repo, Cirrus CI app/account/API token, an empty `.cirrus.yml`, and GitLab CI/CD variables `CIRRUS_GITHUB_REPO` and `CIRRUS_API_TOKEN` (masked, not protected).
```
