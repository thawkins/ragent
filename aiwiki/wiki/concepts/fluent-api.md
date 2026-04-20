---
title: "Fluent API"
type: concept
generated: "2026-04-19T16:40:30.588028775+00:00"
---

# Fluent API

### From: metadata

A fluent API is an object-oriented design pattern that aims to make code more readable and discoverable by allowing method calls to be chained in a manner that resembles natural language or domain-specific expressions. The `MetadataBuilder` in RAgent Core exemplifies this approach through its extensive use of method chaining, where each configuration method returns the receiver (`self`), enabling sequences like `MetadataBuilder::new().path("/file").line_count(42).build()`. This style eliminates the need for intermediate variables and creates self-documenting code where the sequence of operations mirrors the logical flow of metadata construction.

The implementation of fluent APIs in Rust presents unique considerations due to the language's ownership and borrowing rules. The `metadata.rs` module adopts the consuming approach to chaining, where methods take `mut self` and return `Self`. This design choice ensures that the builder's internal state can be modified while maintaining thread safety and preventing aliasing issues. The `#[must_use]` attribute on each setter method addresses a potential pitfall of fluent APIs in Rust: because the methods consume and return `self`, discarding the return value would silently drop the builder with partial configuration. This attribute forces the compiler to warn when return values are ignored, maintaining safety without sacrificing fluency.

The domain-specific design of the fluent API in this module reflects careful analysis of tool use cases in agent systems. Method names like `summarized`, `truncated`, and `timed_out` use past participles and adjectives that describe the resulting state, while methods like `edit_lines` and `status_code` use nouns that identify the data being added. This naming consistency aids API discoverability through IDE autocomplete and makes code review more intuitive. The API's granularity—separate methods for semantically similar counts like `count`, `file_count`, `entries`, and `matches`—prevents errors from using generic fields for specific purposes while maintaining the flexibility of a `custom` method for exceptional cases. The extensive test suite leverages the fluent API's readability, with test cases that serve as executable documentation of expected usage patterns.

## External Resources

- [Wikipedia: Fluent Interface](https://en.wikipedia.org/wiki/Fluent_interface) - Wikipedia: Fluent Interface
- [Martin Fowler on Fluent Interfaces](https://martinfowler.com/bliki/FluentInterface.html) - Martin Fowler on Fluent Interfaces

## Related

- [Builder Pattern](builder-pattern.md)

## Sources

- [metadata](../sources/metadata.md)
