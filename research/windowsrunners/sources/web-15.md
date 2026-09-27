# Web source

- URL: https://git.cs.hofstra.edu/help/user/gitlab_com/index.md
- Title: Index · Gitlab com · User · Help
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:11:19.580735293+00:00
- Relevance: Medium — partial query match


```text
This page documents GitLab.com-specific settings, including SSH host key fingerprints for DSA/ECDSA/ED25519, an alternative SSH endpoint at `altssh.gitlab.com` on port 443, mail sent from `mg.gitlab.com` via Mailgun with dedicated IP `198.61.254.240`, and GitLab Pages defaults (`gitlab.io`, IP `52.167.214.135`, custom domains and TLS supported, page size limited by CI/CD artifacts max). For CI/CD, GitLab.com allows `1G` artifacts (default `100M`), keeps artifacts forever (default 30 days), caps repositories including LFS at `10G`, and uses Shared Runners that autoscale on Google Cloud Platform with `n1-standard-1` instances (3.75GB RAM, 1 vCPU, 25GB HDD, US-East1), free for public open source but limited to 2,000 CI minutes/month per group for private projects, with jobs timing out after 3 hours. It also lists Sidekiq settings (`--timeout=4 --concurrency=4`, RSS killer `1000000`, `pipeline_schedule_worker` at `19 * * * *`), scaled PostgreSQL settings (e.g., `shared_buffers` 112896MB, `max_wal_senders` 320, hot standby on), Unicorn memory limits (base 750–1024MiB, web 1024–1280MiB), and scale tooling including ELK, Prometheus, Grafana, Sentry, Consul, and Haproxy.
```
