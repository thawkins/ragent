---
title: "DedupResult"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:55:53.399962336+00:00"
---

# DedupResult

**Type:** technology

### From: compact

DedupResult is a Rust enum that encapsulates the complete outcome state of a memory deduplication check, providing rich semantic information to guide downstream decision-making. The enum defines three distinct variants: NoDuplicate indicates that the proposed memory is sufficiently novel to warrant independent storage; Duplicate represents cases where similarity exceeds 0.95, triggering automatic merging with the existing memory; and NearDuplicate covers the intermediate zone between 0.8 and 0.95 similarity where merging may be beneficial but requires explicit user confirmation. Each variant carries appropriate metadata—the Duplicate and NearDuplicate variants include the existing memory's row identifier, the computed similarity score, and proposed merged values for content, confidence, and tags. This structured approach enables the system to implement graduated response strategies, from silent automatic merging for obvious duplicates through interactive workflows for borderline cases. The enum derives Serialize and Deserialize traits, facilitating persistence and transmission across system boundaries, while the Debug trait enables comprehensive logging for audit purposes.

## External Resources

- [Serde enum serialization patterns for structured data exchange](https://serde.rs/enum-representations.html) - Serde enum serialization patterns for structured data exchange

## Sources

- [compact](../sources/compact.md)
