//! Extra coverage tests for the Gradle (Groovy DSL) parser
//! (`crates/ragent-codeindex/src/parser/gradle.rs`).
//!
//! The relocated inline tests only exercise classes, plugins/dependencies
//! blocks, one import, and the empty source.  These tests drive the remaining
//! extraction branches: interfaces, enums, constructors, standalone `def`
//! functions, variable/field/constant declarations, annotations, visibility
//! modifiers, and the class signature builder.

use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::gradle::GradleParser;
use ragent_codeindex::types::{SymbolKind, Visibility};

#[path = "support/mod.rs"]
mod support;

fn parse_gradle(source: &str) -> ParsedFile {
    support::parse_with(&GradleParser::new(), source)
}

#[test]
fn test_gradle_interface_declaration() {
    let src = r"
interface Greeter {
    void greet()
}
";
    let pf = parse_gradle(src);
    let ifaces: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Interface)
        .collect();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0].name, "Greeter");
    assert_eq!(ifaces[0].signature.as_deref(), Some("interface Greeter"));
    // The method inside the interface body is walked and extracted.
    let methods: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].qualified_name.as_deref(), Some("Greeter.greet"));
}

#[test]
fn test_gradle_enum_declaration() {
    let src = r"
enum Color {
    RED,
    GREEN
}
";
    let pf = parse_gradle(src);
    let enums: Vec<_> = pf
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Color");
    assert_eq!(enums[0].signature.as_deref(), Some("enum Color"));
}

#[test]
fn test_gradle_constructor_and_visibility() {
    let src = r"
class Foo {
    private int x = 1
    Foo(int a) {}
}
";
    let pf = parse_gradle(src);
    let ctor = pf
        .symbols
        .iter()
        .find(|s| s.name == "Foo" && s.kind == SymbolKind::Method)
        .expect("constructor should be extracted as a method");
    assert_eq!(ctor.signature.as_deref(), Some("Foo(int a)"));
    assert_eq!(ctor.qualified_name.as_deref(), Some("Foo.Foo"));

    let field = pf
        .symbols
        .iter()
        .find(|s| s.name == "x")
        .expect("field should be extracted");
    assert_eq!(field.kind, SymbolKind::Field);
    assert_eq!(field.visibility, Visibility::Private);
}

#[test]
fn test_gradle_standalone_def_function() {
    let src = r"
def buildVersion(String name) {
    println(name)
}
";
    let pf = parse_gradle(src);
    let f = pf
        .symbols
        .iter()
        .find(|s| s.name == "buildVersion")
        .expect("def function should be extracted");
    assert_eq!(f.kind, SymbolKind::Function);
    assert_eq!(
        f.signature.as_deref(),
        Some("def buildVersion(String name)")
    );
}

#[test]
fn test_gradle_top_level_variable_is_constant() {
    let src = "def version = \"1.0\"";
    let pf = parse_gradle(src);
    let v = pf
        .symbols
        .iter()
        .find(|s| s.name == "version")
        .expect("variable should be extracted");
    assert_eq!(v.kind, SymbolKind::Constant);
}

#[test]
fn test_gradle_annotation_reference() {
    let src = r"
@Override
class Annotated {}
";
    let pf = parse_gradle(src);
    let ann = pf
        .references
        .iter()
        .find(|r| r.symbol_name == "Override")
        .expect("annotation should become a type reference");
    assert_eq!(ann.kind, "type");
}

#[test]
fn test_gradle_class_signature_extends_implements() {
    let src = "class Foo extends Bar implements Baz {}";
    let pf = parse_gradle(src);
    let c = pf.symbols.iter().find(|s| s.name == "Foo").unwrap();
    let sig = c.signature.as_deref().unwrap();
    assert!(sig.contains("class Foo"), "sig: {sig}");
    assert!(sig.contains("extends"), "sig: {sig}");
    assert!(sig.contains("Bar"), "sig: {sig}");
    assert!(sig.contains("implements"), "sig: {sig}");
    assert!(sig.contains("Baz"), "sig: {sig}");
}

#[test]
fn test_gradle_public_modifier_visibility() {
    let src = "public class Pub {}";
    let pf = parse_gradle(src);
    let c = pf.symbols.iter().find(|s| s.name == "Pub").unwrap();
    assert_eq!(c.visibility, Visibility::Public);
}

#[test]
fn test_gradle_import_static_kind() {
    let src = "import static java.lang.Math.PI";
    let pf = parse_gradle(src);
    let imp = pf
        .imports
        .iter()
        .find(|i| i.imported_name == "PI")
        .expect("static import should be extracted");
    assert_eq!(imp.kind, "import_static");
    assert_eq!(imp.source_module, "java.lang.Math.PI");
}

#[test]
fn test_gradle_method_invocation_reference() {
    let src = r#"println("hello")"#;
    let pf = parse_gradle(src);
    assert!(
        pf.references.iter().any(|r| r.symbol_name == "println"),
        "method invocation should become a call reference"
    );
}
