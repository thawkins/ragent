//! Integration test M2.5: scan → parse → store end-to-end pipeline.
//!
//! Creates temp directories with Rust source files, scans them, parses
//! with tree-sitter, stores symbols/imports in SQLite, and queries back.
#![allow(missing_docs)]

use ragent_code::parser::ParserRegistry;
use ragent_code::scanner::scan_directory;
use ragent_code::store::IndexStore;
use ragent_code::types::{ScanConfig, SymbolFilter, SymbolKind};
use std::fs;
use tempfile::TempDir;

/// Create a temp project with realistic Rust files.
fn create_rust_project() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path();

    fs::create_dir_all(root.join("src")).unwrap();

    // A file with a struct, impl block, and methods
    fs::write(
        root.join("src/config.rs"),
        r#"
/// Application configuration.
pub struct Config {
    pub name: String,
    pub port: u16,
}

impl Config {
    /// Create a new configuration with defaults.
    pub fn new(name: &str) -> Self {
        Config {
            name: name.to_string(),
            port: 8080,
        }
    }

    /// Get the binding address.
    pub fn address(&self) -> String {
        format!("0.0.0.0:{}", self.port)
    }
}
"#,
    )
    .unwrap();

    // A file with free functions, imports, and a test
    fs::write(
        root.join("src/utils.rs"),
        r#"
use std::path::PathBuf;
use std::collections::HashMap;

/// Greet the user.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

fn helper() -> i32 {
    42
}

pub const VERSION: &str = "1.0.0";

#[test]
fn test_greet() {
    assert_eq!(greet("world"), "Hello, world!");
}
"#,
    )
    .unwrap();

    // A file with a trait and enum
    fs::write(
        root.join("src/types.rs"),
        r#"
/// Severity levels for log messages.
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Something that can be rendered to text.
pub trait Renderable {
    fn render(&self) -> String;
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub mod inner {
    pub fn nested_fn() -> bool {
        true
    }
}
"#,
    )
    .unwrap();

    dir
}

/// Run the full pipeline: scan → parse → store for all files in a project.
/// Returns the store and a list of (path, file_id) pairs.
fn index_project(dir: &TempDir) -> (IndexStore, Vec<(String, i64)>) {
    let config = ScanConfig::default();
    let scanned = scan_directory(dir.path(), &config).unwrap();
    let store = IndexStore::open_in_memory().unwrap();
    let registry = ParserRegistry::new();

    let diff = store.get_stale_files(&scanned).unwrap();
    store.apply_diff(&diff).unwrap();

    let mut indexed_files = Vec::new();

    for file in &scanned {
        let path_str = file.path.to_string_lossy().to_string();
        let file_id = store.get_file_id(&path_str).unwrap().unwrap();

        let lang = file.language.as_deref().unwrap_or("unknown");
        if let Some(Ok(parsed)) =
            registry.parse(lang, &fs::read(dir.path().join(&file.path)).unwrap())
        {
            // Patch file_id into symbols and imports
            let mut symbols = parsed.symbols;
            for sym in &mut symbols {
                sym.file_id = file_id;
            }
            store.upsert_symbols(file_id, &symbols).unwrap();

            let mut imports = parsed.imports;
            for imp in &mut imports {
                imp.file_id = file_id;
            }
            store.upsert_imports(file_id, &imports).unwrap();

            store.upsert_refs(file_id, &parsed.references).unwrap();
        }

        indexed_files.push((path_str, file_id));
    }

    (store, indexed_files)
}

#[test]
fn test_full_pipeline_scan_parse_store() {
    let dir = create_rust_project();
    let (store, indexed_files) = index_project(&dir);

    // We should have indexed 3 Rust files
    let rust_files: Vec<_> = indexed_files
        .iter()
        .filter(|(p, _)| p.ends_with(".rs"))
        .collect();
    assert_eq!(rust_files.len(), 3, "should index 3 Rust files");

    // Should have extracted a meaningful number of symbols
    let total = store.symbol_count().unwrap();
    assert!(total >= 10, "expected at least 10 symbols, got {total}");
}

