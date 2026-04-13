//! Python language parser using tree-sitter.
//!
//! Extracts functions, classes, methods, decorators, imports, module-level
//! constants, and type hints from Python source code.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

/// Tree-sitter parser for the Python programming language.
pub struct PythonParser {
    _private: (),
}

impl PythonParser {
    /// Create a new Python parser.
    pub fn new() -> Self {
        Self { _private: () }
    }

    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        let language = tree_sitter_python::LANGUAGE;
        parser
            .set_language(&language.into())
            .context("failed to load Python grammar")?;
        Ok(parser)
    }
}

impl LanguageParser for PythonParser {
    fn language_id(&self) -> &'static str {
        "python"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let mut parser = Self::create_parser()?;
        let tree = parser
            .parse(source, None)
            .context("tree-sitter parse returned None")?;
        let root = tree.root_node();

        let mut ctx = ExtractCtx {
            source,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            next_id: 0,
        };

        extract_node(&mut ctx, root, None, &[]);

        Ok(ParsedFile {
            symbols: ctx.symbols,
            imports: ctx.imports,
            references: ctx.references,
            tree: Some(tree),
        })
    }
}

struct ExtractCtx<'a> {
    source: &'a [u8],
    symbols: Vec<Symbol>,
    imports: Vec<ImportEntry>,
    references: Vec<SymbolRef>,
    next_id: i64,
}

impl ExtractCtx<'_> {
    fn alloc_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source).unwrap_or("")
    }
}

// ── Recursive extraction ────────────────────────────────────────────────────

fn extract_node(ctx: &mut ExtractCtx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    match node.kind() {
        "function_definition" => extract_function(ctx, node, parent_id, scope),
        "class_definition" => extract_class(ctx, node, parent_id, scope),
        "import_statement" => extract_import(ctx, node),
        "import_from_statement" => extract_from_import(ctx, node),
        "expression_statement" => {
            // Module-level assignments may be constants (UPPER_CASE).
            if parent_id.is_none() || scope.is_empty() {
                try_extract_assignment(ctx, node, parent_id, scope);
            }
            // Recurse for nested expressions
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                extract_node(ctx, child, parent_id, scope);
            }
        }
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                extract_node(ctx, child, parent_id, scope);
            }
        }
    }
}

// ── Function / Method ───────────────────────────────────────────────────────

fn extract_function(
    ctx: &mut ExtractCtx,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let decorators = collect_decorators(ctx, node);
    let is_static = decorators.iter().any(|d| d == "staticmethod");
    let is_classmethod = decorators.iter().any(|d| d == "classmethod");
    let is_property = decorators.iter().any(|d| d == "property");

    let kind = if parent_id.is_some() {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    };

    let visibility = python_visibility(&name);
    let doc_comment = extract_docstring(ctx, node);
    let signature = extract_function_sig(ctx, node, is_static, is_classmethod, is_property);
    let qualified_name = build_qualified(scope, &name);
    let body_hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qualified_name),
        kind,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(signature),
        doc_comment,
        body_hash: Some(body_hash),
    });

    // Recurse into function body for nested classes/functions.
    let new_scope = extend_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            extract_node(ctx, child, Some(id), &new_scope);
        }
    }
}

// ── Class ───────────────────────────────────────────────────────────────────

fn extract_class(
    ctx: &mut ExtractCtx,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = python_visibility(&name);
    let doc_comment = extract_docstring(ctx, node);
    let superclasses = child_by_field_text(ctx, node, "superclasses");
    let signature = match superclasses {
        Some(s) => format!("class {name}{s}"),
        None => format!("class {name}"),
    };
    let qualified_name = build_qualified(scope, &name);
    let body_hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qualified_name),
        kind: SymbolKind::Class,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(signature),
        doc_comment,
        body_hash: Some(body_hash),
    });

    // Recurse into class body for methods and nested classes.
    let new_scope = extend_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            extract_node(ctx, child, Some(id), &new_scope);
        }
    }
}

// ── Imports ─────────────────────────────────────────────────────────────────

fn extract_import(ctx: &mut ExtractCtx, node: Node) {
    let line = node.start_position().row as u32 + 1;
    let text = ctx.node_text(node).trim().to_string();

    // `import foo` or `import foo.bar as baz`
    let path = text.strip_prefix("import ").unwrap_or(&text).trim();
    for part in path.split(',') {
        let part = part.trim();
        let (name, alias) = if let Some(idx) = part.find(" as ") {
            (part[..idx].trim().to_string(), Some(part[idx + 4..].trim().to_string()))
        } else {
            (part.to_string(), None)
        };
        ctx.imports.push(ImportEntry {
            file_id: 0,
            imported_name: name.clone(),
            source_module: name,
            alias,
            line,
            kind: "import".to_string(),
        });
    }
}

