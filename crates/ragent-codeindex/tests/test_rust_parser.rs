#![allow(clippy::assert_is_empty)]
//! External integration tests for the Rust language parser.

use ragent_codeindex::parser::rust::RustParser;
use ragent_codeindex::parser::{LanguageParser, ParsedFile};
use ragent_codeindex::types::{SymbolKind, Visibility};

fn parse_rust(source: &str) -> ParsedFile {
    let parser = RustParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_simple_function() {
    let parsed = parse_rust("fn hello() { println!(\"hi\"); }");
    assert_eq!(parsed.symbols.len(), 1);
    assert_eq!(parsed.symbols[0].name, "hello");
    assert_eq!(parsed.symbols[0].kind, SymbolKind::Function);
    assert_eq!(parsed.symbols[0].visibility, Visibility::Private);
}

#[test]
fn test_pub_function_with_return() {
    let parsed = parse_rust("pub fn add(a: i32, b: i32) -> i32 { a + b }");
    assert_eq!(parsed.symbols.len(), 1);
    let sym = &parsed.symbols[0];
    assert_eq!(sym.name, "add");
    assert_eq!(sym.visibility, Visibility::Public);
    assert!(sym.signature.as_ref().unwrap().contains("-> i32"));
    assert!(sym.signature.as_ref().unwrap().contains("pub fn add"));
}

#[test]
fn test_struct_with_fields() {
    let source = r"
pub struct Config {
    pub name: String,
    value: i32,
}
";
    let parsed = parse_rust(source);
    let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Config"), "got: {names:?}");
    assert!(names.contains(&"name"), "got: {names:?}");
    assert!(names.contains(&"value"), "got: {names:?}");

    let config = parsed.symbols.iter().find(|s| s.name == "Config").unwrap();
    assert_eq!(config.kind, SymbolKind::Struct);
    assert_eq!(config.visibility, Visibility::Public);

    let name_field = parsed.symbols.iter().find(|s| s.name == "name").unwrap();
    assert_eq!(name_field.kind, SymbolKind::Field);
    assert_eq!(name_field.visibility, Visibility::Public);
    assert!(name_field.parent_id.is_some());
}

#[test]
fn test_enum_with_variants() {
    let source = r"
pub enum Color {
    Red,
    Green,
    Blue,
}
";
    let parsed = parse_rust(source);
    let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Color"));
    assert!(names.contains(&"Red"));
    assert!(names.contains(&"Green"));
    assert!(names.contains(&"Blue"));

    let red = parsed.symbols.iter().find(|s| s.name == "Red").unwrap();
    assert_eq!(red.kind, SymbolKind::EnumVariant);
}

#[test]
fn test_trait_with_methods() {
    let source = r"
pub trait Greet {
    fn greet(&self) -> String;
}
";
    let parsed = parse_rust(source);
    let greet_trait = parsed.symbols.iter().find(|s| s.name == "Greet").unwrap();
    assert_eq!(greet_trait.kind, SymbolKind::Trait);

    let greet_fn = parsed.symbols.iter().find(|s| s.name == "greet").unwrap();
    assert_eq!(greet_fn.kind, SymbolKind::Method);
    assert_eq!(greet_fn.parent_id, Some(greet_trait.id));
}

#[test]
fn test_impl_block_with_methods() {
    let source = r"
struct Foo;

impl Foo {
    pub fn new() -> Self { Foo }
    fn helper(&self) {}
}
";
    let parsed = parse_rust(source);
    let impl_sym = parsed
        .symbols
        .iter()
        .find(|s| s.kind == SymbolKind::Impl)
        .unwrap();
    assert!(impl_sym.name.contains("Foo"));

    let new_fn = parsed.symbols.iter().find(|s| s.name == "new").unwrap();
    assert_eq!(new_fn.kind, SymbolKind::Method);
    assert_eq!(new_fn.parent_id, Some(impl_sym.id));
    assert_eq!(new_fn.visibility, Visibility::Public);

    let helper = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
    assert_eq!(helper.kind, SymbolKind::Method);
    assert_eq!(helper.visibility, Visibility::Private);
}

#[test]
fn test_trait_impl() {
    let source = r#"
struct Foo;

impl Display for Foo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Foo")
    }
}
"#;
    let parsed = parse_rust(source);
    let impl_sym = parsed
        .symbols
        .iter()
        .find(|s| s.kind == SymbolKind::Impl)
        .unwrap();
    assert!(
        impl_sym.name.contains("Display") && impl_sym.name.contains("Foo"),
        "impl name should contain trait and type: {}",
        impl_sym.name
    );
}

#[test]
fn test_const_and_static() {
    let source = r"
pub const MAX_SIZE: usize = 1024;
static COUNTER: i32 = 0;
";
    let parsed = parse_rust(source);
    let max_size = parsed
        .symbols
        .iter()
        .find(|s| s.name == "MAX_SIZE")
        .unwrap();
    assert_eq!(max_size.kind, SymbolKind::Constant);
    assert_eq!(max_size.visibility, Visibility::Public);

    let counter = parsed.symbols.iter().find(|s| s.name == "COUNTER").unwrap();
    assert_eq!(counter.kind, SymbolKind::Static);
}

#[test]
fn test_module() {
    let source = r"
pub mod utils {
    pub fn helper() {}
}
";
    let parsed = parse_rust(source);
    let module = parsed.symbols.iter().find(|s| s.name == "utils").unwrap();
    assert_eq!(module.kind, SymbolKind::Module);

    let helper = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
    assert_eq!(helper.kind, SymbolKind::Function);
    assert_eq!(helper.parent_id, Some(module.id));
    assert_eq!(helper.qualified_name.as_deref(), Some("utils::helper"));
}

