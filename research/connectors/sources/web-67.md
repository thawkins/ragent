# Web source

- URL: https://forum.confluent.io/t/json-with-schema-registry/10261
- Title: Json with schema registry
- Author(s): -
- Language: English
- Published (UTC): 2024-02-26T14:57:00.423+00:00
- Captured (UTC): 2026-10-02T13:31:42.851853449+00:00
- Relevance: High - title + snippet match query


```text
A Kafka newcomer who previously used HTTP connectors with `value.converter.schemas.enable=false` added a JSON schema (`schemaType: JSON`) to Schema Registry and configured `io.confluent.connect.json.JsonSchemaConverter` with `value.converter.schemas.enable=true` and registry URL `https://kafka-schemaregistry:8081/`, but encounters the “Unknown magic byte!” error on topic `mytopic` (partition 0, offset 34, timestamp 1708945734273, `CreateTime`); the stack trace shows a `DataException` in `JsonSchemaConverter.toConnectData` (line 144), caused by a `SerializationException` deserializing a JSON message for id `-1` in `AbstractKafkaJsonSchemaDeserializer` (line 236). The poster, referencing Confluent’s “Unknown magic byte” blog, asks whether the schema should validate the JSON before or after transformations and whether/how they must tell the connector which Schema Registry schema to use.
```
