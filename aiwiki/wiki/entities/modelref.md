---
title: "ModelRef"
entity_type: "technology"
type: entity
generated: "2026-04-19T19:31:06.814042684+00:00"
---

# ModelRef

**Type:** technology

### From: team_spawn

The `ModelRef` struct serves as a structured reference to AI model configurations, imported from `crate::agent` and utilized within `TeamSpawnTool` for flexible model assignment to spawned teammates. This type encapsulates the provider-model pairing pattern increasingly standard in multi-provider AI systems, with `provider_id` and `model_id` string fields that support provider-agnostic model specification. The design enables runtime model selection without hardcoded provider dependencies, supporting the framework's goal of provider flexibility.

Within `TeamSpawnTool`, `ModelRef` parsing demonstrates sophisticated input handling with fallback strategies. The `execute` method attempts to parse the optional `model` parameter using two delimiter patterns—forward slash (`/`) and colon (`:`)—accommodating various user conventions and external configuration formats. This permissive parsing, combined with comprehensive fallback chains (explicit parameter → inherited lead model → default), ensures robust behavior across diverse deployment scenarios while maintaining type safety through `Option<ModelRef>`.

The `ModelRef` abstraction enables significant operational capabilities including per-teammate model specialization for cost optimization, capability matching, or workload distribution. The display formatting logic that distinguishes explicitly selected models from inherited ones (`{}/{} (inherited)`) provides important provenance information for debugging and auditing. This struct's presence in the public API surface—both as input parameter and metadata output—positions it as a foundational type for the framework's model management strategy, likely shared across multiple tools and agent configurations beyond team spawning alone.

## External Resources

- [Serde serialization framework for Rust data structures](https://docs.rs/serde/latest/serde/) - Serde serialization framework for Rust data structures
- [OpenAI model reference for provider/model identification patterns](https://platform.openai.com/docs/models) - OpenAI model reference for provider/model identification patterns

## Sources

- [team_spawn](../sources/team-spawn.md)
