---
title: "Capability-Based Agent Discovery"
type: concept
generated: "2026-04-19T20:52:42.206238136+00:00"
---

# Capability-Based Agent Discovery

### From: transport

Capability-based discovery is a service location paradigm where agents advertise functional characteristics rather than fixed network addresses, enabling dynamic matching between work requirements and service providers. The ragent implementation uses string tags in the `RemoteAgentDescriptor::capabilities` field, with the `HttpRouter::match_agents` method implementing substring-based intersection matching. This approach allows agents to declare complex competencies like `"llama-70b-inference"` or `"rust-code-generation"` while requesters specify required capabilities as criteria vectors, decoupling service identification from physical deployment locations.

The substring matching strategy (`c.contains(r.as_str)`) provides flexibility for hierarchical capability namespaces. An agent advertising `"gpu-cuda-rtx4090"` would match requests for `"gpu"`, `"cuda"`, or the specific hardware model, enabling both broad and precise targeting. This design reflects lessons from large-scale distributed systems like Google's Borg and Kubernetes' label selectors, where explicit capability advertisement prevents the fragility of implicit naming conventions. However, the current implementation's O(n×m×k) complexity for n agents, m required capabilities, and k advertised capabilities suggests optimization opportunities for large registries through inverted index structures or prefix tree (trie) data structures.

Capability-based routing enables critical operational patterns including workload-specific scaling, hardware-aware scheduling, and progressive deployment of new agent versions. When a new image generation model becomes available, operators deploy agents advertising the new capability tag alongside existing agents; requesters targeting that specific capability automatically route to new instances while legacy traffic continues uninterrupted. This model supports A/B testing, canary deployments, and blue-green migration strategies without coordination overhead. The approach contrasts with traditional DNS-based or load-balancer discovery by making semantic fitness explicit rather than assuming uniform service behavior across identically-named endpoints.

## External Resources

- [Kubernetes labels and selectors](https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/) - Kubernetes labels and selectors
- [Borg, Omega, and Kubernetes - Lessons from large-scale cluster managers](https://research.google/pubs/pub43438/) - Borg, Omega, and Kubernetes - Lessons from large-scale cluster managers
- [Trie data structure for efficient string matching](https://en.wikipedia.org/wiki/Trie) - Trie data structure for efficient string matching

## Sources

- [transport](../sources/transport.md)
