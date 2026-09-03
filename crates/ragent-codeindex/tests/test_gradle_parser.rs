#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-codeindex/src/parser/gradle.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::gradle::GradleParser;
use ragent_codeindex::parser::{LanguageParser, ParsedFile};
use ragent_codeindex::types::SymbolKind;

fn parse_gradle(source: &str) -> ParsedFile {
    let parser = GradleParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_gradle_plugins_block() {
    let src = r#"
plugins {
    id("java")
    id("org.springframework.boot") version "3.0.0"
}
"#;
    let pf = parse_gradle(src);
    // plugins block should appear as a juxt_function_call reference
    assert!(!pf.references.is_empty());
}

#[test]
fn test_gradle_dependencies() {
    let src = r#"
dependencies {
    implementation("com.example:lib:1.0")
}
"#;
    let pf = parse_gradle(src);
    assert!(!pf.references.is_empty());
}

#[test]
fn test_class_declaration() {
    let src = r#"
class MyPlugin implements Plugin<Project> {
    void apply(Project project) {
        project.tasks.create("myTask") {}
    }
}
"#;
    let pf = parse_gradle(src);
    let classes: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyPlugin");
}

#[test]
fn test_import() {
    let src = "import org.gradle.api.Plugin;";
    let pf = parse_gradle(src);
    assert!(!pf.imports.is_empty());
    assert_eq!(pf.imports[0].imported_name, "Plugin");
}

#[test]
fn test_empty_source() {
    let pf = parse_gradle("");
    assert!(pf.symbols.is_empty());
    assert!(pf.imports.is_empty());
}
