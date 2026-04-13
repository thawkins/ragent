//! Go language parser using tree-sitter.
//!
//! Extracts functions, methods (receiver-based), structs, interfaces,
//! constants, type aliases, and import blocks from Go source code.
//! Visibility: capitalised names = exported (Public).

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

/// Tree-sitter parser for the Go programming language.
pub struct GoParser {
    _private: (),
}

impl GoParser {
    /// Create a new Go parser.
    pub fn new() -> Self {
        Self { _private: () }
    }

    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        let lang = tree_sitter_go::LANGUAGE;
        parser
            .set_language(&lang.into())
            .context("failed to load Go grammar")?;
        Ok(parser)
    }
}

impl LanguageParser for GoParser {
    fn language_id(&self) -> &'static str {
        "go"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let mut parser = Self::create_parser()?;
        let tree = parser
            .parse(source, None)
            .context("tree-sitter parse returned None")?;
        let root = tree.root_node();

        let mut ctx = Ctx {
            source,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            next_id: 0,
        };

        walk(&mut ctx, root, None, &[]);

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
        "function_declaration" => extract_function(ctx, node, parent, scope),
        "method_declaration" => extract_method(ctx, node, parent, scope),
        "type_declaration" => extract_type_decl(ctx, node, parent, scope),
        "const_declaration" | "var_declaration" => extract_const_var(ctx, node, parent, scope),
        "import_declaration" => extract_imports(ctx, node),
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
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = go_visibility(&name);
    let doc = extract_go_doc(ctx, node);
    let sig = build_func_sig(ctx, node, &name);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind: SymbolKind::Function,
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

// ── Method (receiver-based) ─────────────────────────────────────────────────

fn extract_method(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let receiver = field_text(ctx, node, "receiver").unwrap_or_default();
    let visibility = go_visibility(&name);
    let doc = extract_go_doc(ctx, node);
    let params = field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let result = field_text(ctx, node, "result");

    let sig = match result {
        Some(r) => format!("func {receiver} {name}{params} {r}"),
        None => format!("func {receiver} {name}{params}"),
    };
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

// ── Type declarations (struct, interface, type alias) ────────────────────────

fn extract_type_decl(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "type_spec" {
            extract_type_spec(ctx, child, parent, scope);
        }
    }
}

fn extract_type_spec(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let type_node = node.child_by_field_name("type");
    let kind = type_node
        .map(|t| match t.kind() {
            "struct_type" => SymbolKind::Struct,
            "interface_type" => SymbolKind::Interface,
            _ => SymbolKind::TypeAlias,
        })
        .unwrap_or(SymbolKind::TypeAlias);

    let visibility = go_visibility(&name);
    let doc = extract_go_doc(ctx, node)
        .or_else(|| extract_go_doc(ctx, node.parent().unwrap_or(node)));
    let sig = format!("type {name}");
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind,
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

    // Extract struct fields.
    if kind == SymbolKind::Struct {
        if let Some(type_n) = type_node {
            let new_scope = ext_scope(scope, &name);
            extract_struct_fields(ctx, type_n, id, &new_scope);
        }
    }
}

fn extract_struct_fields(ctx: &mut Ctx, struct_node: Node, parent_id: i64, scope: &[String]) {
    let cursor = &mut struct_node.walk();
    for child in struct_node.children(cursor) {
        if child.kind() == "field_declaration_list" {
            let fc = &mut child.walk();
            for field in child.children(fc) {
                if field.kind() == "field_declaration" {
                    let name = field_text(ctx, field, "name").unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    let type_text = field_text(ctx, field, "type").unwrap_or_default();
                    let vis = go_visibility(&name);
                    let qname = build_qname(scope, &name);

                    let id = ctx.alloc_id();
                    ctx.symbols.push(Symbol {
                        id,
                        file_id: 0,
                        name: name.clone(),
                        qualified_name: Some(qname),
                        kind: SymbolKind::Field,
                        visibility: vis,
                        start_line: field.start_position().row as u32 + 1,
                        end_line: field.end_position().row as u32 + 1,
                        start_col: field.start_position().column as u32,
                        end_col: field.end_position().column as u32,
                        parent_id: Some(parent_id),
                        signature: Some(format!("{name} {type_text}")),
                        doc_comment: None,
                        body_hash: None,
                    });
                }
            }
        }
    }
}

// ── Constants / Variables ───────────────────────────────────────────────────

fn extract_const_var(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let is_const = node.kind() == "const_declaration";
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "const_spec" || child.kind() == "var_spec" {
            let name = field_text(ctx, child, "name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let type_text = field_text(ctx, child, "type").unwrap_or_default();
            let vis = go_visibility(&name);
            let qname = build_qname(scope, &name);
            let kind = if is_const {
                SymbolKind::Constant
            } else {
                SymbolKind::Static
            };
            let label = if is_const { "const" } else { "var" };

            let id = ctx.alloc_id();
            ctx.symbols.push(Symbol {
                id,
                file_id: 0,
                name,
                qualified_name: Some(qname),
                kind,
                visibility: vis,
                start_line: child.start_position().row as u32 + 1,
                end_line: child.end_position().row as u32 + 1,
                start_col: child.start_position().column as u32,
                end_col: child.end_position().column as u32,
                parent_id: parent,
                signature: Some(format!("{label} {type_text}")),
                doc_comment: None,
                body_hash: None,
            });
        }
    }
}

// ── Imports ─────────────────────────────────────────────────────────────────

fn extract_imports(ctx: &mut Ctx, node: Node) {
    let line = node.start_position().row as u32 + 1;
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "import_spec" || child.kind() == "import_spec_list" {
            extract_import_specs(ctx, child, line);
        }
        if child.kind() == "interpreted_string_literal" {
            let path = ctx.text(child).trim_matches('"').to_string();
            ctx.imports.push(ImportEntry {
                file_id: 0,
                imported_name: path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&path)
                    .to_string(),
                source_module: path,
                alias: None,
                line,
                kind: "import".to_string(),
            });
        }
    }
}