#[test]
fn test_query_symbols_by_name() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let results = store
        .query_symbols(&SymbolFilter {
            name: Some("Config".to_string()),
            ..Default::default()
        })
        .unwrap();

    assert!(
        results
            .iter()
            .any(|s| s.name == "Config" && s.kind == SymbolKind::Struct),
        "should find Config struct, got: {:?}",
        results
            .iter()
            .map(|s| (&s.name, &s.kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_query_symbols_by_kind() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    // Functions
    let functions = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Function),
            ..Default::default()
        })
        .unwrap();
    let fn_names: Vec<&str> = functions.iter().map(|s| s.name.as_str()).collect();
    assert!(
        fn_names.contains(&"greet"),
        "should find greet function: {fn_names:?}"
    );
    assert!(
        fn_names.contains(&"helper"),
        "should find helper function: {fn_names:?}"
    );

    // Methods (inside impl Config)
    let methods = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Method),
            ..Default::default()
        })
        .unwrap();
    let method_names: Vec<&str> = methods.iter().map(|s| s.name.as_str()).collect();
    assert!(
        method_names.contains(&"new"),
        "should find new method: {method_names:?}"
    );
    assert!(
        method_names.contains(&"address"),
        "should find address method: {method_names:?}"
    );

    // Structs
    let structs = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Struct),
            ..Default::default()
        })
        .unwrap();
    assert!(
        structs.iter().any(|s| s.name == "Config"),
        "should find Config struct"
    );

    // Enums
    let enums = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Enum),
            ..Default::default()
        })
        .unwrap();
    assert!(
        enums.iter().any(|s| s.name == "Severity"),
        "should find Severity enum"
    );

    // Traits
    let traits = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Trait),
            ..Default::default()
        })
        .unwrap();
    assert!(
        traits.iter().any(|s| s.name == "Renderable"),
        "should find Renderable trait"
    );

    // Tests
    let tests = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Test),
            ..Default::default()
        })
        .unwrap();
    assert!(
        tests.iter().any(|s| s.name == "test_greet"),
        "should find test_greet test function"
    );

    // Constants
    let constants = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Constant),
            ..Default::default()
        })
        .unwrap();
    assert!(
        constants.iter().any(|s| s.name == "VERSION"),
        "should find VERSION constant"
    );
}

#[test]
fn test_query_symbols_by_visibility() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let pub_symbols = store
        .query_symbols(&SymbolFilter {
            visibility: Some(ragent_code::types::Visibility::Public),
            ..Default::default()
        })
        .unwrap();

    let pub_names: Vec<&str> = pub_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(pub_names.contains(&"Config"), "Config should be public");
    assert!(pub_names.contains(&"greet"), "greet should be public");
    assert!(pub_names.contains(&"Severity"), "Severity should be public");
    assert!(
        !pub_names.contains(&"helper"),
        "helper should not be public"
    );
}

#[test]
fn test_method_parent_relationship() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let all = store.query_symbols(&SymbolFilter::default()).unwrap();

    // Find the impl block for Config
    let impl_sym = all
        .iter()
        .find(|s| s.kind == SymbolKind::Impl && s.name.contains("Config"));
    assert!(impl_sym.is_some(), "should find impl Config block");
    let impl_id = impl_sym.unwrap().id;

    // Methods should reference the impl block as parent
    let new_method = all
        .iter()
        .find(|s| s.name == "new" && s.kind == SymbolKind::Method);
    assert!(new_method.is_some(), "should find new method");
    assert_eq!(
        new_method.unwrap().parent_id,
        Some(impl_id),
        "new method should be child of impl Config"
    );
}

#[test]
fn test_imports_stored() {
    let dir = create_rust_project();
    let (store, indexed_files) = index_project(&dir);

    // Find utils.rs file_id
    let utils_file = indexed_files
        .iter()
        .find(|(p, _)| p.contains("utils.rs"))
        .expect("should have utils.rs");

    let imports = store.get_file_imports(utils_file.1).unwrap();
    assert!(
        imports.len() >= 2,
        "utils.rs should have at least 2 imports, got {}",
        imports.len()
    );

    let imported_names: Vec<&str> = imports.iter().map(|i| i.imported_name.as_str()).collect();
    assert!(
        imported_names
            .iter()
            .any(|n| n.contains("PathBuf") || n.contains("std::path::PathBuf")),
        "should import PathBuf: {imported_names:?}"
    );
}

#[test]
fn test_doc_comments_extracted() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let configs = store
        .query_symbols(&SymbolFilter {
            name: Some("Config".to_string()),
            kind: Some(SymbolKind::Struct),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(configs.len(), 1);
    let doc = configs[0].doc_comment.as_deref().unwrap_or("");
    assert!(
        doc.contains("Application configuration"),
        "Config doc comment should contain 'Application configuration', got: {doc:?}"
    );
}

#[test]
fn test_signature_extracted() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let greet_fns = store
        .query_symbols(&SymbolFilter {
            name: Some("greet".to_string()),
            kind: Some(SymbolKind::Function),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(greet_fns.len(), 1);
    let sig = greet_fns[0].signature.as_deref().unwrap_or("");
    assert!(
        sig.contains("fn greet"),
        "greet signature should contain 'fn greet', got: {sig:?}"
    );
    assert!(
        sig.contains("-> String"),
        "greet signature should contain return type, got: {sig:?}"
    );
}

#[test]
fn test_qualified_names() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let nested = store
        .query_symbols(&SymbolFilter {
            name: Some("nested_fn".to_string()),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(nested.len(), 1);
    let qn = nested[0].qualified_name.as_deref().unwrap_or("");
    assert!(
        qn.contains("inner::nested_fn"),
        "nested_fn qualified name should contain 'inner::nested_fn', got: {qn:?}"
    );
}

#[test]
fn test_enum_variants_stored() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let variants = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::EnumVariant),
            ..Default::default()
        })
        .unwrap();

    let variant_names: Vec<&str> = variants.iter().map(|s| s.name.as_str()).collect();
    assert!(variant_names.contains(&"Info"), "should find Info variant");
    assert!(
        variant_names.contains(&"Warning"),
        "should find Warning variant"
    );
    assert!(
        variant_names.contains(&"Error"),
        "should find Error variant"
    );

    // Variants should have the Severity enum as parent
    let all = store.query_symbols(&SymbolFilter::default()).unwrap();
    let severity = all.iter().find(|s| s.name == "Severity").unwrap();
    for v in &variants {
        assert_eq!(
            v.parent_id,
            Some(severity.id),
            "{} variant should be child of Severity enum",
            v.name
        );
    }
}

