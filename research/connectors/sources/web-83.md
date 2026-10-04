# Web source

- URL: https://fivetran.com/docs/connector-sdk/connector-sdk-concepts/schema-management
- Title: Connector SDK Concepts | Schema Management
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:33:06.910150722+00:00
- Relevance: Medium - multiple title terms match query


```text
Fivetran’s Connector SDK makes schema declaration optional because Fivetran automatically builds and updates destination tables, but it recommends declaring primary keys for de-duplication and row updates—otherwise Fivetran generates an internal surrogate primary key. In `schema()`, developers can define tables, primary keys, and data types (including `STRING`, `UTC_DATETIME`, `LONG`, and `DECIMAL`), while `update()` sends rows and Fivetran can infer undeclared columns and types; composite primary keys are supported, and type mismatches cause sync failures. For schema evolution, Fivetran keeps deleted or renamed tables/columns unchanged, creates new tables/columns when data arrives, applies widening type changes automatically but not narrowing changes, and requires manual drop/recreate/resync for primary key changes; validation is done with `fivetran debug` and the `files/warehouse.db` debug destination.
```