#[test]
fn test_type_alias() {
    let source = "pub type Result<T> = std::result::Result<T, Error>;";
    let parsed = parse_rust(source);
    assert_eq!(parsed.symbols.len(), 1);
    assert_eq!(parsed.symbols[0].name, "Result");
    assert_eq!(parsed.symbols[0].kind, SymbolKind::TypeAlias);
}

#[test]
fn test_macro_definition() {
    let source = r"
macro_rules! my_macro {
    () => {};
}
";
    let parsed = parse_rust(source);
    let mac = parsed
        .symbols
        .iter()
        .find(|s| s.name == "my_macro")
        .unwrap();
    assert_eq!(mac.kind, SymbolKind::Macro);
}

#[test]
fn test_use_statements() {
    let source = r"
use std::collections::HashMap;
use std::io::{self, Write};
use crate::config::Config as AppConfig;
";
    let parsed = parse_rust(source);
    assert!(
        parsed.imports.len() >= 2,
        "got {} imports",
        parsed.imports.len()
    );

    let hashmap = parsed
        .imports
        .iter()
        .find(|i| i.imported_name.contains("HashMap"))
        .unwrap();
    assert_eq!(hashmap.source_module, "std::collections");
}

#[test]
fn test_test_function() {
    let source = r"
#[test]
fn test_something() {
    assert!(true);
}

#[tokio::test]
async fn test_async() {
    assert!(true);
}
";
    let parsed = parse_rust(source);
    let test1 = parsed
        .symbols
        .iter()
        .find(|s| s.name == "test_something")
        .unwrap();
    assert_eq!(test1.kind, SymbolKind::Test);

    let test2 = parsed
        .symbols
        .iter()
        .find(|s| s.name == "test_async")
        .unwrap();
    assert_eq!(test2.kind, SymbolKind::Test);
}

#[test]
fn test_doc_comments() {
    let source = r"
/// This is a documented function.
/// It does cool things.
pub fn documented() {}
";
    let parsed = parse_rust(source);
    let sym = parsed
        .symbols
        .iter()
        .find(|s| s.name == "documented")
        .unwrap();
    let doc = sym.doc_comment.as_ref().unwrap();
    assert!(doc.contains("documented function"), "doc: {doc}");
    assert!(doc.contains("cool things"), "doc: {doc}");
}

#[test]
fn test_line_numbers() {
    let source = "fn first() {}\nfn second() {}\nfn third() {}\n";
    let parsed = parse_rust(source);

    let first = parsed.symbols.iter().find(|s| s.name == "first").unwrap();
    assert_eq!(first.start_line, 1);

    let second = parsed.symbols.iter().find(|s| s.name == "second").unwrap();
    assert_eq!(second.start_line, 2);

    let third = parsed.symbols.iter().find(|s| s.name == "third").unwrap();
    assert_eq!(third.start_line, 3);
}

#[test]
fn test_qualified_names_nested() {
    let source = r"
mod outer {
    mod inner {
        fn deep() {}
    }
}
";
    let parsed = parse_rust(source);
    let deep = parsed.symbols.iter().find(|s| s.name == "deep").unwrap();
    assert_eq!(deep.qualified_name.as_deref(), Some("outer::inner::deep"));
}

#[test]
fn test_body_hash_changes() {
    let source1 = "fn foo() { 1 + 1 }";
    let source2 = "fn foo() { 2 + 2 }";
    let p1 = parse_rust(source1);
    let p2 = parse_rust(source2);

    assert_ne!(
        p1.symbols[0].body_hash, p2.symbols[0].body_hash,
        "body hash should change when body changes"
    );
}

#[test]
fn test_empty_source() {
    let parsed = parse_rust("");
    assert!(parsed.symbols.is_empty());
    assert!(parsed.imports.is_empty());
}

#[test]
fn test_complex_real_world() {
    let source = r"
//! Module documentation.

use std::collections::HashMap;
use std::sync::Arc;

pub struct ConfigManager {
    store: HashMap<String, String>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self { store: HashMap::new() }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.store.get(key)
    }

    fn internal_helper(&self) {}
}

pub enum Status {
    Active,
    Inactive,
}

pub trait Loadable {
    fn load(&self) -> Result<(), String>;
}

impl Loadable for ConfigManager {
    fn load(&self) -> Result<(), String> {
        Ok(())
    }
}

pub const MAX_ITEMS: usize = 100;

pub type Config = ConfigManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let cm = ConfigManager::new();
        assert!(cm.store.is_empty());
    }
}
";
    let parsed = parse_rust(source);

    // Count by kind.
    let count = |k: SymbolKind| parsed.symbols.iter().filter(|s| s.kind == k).count();

    assert!(
        count(SymbolKind::Struct) >= 1,
        "should find ConfigManager struct"
    );
    assert!(count(SymbolKind::Enum) >= 1, "should find Status enum");
    assert!(count(SymbolKind::Trait) >= 1, "should find Loadable trait");
    assert!(count(SymbolKind::Impl) >= 2, "should find 2 impl blocks");
    assert!(count(SymbolKind::Constant) >= 1, "should find MAX_ITEMS");
    assert!(
        count(SymbolKind::TypeAlias) >= 1,
        "should find Config type alias"
    );
    assert!(count(SymbolKind::Test) >= 1, "should find test_new");
    assert!(count(SymbolKind::Field) >= 1, "should find struct fields");
    assert!(count(SymbolKind::Module) >= 1, "should find tests module");

    // Verify imports.
    assert!(parsed.imports.len() >= 2, "should have at least 2 imports");
}
