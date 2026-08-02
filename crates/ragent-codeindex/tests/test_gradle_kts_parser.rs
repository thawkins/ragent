//! External tests for `tests` from `crates/ragent-codeindex/src/parser/gradle_kts.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::gradle_kts::GradleKtsParser;
use ragent_codeindex::types::SymbolKind;

fn parse_kts(source: &str) -> ParsedFile {
    let parser = GradleKtsParser::new();
    parser.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_plugins_block() {
    let src = r#"
plugins {
    java
    id("org.springframework.boot") version "3.0.0"
}
"#;
    let pf = parse_kts(src);
    assert!(!pf.references.is_empty());
}

#[test]
fn test_dependencies_block() {
    let src = r#"
dependencies {
    implementation("com.example:lib:1.0")
}
"#;
    let pf = parse_kts(src);
    assert!(!pf.references.is_empty());
}

#[test]
fn test_class_declaration() {
    let src = r#"
open class MyPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        project.tasks.register("myTask")
    }
}
"#;
    let pf = parse_kts(src);
    assert!(pf.symbols.iter().any(|s| s.kind == SymbolKind::Class));
}

#[test]
fn test_function_declaration() {
    let src = r#"
fun customTask(project: Project) {
    println("Hello")
}
"#;
    let pf = parse_kts(src);
    assert!(pf.symbols.iter().any(|s| s.kind == SymbolKind::Function));
}

#[test]
fn test_import() {
    let src = "import org.gradle.api.Plugin";
    let pf = parse_kts(src);
    assert!(!pf.imports.is_empty());
    assert_eq!(pf.imports[0].imported_name, "Plugin");
}

#[test]
fn test_property_declaration() {
    let src = r#"val version = "1.0""#;
    let pf = parse_kts(src);
    assert!(pf.symbols.iter().any(|s| s.kind == SymbolKind::Constant));
}

#[test]
fn test_empty_source() {
    let pf = parse_kts("");
    assert!(pf.symbols.is_empty());
    assert!(pf.imports.is_empty());
}
