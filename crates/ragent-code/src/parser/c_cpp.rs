//! C and C++ language parsers using tree-sitter.
//!
//! Extracts functions, structs, classes (C++), enums, typedefs, `#include`
//! directives, namespaces (C++), and templates (C++).
//! Visibility: header-based heuristic (public if in `.h`).

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

// ── C Parser ────────────────────────────────────────────────────────────────

/// Tree-sitter parser for the C programming language.
pub struct CParser {
    _private: (),
}

impl CParser {
    /// Create a new C parser.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl LanguageParser for CParser {
    fn language_id(&self) -> &'static str {
        "c"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let mut parser = Parser::new();
        let lang = tree_sitter_c::LANGUAGE;
        parser
            .set_language(&lang.into())
            .context("failed to load C grammar")?;
        let tree = parser
            .parse(source, None)
            .context("tree-sitter parse returned None")?;

        let mut ctx = Ctx {
            source,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            next_id: 0,
            is_cpp: false,
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

// ── C++ Parser ──────────────────────────────────────────────────────────────

/// Tree-sitter parser for the C++ programming language.
pub struct CppParser {
    _private: (),
}

impl CppParser {
    /// Create a new C++ parser.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl LanguageParser for CppParser {
    fn language_id(&self) -> &'static str {
        "cpp"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let mut parser = Parser::new();
        let lang = tree_sitter_cpp::LANGUAGE;
        parser
            .set_language(&lang.into())
            .context("failed to load C++ grammar")?;
        let tree = parser
            .parse(source, None)
            .context("tree-sitter parse returned None")?;

        let mut ctx = Ctx {
            source,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            next_id: 0,
            is_cpp: true,
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

// ── Shared implementation ───────────────────────────────────────────────────

struct Ctx<'a> {
    source: &'a [u8],
    symbols: Vec<Symbol>,
    imports: Vec<ImportEntry>,
    references: Vec<SymbolRef>,
    next_id: i64,
    is_cpp: bool,
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

fn walk(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    match node.kind() {
        "function_definition" | "function_declarator" => {
            if node.kind() == "function_definition" {
                extract_function(ctx, node, parent, scope);
            }
        }
        "declaration" => extract_declaration(ctx, node, parent, scope),
        "struct_specifier" => extract_struct(ctx, node, parent, scope),
        "enum_specifier" => extract_enum(ctx, node, parent, scope),
        "type_definition" => extract_typedef(ctx, node, parent, scope),
        "preproc_include" => extract_include(ctx, node),

        // C++ specific
        "class_specifier" if ctx.is_cpp => extract_class(ctx, node, parent, scope),
        "namespace_definition" if ctx.is_cpp => extract_namespace(ctx, node, parent, scope),
        "template_declaration" if ctx.is_cpp => {
            // Recurse into the template body
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent, scope);
            }
        }

        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent, scope);
            }
        }
    }
}

// ── Function ────────────────────────────────────────────────────────────────

fn extract_function(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    // The declarator child contains the function name.
    let declarator = node.child_by_field_name("declarator");
    let name = declarator
        .and_then(|d| find_identifier(ctx, d))
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let doc = extract_c_doc(ctx, node);
    let sig = extract_fn_signature(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
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

// ── Declaration (may be function prototype or variable) ─────────────────────

fn extract_declaration(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let text = ctx.text(node).trim().to_string();

    // Check if this looks like a function declaration (has parentheses in declarator)
    let declarator = node.child_by_field_name("declarator");
    if let Some(decl) = declarator {
        if decl.kind() == "function_declarator" || decl.kind() == "init_declarator" {
            let name = find_identifier(ctx, decl).unwrap_or_default();
            if !name.is_empty() && text.contains('(') {
                // Function prototype
                let doc = extract_c_doc(ctx, node);
                let qname = build_qname(scope, &name);

                let id = ctx.alloc_id();
                ctx.symbols.push(Symbol {
                    id,
                    file_id: 0,
                    name,
                    qualified_name: Some(qname),
                    kind: SymbolKind::Function,
                    visibility: Visibility::Public,
                    start_line: node.start_position().row as u32 + 1,
                    end_line: node.end_position().row as u32 + 1,
                    start_col: node.start_position().column as u32,
                    end_col: node.end_position().column as u32,
                    parent_id: parent,
                    signature: Some(text),
                    doc_comment: doc,
                    body_hash: None,
                });
                return;
            }
        }
    }

    // Recurse to pick up nested struct/enum specifiers
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, parent, scope);
    }
}