#[test]
fn test_incremental_reindex_updates_symbols() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    // Verify initial state: greet returns String
    let greet_v1 = store
        .query_symbols(&SymbolFilter {
            name: Some("greet".to_string()),
            kind: Some(SymbolKind::Function),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(greet_v1.len(), 1);
    let sig_v1 = greet_v1[0].signature.as_deref().unwrap_or("");
    assert!(sig_v1.contains("-> String"), "v1 should return String");

    // Modify utils.rs: change greet signature
    fs::write(
        dir.path().join("src/utils.rs"),
        r#"
use std::path::PathBuf;

/// Greet the user with enthusiasm.
pub fn greet(name: &str, excited: bool) -> &'static str {
    if excited { "HI!" } else { "hi" }
}

pub const VERSION: &str = "2.0.0";
"#,
    )
    .unwrap();

    // Re-scan and re-index only changed files
    let config = ScanConfig::default();
    let scanned = scan_directory(dir.path(), &config).unwrap();
    let diff = store.get_stale_files(&scanned).unwrap();

    assert!(
        diff.to_update
            .iter()
            .any(|f| f.path.to_string_lossy().contains("utils.rs")),
        "utils.rs should be in to_update"
    );

    store.apply_diff(&diff).unwrap();

    // Re-parse only updated files
    let registry = ParserRegistry::new();
    for file in &diff.to_update {
        let path_str = file.path.to_string_lossy().to_string();
        let file_id = store.get_file_id(&path_str).unwrap().unwrap();
        let lang = file.language.as_deref().unwrap_or("unknown");
        let content = fs::read(dir.path().join(&file.path)).unwrap();
        if let Some(Ok(parsed)) = registry.parse(lang, &content) {
            let mut symbols = parsed.symbols;
            for sym in &mut symbols {
                sym.file_id = file_id;
            }
            store.upsert_symbols(file_id, &symbols).unwrap();

            let mut imports = parsed.imports;
            for imp in &mut imports {
                imp.file_id = file_id;
            }
            store.upsert_imports(file_id, &imports).unwrap();
        }
    }

    // Verify updated greet signature
    let greet_v2 = store
        .query_symbols(&SymbolFilter {
            name: Some("greet".to_string()),
            kind: Some(SymbolKind::Function),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(greet_v2.len(), 1);
    let sig_v2 = greet_v2[0].signature.as_deref().unwrap_or("");
    assert!(
        sig_v2.contains("excited"),
        "v2 signature should contain new param 'excited', got: {sig_v2:?}"
    );

    // Verify doc comment updated
    let doc_v2 = greet_v2[0].doc_comment.as_deref().unwrap_or("");
    assert!(
        doc_v2.contains("enthusiasm"),
        "v2 doc should contain 'enthusiasm', got: {doc_v2:?}"
    );

    // test_greet should be gone (removed from file)
    let tests = store
        .query_symbols(&SymbolFilter {
            name: Some("test_greet".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert!(
        tests.is_empty(),
        "test_greet should be removed after re-index"
    );

    // Symbols from OTHER files should be untouched
    let config_struct = store
        .query_symbols(&SymbolFilter {
            name: Some("Config".to_string()),
            kind: Some(SymbolKind::Struct),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        config_struct.len(),
        1,
        "Config struct from config.rs should still exist"
    );
}

#[test]
fn test_stats_after_indexing() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let stats = store.get_stats().unwrap();
    assert!(stats.files_indexed >= 3, "should have at least 3 files");
    assert!(stats.total_symbols >= 10, "should have at least 10 symbols");
    assert!(stats.total_bytes > 0, "total bytes should be positive");
    assert!(
        stats.languages.iter().any(|(l, _)| l == "rust"),
        "should include rust language"
    );
}

#[test]
fn test_limit_filter() {
    let dir = create_rust_project();
    let (store, _) = index_project(&dir);

    let limited = store
        .query_symbols(&SymbolFilter {
            limit: Some(3),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(limited.len(), 3, "limit filter should cap results at 3");
}
