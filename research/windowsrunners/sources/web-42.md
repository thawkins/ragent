# Web source

- URL: https://github.com/libguestfs/nbdkit/tree/1965e89b5fc2d815ca238adf14742e87261d462e/ci
- Title: nbdkit/ci at 1965e89b5fc2d815ca238adf14742e87261d462e · libguestfs/nbdkit
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:13:27.513060762+00:00
- Relevance: Medium — partial query match


```text
The nbdkit CI runs checks via `ci/build.sh`, using libvirt-ci’s `lcitool` to generate Dockerfiles under `ci/containers` and handle distro dependencies/naming; GitLab CI runs Linux container jobs (cached and rebuildable) and can update images with `lcitool manifest ci/manifest.yml`. Local reproduction uses `podman`/`docker` to build a chosen image like `nbdkit-fedora-rawhide` from `ci/containers/fedora-rawhide.Dockerfile` and run it with the repo bind-mounted at `/repo`. Because GitLab lacks shared runners for FreeBSD/macOS, the project uses `cirrus-run` to trigger Cirrus CI jobs from GitLab CI, requiring a one-time GitHub repo, Cirrus CI account/API token, and masked GitLab variables `CIRRUS_GITHUB_REPO` and `CIRRUS_API_TOKEN` (not protected).
```
