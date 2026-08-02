//! External tests for `tests` from `crates/ragent-codeindex/src/parser/python.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::python::PythonParser;
use ragent_codeindex::parser::{LanguageParser, ParsedFile};
use ragent_codeindex::types::{SymbolKind, Visibility};

fn parse_py(source: &str) -> ParsedFile {
    let parser = PythonParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_simple_function() {
    let parsed = parse_py("def hello():\n    pass\n");
    assert_eq!(parsed.symbols.len(), 1);
    assert_eq!(parsed.symbols[0].name, "hello");
    assert_eq!(parsed.symbols[0].kind, SymbolKind::Function);
}

#[test]
fn test_class_with_methods() {
    let source = r#"
class Dog:
    """A dog class."""

    def __init__(self, name: str):
        self.name = name

    def bark(self) -> str:
        return "Woof!"

    @staticmethod
    def species() -> str:
        return "Canis lupus"
"#;
    let parsed = parse_py(source);
    let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Dog"), "got: {names:?}");
    assert!(names.contains(&"__init__"), "got: {names:?}");
    assert!(names.contains(&"bark"), "got: {names:?}");
    assert!(names.contains(&"species"), "got: {names:?}");

    let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
    assert_eq!(dog.kind, SymbolKind::Class);
    assert!(dog.doc_comment.as_ref().unwrap().contains("dog class"));

    let bark = parsed.symbols.iter().find(|s| s.name == "bark").unwrap();
    assert_eq!(bark.kind, SymbolKind::Method);
    assert_eq!(bark.parent_id, Some(dog.id));
}

#[test]
fn test_visibility_conventions() {
    let source = r"
def public_fn():
    pass

def _private_fn():
    pass

def __mangled_fn():
    pass

def __dunder__():
    pass
";
    let parsed = parse_py(source);

    let public = parsed
        .symbols
        .iter()
        .find(|s| s.name == "public_fn")
        .unwrap();
    assert_eq!(public.visibility, Visibility::Public);

    let private = parsed
        .symbols
        .iter()
        .find(|s| s.name == "_private_fn")
        .unwrap();
    assert_eq!(private.visibility, Visibility::PubCrate);

    let mangled = parsed
        .symbols
        .iter()
        .find(|s| s.name == "__mangled_fn")
        .unwrap();
    assert_eq!(mangled.visibility, Visibility::Private);

    let dunder = parsed
        .symbols
        .iter()
        .find(|s| s.name == "__dunder__")
        .unwrap();
    assert_eq!(dunder.visibility, Visibility::Public);
}

#[test]
fn test_imports() {
    let source = r"
import os
import sys
from pathlib import Path
from typing import List, Optional
from collections import OrderedDict as OD
";
    let parsed = parse_py(source);
    assert!(
        parsed.imports.len() >= 4,
        "got {} imports",
        parsed.imports.len()
    );

    let os_imp = parsed
        .imports
        .iter()
        .find(|i| i.imported_name == "os")
        .unwrap();
    assert_eq!(os_imp.kind, "import");

    let path_imp = parsed
        .imports
        .iter()
        .find(|i| i.imported_name == "Path")
        .unwrap();
    assert_eq!(path_imp.source_module, "pathlib");
    assert_eq!(path_imp.kind, "from_import");

    let od = parsed
        .imports
        .iter()
        .find(|i| i.imported_name == "OrderedDict")
        .unwrap();
    assert_eq!(od.alias.as_deref(), Some("OD"));
}

#[test]
fn test_module_constants() {
    let source = r#"
MAX_SIZE = 1024
DEFAULT_NAME = "hello"
_not_constant = True
regular_var = 42
"#;
    let parsed = parse_py(source);
    let const_names: Vec<&str> = parsed
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Constant)
        .map(|s| s.name.as_str())
        .collect();
    assert!(const_names.contains(&"MAX_SIZE"), "got: {const_names:?}");
    assert!(
        const_names.contains(&"DEFAULT_NAME"),
        "got: {const_names:?}"
    );
    // _not_constant and regular_var should NOT be extracted as constants
    assert!(!const_names.contains(&"_not_constant"));
    assert!(!const_names.contains(&"regular_var"));
}

#[test]
fn test_async_function() {
    let source = r"
async def fetch_data(url: str) -> bytes:
    pass
";
    let parsed = parse_py(source);
    let func = &parsed.symbols[0];
    assert_eq!(func.name, "fetch_data");
    assert!(func.signature.as_ref().unwrap().contains("async def"));
}

#[test]
fn test_class_inheritance() {
    let source = r"
class Animal:
    pass

class Dog(Animal):
    pass
";
    let parsed = parse_py(source);
    let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
    assert!(dog.signature.as_ref().unwrap().contains("(Animal)"));
}

#[test]
fn test_qualified_names() {
    let source = r"
class Outer:
    class Inner:
        def method(self):
            pass
";
    let parsed = parse_py(source);
    let method = parsed.symbols.iter().find(|s| s.name == "method").unwrap();
    assert_eq!(method.qualified_name.as_deref(), Some("Outer.Inner.method"));
}

#[test]
fn test_empty_source() {
    let parsed = parse_py("");
    assert!(parsed.symbols.is_empty());
    assert!(parsed.imports.is_empty());
}

#[test]
fn test_decorator_property() {
    let source = r"
class Config:
    @property
    def name(self) -> str:
        return self._name
";
    let parsed = parse_py(source);
    let name_fn = parsed.symbols.iter().find(|s| s.name == "name").unwrap();
    assert!(name_fn.signature.as_ref().unwrap().contains("@property"));
}
