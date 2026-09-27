# Web source

- URL: https://docs.gitlab.com/runner/executors/docker_autoscaler
- Title: Docker Autoscaler executor | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:12:01.290156875+00:00
- Relevance: Medium — multiple title terms match query


```text
GitLab Runner’s Docker Autoscaler executor is an autoscaling Docker executor that wraps all Docker executor options/features and uses fleeting plugins to autoscale instance groups on AWS, Google Cloud, and Azure; each configuration needs a dedicated autoscaling resource (AWS ASG, GCP instance group, Azure scale set) that must not be shared across runner managers or `[[runners]]` entries because conflicting scaling can cause unpredictable behavior, job failures, and higher costs. Documented examples use `capacity_per_instance=1`, `max_use_count=1`, `max_instances=10`, `idle_count=5`, `idle_time=20m`, and `concurrent=10`, with prerequisites such as Docker Engine images that do not register as GitLab runners, AWS scale-in protection and disabled AZRebalance, GCP single-zone “Do not autoscale” groups (multi-zone unsupported), and Azure manual scale sets with `overprovision=false`. The executor supports slot-based cgroups via `--cgroup-parent` and separate `service_slot_cgroup_template`. Troubleshooting notes external instance removal causing `ssh tunnel: EOF` and `instance unexpectedly removed` (check AWS CloudTrail for `ec2.amazonaws.com`), AWS `context deadline exceeded` errors linked to oscillating reserved counts and AZRebalance, and Azure VMSS overprovisioning job failures.
```