fn extract_import_specs(ctx: &mut Ctx, node: Node, line: u32) {
    if node.kind() == "import_spec" {
        let alias = field_text(ctx, node, "name");
        let path_node = node.child_by_field_name("path");
        if let Some(pn) = path_node {
            let path = ctx.text(pn).trim_matches('"').to_string();
            let imported = path.rsplit('/').next().unwrap_or(&path).to_string();
            ctx.imports.push(ImportEntry {
                file_id: 0,
                imported_name: imported,
                source_module: path,
                alias,
                line,
                kind: "import".to_string(),
            });
        }
    } else {
        let cursor = &mut node.walk();
        for child in node.children(cursor) {
            extract_import_specs(ctx, child, line);
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn go_visibility(name: &str) -> Visibility {
    if name.starts_with(|c: char| c.is_uppercase()) {
        Visibility::Public
    } else {
        Visibility::Private
    }
}

fn extract_go_doc(ctx: &Ctx, node: Node) -> Option<String> {
    let mut sib = node.prev_sibling();
    let mut lines = Vec::new();
    while let Some(s) = sib {
        if s.kind() == "comment" {
            let text = ctx.text(s);
            if text.starts_with("//") {
                let content = text.trim_start_matches("//").trim();
                lines.push(content.to_string());
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

fn build_func_sig(ctx: &Ctx, node: Node, name: &str) -> String {
    let params = field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let result = field_text(ctx, node, "result");
    match result {
        Some(r) => format!("func {name}{params} {r}"),
        None => format!("func {name}{params}"),
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

    fn parse_go(source: &str) -> ParsedFile {
        let p = GoParser::new();
        p.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_function() {
        let source = "package main\n\nfunc Hello() string {\n\treturn \"hi\"\n}\n";
        let parsed = parse_go(source);
        let hello = parsed.symbols.iter().find(|s| s.name == "Hello").unwrap();
        assert_eq!(hello.kind, SymbolKind::Function);
        assert_eq!(hello.visibility, Visibility::Public);
    }

    #[test]
    fn test_unexported_function() {
        let source = "package main\n\nfunc helper() {}\n";
        let parsed = parse_go(source);
        let h = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(h.visibility, Visibility::Private);
    }

    #[test]
    fn test_struct_with_fields() {
        let source = r#"
package main

type Config struct {
    Name string
    value int
}
"#;
        let parsed = parse_go(source);
        let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Config"), "got: {names:?}");
        assert!(names.contains(&"Name"), "got: {names:?}");
        assert!(names.contains(&"value"), "got: {names:?}");

        let config = parsed.symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config.kind, SymbolKind::Struct);
        assert_eq!(config.visibility, Visibility::Public);

        let name_f = parsed.symbols.iter().find(|s| s.name == "Name").unwrap();
        assert_eq!(name_f.kind, SymbolKind::Field);
        assert_eq!(name_f.visibility, Visibility::Public);

        let value_f = parsed.symbols.iter().find(|s| s.name == "value").unwrap();
        assert_eq!(value_f.visibility, Visibility::Private);
    }

    #[test]
    fn test_interface() {
        let source = r#"
package main

type Reader interface {
    Read(p []byte) (n int, err error)
}
"#;
        let parsed = parse_go(source);
        let reader = parsed.symbols.iter().find(|s| s.name == "Reader").unwrap();
        assert_eq!(reader.kind, SymbolKind::Interface);
        assert_eq!(reader.visibility, Visibility::Public);
    }

    #[test]
    fn test_method_with_receiver() {
        let source = r#"
package main

type Dog struct {}

func (d *Dog) Bark() string {
    return "Woof"
}
"#;
        let parsed = parse_go(source);
        let bark = parsed.symbols.iter().find(|s| s.name == "Bark").unwrap();
        assert_eq!(bark.kind, SymbolKind::Method);
        assert!(bark.signature.as_ref().unwrap().contains("(d *Dog)"));
    }

    #[test]
    fn test_constants() {
        let source = r#"
package main

const MaxSize = 1024
const (
    A = iota
    B
)
"#;
        let parsed = parse_go(source);
        let max = parsed.symbols.iter().find(|s| s.name == "MaxSize").unwrap();
        assert_eq!(max.kind, SymbolKind::Constant);
        assert_eq!(max.visibility, Visibility::Public);
    }

    #[test]
    fn test_imports() {
        let source = r#"
package main

import (
    "fmt"
    "os"
    log "github.com/sirupsen/logrus"
)
"#;
        let parsed = parse_go(source);
        assert!(parsed.imports.len() >= 2, "got {} imports", parsed.imports.len());

        let fmt_imp = parsed.imports.iter().find(|i| i.imported_name == "fmt");
        assert!(fmt_imp.is_some());
    }

    #[test]
    fn test_type_alias() {
        let source = "package main\n\ntype MyString string\n";
        let parsed = parse_go(source);
        let ms = parsed.symbols.iter().find(|s| s.name == "MyString").unwrap();
        assert_eq!(ms.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_empty_source() {
        let parsed = parse_go("package main\n");
        assert!(parsed.symbols.is_empty());
    }
}