fn extract_from_import(ctx: &mut ExtractCtx, node: Node) {
    let line = node.start_position().row as u32 + 1;
    let text = ctx.node_text(node).trim().to_string();

    // `from foo.bar import Baz, Quux as Q`
    let rest = text.strip_prefix("from ").unwrap_or(&text);
    if let Some(idx) = rest.find(" import ") {
        let module = rest[..idx].trim().to_string();
        let imports_str = rest[idx + 8..].trim();
        for part in imports_str.split(',') {
            let part = part.trim();
            let (name, alias) = if let Some(ai) = part.find(" as ") {
                (part[..ai].trim().to_string(), Some(part[ai + 4..].trim().to_string()))
            } else {
                (part.to_string(), None)
            };
            ctx.imports.push(ImportEntry {
                file_id: 0,
                imported_name: name,
                source_module: module.clone(),
                alias,
                line,
                kind: "from_import".to_string(),
            });
        }
    }
}

// ── Module-level constants ──────────────────────────────────────────────────

fn try_extract_assignment(
    ctx: &mut ExtractCtx,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "assignment" {
            if let Some(left) = child.child_by_field_name("left") {
                let name = ctx.node_text(left).trim().to_string();
                // Only treat ALL_CAPS names as constants.
                if !name.is_empty()
                    && name.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
                    && name.chars().any(|c| c.is_alphabetic())
                {
                    let type_ann = child
                        .child_by_field_name("type")
                        .map(|n| ctx.node_text(n).to_string());
                    let sig = match &type_ann {
                        Some(t) => format!("{name}: {t}"),
                        None => name.clone(),
                    };
                    let qualified_name = build_qualified(scope, &name);

                    let id = ctx.alloc_id();
                    ctx.symbols.push(Symbol {
                        id,
                        file_id: 0,
                        name,
                        qualified_name: Some(qualified_name),
                        kind: SymbolKind::Constant,
                        visibility: Visibility::Public,
                        start_line: child.start_position().row as u32 + 1,
                        end_line: child.end_position().row as u32 + 1,
                        start_col: child.start_position().column as u32,
                        end_col: child.end_position().column as u32,
                        parent_id,
                        signature: Some(sig),
                        doc_comment: None,
                        body_hash: None,
                    });
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn python_visibility(name: &str) -> Visibility {
    if name.starts_with("__") && name.ends_with("__") && name.len() > 4 {
        Visibility::Public // dunder methods are public
    } else if name.starts_with("__") {
        Visibility::Private // name-mangled
    } else if name.starts_with('_') {
        Visibility::PubCrate // conventionally private
    } else {
        Visibility::Public
    }
}

fn collect_decorators(ctx: &ExtractCtx, node: Node) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut sib = node.prev_sibling();
    while let Some(s) = sib {
        if s.kind() == "decorator" {
            let text = ctx.node_text(s).trim().to_string();
            let name = text
                .strip_prefix('@')
                .unwrap_or(&text)
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            decorators.push(name);
        } else if s.kind() == "comment" {
            // skip comments between decorators
        } else {
            break;
        }
        sib = s.prev_sibling();
    }
    decorators
}

fn extract_docstring(ctx: &ExtractCtx, node: Node) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let first = body.child(0)?;
    if first.kind() == "expression_statement" {
        let inner = first.child(0)?;
        if inner.kind() == "string" || inner.kind() == "concatenated_string" {
            let text = ctx.node_text(inner);
            let trimmed = text
                .trim_start_matches("\"\"\"")
                .trim_start_matches("'''")
                .trim_end_matches("\"\"\"")
                .trim_end_matches("'''")
                .trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_function_sig(
    ctx: &ExtractCtx,
    node: Node,
    is_static: bool,
    is_classmethod: bool,
    is_property: bool,
) -> String {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    let params = child_by_field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let return_type = child_by_field_text(ctx, node, "return_type");

    let mut prefix = String::new();
    if is_static {
        prefix.push_str("@staticmethod ");
    }
    if is_classmethod {
        prefix.push_str("@classmethod ");
    }
    if is_property {
        prefix.push_str("@property ");
    }

    // Check for async
    let is_async = {
        let cursor = &mut node.walk();
        node.children(cursor)
            .any(|c| c.kind() == "async" || ctx.node_text(c) == "async")
    };
    let def_kw = if is_async { "async def" } else { "def" };

    match return_type {
        Some(rt) => format!("{prefix}{def_kw} {name}{params} -> {rt}"),
        None => format!("{prefix}{def_kw} {name}{params}"),
    }
}

fn child_by_field_text(ctx: &ExtractCtx, node: Node, field: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| ctx.node_text(n).to_string())
}

fn build_qualified(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", scope.join("."), name)
    }
}

fn extend_scope(scope: &[String], name: &str) -> Vec<String> {
    let mut s = scope.to_vec();
    s.push(name.to_string());
    s
}

fn hash_node(ctx: &ExtractCtx, node: Node) -> String {
    let text = ctx.node_text(node);
    crate::scanner::hash_content(text.as_bytes())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_py(source: &str) -> ParsedFile {
        let parser = PythonParser::new();
        parser.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_simple_function() {
        let parsed = parse_py("def hello():\n    pass\n");
        assert_eq!(parsed.symbols.len(), 1);
        assert_eq!(parsed.symbols[0].name, "hello");
        assert_eq!(parsed.symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_class_with_methods() {
        let source = r#"
class Dog:
    """A dog class."""

    def __init__(self, name: str):
        self.name = name

    def bark(self) -> str:
        return "Woof!"

    @staticmethod
    def species() -> str:
        return "Canis lupus"
"#;
        let parsed = parse_py(source);
        let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Dog"), "got: {names:?}");
        assert!(names.contains(&"__init__"), "got: {names:?}");
        assert!(names.contains(&"bark"), "got: {names:?}");
        assert!(names.contains(&"species"), "got: {names:?}");

        let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
        assert_eq!(dog.kind, SymbolKind::Class);
        assert!(dog.doc_comment.as_ref().unwrap().contains("dog class"));

        let bark = parsed.symbols.iter().find(|s| s.name == "bark").unwrap();
        assert_eq!(bark.kind, SymbolKind::Method);
        assert_eq!(bark.parent_id, Some(dog.id));
    }

    #[test]
    fn test_visibility_conventions() {
        let source = r#"
def public_fn():
    pass

def _private_fn():
    pass

def __mangled_fn():
    pass

def __dunder__():
    pass
"#;
        let parsed = parse_py(source);

        let public = parsed.symbols.iter().find(|s| s.name == "public_fn").unwrap();
        assert_eq!(public.visibility, Visibility::Public);

        let private = parsed.symbols.iter().find(|s| s.name == "_private_fn").unwrap();
        assert_eq!(private.visibility, Visibility::PubCrate);

        let mangled = parsed.symbols.iter().find(|s| s.name == "__mangled_fn").unwrap();
        assert_eq!(mangled.visibility, Visibility::Private);

        let dunder = parsed.symbols.iter().find(|s| s.name == "__dunder__").unwrap();
        assert_eq!(dunder.visibility, Visibility::Public);
    }

    #[test]
    fn test_imports() {
        let source = r#"
import os
import sys
from pathlib import Path
from typing import List, Optional
from collections import OrderedDict as OD
"#;
        let parsed = parse_py(source);
        assert!(parsed.imports.len() >= 4, "got {} imports", parsed.imports.len());

        let os_imp = parsed.imports.iter().find(|i| i.imported_name == "os").unwrap();
        assert_eq!(os_imp.kind, "import");

        let path_imp = parsed.imports.iter().find(|i| i.imported_name == "Path").unwrap();
        assert_eq!(path_imp.source_module, "pathlib");
        assert_eq!(path_imp.kind, "from_import");

        let od = parsed.imports.iter().find(|i| i.imported_name == "OrderedDict").unwrap();
        assert_eq!(od.alias.as_deref(), Some("OD"));
    }

    #[test]
    fn test_module_constants() {
        let source = r#"
MAX_SIZE = 1024
DEFAULT_NAME = "hello"
_not_constant = True
regular_var = 42
"#;
        let parsed = parse_py(source);
        let const_names: Vec<&str> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Constant)
            .map(|s| s.name.as_str())
            .collect();
        assert!(const_names.contains(&"MAX_SIZE"), "got: {const_names:?}");
        assert!(const_names.contains(&"DEFAULT_NAME"), "got: {const_names:?}");
        // _not_constant and regular_var should NOT be extracted as constants
        assert!(!const_names.contains(&"_not_constant"));
        assert!(!const_names.contains(&"regular_var"));
    }

    #[test]
    fn test_async_function() {
        let source = r#"
async def fetch_data(url: str) -> bytes:
    pass
"#;
        let parsed = parse_py(source);
        let func = &parsed.symbols[0];
        assert_eq!(func.name, "fetch_data");
        assert!(func.signature.as_ref().unwrap().contains("async def"));
    }

    #[test]
    fn test_class_inheritance() {
        let source = r#"
class Animal:
    pass

class Dog(Animal):
    pass
"#;
        let parsed = parse_py(source);
        let dog = parsed.symbols.iter().find(|s| s.name == "Dog").unwrap();
        assert!(dog.signature.as_ref().unwrap().contains("(Animal)"));
    }

    #[test]
    fn test_qualified_names() {
        let source = r#"
class Outer:
    class Inner:
        def method(self):
            pass
"#;
        let parsed = parse_py(source);
        let method = parsed.symbols.iter().find(|s| s.name == "method").unwrap();
        assert_eq!(
            method.qualified_name.as_deref(),
            Some("Outer.Inner.method")
        );
    }

    #[test]
    fn test_empty_source() {
        let parsed = parse_py("");
        assert!(parsed.symbols.is_empty());
        assert!(parsed.imports.is_empty());
    }

    #[test]
    fn test_decorator_property() {
        let source = r#"
class Config:
    @property
    def name(self) -> str:
        return self._name
"#;
        let parsed = parse_py(source);
        let name_fn = parsed.symbols.iter().find(|s| s.name == "name").unwrap();
        assert!(name_fn.signature.as_ref().unwrap().contains("@property"));
    }
}
