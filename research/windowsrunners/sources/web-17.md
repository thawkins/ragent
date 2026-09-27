# Web source

- URL: https://docs.gitlab.com/runner/configuration/advanced-configuration
- Title: Advanced configuration | GitLab Docs
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-24T14:11:42.891392913+00:00
- Relevance: Medium — partial query match


```text
GitLab Runner’s advanced configuration is set in `config.toml`, found at `/etc/gitlab-runner/` for root on *nix, `~/.gitlab-runner/` for non-root, or `./` on other systems; most options reload automatically because the runner checks every 3 seconds and on `SIGHUP`, except `listen_address`, and configuration validation is informational only. Global settings include `concurrent`, `log_level`/`log_format`, `check_interval` (default 3s), `connection_max_age` (default 15m), `shutdown_timeout` (default 30s), and `sentry_dsn`; long-polling warnings cite GitLab Workhorse’s `-apiCiLongPollingDuration` default of 50s and `request_concurrency` default of 1, recommending higher `concurrent`/`request_concurrency` and `FF_USE_ADAPTIVE_REQUEST_CONCURRENCY`. Other sections cover `[machine]` (introduced in GitLab Runner 18.10), `[machine.shutdown_drain]`, `[machine.heartbeat]`, `[session_server]` (e.g., port 8093, `session_timeout` 1800), and `[[runners]]` settings such as `url`, `token`, `limit`, `executor`, `request_concurrency`, `strict_check_interval`, `output_limit` (4096 KB), `prepare_timeout`, `get_sources_timeout`, and `[runners.experimental.boot_verify]`. Executors include `shell`, `docker`, `docker-windows`, `ssh`, `parallels`, `virtualbox`, `docker+machine`, `kubernetes`, `docker-autoscaler`, and `instance`; `[runners.docker]` settings cover `allowed_images`/`allowed_services`, privileged mode, `cache_dir`, `cap_add`/`cap_drop`, `cpus`/`cpuset_cpus`, `devices`, and `disable_cache`.
```
