//! TypeScript / JavaScript parser using tree-sitter.
//!
//! Handles `.ts`, `.tsx`, `.js`, `.jsx` via a shared [`TypeScriptParser`] that
//! is parameterised by [`TsVariant`].

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

/// Which flavour of JS/TS grammar to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsVariant {
    /// Standard TypeScript (.ts)
    TypeScript,
    /// TypeScript with JSX (.tsx)
    Tsx,
    /// Standard JavaScript (.js)
    JavaScript,
    /// JavaScript with JSX (.jsx)
    Jsx,
}

/// Tree-sitter parser for TypeScript and JavaScript.
pub struct TypeScriptParser {
    variant: TsVariant,
}

impl TypeScriptParser {
    /// Create a new TypeScript/JavaScript parser for the given variant.
    #[must_use]
    pub const fn new(variant: TsVariant) -> Self {
        Self { variant }
    }

    /// Create a cached tree-sitter parser configured for this variant.
    ///
    /// M-027: a per-variant `OnceLock<Mutex<Parser>>` avoids re-constructing
    /// the parser (and re-setting the grammar) on every `parse` call.
    fn create_parser(&self) -> Result<std::sync::MutexGuard<'static, Parser>> {
        static CACHE_TYPESCRIPT: std::sync::OnceLock<std::sync::Mutex<Parser>> =
            std::sync::OnceLock::new();
        static CACHE_TSX: std::sync::OnceLock<std::sync::Mutex<Parser>> =
            std::sync::OnceLock::new();
        static CACHE_JS: std::sync::OnceLock<std::sync::Mutex<Parser>> = std::sync::OnceLock::new();

        let mutex = match self.variant {
            TsVariant::TypeScript => CACHE_TYPESCRIPT.get_or_init(|| {
                let mut parser = Parser::new();
                parser
                    .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                    .expect("TS grammar is a statically-linked tree-sitter grammar");
                std::sync::Mutex::new(parser)
            }),
            TsVariant::Tsx => CACHE_TSX.get_or_init(|| {
                let mut parser = Parser::new();
                parser
                    .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
                    .expect("TSX grammar is a statically-linked tree-sitter grammar");
                std::sync::Mutex::new(parser)
            }),
            TsVariant::JavaScript | TsVariant::Jsx => CACHE_JS.get_or_init(|| {
                let mut parser = Parser::new();
                parser
                    .set_language(&tree_sitter_javascript::LANGUAGE.into())
                    .expect("JS grammar is a statically-linked tree-sitter grammar");
                std::sync::Mutex::new(parser)
            }),
        };
        Ok(mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

impl LanguageParser for TypeScriptParser {
    fn language_id(&self) -> &'static str {
        match self.variant {
            TsVariant::TypeScript => "typescript",
            TsVariant::Tsx => "tsx",
            TsVariant::JavaScript => "javascript",
            TsVariant::Jsx => "jsx",
        }
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let mut parser = self.create_parser()?;
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

        walk(&mut ctx, root, None, &[], false);

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
    const fn alloc_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    fn text(&self, node: Node) -> &str {
        node.utf8_text(self.source).unwrap_or("")
    }
}

// ── Recursive walk ──────────────────────────────────────────────────────────

fn walk(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String], exported: bool) {
    match node.kind() {
        // Declarations
        "function_declaration" | "generator_function_declaration" => {
            extract_function(ctx, node, parent, scope, exported);
        }
        "class_declaration" => extract_class(ctx, node, parent, scope, exported),
        "interface_declaration" => extract_interface(ctx, node, parent, scope, exported),
        "enum_declaration" => extract_enum(ctx, node, parent, scope, exported),
        "type_alias_declaration" => extract_type_alias(ctx, node, parent, scope, exported),

        // Variable declarations may contain arrow functions
        "lexical_declaration" | "variable_declaration" => {
            extract_variable_decl(ctx, node, parent, scope, exported);
        }

        // Export wraps a declaration
        "export_statement" => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent, scope, true);
            }
        }

        // Imports
        "import_statement" => extract_import(ctx, node),

        // Method definitions inside class body
        "method_definition" | "public_field_definition" | "field_definition" => {
            extract_method(ctx, node, parent, scope);
        }

        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent, scope, exported);
            }
        }
    }
}

