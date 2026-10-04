# Web source

- URL: https://github.com/openai/codex/commit/e6cfd40c3f444aadd6017c9eeab01db70f48961a
- Title: Expose connector candidates in external agent detection (#36218) · openai/codex@e6cfd40
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:22.754534339+00:00
- Relevance: Medium - multiple title terms match query


```text
Commit e6cfd40c adds connector detection to the external-agent config API: `ExternalAgentConfigDetectResponse` gains `connectors: Vec<ExternalAgentDetectedConnectorCandidate>`, with new `ExternalAgentDetectedConnectorSource` and `ExternalAgentDetectedConnectorCandidate` types including a `source` field. A test named `external_agent_config_detect_response_defaults_connectors_for_older_servers` checks that older detect responses deserialize with default connectors, and docs for `externalAgentConfig/detect` now say the response includes connector candidates inferred from detected source sessions, with a normalized display `name`, the number of detected sessions that used the connector, and the source metadata field used for detection.
```
