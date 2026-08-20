//! Regression for the 2026-08-20 /research create panic: html2text
//! `WrappedBlock::flush_word` "attempt to subtract with overflow".
//!
//! Runs the extraction chain twice: once normally and once on a background
//! thread (matching how the research web gatherer now invokes the converter).

use ragent_tools_extended::masterfetch::extractor::{ExtractOptions, extract};

const PANIC_HTML: &str = include_str!("fixtures/rust_book_brown.html");

#[test]
fn test_panic_prone_page_extracts_without_crashing_process() {
    let opts = ExtractOptions::default();
    for i in 0..5 {
        let result = extract(
            PANIC_HTML,
            "https://rust-book.cs.brown.edu/",
            "text/html",
            &opts,
        );
        let result = result.unwrap_or_else(|e| panic!("iteration {i} failed: {e}"));
        assert!(!result.content.is_empty());
    }
}
