//! Shared surface helpers for the ragent `/plugins` and `/connectors` command
//! families (spec `plugins` T-015 / spec `connectors` T-010; FR-004, FR-006,
//! FR-013, FR-015, FR-017).
//!
//! The two families were built as parallel surfaces, and their usage
//! attribution, subcommand tokeniser, store-directory resolution, harness step
//! model, and JSON-schema sample generator had drifted into byte-identical
//! copies. Each helper now has a single implementation here and both crates
//! delegate, so the two `/test` and `help` surfaces cannot diverge.
//!
//! Module layout:
//!
//! | Module    | Responsibility                                            |
//! | --------- | --------------------------------------------------------- |
//! | [`harness`] | step model, [`harness::sample_for_schema`], [`harness::truncate`] |
//! | [`help`]    | usage [`help::attribution`] and [`help::subcommand_of`]    |
//! | [`store`]   | [`store::StoreDirs`] resolution and store-entry checks     |

pub mod harness;
pub mod help;
pub mod store;
