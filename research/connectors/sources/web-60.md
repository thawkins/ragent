# Web source

- URL: https://i2group.github.io/analyze-connect/content/schemas/connector-schema.html
- Title: Connector schemas
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:08.087788582+00:00
- Relevance: Medium - multiple title terms match query


```text
A connector schema is an i2 Analyze schema supplied by a connector that defines some or all of the item types the connector can return, letting connectors be added to or removed from i2 Analyze deployments without changing existing Information Store or gateway schemas and making them easier to share across deployments. Connectors create schemas with i2 Analyze Schema Designer, and must expose GET endpoints for the schema XML (`schemaUrl`) and optional charting scheme XML (`chartingSchemesUrl`), both relative to the connector’s base URL. Every connector schema has a unique short name shown to analysts in i2 Analyst’s Notebook for its entity and link types; it can be set via the connector configuration’s `schemaShortName` field or the topology’s `schema-short-name` attribute, with the topology value taking precedence, and if neither is set, the connector’s topology identifier is used.
```
