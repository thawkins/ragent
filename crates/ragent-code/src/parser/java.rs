//! Java language parser using tree-sitter.
//!
//! Extracts classes, interfaces, methods, fields, enums, annotations,
//! import statements, and packages from Java source code.
//! Visibility from modifiers: public, protected, private, package-private.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

/// Tree-sitter parser for the Java programming language.
pub struct JavaParser {
    _private: (),
}

impl JavaParser {
    /// Create a new Java parser.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl LanguageParser for JavaParser {
    fn language_id(&self) -> &'static str {
        "java"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let mut parser = Parser::new();
        let lang = tree_sitter_java::LANGUAGE;
        parser
            .set_language(&lang.into())
            .context("failed to load Java grammar")?;
        let tree = parser
            .parse(source, None)
            .context("tree-sitter parse returned None")?;

        let mut ctx = Ctx {
            source,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            next_id: 0,
        };

        walk(&mut ctx, tree.root_node(), None, &[]);

        Ok(ParsedFile {
            symbols: ctx.symbols,
            imports: ctx.imports,
            references: ctx.references,
            tree: Some(tree),
        })
    }
}

struct Ctx<'a> {
    source: &'a [u8],
    symbols: Vec<Symbol>,
    imports: Vec<ImportEntry>,
    references: Vec<SymbolRef>,
    next_id: i64,
}

impl Ctx<'_> {
    fn alloc_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    fn text(&self, n: Node) -> &str {
        n.utf8_text(self.source).unwrap_or("")
    }
}

// ── Walk ────────────────────────────────────────────────────────────────────

fn walk(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    match node.kind() {
        "class_declaration" => extract_class(ctx, node, parent, scope),
        "interface_declaration" => extract_interface(ctx, node, parent, scope),
        "enum_declaration" => extract_enum(ctx, node, parent, scope),
        "method_declaration" | "constructor_declaration" => {
            extract_method(ctx, node, parent, scope);
        }
        "field_declaration" => extract_field(ctx, node, parent, scope),
        "import_declaration" => extract_import(ctx, node),
        "annotation_type_declaration" => extract_annotation_type(ctx, node, parent, scope),
        "constant_declaration" => extract_constant(ctx, node, parent, scope),
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent, scope);
            }
        }
    }
}

// ── Class ───────────────────────────────────────────────────────────────────

fn extract_class(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_java_visibility(ctx, node);
    let doc = extract_javadoc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let superclass = field_text(ctx, node, "superclass");
    let sig = match superclass {
        Some(sc) => format!("class {name} extends {sc}"),
        None => format!("class {name}"),
    };

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Class,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(sig),
        doc_comment: doc,
        body_hash: Some(hash),
    });

    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id), &new_scope);
        }
    }
}

// ── Interface ───────────────────────────────────────────────────────────────

fn extract_interface(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_java_visibility(ctx, node);
    let doc = extract_javadoc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Interface,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(format!("interface {name}")),
        doc_comment: doc,
        body_hash: Some(hash),
    });

    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id), &new_scope);
        }
    }
}

// ── Enum ────────────────────────────────────────────────────────────────────

fn extract_enum(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_java_visibility(ctx, node);
    let doc = extract_javadoc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Enum,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(format!("enum {name}")),
        doc_comment: doc,
        body_hash: Some(hash),
    });

    // Extract enum constants.
    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            if child.kind() == "enum_constant" {
                let cname = field_text(ctx, child, "name").unwrap_or_default();
                if !cname.is_empty() {
                    let cid = ctx.alloc_id();
                    ctx.symbols.push(Symbol {
                        id: cid,
                        file_id: 0,
                        name: cname.clone(),
                        qualified_name: Some(build_qname(&new_scope, &cname)),
                        kind: SymbolKind::EnumVariant,
                        visibility: Visibility::Public,
                        start_line: child.start_position().row as u32 + 1,
                        end_line: child.end_position().row as u32 + 1,
                        start_col: child.start_position().column as u32,
                        end_col: child.end_position().column as u32,
                        parent_id: Some(id),
                        signature: None,
                        doc_comment: None,
                        body_hash: None,
                    });
                }
            }
            // Also recurse for methods inside enum body
            walk(ctx, child, Some(id), &new_scope);
        }
    }
}

// ── Method / Constructor ────────────────────────────────────────────────────

fn extract_method(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_java_visibility(ctx, node);
    let doc = extract_javadoc(ctx, node);
    let sig = extract_method_sig(ctx, node, &name);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind: SymbolKind::Method,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(sig),
        doc_comment: doc,
        body_hash: Some(hash),
    });
}

