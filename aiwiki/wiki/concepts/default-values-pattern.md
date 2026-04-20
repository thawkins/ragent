---
title: "Default Values Pattern"
type: concept
generated: "2026-04-19T22:12:49.360765939+00:00"
---

# Default Values Pattern

### From: test_config

The default values pattern in software design ensures that systems are usable immediately without requiring exhaustive configuration upfront. By providing sensible defaults for all optional settings, developers reduce cognitive load and enable progressive disclosure of complexity. Users can start with minimal configuration and gradually customize as needed. This pattern is particularly important in agent frameworks like ragent where new users should be able to run basic agents without understanding every available option.

The test_config_default_values test serves as executable documentation of these defaults. The "general" default agent suggests a versatile, multi-purpose agent suitable for common tasks. Empty collections for providers, permissions, agents, commands, and MCP indicate optional integrations that can be added incrementally. The disabled experimental flag follows the principle of stability by default—new features must be explicitly opted into.

Implementing this pattern in Rust typically combines Serde's default attributes with the Default trait. For simple types, #[serde(default)] uses the type's Default implementation. For custom logic, #[serde(default = "path")] specifies a function. The pattern extends beyond deserialization: runtime configuration builders often start from defaults and apply overrides. This creates a clear precedence: hardcoded defaults < configuration file < environment variables < command-line flags, with each layer able to modify only what it needs.

## External Resources

- [Rust API guidelines on implementing common traits including Default](https://rust-lang.github.io/api-guidelines/interoperability.html#c-common-traits) - Rust API guidelines on implementing common traits including Default
- [Convention over configuration - Wikipedia](https://en.wikipedia.org/wiki/Convention_over_configuration) - Convention over configuration - Wikipedia

## Related

- [Serde Deserialization](serde-deserialization.md)
- [Configuration Merging](configuration-merging.md)

## Sources

- [test_config](../sources/test-config.md)
