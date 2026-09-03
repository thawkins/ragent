#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-codeindex/src/search.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::search::{FtsIndex, FtsSymbol, SearchResult};

fn sample_symbols() -> Vec<FtsSymbol<'static>> {
    vec![
        FtsSymbol {
            name: "parse_config",
            qualified_name: Some("crate::config::parse_config"),
            kind: "function",
            file_path: "src/config.rs",
            signature: Some("fn parse_config(path: &Path) -> Result<Config>"),
            doc_comment: Some("Parse the configuration file from disk."),
            body_snippet: Some("let content = fs::read_to_string(path)?;"),
            start_line: 10,
            end_line: 25,
        },
        FtsSymbol {
            name: "Config",
            qualified_name: Some("crate::config::Config"),
            kind: "struct",
            file_path: "src/config.rs",
            signature: Some("pub struct Config"),
            doc_comment: Some("Application configuration loaded from TOML."),
            body_snippet: Some("name: String, port: u16, debug: bool"),
            start_line: 1,
            end_line: 8,
        },
        FtsSymbol {
            name: "serve",
            qualified_name: Some("crate::server::serve"),
            kind: "function",
            file_path: "src/server.rs",
            signature: Some("pub async fn serve(config: &Config) -> Result<()>"),
            doc_comment: Some("Start the HTTP server."),
            body_snippet: Some("let listener = TcpListener::bind(config.addr()).await?;"),
            start_line: 15,
            end_line: 45,
        },
    ]
}

#[test]
fn test_open_in_memory() {
    let fts = FtsIndex::open_in_memory().unwrap();
    assert_eq!(fts.doc_count().unwrap(), 0);
}

#[test]
fn test_add_and_search_by_name() {
    let fts = FtsIndex::open_in_memory().unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();

    let results = fts.search("parse_config", 10).unwrap();
    assert!(!results.is_empty(), "should find parse_config");
    assert_eq!(results[0].symbol_name, "parse_config");
    assert_eq!(results[0].file_path, "src/config.rs");
    assert_eq!(results[0].line, 10);
}

#[test]
fn test_search_by_doc_comment() {
    let fts = FtsIndex::open_in_memory().unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();

    let results = fts.search("HTTP server", 10).unwrap();
    assert!(!results.is_empty(), "should find by doc comment");
    assert_eq!(results[0].symbol_name, "serve");
}

#[test]
fn test_search_by_body_snippet() {
    let fts = FtsIndex::open_in_memory().unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();

    let results = fts.search("read_to_string", 10).unwrap();
    assert!(!results.is_empty(), "should find by body snippet");
    assert_eq!(results[0].symbol_name, "parse_config");
}

#[test]
fn test_search_by_signature() {
    let fts = FtsIndex::open_in_memory().unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();

    let results = fts.search("Result<Config>", 10).unwrap();
    assert!(!results.is_empty(), "should find by signature");
    assert_eq!(results[0].symbol_name, "parse_config");
}

#[test]
fn test_remove_file() {
    let fts = FtsIndex::open_in_memory().unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();
    assert_eq!(fts.doc_count().unwrap(), 3);

    fts.remove_file("src/config.rs").unwrap();
    // After remove + commit, docs from config.rs should be gone.
    // Note: tantivy soft-deletes, so num_docs may still report them
    // until a merge; but search should not return them.
    let results = fts.search("parse_config", 10).unwrap();
    assert!(
        results.is_empty(),
        "parse_config should be gone after remove_file"
    );
}

#[test]
fn test_search_limit() {
    let fts = FtsIndex::open_in_memory().unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();

    // Search for something matching all (a common term)
    let results = fts.search("config", 1).unwrap();
    assert!(results.len() <= 1, "limit should cap results");
}

#[test]
fn test_name_boost_over_body() {
    let fts = FtsIndex::open_in_memory().unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();

    // "Config" is a name hit AND appears in body/doc of other symbols.
    // The name-match should rank highest.
    let results = fts.search("Config", 10).unwrap();
    assert!(!results.is_empty());
    assert_eq!(
        results[0].symbol_name, "Config",
        "name match should rank first"
    );
}

#[test]
fn test_display_compact() {
    let r = SearchResult {
        symbol_name: "foo".into(),
        qualified_name: "crate::foo".into(),
        kind: "function".into(),
        file_path: "src/lib.rs".into(),
        line: 42,
        end_line: 50,
        score: 1.5,
        signature: "fn foo()".into(),
        doc_snippet: "Does something.".into(),
    };
    let compact = format!("{r}");
    assert!(compact.contains("function foo"));
    assert!(compact.contains("src/lib.rs:42"));
}

#[test]
fn test_display_detailed() {
    let r = SearchResult {
        symbol_name: "foo".into(),
        qualified_name: "crate::foo".into(),
        kind: "function".into(),
        file_path: "src/lib.rs".into(),
        line: 42,
        end_line: 50,
        score: 1.5,
        signature: "fn foo()".into(),
        doc_snippet: "Does something.".into(),
    };
    let detailed = format!("{r:#}");
    assert!(detailed.contains("qualified: crate::foo"));
    assert!(detailed.contains("signature: fn foo()"));
    assert!(detailed.contains("doc: Does something."));
}

#[test]
fn test_search_with_scope_operator() {
    let fts = FtsIndex::open_in_memory().unwrap();
    let syms = vec![FtsSymbol {
        name: "new",
        qualified_name: Some("Widget::new"),
        kind: "function",
        file_path: "src/widget.rs",
        signature: Some("fn new() -> Self"),
        doc_comment: Some("Create a new Widget instance."),
        body_snippet: Some("Self { }"),
        start_line: 10,
        end_line: 15,
    }];
    fts.add_symbols(&syms).unwrap();

    // Query with :: should not cause a parse error
    let results = fts.search("Widget::new", 10).unwrap();
    assert!(!results.is_empty(), "should find Widget::new");
}

#[test]
fn test_sanitize_query_escapes_special_chars() {
    // :: is replaced with space so path segments become separate terms
    let escaped = FtsIndex::sanitize_query("Widget::new");
    assert_eq!(escaped, "Widget new");

    let escaped2 = FtsIndex::sanitize_query("Result<Config>");
    assert!(escaped2.contains("Result"));
    assert!(escaped2.contains("Config"));

    // Plain text should pass through unchanged
    let plain = FtsIndex::sanitize_query("hello world");
    assert_eq!(plain, "hello world");

    // Single colon (field syntax) is escaped
    let single = FtsIndex::sanitize_query("field:value");
    assert_eq!(single, "field\\:value");
}

#[test]
fn test_open_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let fts_path = dir.path().join("fts");
    let fts = FtsIndex::open(&fts_path).unwrap();
    fts.add_symbols(&sample_symbols()).unwrap();
    assert_eq!(fts.doc_count().unwrap(), 3);
    drop(fts);

    // Re-open and verify data persisted
    let fts2 = FtsIndex::open(&fts_path).unwrap();
    let results = fts2.search("parse_config", 10).unwrap();
    assert!(!results.is_empty());
}