// ── Field ───────────────────────────────────────────────────────────────────

fn extract_field(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let visibility = extract_java_visibility(ctx, node);
    let type_text = field_text(ctx, node, "type").unwrap_or_default();

    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "variable_declarator" {
            let name = field_text(ctx, child, "name").unwrap_or_default();
            if !name.is_empty() {
                let qname = build_qname(scope, &name);
                let id = ctx.alloc_id();
                ctx.symbols.push(Symbol {
                    id,
                    file_id: 0,
                    name: name.clone(),
                    qualified_name: Some(qname),
                    kind: SymbolKind::Field,
                    visibility,
                    start_line: child.start_position().row as u32 + 1,
                    end_line: child.end_position().row as u32 + 1,
                    start_col: child.start_position().column as u32,
                    end_col: child.end_position().column as u32,
                    parent_id: parent,
                    signature: Some(format!("{type_text} {name}")),
                    doc_comment: None,
                    body_hash: None,
                });
            }
        }
    }
}

// ── Constants (interface-level) ─────────────────────────────────────────────

fn extract_constant(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let type_text = field_text(ctx, node, "type").unwrap_or_default();
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "variable_declarator" {
            let name = field_text(ctx, child, "name").unwrap_or_default();
            if !name.is_empty() {
                let qname = build_qname(scope, &name);
                let id = ctx.alloc_id();
                ctx.symbols.push(Symbol {
                    id,
                    file_id: 0,
                    name: name.clone(),
                    qualified_name: Some(qname),
                    kind: SymbolKind::Constant,
                    visibility: Visibility::Public,
                    start_line: child.start_position().row as u32 + 1,
                    end_line: child.end_position().row as u32 + 1,
                    start_col: child.start_position().column as u32,
                    end_col: child.end_position().column as u32,
                    parent_id: parent,
                    signature: Some(format!("{type_text} {name}")),
                    doc_comment: None,
                    body_hash: None,
                });
            }
        }
    }
}

// ── Annotation type ─────────────────────────────────────────────────────────

fn extract_annotation_type(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_java_visibility(ctx, node);
    let doc = extract_javadoc(ctx, node);
    let qname = build_qname(scope, &name);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Interface,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(format!("@interface {name}")),
        doc_comment: doc,
        body_hash: None,
    });
}

// ── Imports ─────────────────────────────────────────────────────────────────

fn extract_import(ctx: &mut Ctx, node: Node) {
    let line = node.start_position().row as u32 + 1;
    let text = ctx.text(node).trim().to_string();

    let path = text
        .strip_prefix("import ")
        .unwrap_or(&text)
        .strip_prefix("static ")
        .unwrap_or(&text.strip_prefix("import ").unwrap_or(&text))
        .trim_end_matches(';')
        .trim()
        .to_string();

    let (source_module, imported_name) = if let Some(idx) = path.rfind('.') {
        (path[..idx].to_string(), path[idx + 1..].to_string())
    } else {
        (String::new(), path.clone())
    };

    let is_static = text.contains("static ");

    ctx.imports.push(ImportEntry {
        file_id: 0,
        imported_name,
        source_module,
        alias: None,
        line,
        kind: if is_static {
            "static_import".to_string()
        } else {
            "import".to_string()
        },
    });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn extract_java_visibility(ctx: &Ctx, node: Node) -> Visibility {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "modifiers" || child.kind() == "modifier" {
            let text = ctx.text(child);
            if text.contains("public") {
                return Visibility::Public;
            } else if text.contains("protected") {
                return Visibility::PubCrate;
            } else if text.contains("private") {
                return Visibility::Private;
            }
        }
    }
    // Package-private (no modifier)
    Visibility::PubCrate
}

fn extract_javadoc(ctx: &Ctx, node: Node) -> Option<String> {
    let mut sib = node.prev_sibling();
    while let Some(s) = sib {
        match s.kind() {
            "block_comment" => {
                let text = ctx.text(s);
                if text.starts_with("/**") {
                    let doc = text
                        .trim_start_matches("/**")
                        .trim_end_matches("*/")
                        .lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !doc.is_empty() {
                        return Some(doc);
                    }
                }
                break;
            }
            "line_comment" => {
                break;
            }
            "marker_annotation" | "annotation" => {
                sib = s.prev_sibling();
                continue;
            }
            _ => break,
        }
    }
    None
}