// ── Struct ──────────────────────────────────────────────────────────────────

fn extract_struct(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let doc = extract_c_doc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Struct,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(format!("struct {name}")),
        doc_comment: doc,
        body_hash: Some(hash),
    });

    // Extract fields from body
    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        extract_c_fields(ctx, body, id, &new_scope);
    }
}

fn extract_c_fields(ctx: &mut Ctx, body: Node, parent_id: i64, scope: &[String]) {
    let cursor = &mut body.walk();
    for child in body.children(cursor) {
        if child.kind() == "field_declaration" {
            let declarator = child.child_by_field_name("declarator");
            let name = declarator
                .and_then(|d| find_identifier(ctx, d))
                .unwrap_or_default();
            if !name.is_empty() {
                let type_text = field_text(ctx, child, "type").unwrap_or_default();
                let qname = build_qname(scope, &name);

                let id = ctx.alloc_id();
                ctx.symbols.push(Symbol {
                    id,
                    file_id: 0,
                    name: name.clone(),
                    qualified_name: Some(qname),
                    kind: SymbolKind::Field,
                    visibility: Visibility::Public,
                    start_line: child.start_position().row as u32 + 1,
                    end_line: child.end_position().row as u32 + 1,
                    start_col: child.start_position().column as u32,
                    end_col: child.end_position().column as u32,
                    parent_id: Some(parent_id),
                    signature: Some(format!("{type_text} {name}")),
                    doc_comment: None,
                    body_hash: None,
                });
            }
        }
    }
}

// ── Enum ────────────────────────────────────────────────────────────────────

fn extract_enum(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let doc = extract_c_doc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Enum,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(format!("enum {name}")),
        doc_comment: doc,
        body_hash: Some(hash),
    });

    // Extract enumerators
    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            if child.kind() == "enumerator" {
                let ename = field_text(ctx, child, "name").unwrap_or_default();
                if !ename.is_empty() {
                    let eid = ctx.alloc_id();
                    ctx.symbols.push(Symbol {
                        id: eid,
                        file_id: 0,
                        name: ename.clone(),
                        qualified_name: Some(build_qname(&new_scope, &ename)),
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
        }
    }
}

// ── Typedef ─────────────────────────────────────────────────────────────────

fn extract_typedef(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let declarator = node.child_by_field_name("declarator");
    let name = declarator
        .and_then(|d| find_identifier(ctx, d))
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let full_text = ctx.text(node).trim().to_string();
    let doc = extract_c_doc(ctx, node);
    let qname = build_qname(scope, &name);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind: SymbolKind::TypeAlias,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(full_text),
        doc_comment: doc,
        body_hash: None,
    });
}

// ── #include ────────────────────────────────────────────────────────────────

fn extract_include(ctx: &mut Ctx, node: Node) {
    let line = node.start_position().row as u32 + 1;
    let path_node = node.child_by_field_name("path");
    let path = path_node
        .map(|n| ctx.text(n).to_string())
        .unwrap_or_default();
    if path.is_empty() {
        return;
    }

    let clean = path
        .trim_matches(|c| c == '<' || c == '>' || c == '"')
        .to_string();
    ctx.imports.push(ImportEntry {
        file_id: 0,
        imported_name: clean.clone(),
        source_module: clean,
        alias: None,
        line,
        kind: "include".to_string(),
    });
}

// ── C++ Class ───────────────────────────────────────────────────────────────

fn extract_class(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let doc = extract_c_doc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Class,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(format!("class {name}")),
        doc_comment: doc,
        body_hash: Some(hash),
    });

    // Recurse into class body for methods and fields
    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id), &new_scope);
        }
    }
}

// ── C++ Namespace ───────────────────────────────────────────────────────────

fn extract_namespace(ctx: &mut Ctx, node: Node, _parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        // anonymous namespace — still recurse
        if let Some(body) = node.child_by_field_name("body") {
            let cursor = &mut body.walk();
            for child in body.children(cursor) {
                walk(ctx, child, None, scope);
            }
        }
        return;
    }

    let qname = build_qname(scope, &name);
    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Module,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: None,
        signature: Some(format!("namespace {name}")),
        doc_comment: None,
        body_hash: None,
    });

    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id), &new_scope);
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn find_identifier(ctx: &Ctx, node: Node) -> Option<String> {
    if node.kind() == "identifier" || node.kind() == "field_identifier"
        || node.kind() == "type_identifier" || node.kind() == "primitive_type"
    {
        return Some(ctx.text(node).to_string());
    }
    // Recurse into declarator wrappers (pointer_declarator, etc.)
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if let Some(name) = find_identifier(ctx, child) {
            return Some(name);
        }
    }
    None
}

