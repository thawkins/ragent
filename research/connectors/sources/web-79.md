# Web source

- URL: https://docs.rs/codewandler-connector-spec/latest/src/connector_spec/ir.rs.html
- Title: ir.rs - source
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:32:29.537746427+00:00
- Relevance: Medium-high - snippet matches query


```text
The page is the Rust source for `connector_spec::ir`, defining a Serde-serializable intermediate representation for connector specs. It declares `Connector`, `Service`, `Operation`, `Param`/`ParamSet`, `Quirks`, `Provenance`, and `Role`, plus enums `HttpMethod`, `Risk`, `Idempotency`, `BodyEncoding`, and `Pagination`, with constants `FREE_FORM_BODY = "body"`, `DEFAULT_SERVICE = "default"`, and `MIN_REPEATABILITY_CONDITION = 24`. `Operation` carries id, service, method, path, risk, idempotency, optional `repeatable_because`, auth, params, response_schema, and quirks; `Connector` carries id, authority, api_version, services, vendor, base_url, description, auth, default_auth, operations, events, channels, config, graphs, verify, and provenance. The module provides lookup/effective-auth/service-filtering helpers, builds an input JSON schema from params, and computes `canonical_json`, `hash_domain`, and `ir_sha256` (SHA-256 of the hash-domain JSON, excluding provenance).
```
