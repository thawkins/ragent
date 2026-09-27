# Web source

- URL: https://docs.gitlab.com/runner/executors/docker_autoscaler.html
- Title: Docker Autoscaler executor | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:12:23.198664793+00:00
- Relevance: High — title + snippet match query


```text
The Docker Autoscaler executor is an autoscale-enabled Docker executor that creates instances on demand, wraps the Docker executor to support all Docker executor options and features, and uses fleeting plugins for cloud providers including AWS, GCP, and Azure. Each runner configuration requires its own dedicated autoscaling resource—AWS Auto Scaling group, GCP instance group, or Azure scale set—which must not be shared across runner managers or `[[runners]]` entries; example configs use `capacity_per_instance=1`, `max_use_count=1`, `max_instances=10`, `idle_count=5`, `idle_time=20m`, and `concurrent=10`, with runner-managed scaling and images that have Docker Engine but no GitLab Runner. The executor supports slot-based cgroups via `--cgroup-parent` and `service_slot_cgroup_template`; documented troubleshooting includes external instance removal causing `ssh tunnel: EOF`, AWS `context deadline exceeded` linked to AZRebalance, and Azure VMSS overprovisioning requiring `overprovision=false`.
```
