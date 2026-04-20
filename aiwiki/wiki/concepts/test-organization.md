---
title: "Test Organization"
type: concept
generated: "2026-04-19T14:54:44.382477538+00:00"
---

# Test Organization

### From: ref:AGENTS

Test organization in these guidelines establishes strict structural requirements for maintaining test code separately from implementation, reflecting Rust community best practices for project scalability. The fundamental rule mandates that all tests reside in tests/ directories—either within each crate for crate-specific tests or at the project root for integration tests—explicitly prohibiting inline tests within source files. This separation of concerns improves compilation times by excluding test code from release builds, enables parallel test execution, and maintains clear boundaries between production and verification code. The guidelines include specific migration procedures for reorganizing existing test code, requiring systematic review and relocation of inline tests to appropriate subdirectories.

The test organization specification extends to execution patterns, distinguishing between synchronous tests with #[test] and asynchronous tests with #[tokio::test]. This dual support reflects modern Rust's async/await ecosystem while maintaining compatibility with synchronous code. The naming convention requirement—test_function_name_scenario (e.g., test_jog_x_positive)—provides descriptive test identification that aids in test selection and failure diagnosis. The import requirement from the public ragent crate enforces API usage patterns that match external consumer access, preventing tests from depending on implementation details that may change.

The timeout infrastructure for test execution, with 10-minute limits and platform-specific commands, addresses Rust's compilation and test execution characteristics where complex test suites may require substantial time. The --test-threads=1 option for sequential execution provides a fallback for tests with shared resource dependencies or non-deterministic ordering requirements. The guidelines' emphasis on test location and organization over test writing specifics suggests organizational experience with test suite maintenance challenges at scale, where discoverability and consistent structure outweigh individual test implementation flexibility.

## External Resources

- [Rust Book chapter on test organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html) - Rust Book chapter on test organization
- [Rust by Example unit testing patterns](https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html) - Rust by Example unit testing patterns

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