// ── Function ────────────────────────────────────────────────────────────────

fn extract_function(
    ctx: &mut Ctx,
    node: Node,
    parent: Option<i64>,
    scope: &[String],
    exported: bool,
) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = if exported {
        Visibility::Public
    } else {
        Visibility::Private
    };
    let sig = build_fn_sig(ctx, node, &name, exported);
    let doc = extract_jsdoc(ctx, node);
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

// ── Class ───────────────────────────────────────────────────────────────────

fn extract_class(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String], exported: bool) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = if exported {
        Visibility::Public
    } else {
        Visibility::Private
    };
    let doc = extract_jsdoc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

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
        signature: Some(format!("class {name}")),
        doc_comment: doc,
        body_hash: Some(hash),
    });

    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id), &new_scope, false);
        }
    }
}

// ── Interface ───────────────────────────────────────────────────────────────

fn extract_interface(
    ctx: &mut Ctx,
    node: Node,
    parent: Option<i64>,
    scope: &[String],
    exported: bool,
) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = if exported {
        Visibility::Public
    } else {
        Visibility::Private
    };
    let doc = extract_jsdoc(ctx, node);
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
}

// ── Enum ────────────────────────────────────────────────────────────────────

fn extract_enum(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String], exported: bool) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = if exported {
        Visibility::Public
    } else {
        Visibility::Private
    };
    let doc = extract_jsdoc(ctx, node);
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

    // Extract enum members.
    let new_scope = ext_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            if child.kind() == "enum_assignment" || child.kind() == "property_identifier" {
                let member_name = if child.kind() == "enum_assignment" {
                    field_text(ctx, child, "name").unwrap_or_default()
                } else {
                    ctx.text(child).to_string()
                };
                if !member_name.is_empty() {
                    let mid = ctx.alloc_id();
                    ctx.symbols.push(Symbol {
                        id: mid,
                        file_id: 0,
                        name: member_name.clone(),
                        qualified_name: Some(build_qname(&new_scope, &member_name)),
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

// ── Type alias ──────────────────────────────────────────────────────────────

fn extract_type_alias(
    ctx: &mut Ctx,
    node: Node,
    parent: Option<i64>,
    scope: &[String],
    exported: bool,
) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = if exported {
        Visibility::Public
    } else {
        Visibility::Private
    };
    let full_text = ctx.text(node).trim().to_string();
    let qname = build_qname(scope, &name);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind: SymbolKind::TypeAlias,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: Some(full_text),
        doc_comment: extract_jsdoc(ctx, node),
        body_hash: None,
    });
}

// ── Variable declarations (const/let/var, arrow functions) ──────────────────

fn extract_variable_decl(
    ctx: &mut Ctx,
    node: Node,
    parent: Option<i64>,
    scope: &[String],
    exported: bool,
) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "variable_declarator" {
            let name = field_text(ctx, child, "name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            // Check if the value is an arrow function or function expression
            let value = child.child_by_field_name("value");
            let is_fn = value.as_ref().is_some_and(|v| {
                v.kind() == "arrow_function"
                    || v.kind() == "function"
                    || v.kind() == "function_expression"
            });

            let visibility = if exported {
                Visibility::Public
            } else {
                Visibility::Private
            };

            if is_fn {
                let doc = extract_jsdoc(ctx, node);
                let qname = build_qname(scope, &name);
                let hash = hash_node(ctx, child);

                let id = ctx.alloc_id();
                let sig = format!("const {name} = ...");
                ctx.symbols.push(Symbol {
                    id,
                    file_id: 0,
                    name,
                    qualified_name: Some(qname),
                    kind: SymbolKind::Function,
                    visibility,
                    start_line: child.start_position().row as u32 + 1,
                    end_line: child.end_position().row as u32 + 1,
                    start_col: child.start_position().column as u32,
                    end_col: child.end_position().column as u32,
                    parent_id: parent,
                    signature: Some(sig),
                    doc_comment: doc,
                    body_hash: Some(hash),
                });
            } else if parent.is_none()
                && name
                    .chars()
                    .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
                && name.chars().any(char::is_alphabetic)
            {
                // Module-level ALL_CAPS constant
                let qname = build_qname(scope, &name);
                let id = ctx.alloc_id();
                ctx.symbols.push(Symbol {
                    id,
                    file_id: 0,
                    name,
                    qualified_name: Some(qname),
                    kind: SymbolKind::Constant,
                    visibility,
                    start_line: child.start_position().row as u32 + 1,
                    end_line: child.end_position().row as u32 + 1,
                    start_col: child.start_position().column as u32,
                    end_col: child.end_position().column as u32,
                    parent_id: parent,
                    signature: None,
                    doc_comment: None,
                    body_hash: None,
                });
            }
        }
    }
}

