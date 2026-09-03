#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-codeindex/src/parser/mod.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::ParserRegistry;

#[test]
fn test_registry_has_rust() {
    let reg = ParserRegistry::new();
    assert!(reg.get("rust").is_some());
    assert!(reg.get("python").is_some());
    assert!(reg.get("openscad").is_some());
    assert!(reg.get("terraform").is_some());
    assert!(reg.get("nonexistent_lang").is_none());
}

#[test]
fn test_supported_languages() {
    let reg = ParserRegistry::new();
    let langs = reg.supported_languages();
    assert!(langs.contains(&"rust"));
    assert!(langs.contains(&"openscad"));
    assert!(langs.contains(&"terraform"));
    assert!(langs.contains(&"cmake"));
    assert!(langs.contains(&"gradle"));
    assert!(langs.contains(&"gradle_kts"));
    assert!(langs.contains(&"maven"));
}

#[test]
fn test_parse_dispatch() {
    let reg = ParserRegistry::new();
    let result = reg.parse("rust", b"fn main() {}");
    assert!(result.is_some());
    let parsed = result.unwrap().unwrap();
    assert!(!parsed.symbols.is_empty());
}

#[test]
fn test_parse_openscad_dispatch() {
    let reg = ParserRegistry::new();
    let result = reg.parse("openscad", b"module foo() { cube(10); }");
    assert!(result.is_some());
    let parsed = result.unwrap().unwrap();
    assert!(!parsed.symbols.is_empty());
}

#[test]
fn test_parse_terraform_dispatch() {
    let reg = ParserRegistry::new();
    let result = reg.parse(
        "terraform",
        b"resource \"aws_instance\" \"web\" { ami = \"ami-123\" }",
    );
    assert!(result.is_some());
    let parsed = result.unwrap().unwrap();
    assert!(!parsed.symbols.is_empty());
}

#[test]
fn test_parse_cmake_dispatch() {
    let reg = ParserRegistry::new();
    let result = reg.parse("cmake", b"function(my_func) endfunction()");
    assert!(result.is_some());
}

#[test]
fn test_parse_gradle_dispatch() {
    let reg = ParserRegistry::new();
    let result = reg.parse("gradle", b"plugins { id 'java' }");
    assert!(result.is_some());
}

#[test]
fn test_parse_gradle_kts_dispatch() {
    let reg = ParserRegistry::new();
    let result = reg.parse("gradle_kts", b"plugins { java }");
    assert!(result.is_some());
}

#[test]
fn test_parse_maven_dispatch() {
    let reg = ParserRegistry::new();
    let result = reg.parse(
        "maven",
        b"<?xml version=\"1.0\"?><project><artifactId>test</artifactId></project>",
    );
    assert!(result.is_some());
}

#[test]
fn test_parse_unknown_language() {
    let reg = ParserRegistry::new();
    assert!(reg.parse("brainfuck", b"+++").is_none());
}
