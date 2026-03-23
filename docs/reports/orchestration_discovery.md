Capability Discovery & Matching (MVP)

Purpose

Define a simple capability discovery scheme and task-matching rules for selecting agents to handle subtasks in the Multi-Agent Orchestration MVP.

Design

1) Capability Representation
- Capabilities are simple human-readable tags (strings), e.g. "search", "build", "plan", "compile".
- Agents expose a Vec<String> of capabilities when registered.

2) Matching Rules (MVP)
- Exact tag containment for simplicity: an agent matches if it contains every required tag as a substring in at least one of its capability strings.
  - Example: required ["search"] matches capabilities ["code-search", "analysis"] because "search" is a substring of "code-search".
- Matching is conjunctive: an agent must satisfy all required tags (logical AND).

3) Ordering & Determinism
- The registry returns matches in insertion order (the order agents were registered). Coordinator selects agents deterministically by iterating matches in this order.

4) Extensibility (future)
- Score-based matching: compute a capability score (tf-idf or semantic cosine similarity) and return ranked candidates.
- Priority & load awareness: include agent load metrics and prefer less-loaded agents.
- Soft matching & fallback: permit partial matches and fallback strategies when no exact matches are found.

Examples

- Job requires ["search", "analysis"]
  - Agent A capabilities: ["code-search", "analysis"] -> matches
  - Agent B capabilities: ["search"] -> does not match (missing "analysis")

- Job requires ["compile"]
  - Agent A capabilities: ["compile", "test"] -> matches
  - Agent B capabilities: ["build-compile"] -> matches (substring match)

Acceptance Criteria

- Registry exposes match_agents(required: &[String]) -> Vec<AgentEntry>
- Matching rules are documented in docs/orchestration_discovery.md
