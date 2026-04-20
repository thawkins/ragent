---
title: "Dimensional Unit Conventions"
type: concept
generated: "2026-04-19T14:54:44.383951675+00:00"
---

# Dimensional Unit Conventions

### From: ref:AGENTS

The dimensional unit conventions establish precise internal representation requirements for physical measurements, specifically mandating millimeters as the base unit for length with f32 floating-point precision and 2 decimal place accuracy. This standardization addresses common integration failures where inconsistent units—mixing meters, millimeters, inches, or points—cause subtle calculation errors that compound through geometric operations. The f32 choice balances precision needs against memory bandwidth and cache efficiency, particularly relevant for graphics, CAD, or robotics applications where vertex data or spatial calculations may be performance-critical.

The 2 decimal place accuracy specification suggests domain context where sub-millimeter precision is unnecessary or where display rounding provides sufficient fidelity. For datetime values, the UTC internal representation with locale translation deferred to the UI layer follows similar architectural separation: core logic operates on unambiguous universal time, while presentation concerns handle timezone conversion and formatting preferences. This pattern prevents the class of bugs where timezone-aware and timezone-naive values are incorrectly combined.

The UTF-8 text encoding mandate for internal string representation, with encoding translation restricted to UI boundaries, ensures consistent text processing throughout application logic. This Unicode normalization prevents encoding-related string corruption that plagued legacy systems with mixed encoding assumptions. The three unit conventions—spatial, temporal, and textual—share a common philosophy: internal representations should be unambiguous, efficient, and divorced from presentation concerns. This architectural separation enables reliable core algorithms while permitting flexible, localized user interfaces without risk of data corruption during transformations.

## External Resources

- [International System of Units (SI) reference](https://en.wikipedia.org/wiki/International_System_of_Units) - International System of Units (SI) reference
- [IANA Time Zone Database](https://www.iana.org/time-zones) - IANA Time Zone Database

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
