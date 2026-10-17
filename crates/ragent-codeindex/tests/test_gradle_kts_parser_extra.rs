//! Extra coverage tests for the Gradle Kotlin DSL parser
//! (`crates/ragent-codeindex/src/parser/gradle_kts.rs`).
//!
//! The relocated inline tests only cover the plugins/dependencies blocks, a
//! class, a function, an import, and a property.  These tests exercise the
//! remaining extraction branches: object declarations, companion objects,
//! type aliases, properties declared inside classes, function signatures with
//! parameters and a return type, and call-expression references.

use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::gradle_kts::GradleKtsParser;
use ragent_codeindex::types::SymbolKind;

#[path = "support/mod.rs"]
mod support;

fn parse_kts(source: &str) -> ParsedFile {
    support::parse_with(&GradleKtsParser::new(), source)
}

#[test]
fn test_kts_object_declaration() {
    let src = "object Registry { }";
    let pf = parse_kts(src);
    let obj = pf
        .symbols
        .iter()
        .find(|s| s.name == "Registry")
        .expect("object declaration should be extracted");
    assert_eq!(obj.kind, SymbolKind::Class);
    assert_eq!(obj.signature.as_deref(), Some("object Registry"));
}

#[test]
fn test_kts_companion_object() {
    let src = r"
class Foo {
    companion object { }
}
";
    let pf = parse_kts(src);
    let companion = pf
        .symbols
        .iter()
        .find(|s| s.name == "Companion")
        .expect("companion object should be extracted");
    assert_eq!(companion.kind, SymbolKind::Class);
    assert_eq!(
        companion.signature.as_deref(),
        Some("companion object Companion")
    );
    assert_eq!(companion.qualified_name.as_deref(), Some("Foo.Companion"));
}

#[test]
fn test_kts_type_alias() {
    let src = "typealias Handler = () -> Unit";
    let pf = parse_kts(src);
    let alias = pf
        .symbols
        .iter()
        .find(|s| s.kind == SymbolKind::TypeAlias)
        .expect("typealias should be extracted");
    assert_eq!(alias.name, "Handler");
    assert_eq!(alias.signature.as_deref(), Some("typealias Handler"));
}

#[test]
fn test_kts_property_inside_class_is_field() {
    let src = r"
class Holder {
    val x = 1
    var y = 2
}
";
    let pf = parse_kts(src);
    let x = pf.symbols.iter().find(|s| s.name == "x").unwrap();
    assert_eq!(x.kind, SymbolKind::Field);
    assert_eq!(x.qualified_name.as_deref(), Some("Holder.x"));
    let y = pf.symbols.iter().find(|s| s.name == "y").unwrap();
    assert_eq!(y.kind, SymbolKind::Field);
}

#[test]
fn test_kts_function_signature_with_params_and_return() {
    let src = "fun isReady(flag: Boolean, count: Int): Boolean { return flag && count > 0 }";
    let pf = parse_kts(src);
    let f = pf
        .symbols
        .iter()
        .find(|s| s.name == "isReady")
        .expect("function should be extracted");
    let sig = f.signature.as_deref().unwrap();
    assert!(sig.starts_with("fun isReady"), "sig: {sig}");
    assert!(sig.contains("Boolean"), "sig: {sig}");
    assert!(sig.ends_with(": Boolean"), "sig: {sig}");
}

#[test]
fn test_kts_call_expression_navigation_reference() {
    let src = r#"project.tasks.register("myTask")"#;
    let pf = parse_kts(src);
    assert!(
        pf.references.iter().any(|r| r.symbol_name == "register"),
        "call expression should produce a call reference"
    );
}

#[test]
fn test_kts_plain_call_expression_reference() {
    let src = r#"println("hi")"#;
    let pf = parse_kts(src);
    assert!(
        pf.references.iter().any(|r| r.symbol_name == "println"),
        "plain call should produce a call reference"
    );
}

#[test]
fn test_kts_import_qualified_name() {
    let src = "import org.gradle.api.Project";
    let pf = parse_kts(src);
    let imp = pf
        .imports
        .iter()
        .find(|i| i.imported_name == "Project")
        .expect("import should be extracted");
    assert_eq!(imp.source_module, "org.gradle.api.Project");
    assert_eq!(imp.kind, "import");
}
