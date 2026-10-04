# Web source

- URL: https://seatunnel.apache.org/docs/2.3.7/connector-v2/source/OneSignal
- Title: OneSignal | Apache SeaTunnel
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:48.065207742+00:00
- Relevance: Medium-high - snippet matches query


```text
This page documents the Apache SeaTunnel 2.3.7 OneSignal source connector, noting that 2.3.7 is no longer actively maintained. The connector reads data from OneSignal and requires `url` and `password`; `method` supports GET or POST, `format` supports json or text (default json), and optional settings include `schema`, `params`, `body`, `json_field`, `content_json`, `poll_interval_millis`, `retry`, `retry_backoff_multiplier_ms` (default 100), `retry_backoff_max_ms` (default 10000), and `enable_multi_lines` (default false). The example config calls `https://onesignal.com/api/v1/apps` with password `"SeaTunnel-test"` and a schema covering fields such as `id`, `name`, `gcm_key`, `players`, and `messageable_players`. The changelog says the next version adds the OneSignal Source Connector and notes Connector-V2 HTTP uses json-path parsing (3510).
```