// ── Method (inside class body) ──────────────────────────────────────────────

fn extract_method(ctx: &mut Ctx, node: Node, parent: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let doc = extract_jsdoc(ctx, node);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind: SymbolKind::Method,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: parent,
        signature: None,
        doc_comment: doc,
        body_hash: Some(hash),
    });
}

// ── Imports ─────────────────────────────────────────────────────────────────

fn extract_import(ctx: &mut Ctx, node: Node) {
    let line = node.start_position().row as u32 + 1;
    let text = ctx.text(node).trim().to_string();

    // Extract the source module from the string literal
    let source_module = node
        .child_by_field_name("source")
        .map(|n| {
            ctx.text(n)
                .trim_matches(|c| c == '\'' || c == '"')
                .to_string()
        })
        .unwrap_or_default();

    // Try to find named imports in import clause
    let cursor = &mut node.walk();
    let mut found_any = false;
    for child in node.children(cursor) {
        if child.kind() == "import_clause" {
            let inner_cursor = &mut child.walk();
            for inner in child.children(inner_cursor) {
                match inner.kind() {
                    "named_imports" => {
                        let ic = &mut inner.walk();
                        for spec in inner.children(ic) {
                            if spec.kind() == "import_specifier" {
                                let imported = field_text(ctx, spec, "name")
                                    .unwrap_or_else(|| ctx.text(spec).to_string());
                                let alias = field_text(ctx, spec, "alias");
                                ctx.imports.push(ImportEntry {
                                    file_id: 0,
                                    imported_name: imported,
                                    source_module: source_module.clone(),
                                    alias,
                                    line,
                                    kind: "import".to_string(),
                                });
                                found_any = true;
                            }
                        }
                    }
                    "identifier" | "namespace_import" => {
                        let name = ctx.text(inner).to_string();
                        ctx.imports.push(ImportEntry {
                            file_id: 0,
                            imported_name: name,
                            source_module: source_module.clone(),
                            alias: None,
                            line,
                            kind: "import".to_string(),
                        });
                        found_any = true;
                    }
                    _ => {}
                }
            }
        }
    }

    if !found_any && !source_module.is_empty() {
        // Side-effect import: `import './polyfill'`
        ctx.imports.push(ImportEntry {
            file_id: 0,
            imported_name: text,
            source_module,
            alias: None,
            line,
            kind: "import".to_string(),
        });
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn extract_jsdoc(ctx: &Ctx, node: Node) -> Option<String> {
    let mut sib = node.prev_sibling();
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
                    return Some(doc);
                }
            }
            break;
        } else if s.kind() == "decorator" {
            sib = s.prev_sibling();
            continue;
        }
        break;
    }
    None
}

fn build_fn_sig(ctx: &Ctx, node: Node, name: &str, exported: bool) -> String {
    let params = field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let ret = field_text(ctx, node, "return_type");
    let export_kw = if exported { "export " } else { "" };
    let is_async = {
        let cursor = &mut node.walk();
        node.children(cursor).any(|c| ctx.text(c) == "async")
    };
    let async_kw = if is_async { "async " } else { "" };
    match ret {
        Some(rt) => format!("{export_kw}{async_kw}function {name}{params}: {rt}"),
        None => format!("{export_kw}{async_kw}function {name}{params}"),
    }
}

fn field_text(ctx: &Ctx, node: Node, field: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| ctx.text(n).to_string())
}

/// Build a qualified name from the current scope and a local name.
///
/// Delegates to [`super::util::build_qname`] with the `.` separator.
#[inline]
fn build_qname(scope: &[String], name: &str) -> String {
    super::util::build_qname(scope, name, ".")
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
