# Web source

- URL: https://forum.gitlab.com/t/google-cloud-autoscale-windows-runner-with-gitlab/39407
- Title: Google Cloud: AutoScale Windows runner with GitLab
- Author(s): —
- Language: English
- Published (UTC): 2020-06-30T07:28:59+00:00
- Captured (UTC): 2026-09-24T14:12:12.563818302+00:00
- Relevance: High — title matches query


```text
A GitLab forum user running GitLab CI autoscale on GCP with Debian runners tried to add a Windows runner by creating a Windows image with OpenSSH and configuring the runner manager’s `config.toml` for `docker+machine`, using the Google machine driver, `MachineName = "autoscale-windows-%s"`, and `google-machine-image=abcd/global/images/autoscale-gitlab-runner-windows-agent` in `us-west1-c` under project `abcd`. A simple test job (`echo "Hello from Windows"`) successfully launches the Windows runner on GCP but fails to SSH into it with the error: “Error creating machine: Error detecting OS: Too many retries waiting for SSH to be available. Last error: Maximum number of retries (60) exceeded driver=google.” They ask whether GitLab CI Autoscale supports Windows runners.
```
