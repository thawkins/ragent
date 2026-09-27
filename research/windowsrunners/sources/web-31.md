# Web source

- URL: https://docs.gitlab.com/18.0/runner/executors/docker_autoscaler
- Title: Docker Autoscaler executor | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:12:15.955625428+00:00
- Relevance: High — title + snippet match query


```text
The GitLab Runner Docker Autoscaler executor wraps the Docker executor, supports all Docker executor options, and uses fleeting plugins (AWS, Google Cloud, Azure) to autoscale on demand. Each configuration requires a dedicated autoscaling resource—AWS auto scaling group, GCP single-zone instance group (multi-zone unsupported), or Azure scale set—and must not be shared across runner managers or `[[runners]]` entries; example configs use `capacity_per_instance=1`, `max_use_count=1`, `max_instances=10`, `idle_count=5`, `idle_time=20m0s`, and `concurrent=10`. Prerequisites include Docker Engine AMI/VM images that do not register as runners, IAM permissions, and autoscaling set to none/Do not autoscale/manual; troubleshooting notes `ssh tunnel: EOF` from external instance removal and AWS `context deadline exceeded` failures resolved by disabling the AZRebalance process.
```
