//! Shared test-support helpers for `ragent-codeindex` integration tests.
//!
//! The per-language parser test files each defined a one-line `parse_<lang>`
//! wrapper around `LanguageParser::parse(...).unwrap()`; the shared generic
//! helper lives here so each wrapper is a single delegation.
//!
//! Not every test binary uses the helper, so silence dead-code warnings per
//! binary.

#![allow(dead_code)]

use ragent_codeindex::parser::{LanguageParser, ParsedFile};

/// Parse `source` with `parser`, panicking on a parse error (test context).
pub fn parse_with<P: LanguageParser>(parser: &P, source: &str) -> ParsedFile {
    parser
        .parse(source.as_bytes())
        .expect("parser should succeed on the test input")
}
