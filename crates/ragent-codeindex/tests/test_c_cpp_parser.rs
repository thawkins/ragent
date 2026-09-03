#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-codeindex/src/parser/c_cpp.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::c_cpp::{CParser, CppParser};
use ragent_codeindex::parser::{LanguageParser, ParsedFile};
use ragent_codeindex::types::SymbolKind;

fn parse_c(source: &str) -> ParsedFile {
    let p = CParser::new();
    p.parse(source.as_bytes()).unwrap()
}

fn parse_cpp(source: &str) -> ParsedFile {
    let p = CppParser::new();
    p.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_c_function() {
    let source = "int add(int a, int b) {\n    return a + b;\n}\n";
    let parsed = parse_c(source);
    let add = parsed.symbols.iter().find(|s| s.name == "add").unwrap();
    assert_eq!(add.kind, SymbolKind::Function);
}

#[test]
fn test_c_struct() {
    let source = r"
struct Point {
    int x;
    int y;
};
";
    let parsed = parse_c(source);
    let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Point"), "got: {names:?}");
    assert!(names.contains(&"x"), "got: {names:?}");
    assert!(names.contains(&"y"), "got: {names:?}");

    let point = parsed.symbols.iter().find(|s| s.name == "Point").unwrap();
    assert_eq!(point.kind, SymbolKind::Struct);
}

#[test]
fn test_c_enum() {
    let source = r"
enum Color {
    RED,
    GREEN,
    BLUE
};
";
    let parsed = parse_c(source);
    let color = parsed.symbols.iter().find(|s| s.name == "Color").unwrap();
    assert_eq!(color.kind, SymbolKind::Enum);

    let red = parsed.symbols.iter().find(|s| s.name == "RED").unwrap();
    assert_eq!(red.kind, SymbolKind::EnumVariant);
}

#[test]
fn test_c_typedef() {
    let source = "typedef unsigned long size_t;\n";
    let parsed = parse_c(source);
    let st = parsed.symbols.iter().find(|s| s.name == "size_t").unwrap();
    assert_eq!(st.kind, SymbolKind::TypeAlias);
}

#[test]
fn test_c_include() {
    let source = r#"
#include <stdio.h>
#include "myheader.h"
"#;
    let parsed = parse_c(source);
    assert!(
        parsed.imports.len() >= 2,
        "got {} imports",
        parsed.imports.len()
    );
}

#[test]
fn test_cpp_class() {
    let source = r"
class Dog {
public:
    void bark();
};
";
    let parsed = parse_cpp(source);
    let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
    assert_eq!(dog.kind, SymbolKind::Class);
}

#[test]
fn test_cpp_namespace() {
    let source = r"
namespace myns {
    int helper() { return 42; }
}
";
    let parsed = parse_cpp(source);
    let ns = parsed.symbols.iter().find(|s| s.name == "myns").unwrap();
    assert_eq!(ns.kind, SymbolKind::Module);

    let helper = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
    assert_eq!(helper.kind, SymbolKind::Function);
    assert_eq!(helper.qualified_name.as_deref(), Some("myns::helper"));
}

#[test]
fn test_empty_c() {
    let parsed = parse_c("");
    assert!(parsed.symbols.is_empty());
}

#[test]
fn test_empty_cpp() {
    let parsed = parse_cpp("");
    assert!(parsed.symbols.is_empty());
}
