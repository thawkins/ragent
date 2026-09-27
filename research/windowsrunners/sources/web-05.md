# Web source

- URL: https://docs.gitlab.com/ci/runners/hosted_runners/windows
- Title: Hosted runners on Windows | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:10:14.460450597+00:00
- Relevance: Medium — multiple title terms match query


```text
GitLab.com hosted runners on Windows are in Beta for Free, Premium, and Ultimate tiers, autoscaling by launching Google Cloud Platform VMs via a GitLab-developed autoscaling driver for the custom executor. The available machine type is `saas-windows-medium-amd64` with 2 vCPUs, 7.5 GB memory, and 75 GB storage; Windows 2022 is GA. These runners do not use the GitLab Docker executor, so pipelines cannot specify `image` or `services`, use PowerShell as the shell, install the latest Docker version at image build, and run jobs as an elevated admin process in a new VM discarded after each job; .NET Framework is 4.8.04161. Known issues include average VM provisioning time of about five minutes during beta, occasional fleet unavailability for maintenance, jobs pending longer than on Linux runners, and possible breaking changes requiring pipeline updates.
```
