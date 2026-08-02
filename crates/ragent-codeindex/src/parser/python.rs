//! Python language parser using tree-sitter.
//!
//! Extracts functions, classes, methods, decorators, imports, module-level
//! constants, and type hints from Python source code.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::Node;

/// Tree-sitter parser for the Python programming language.
pub struct PythonParser {
    _private: (),
}

impl PythonParser {
    /// Create a new Python parser.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }

    super::util::tree_sitter_parser!(
        tree_sitter_python::LANGUAGE,
        "failed to load Python grammar",
        "tree-sitter parse returned None"
    );
}

impl LanguageParser for PythonParser {
    fn language_id(&self) -> &'static str {
        "python"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let tree = Self::parse_tree(source)?;
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
    const fn alloc_id(&mut self) -> i64 {
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

fn extract_function(ctx: &mut ExtractCtx, node: Node, parent_id: Option<i64>, scope: &[String]) {
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

fn extract_class(ctx: &mut ExtractCtx, node: Node, parent_id: Option<i64>, scope: &[String]) {
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
            (
                part[..idx].trim().to_string(),
                Some(part[idx + 4..].trim().to_string()),
            )
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
                (
                    part[..ai].trim().to_string(),
                    Some(part[ai + 4..].trim().to_string()),
                )
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
        if child.kind() == "assignment"
            && let Some(left) = child.child_by_field_name("left")
        {
            let name = ctx.node_text(left).trim().to_string();
            // Only treat ALL_CAPS names as constants.
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
                && name.chars().any(char::is_alphabetic)
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

/// Build a qualified name from the current scope and a local name.
///
/// Delegates to [`super::util::build_qname`] with the `.` separator.
#[inline]
fn build_qualified(scope: &[String], name: &str) -> String {
    super::util::build_qname(scope, name, ".")
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
