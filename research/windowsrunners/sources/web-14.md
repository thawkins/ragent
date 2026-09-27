# Web source

- URL: https://gitlab.musictribe.com/help/user/gitlab_com/index.md
- Title: Index · Gitlab com · User · Help
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:11:04.526084542+00:00
- Relevance: Medium — partial query match


```text
GitLab.com lists SSH host key fingerprints/known_hosts entries and an alternative SSH endpoint at altssh.gitlab.com:443; email is sent from mg.gitlab.com via Mailgun from dedicated IP 198.61.254.240. GitLab Pages uses gitlab.io and 35.185.44.232, supports custom domains and TLS on GitLab.com (not by default), and allows 1G uncompressed sites vs. 100M default. CI/CD settings include a 10G repository limit including LFS (unlimited default), web/API traffic from 34.74.90.64/28, 100 project and 50 group webhooks, Linux shared runners on n1-standard-1 instances (3.75GB RAM, 1 vCPU, 25GB HDD) with 2000 private CI minutes/month per group and a 3-hour job timeout, plus beta Windows runners on n1-standard-2 (2 vCPU, 7.5GB RAM) with ~5-minute VM provisioning. Rate limits include 429 for API over 10 requests/sec/IP and protected-path POSTs over 10/min/IP, and 403 for 1 hour after 30 failed Git/container-registry auth requests in 3 minutes from one IP; Internal visibility is disabled for projects/groups/snippets as of GitLab 12.2, and GitLab.com also runs Sidekiq, PostgreSQL with streaming replication/hot standby, Unicorn memory limits, Fluentd to Stackdriver/Cloud Pub/Sub, and Elasticsearch/Kibana, Prometheus, Grafana, Sentry, Consul, and HAProxy.
```
