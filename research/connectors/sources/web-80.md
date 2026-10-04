# Web source

- URL: https://docs.rs/codewandler-connector-spec/latest/connector_spec/provider/constant.PROVIDER_TOML_JSON_SCHEMA.html
- Title: PROVIDER_TOML_JSON_SCHEMA in connector_spec::provider - Rust
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:32:36.585152489+00:00
- Relevance: High - title matches query


```text
`PROVIDER_TOML_JSON_SCHEMA` is a public `&str` in `codewandler-connector-spec` containing a JSON Schema (draft 2020-12, `$id` `github.com/codewandler/flux-connectors/schema/provider-toml.schema.json`) titled “flux-connectors provider definition.” It describes `providers/<name>.toml` for both hand-authored connectors and vendored-spec-plus-patch connectors, producing the same connector IR, and requires `id` and `base_url`; all objects are closed so unrecognized keys are errors. The schema models services, operations, events, channels, replies, graphs, config fields, auth, HMAC verification, and the `webhook`, `socket`, and `poll` transports, and it states that no credential value ever appears in a provider file—`env` and `user_env` only name environment variables.
```
