# Web source

- URL: https://syndesis.io/docs/connectors/connector-schema
- Title: Connector Schema
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:32:11.941985033+00:00
- Relevance: High - title + snippet match query


```text
Syndesis’s connector schema page defines a draft-04 JSON-schema descriptor for the Connector Proxy that links the GUI, server, meta, and integration runtime (with Apache Camel as the reference). The descriptor is validated by a Maven plugin and expected at `syndesis/app/connector/<connectorName>/src/main/resources/META-INF/syndesis/connector/<connectorId>.json`; required top-level fields are `actions`, `description`, `icon`, `id`, and `name` (`additionalProperties: false`), with optional `componentScheme`, `configuredProperties`, `connectorCustomizers`, `connectorFactory`, `dependencies`, `metadata`, `properties`, and `tags`. Each action requires `actionType` (connector; step is reserved for extensions), `description`, `descriptor`, `id`, `name`, and `pattern` (`From`, `To`, or `Pipe`, though `Pipe` is not fully supported), while action descriptors require `inputDataShape` and `outputDataShape` and can define `componentScheme` (overridable at action level), `configuredProperties`, `connectorFactory`, `connectorCustomizers`, `propertyDefinitionSteps`, and `standardizedErrors`. Dependencies support `MAVEN`, `EXTENSION`, `EXTENSION_TAG`, and `ICON`; all connectors must declare their own Maven dependency, icons use `assets:<image.ext>` under `app/ui-react/syndesis/public/icons/`, metadata flags include `tech-preview`, `hide-from-connection-pages`, and `hide-from-step-select`, tags include `verifier` and (since 2.0) `dynamic`, connector `properties` feed Connections while action `propertyDefinitionSteps` feed Integrations, and descriptors are bundled into a support catalog library used by server and integration.
```