fn extract_fn_signature(ctx: &Ctx, node: Node) -> String {
    // Take the first line as signature
    let text = ctx.text(node);
    if let Some(brace) = text.find('{') {
        text[..brace].trim().to_string()
    } else {
        text.lines().next().unwrap_or("").trim().to_string()
    }
}

fn extract_c_doc(ctx: &Ctx, node: Node) -> Option<String> {
    let mut sib = node.prev_sibling();
    let mut lines = Vec::new();
    while let Some(s) = sib {
        if s.kind() == "comment" {
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
                    lines.push(doc);
                }
                break;
            } else if text.starts_with("//") {
                lines.push(text.trim_start_matches("//").trim().to_string());
            }
        } else {
            break;
        }
        sib = s.prev_sibling();
    }
    if lines.is_empty() {
        None
    } else {
        lines.reverse();
        Some(lines.join("\n"))
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
        format!("{}::{}", scope.join("::"), name)
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

    fn parse_c(source: &str) -> ParsedFile {
        let p = CParser::new();
        p.parse(source.as_bytes()).unwrap()
    }

    fn parse_cpp(source: &str) -> ParsedFile {
        let p = CppParser::new();
        p.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_c_function() {
        let source = "int add(int a, int b) {\n    return a + b;\n}\n";
        let parsed = parse_c(source);
        let add = parsed.symbols.iter().find(|s| s.name == "add").unwrap();
        assert_eq!(add.kind, SymbolKind::Function);
    }

    #[test]
    fn test_c_struct() {
        let source = r#"
struct Point {
    int x;
    int y;
};
"#;
        let parsed = parse_c(source);
        let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Point"), "got: {names:?}");
        assert!(names.contains(&"x"), "got: {names:?}");
        assert!(names.contains(&"y"), "got: {names:?}");

        let point = parsed.symbols.iter().find(|s| s.name == "Point").unwrap();
        assert_eq!(point.kind, SymbolKind::Struct);
    }

    #[test]
    fn test_c_enum() {
        let source = r#"
enum Color {
    RED,
    GREEN,
    BLUE
};
"#;
        let parsed = parse_c(source);
        let color = parsed.symbols.iter().find(|s| s.name == "Color").unwrap();
        assert_eq!(color.kind, SymbolKind::Enum);

        let red = parsed.symbols.iter().find(|s| s.name == "RED").unwrap();
        assert_eq!(red.kind, SymbolKind::EnumVariant);
    }

    #[test]
    fn test_c_typedef() {
        let source = "typedef unsigned long size_t;\n";
        let parsed = parse_c(source);
        let st = parsed.symbols.iter().find(|s| s.name == "size_t").unwrap();
        assert_eq!(st.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_c_include() {
        let source = r#"
#include <stdio.h>
#include "myheader.h"
"#;
        let parsed = parse_c(source);
        assert!(parsed.imports.len() >= 2, "got {} imports", parsed.imports.len());
    }

    #[test]
    fn test_cpp_class() {
        let source = r#"
class Dog {
public:
    void bark();
};
"#;
        let parsed = parse_cpp(source);
        let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
        assert_eq!(dog.kind, SymbolKind::Class);
    }

    #[test]
    fn test_cpp_namespace() {
        let source = r#"
namespace myns {
    int helper() { return 42; }
}
"#;
        let parsed = parse_cpp(source);
        let ns = parsed.symbols.iter().find(|s| s.name == "myns").unwrap();
        assert_eq!(ns.kind, SymbolKind::Module);

        let helper = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(helper.kind, SymbolKind::Function);
        assert_eq!(helper.qualified_name.as_deref(), Some("myns::helper"));
    }

    #[test]
    fn test_empty_c() {
        let parsed = parse_c("");
        assert!(parsed.symbols.is_empty());
    }

    #[test]
    fn test_empty_cpp() {
        let parsed = parse_cpp("");
        assert!(parsed.symbols.is_empty());
    }
}
