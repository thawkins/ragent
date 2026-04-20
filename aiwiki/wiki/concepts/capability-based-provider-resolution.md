---
title: "Capability-Based Provider Resolution"
type: concept
generated: "2026-04-19T14:56:28.627712958+00:00"
---

# Capability-Based Provider Resolution

### From: main

ragent implements capability-based provider resolution to enable flexible, user-configurable AI model selection while respecting agent-specific constraints. The resolution hierarchy demonstrates sophisticated priority handling: CLI --model flag takes highest precedence, followed by user-stored preferences in the database (set via TUI commands), with agent defaults as fallback. The model_pinned flag provides an escape hatch for custom agents that must use specific models regardless of global configuration.

The provider/model format (e.g., "copilot/claude-sonnet-4.5") encodes both the routing destination and the specific model identifier, enabling a single string to fully specify the inference endpoint. The split_once parsing enforces this structure, providing clear error messages for malformed input. The ModelRef struct normalizes this into a typed representation with provider_id and model_id fields, preventing stringly-typed errors downstream.

This architecture supports complex deployment scenarios: organizations might pin specific models for compliance, individual developers might prefer local Ollama instances for cost control, and specific tasks might require cloud providers for capability reasons. The resolution ordering ensures user intent is respected while preventing broken configurations—pinned agents maintain their guarantees while non-pinned agents accept user preferences. The integration with storage for persistent preferences enables a learn-and-remember UX where the TUI can update defaults based on explicit user selection.

## External Resources

- [Capability-based security on Wikipedia](https://en.wikipedia.org/wiki/Capability-based_security) - Capability-based security on Wikipedia

## Sources

- [main](../sources/main.md)
