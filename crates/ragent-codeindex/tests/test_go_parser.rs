//! External tests for `tests` from `crates/ragent-codeindex/src/parser/go.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::go::GoParser;
use ragent_codeindex::types::{SymbolKind, Visibility};

fn parse_go(source: &str) -> ParsedFile {
    let p = GoParser::new();
    p.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_function() {
    let source = "package main\n\nfunc Hello() string {\n\treturn \"hi\"\n}\n";
    let parsed = parse_go(source);
    let hello = parsed.symbols.iter().find(|s| s.name == "Hello").unwrap();
    assert_eq!(hello.kind, SymbolKind::Function);
    assert_eq!(hello.visibility, Visibility::Public);
}

#[test]
fn test_unexported_function() {
    let source = "package main\n\nfunc helper() {}\n";
    let parsed = parse_go(source);
    let h = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
    assert_eq!(h.visibility, Visibility::Private);
}

#[test]
fn test_struct_with_fields() {
    let source = r"
package main

type Config struct {
    Name string
    value int
}
";
    let parsed = parse_go(source);
    let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Config"), "got: {names:?}");
    assert!(names.contains(&"Name"), "got: {names:?}");
    assert!(names.contains(&"value"), "got: {names:?}");

    let config = parsed.symbols.iter().find(|s| s.name == "Config").unwrap();
    assert_eq!(config.kind, SymbolKind::Struct);
    assert_eq!(config.visibility, Visibility::Public);

    let name_f = parsed.symbols.iter().find(|s| s.name == "Name").unwrap();
    assert_eq!(name_f.kind, SymbolKind::Field);
    assert_eq!(name_f.visibility, Visibility::Public);

    let value_f = parsed.symbols.iter().find(|s| s.name == "value").unwrap();
    assert_eq!(value_f.visibility, Visibility::Private);
}

#[test]
fn test_interface() {
    let source = r"
package main

type Reader interface {
    Read(p []byte) (n int, err error)
}
";
    let parsed = parse_go(source);
    let reader = parsed.symbols.iter().find(|s| s.name == "Reader").unwrap();
    assert_eq!(reader.kind, SymbolKind::Interface);
    assert_eq!(reader.visibility, Visibility::Public);
}

#[test]
fn test_method_with_receiver() {
    let source = r#"
package main

type Dog struct {}

func (d *Dog) Bark() string {
    return "Woof"
}
"#;
    let parsed = parse_go(source);
    let bark = parsed.symbols.iter().find(|s| s.name == "Bark").unwrap();
    assert_eq!(bark.kind, SymbolKind::Method);
    assert!(bark.signature.as_ref().unwrap().contains("(d *Dog)"));
}

#[test]
fn test_constants() {
    let source = r"
package main

const MaxSize = 1024
const (
    A = iota
    B
)
";
    let parsed = parse_go(source);
    let max = parsed.symbols.iter().find(|s| s.name == "MaxSize").unwrap();
    assert_eq!(max.kind, SymbolKind::Constant);
    assert_eq!(max.visibility, Visibility::Public);
}

#[test]
fn test_imports() {
    let source = r#"
package main

import (
    "fmt"
    "os"
    log "github.com/sirupsen/logrus"
)
"#;
    let parsed = parse_go(source);
    assert!(
        parsed.imports.len() >= 2,
        "got {} imports",
        parsed.imports.len()
    );

    let fmt_imp = parsed.imports.iter().find(|i| i.imported_name == "fmt");
    assert!(fmt_imp.is_some());
}

#[test]
fn test_type_alias() {
    let source = "package main\n\ntype MyString string\n";
    let parsed = parse_go(source);
    let ms = parsed
        .symbols
        .iter()
        .find(|s| s.name == "MyString")
        .unwrap();
    assert_eq!(ms.kind, SymbolKind::TypeAlias);
}

#[test]
fn test_empty_source() {
    let parsed = parse_go("package main\n");
    assert!(parsed.symbols.is_empty());
}
