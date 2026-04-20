---
title: "Sequential ID Generation"
type: concept
generated: "2026-04-19T21:17:40.434775636+00:00"
---

# Sequential ID Generation

### From: store

Sequential ID generation is a resource naming strategy that produces human-readable, ordered identifiers by maintaining a counter that increments with each new resource creation. The RAgent team store implements this pattern for both task identifiers (`task-NNN` format) and team member identifiers (`tm-NNN` format), where `NNN` represents a zero-padded three-digit number. This approach prioritizes human interpretability and lexicographic sorting over cryptographic uniqueness or distributed allocation, making it well-suited for single-user or small-team scenarios where global uniqueness is not required.

The implementation in `next_task_id()` and `next_agent_id()` demonstrates a scan-and-increment algorithm that derives the next available number by examining existing collections. For tasks, the method opens the `TaskStore`, reads the current task list, and filters for IDs matching the expected prefix, parsing the numeric component and finding the maximum assigned value. The `filter_map` combinator elegantly handles parse failures by converting them to `None` and excluding them from the maximum calculation. The `unwrap_or(0)` pattern provides a default starting point when no existing IDs are found, ensuring that the first task receives ID `task-001` rather than causing a panic.

This ID generation strategy carries implicit assumptions about concurrency and persistence that shape its applicability. The scan-based approach requires loading the entire collection to determine the next ID, which scales poorly for large datasets but remains efficient for typical team sizes. The three-digit zero-padding accommodates up to 999 resources of each type, with lexicographic sorting producing intuitive ordering (`task-001` before `task-002`). The pattern is not thread-safe across concurrent processes, as race conditions could produce duplicate IDs if multiple RAgent instances simultaneously scan and increment. For the intended use case of personal or small-team agent orchestration, these limitations are acceptable trade-offs for simplicity and inspectability.

## External Resources

- [UUID specification as alternative to sequential IDs](https://en.wikipedia.org/wiki/Universally_unique_identifier) - UUID specification as alternative to sequential IDs
- [ULID (Universally Unique Lexicographically Sortable Identifier) crate](https://docs.rs/ulid/latest/ulid/) - ULID (Universally Unique Lexicographically Sortable Identifier) crate

## Sources

- [store](../sources/store.md)