fn extract_method_sig(ctx: &Ctx, node: Node, name: &str) -> String {
    let vis = extract_java_visibility(ctx, node);
    let vis_str = match vis {
        Visibility::Public => "public ",
        Visibility::PubCrate => "",
        Visibility::PubSuper => "protected ",
        Visibility::Private => "private ",
    };
    let type_text = field_text(ctx, node, "type").unwrap_or_default();
    let params = field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());

    if type_text.is_empty() {
        // Constructor
        format!("{vis_str}{name}{params}")
    } else {
        format!("{vis_str}{type_text} {name}{params}")
    }
}

fn field_text(ctx: &Ctx, node: Node, field: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| ctx.text(n).to_string())
}

fn build_qname(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", scope.join("."), name)
    }
}

fn ext_scope(scope: &[String], name: &str) -> Vec<String> {
    let mut s = scope.to_vec();
    s.push(name.to_string());
    s
}

fn hash_node(ctx: &Ctx, node: Node) -> String {
    crate::scanner::hash_content(ctx.text(node).as_bytes())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
        let source = r#"
public interface Greeter {
    String greet(String name);
}
"#;
        let parsed = parse_java(source);
        let greeter = parsed.symbols.iter().find(|s| s.name == "Greeter").unwrap();
        assert_eq!(greeter.kind, SymbolKind::Interface);
        assert_eq!(greeter.visibility, Visibility::Public);
    }

    #[test]
    fn test_enum_with_constants() {
        let source = r#"
public enum Color {
    RED,
    GREEN,
    BLUE
}
"#;
        let parsed = parse_java(source);
        let color = parsed.symbols.iter().find(|s| s.name == "Color").unwrap();
        assert_eq!(color.kind, SymbolKind::Enum);

        let red = parsed.symbols.iter().find(|s| s.name == "RED").unwrap();
        assert_eq!(red.kind, SymbolKind::EnumVariant);
        assert_eq!(red.parent_id, Some(color.id));
    }

    #[test]
    fn test_visibility() {
        let source = r#"
public class Vis {
    public String pub_field;
    private int priv_field;
    protected double prot_field;
    String pkg_field;
}
"#;
        let parsed = parse_java(source);

        let pub_f = parsed.symbols.iter().find(|s| s.name == "pub_field").unwrap();
        assert_eq!(pub_f.visibility, Visibility::Public);

        let priv_f = parsed.symbols.iter().find(|s| s.name == "priv_field").unwrap();
        assert_eq!(priv_f.visibility, Visibility::Private);
    }

    #[test]
    fn test_imports() {
        let source = r#"
import java.util.List;
import java.util.Map;
import static java.lang.Math.PI;
"#;
        let parsed = parse_java(source);
        assert!(parsed.imports.len() >= 3, "got {} imports", parsed.imports.len());

        let list = parsed.imports.iter().find(|i| i.imported_name == "List").unwrap();
        assert_eq!(list.source_module, "java.util");
        assert_eq!(list.kind, "import");

        let pi = parsed.imports.iter().find(|i| i.imported_name == "PI").unwrap();
        assert_eq!(pi.kind, "static_import");
    }

    #[test]
    fn test_javadoc() {
        let source = r#"
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
"#;
        let parsed = parse_java(source);
        let utils = parsed.symbols.iter().find(|s| s.name == "Utils").unwrap();
        assert!(utils.doc_comment.as_ref().unwrap().contains("utility class"));

        let add = parsed.symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(add.doc_comment.as_ref().unwrap().contains("Adds two numbers"));
    }

    #[test]
    fn test_inner_class() {
        let source = r#"
public class Outer {
    public class Inner {
        public void method() {}
    }
}
"#;
        let parsed = parse_java(source);
        let inner = parsed.symbols.iter().find(|s| s.name == "Inner").unwrap();
        assert_eq!(inner.qualified_name.as_deref(), Some("Outer.Inner"));

        let method = parsed.symbols.iter().find(|s| s.name == "method").unwrap();
        assert_eq!(
            method.qualified_name.as_deref(),
            Some("Outer.Inner.method")
        );
    }

    #[test]
    fn test_constructor() {
        let source = r#"
public class Dog {
    public Dog(String name) {
        this.name = name;
    }
}
"#;
        let parsed = parse_java(source);
        let ctor = parsed.symbols.iter().find(|s| s.name == "Dog" && s.kind == SymbolKind::Method).unwrap();
        assert_eq!(ctor.kind, SymbolKind::Method);
    }

    #[test]
    fn test_empty_source() {
        let parsed = parse_java("");
        assert!(parsed.symbols.is_empty());
    }
}
