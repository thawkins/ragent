# Web source

- URL: https://archives.docs.gitlab.com/17.2/runner/executors/docker_autoscaler.html
- Title: Docker Autoscaler executor | GitLab
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:12:08.215439411+00:00
- Relevance: High — title matches query


```text
The Docker Autoscaler executor is an autoscale-enabled wrapper around the Docker executor that creates instances on demand for jobs and supports all Docker executor options and features, using fleeting plugins for cloud providers including GCP, AWS, and Azure. Only the GCP fleeting plugin is generally available; AWS and Azure plugins are still beta (issue 408131). Configuration in `config.toml` covers AWS Autoscaling Groups, Google Cloud instance groups, and Azure scale sets, with examples using `capacity_per_instance=1`, `max_use_count=1`, `max_instances=10`, `idle_count=5`, `idle_time="20m0s"`, and `concurrent=10`; each job gets a secure ephemeral instance deleted after completion, while 5 idle instances are kept, and prerequisites include Docker Engine images plus cloud groups with provider autoscaling disabled/manual and IAM permissions.
```
