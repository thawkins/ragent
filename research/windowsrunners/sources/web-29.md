# Web source

- URL: https://archives.docs.gitlab.com/17.0/runner/executors/docker_autoscaler.html
- Title: Docker Autoscaler executor | GitLab
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:11:56.507376199+00:00
- Relevance: High — title matches query


```text
The GitLab Runner 17.0 Docker Autoscaler executor is a Beta, production-not-recommended autoscaling Docker executor that creates cloud instances on demand and wraps the Docker executor to support all its options and features; production environments are directed to the Docker Machine executor. It uses fleeting plugins for AWS, Google Cloud, and Azure, configured in `config.toml`. Example configs use `capacity_per_instance=1`, `max_use_count=1`, `max_instances=10`, `idle_count=5`, `idle_time="20m0s"`, and `concurrent=10`, giving each job a secure ephemeral instance that is deleted immediately after completion while the runner keeps five instances idle for at least 20 minutes. For Runner >=16.11, run `gitlab-runner fleeting install`; for <17.0, manually install plugins such as `fleeting-plugin-aws`, `fleeting-plugin-googlecompute`, or `fleeting-plugin-azure`, and cloud scale groups must disable autoscaling because the runner handles scaling.
```
