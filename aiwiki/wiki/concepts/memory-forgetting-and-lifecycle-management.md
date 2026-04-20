---
title: "Memory Forgetting and Lifecycle Management"
type: concept
generated: "2026-04-19T21:44:33.457117924+00:00"
---

# Memory Forgetting and Lifecycle Management

### From: store

The ForgetFilter abstraction implements intentional forgetting—a critical but often neglected capability in knowledge management systems. Unbounded growth degrades retrieval performance and signal-to-noise ratios; proactive forgetting maintains system efficiency. The dual-mode design (Id vs Filter) supports both surgical precision and bulk operations, with has_any_criterion preventing dangerous unbounded deletions. This safety mechanism reflects operational experience with accidental data loss in filter-based APIs.

The filter criteria encode domain-specific forgetting policies. Age-based deletion (older_than_days) implements temporal decay—memories lose relevance over time unless reinforced. Confidence-based deletion (max_confidence) prunes unreliable knowledge, potentially triggering re-learning cycles. Category-based deletion enables schema evolution when categories are deprecated or redefined. Tag-based intersection (all specified tags must match) supports targeted cleanup like "remove all experimental workflow memories from test projects." The conjunction of criteria (implicit AND) allows precise targeting; the absence of OR or NOT operators simplifies the implementation while covering common use cases.

The empty_tags handling in has_any_criterion demonstrates careful attention to Option<Vec> semantics. In Rust, Some(vec![]) and None both represent absent meaningful constraints, but the code correctly treats empty vectors as non-criteria to prevent surprising behavior where specifying empty tags inadvertently enables deletion. This edge case handling separates robust production code from naive implementations. The integration with SQLite likely uses parameterized DELETE statements with dynamic WHERE clause construction, requiring careful SQL injection prevention despite the type-safe Rust layer above.

## External Resources

- [Information forgetting in knowledge representation](https://en.wikipedia.org/wiki/Information_forgetting) - Information forgetting in knowledge representation
- [SQLite DELETE statement documentation](https://www.sqlite.org/lang_delete.html) - SQLite DELETE statement documentation
- [CWE-89: SQL Injection prevention](https://cwe.mitre.org/data/definitions/89.html) - CWE-89: SQL Injection prevention

## Sources

- [store](../sources/store.md)
