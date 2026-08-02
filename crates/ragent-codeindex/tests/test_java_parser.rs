//! External tests for `tests` from `crates/ragent-codeindex/src/parser/java.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::ParsedFile;
use ragent_codeindex::parser::java::JavaParser;
use ragent_codeindex::types::{SymbolKind, Visibility};

fn parse_java(source: &str) -> ParsedFile {
    let p = JavaParser::new();
    p.parse(source.as_bytes()).unwrap()
}

#[test]
fn test_class_with_method() {
    let source = r#"
public class Dog {
    public void bark() {
        System.out.println("Woof!");
    }
}
"#;
    let parsed = parse_java(source);
    let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
    assert_eq!(dog.kind, SymbolKind::Class);
    assert_eq!(dog.visibility, Visibility::Public);

    let bark = parsed.symbols.iter().find(|s| s.name == "bark").unwrap();
    assert_eq!(bark.kind, SymbolKind::Method);
    assert_eq!(bark.parent_id, Some(dog.id));
}

#[test]
fn test_interface() {
    let source = r"
public interface Greeter {
    String greet(String name);
}
";
    let parsed = parse_java(source);
    let greeter = parsed.symbols.iter().find(|s| s.name == "Greeter").unwrap();
    assert_eq!(greeter.kind, SymbolKind::Interface);
    assert_eq!(greeter.visibility, Visibility::Public);
}

#[test]
fn test_enum_with_constants() {
    let source = r"
public enum Color {
    RED,
    GREEN,
    BLUE
}
";
    let parsed = parse_java(source);
    let color = parsed.symbols.iter().find(|s| s.name == "Color").unwrap();
    assert_eq!(color.kind, SymbolKind::Enum);

    let red = parsed.symbols.iter().find(|s| s.name == "RED").unwrap();
    assert_eq!(red.kind, SymbolKind::EnumVariant);
    assert_eq!(red.parent_id, Some(color.id));
}

#[test]
fn test_visibility() {
    let source = r"
public class Vis {
    public String pub_field;
    private int priv_field;
    protected double prot_field;
    String pkg_field;
}
";
    let parsed = parse_java(source);

    let pub_f = parsed
        .symbols
        .iter()
        .find(|s| s.name == "pub_field")
        .unwrap();
    assert_eq!(pub_f.visibility, Visibility::Public);

    let priv_f = parsed
        .symbols
        .iter()
        .find(|s| s.name == "priv_field")
        .unwrap();
    assert_eq!(priv_f.visibility, Visibility::Private);
}

#[test]
fn test_imports() {
    let source = r"
import java.util.List;
import java.util.Map;
import static java.lang.Math.PI;
";
    let parsed = parse_java(source);
    assert!(
        parsed.imports.len() >= 3,
        "got {} imports",
        parsed.imports.len()
    );

    let list = parsed
        .imports
        .iter()
        .find(|i| i.imported_name == "List")
        .unwrap();
    assert_eq!(list.source_module, "java.util");
    assert_eq!(list.kind, "import");

    let pi = parsed
        .imports
        .iter()
        .find(|i| i.imported_name == "PI")
        .unwrap();
    assert_eq!(pi.kind, "static_import");
}

#[test]
fn test_javadoc() {
    let source = r"
/**
 * A utility class.
 */
public class Utils {
    /**
     * Adds two numbers.
     * @param a first number
     * @param b second number
     */
    public int add(int a, int b) {
        return a + b;
    }
}
";
    let parsed = parse_java(source);
    let utils = parsed.symbols.iter().find(|s| s.name == "Utils").unwrap();
    assert!(
        utils
            .doc_comment
            .as_ref()
            .unwrap()
            .contains("utility class")
    );

    let add = parsed.symbols.iter().find(|s| s.name == "add").unwrap();
    assert!(
        add.doc_comment
            .as_ref()
            .unwrap()
            .contains("Adds two numbers")
    );
}

#[test]
fn test_inner_class() {
    let source = r"
public class Outer {
    public class Inner {
        public void method() {}
    }
}
";
    let parsed = parse_java(source);
    let inner = parsed.symbols.iter().find(|s| s.name == "Inner").unwrap();
    assert_eq!(inner.qualified_name.as_deref(), Some("Outer.Inner"));

    let method = parsed.symbols.iter().find(|s| s.name == "method").unwrap();
    assert_eq!(method.qualified_name.as_deref(), Some("Outer.Inner.method"));
}

#[test]
fn test_constructor() {
    let source = r"
public class Dog {
    public Dog(String name) {
        this.name = name;
    }
}
";
    let parsed = parse_java(source);
    let ctor = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Dog" && s.kind == SymbolKind::Method)
        .unwrap();
    assert_eq!(ctor.kind, SymbolKind::Method);
}

#[test]
fn test_empty_source() {
    let parsed = parse_java("");
    assert!(parsed.symbols.is_empty());
}
