//! Extra coverage tests for the Java parser
//! (`crates/ragent-codeindex/src/parser/java.rs`).
//!
//! The relocated inline tests cover classes, interfaces, enums, visibility,
//! imports, javadoc, inner classes and constructors.  These tests drive the
//! remaining branches: `@interface` annotation types, interface/class-level
//! constants, plain fields, and the method-signature builder for a
//! constructor versus a typed method.

use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::java::JavaParser;
use ragent_codeindex::types::{SymbolKind, Visibility};

#[path = "support/mod.rs"]
mod support;

fn parse_java(source: &str) -> ParsedFile {
    support::parse_with(&JavaParser::new(), source)
}

#[test]
fn test_annotation_type_declaration() {
    let source = r"
public @interface MyAnno {
    String value();
}
";
    let parsed = parse_java(source);
    let ann = parsed
        .symbols
        .iter()
        .find(|s| s.name == "MyAnno")
        .expect("@interface should be extracted");
    assert_eq!(ann.kind, SymbolKind::Interface);
    assert_eq!(ann.visibility, Visibility::Public);
    assert_eq!(ann.signature.as_deref(), Some("@interface MyAnno"));
}

#[test]
fn test_private_field_and_public_field() {
    let source = r"
public class Holder {
    private int hidden;
    public String shown;
}
";
    let parsed = parse_java(source);
    let hidden = parsed
        .symbols
        .iter()
        .find(|s| s.name == "hidden")
        .expect("private field should be extracted");
    assert_eq!(hidden.kind, SymbolKind::Field);
    assert_eq!(hidden.visibility, Visibility::Private);
    assert_eq!(hidden.signature.as_deref(), Some("int hidden"));

    let shown = parsed
        .symbols
        .iter()
        .find(|s| s.name == "shown")
        .expect("public field should be extracted");
    assert_eq!(shown.visibility, Visibility::Public);
    assert_eq!(shown.signature.as_deref(), Some("String shown"));
}

#[test]
fn test_interface_constant_extraction() {
    let source = r#"
public interface Constants {
    int MAX = 100;
    String NAME = "ragent";
}
"#;
    let parsed = parse_java(source);
    let max = parsed
        .symbols
        .iter()
        .find(|s| s.name == "MAX")
        .expect("interface constant should be extracted");
    assert_eq!(max.kind, SymbolKind::Constant);
    assert_eq!(max.signature.as_deref(), Some("int MAX"));
    assert!(
        parsed.symbols.iter().any(|s| s.name == "NAME"),
        "second constant should be extracted"
    );
}

#[test]
fn test_constructor_signature_has_no_return_type() {
    let source = r"
public class Widget {
    public Widget(int size) {
        this.size = size;
    }
}
";
    let parsed = parse_java(source);
    let ctor = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Widget" && s.kind == SymbolKind::Method)
        .expect("constructor should be extracted as a method");
    // Constructor signature: visibility + name + params, no return type.
    assert_eq!(ctor.signature.as_deref(), Some("public Widget(int size)"));
}

#[test]
fn test_typed_method_signature_includes_return_type() {
    let source = r"
public class Calc {
    public int add(int a, int b) {
        return a + b;
    }
}
";
    let parsed = parse_java(source);
    let add = parsed
        .symbols
        .iter()
        .find(|s| s.name == "add")
        .expect("method should be extracted");
    assert_eq!(
        add.signature.as_deref(),
        Some("public int add(int a, int b)")
    );
}

#[test]
fn test_protected_member_visibility() {
    let source = r"
public class Base {
    protected int value;
}
";
    let parsed = parse_java(source);
    let v = parsed
        .symbols
        .iter()
        .find(|s| s.name == "value")
        .expect("protected field should be extracted");
    assert_eq!(v.visibility, Visibility::PubCrate);
}
