//! Rust language parser using tree-sitter.
//!
//! Extracts functions, methods, structs, enums, traits, impl blocks,
//! constants, statics, modules, type aliases, macros, use statements,
//! and test functions from Rust source code.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

/// Tree-sitter parser for the Rust programming language.
pub struct RustParser {
    _private: (), // force use of `new()`
}

impl RustParser {
    /// Create a new Rust parser.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a tree-sitter parser configured for Rust.
    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .context("failed to load Rust grammar")?;
        Ok(parser)
    }

    /// Parse source code into a tree-sitter Tree.
    fn parse_tree(source: &[u8]) -> Result<Tree> {
        let mut parser = Self::create_parser()?;
        parser
            .parse(source, None)
            .context("tree-sitter parse returned None")
    }
}

impl LanguageParser for RustParser {
    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let tree = Self::parse_tree(source)?;
        let root = tree.root_node();

        let mut ctx = ExtractionContext {
            source,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            // Temporary IDs start at 0; the store assigns real IDs.
            next_temp_id: 0,
        };

        extract_node(&mut ctx, root, None, &[], false);

        Ok(ParsedFile {
            symbols: ctx.symbols,
            imports: ctx.imports,
            references: ctx.references,
            tree: Some(tree),
        })
    }
}

/// Mutable context threaded through recursive extraction.
struct ExtractionContext<'a> {
    source: &'a [u8],
    symbols: Vec<Symbol>,
    imports: Vec<ImportEntry>,
    references: Vec<SymbolRef>,
    next_temp_id: i64,
}

impl ExtractionContext<'_> {
    fn alloc_id(&mut self) -> i64 {
        let id = self.next_temp_id;
        self.next_temp_id += 1;
        id
    }

    fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source).unwrap_or("")
    }
}

// ── Recursive extraction ────────────────────────────────────────────────────

/// Walk a tree-sitter node, extracting symbols and imports.
fn extract_node(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
    method_context: bool,
) {
    match node.kind() {
        "function_item" | "function_signature_item" => {
            extract_function(ctx, node, parent_id, scope, method_context);
            // Recurse into the function body to capture references (calls,
            // field accesses, type refs) inside function/method bodies.
            if let Some(body) = node.child_by_field_name("body") {
                let cursor = &mut body.walk();
                for child in body.children(cursor) {
                    extract_node(ctx, child, parent_id, scope, method_context);
                }
            }
        }
        "struct_item" => extract_struct(ctx, node, parent_id, scope),
        "enum_item" => extract_enum(ctx, node, parent_id, scope),
        "trait_item" => extract_trait(ctx, node, parent_id, scope),
        "impl_item" => extract_impl(ctx, node, parent_id, scope),
        "const_item" => {
            extract_const_or_static(ctx, node, parent_id, scope, SymbolKind::Constant);
            // Recurse into initializer value for references.
            if let Some(val) = node.child_by_field_name("value") {
                extract_node(ctx, val, parent_id, scope, method_context);
            }
        }
        "static_item" => {
            extract_const_or_static(ctx, node, parent_id, scope, SymbolKind::Static);
            // Recurse into initializer value for references.
            if let Some(val) = node.child_by_field_name("value") {
                extract_node(ctx, val, parent_id, scope, method_context);
            }
        }
        "mod_item" => extract_module(ctx, node, parent_id, scope),
        "type_item" => extract_type_alias(ctx, node, parent_id, scope),
        "macro_definition" => extract_macro(ctx, node, parent_id, scope),
        "use_declaration" => extract_use(ctx, node),
        // Reference extraction — capture calls, type refs, field accesses.
        "call_expression" => {
            extract_call_reference(ctx, node);
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                extract_node(ctx, child, parent_id, scope, method_context);
            }
        }
        "field_expression" => {
            extract_field_reference(ctx, node);
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                extract_node(ctx, child, parent_id, scope, method_context);
            }
        }
        "type_identifier" => {
            let name = ctx.node_text(node);
            if !name.is_empty() {
                ctx.references.push(SymbolRef {
                    symbol_name: name.to_string(),
                    file_id: 0,
                    file_path: String::new(),
                    line: node.start_position().row as u32 + 1,
                    col: node.start_position().column as u32,
                    kind: "type".to_string(),
                });
            }
            // type_identifier is a leaf — no children to recurse.
        }
        "macro_invocation" => {
            if let Some(name_node) = node.child_by_field_name("macro") {
                let name = ctx.node_text(name_node);
                if !name.is_empty() {
                    ctx.references.push(SymbolRef {
                        symbol_name: name.to_string(),
                        file_id: 0,
                        file_path: String::new(),
                        line: name_node.start_position().row as u32 + 1,
                        col: name_node.start_position().column as u32,
                        kind: "call".to_string(),
                    });
                }
            }
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                extract_node(ctx, child, parent_id, scope, method_context);
            }
        }
        _ => {
            // Recurse into children for container nodes.
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                extract_node(ctx, child, parent_id, scope, method_context);
            }
        }
    }
}

/// Extract a function/method call reference from a call_expression node.
fn extract_call_reference(ctx: &mut ExtractionContext, node: Node) {
    // call_expression has a "function" field that is the callee.
    if let Some(func_node) = node.child_by_field_name("function") {
        let name = match func_node.kind() {
            "identifier" => ctx.node_text(func_node).to_string(),
            "scoped_identifier" => {
                // e.g. Foo::bar — extract the last segment
                if let Some(name_node) = func_node.child_by_field_name("name") {
                    ctx.node_text(name_node).to_string()
                } else {
                    ctx.node_text(func_node).to_string()
                }
            }
            "field_expression" => {
                // e.g. obj.method() — extract the field name
                if let Some(field_node) = func_node.child_by_field_name("field") {
                    ctx.node_text(field_node).to_string()
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        if !name.is_empty() {
            ctx.references.push(SymbolRef {
                symbol_name: name,
                file_id: 0,
                file_path: String::new(),
                line: func_node.start_position().row as u32 + 1,
                col: func_node.start_position().column as u32,
                kind: "call".to_string(),
            });
        }
    }
}

/// Extract a field access reference from a field_expression node.
fn extract_field_reference(ctx: &mut ExtractionContext, node: Node) {
    if let Some(field_node) = node.child_by_field_name("field") {
        // Skip if the parent is a call_expression (already captured as a call reference).
        if node.parent().is_some_and(|p| p.kind() == "call_expression") {
            return;
        }
        let name = ctx.node_text(field_node);
        if !name.is_empty() {
            ctx.references.push(SymbolRef {
                symbol_name: name.to_string(),
                file_id: 0,
                file_path: String::new(),
                line: field_node.start_position().row as u32 + 1,
                col: field_node.start_position().column as u32,
                kind: "field_access".to_string(),
            });
        }
    }
}

// ── Function / Method extraction ────────────────────────────────────────────

fn extract_function(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
    method_context: bool,
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_visibility(ctx, node);
    let doc_comment = extract_doc_comment(ctx, node);
    let signature = extract_function_signature(ctx, node);
    let is_test = has_test_attribute(ctx, node);

    let kind = if is_test {
        SymbolKind::Test
    } else if method_context {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    };

    let qualified_name = build_qualified_name(scope, &name);
    let body_hash = extract_body_hash(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0, // set by caller
        name,
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
}

// ── Struct extraction ───────────────────────────────────────────────────────

fn extract_struct(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_visibility(ctx, node);
    let doc_comment = extract_doc_comment(ctx, node);
    let qualified_name = build_qualified_name(scope, &name);
    let body_hash = extract_body_hash(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qualified_name),
        kind: SymbolKind::Struct,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(format!("struct {name}")),
        doc_comment,
        body_hash: Some(body_hash),
    });

    // Extract fields from field_declaration_list.
    let new_scope = extend_scope(scope, &name);
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "field_declaration_list" {
            extract_fields(ctx, child, id, &new_scope);
        }
    }
}

fn extract_fields(ctx: &mut ExtractionContext, list_node: Node, parent_id: i64, scope: &[String]) {
    let cursor = &mut list_node.walk();
    for child in list_node.children(cursor) {
        if child.kind() == "field_declaration" {
            let name = child_by_field_text(ctx, child, "name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let visibility = extract_visibility(ctx, child);
            let type_text = child_by_field_text(ctx, child, "type").unwrap_or_default();
            let doc_comment = extract_doc_comment(ctx, child);
            let qualified_name = build_qualified_name(scope, &name);

            let id = ctx.alloc_id();
            ctx.symbols.push(Symbol {
                id,
                file_id: 0,
                name: name.clone(),
                qualified_name: Some(qualified_name),
                kind: SymbolKind::Field,
                visibility,
                start_line: child.start_position().row as u32 + 1,
                end_line: child.end_position().row as u32 + 1,
                start_col: child.start_position().column as u32,
                end_col: child.end_position().column as u32,
                parent_id: Some(parent_id),
                signature: Some(format!("{name}: {type_text}")),
                doc_comment,
                body_hash: None,
            });
        }
    }
}

// ── Enum extraction ─────────────────────────────────────────────────────────

fn extract_enum(ctx: &mut ExtractionContext, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_visibility(ctx, node);
    let doc_comment = extract_doc_comment(ctx, node);
    let qualified_name = build_qualified_name(scope, &name);
    let body_hash = extract_body_hash(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qualified_name),
        kind: SymbolKind::Enum,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(format!("enum {name}")),
        doc_comment,
        body_hash: Some(body_hash),
    });

    // Extract variants.
    let new_scope = extend_scope(scope, &name);
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "enum_variant_list" {
            extract_enum_variants(ctx, child, id, &new_scope);
        }
    }
}

fn extract_enum_variants(
    ctx: &mut ExtractionContext,
    list_node: Node,
    parent_id: i64,
    scope: &[String],
) {
    let cursor = &mut list_node.walk();
    for child in list_node.children(cursor) {
        if child.kind() == "enum_variant" {
            let name = child_by_field_text(ctx, child, "name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let doc_comment = extract_doc_comment(ctx, child);
            let qualified_name = build_qualified_name(scope, &name);

            let id = ctx.alloc_id();
            ctx.symbols.push(Symbol {
                id,
                file_id: 0,
                name,
                qualified_name: Some(qualified_name),
                kind: SymbolKind::EnumVariant,
                visibility: Visibility::Public, // enum variants inherit enum visibility
                start_line: child.start_position().row as u32 + 1,
                end_line: child.end_position().row as u32 + 1,
                start_col: child.start_position().column as u32,
                end_col: child.end_position().column as u32,
                parent_id: Some(parent_id),
                signature: None,
                doc_comment,
                body_hash: None,
            });
        }
    }
}

// ── Trait extraction ────────────────────────────────────────────────────────

fn extract_trait(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_visibility(ctx, node);
    let doc_comment = extract_doc_comment(ctx, node);
    let qualified_name = build_qualified_name(scope, &name);
    let body_hash = extract_body_hash(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qualified_name),
        kind: SymbolKind::Trait,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(format!("trait {name}")),
        doc_comment,
        body_hash: Some(body_hash),
    });

    // Extract trait methods.
    let new_scope = extend_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            extract_node(ctx, child, Some(id), &new_scope, true);
        }
    }
}

// ── Impl extraction ─────────────────────────────────────────────────────────

fn extract_impl(
    ctx: &mut ExtractionContext,
    node: Node,
    _parent_id: Option<i64>,
    scope: &[String],
) {
    // Build impl name from "type" field and optional trait.
    let type_name = child_by_field_text(ctx, node, "type").unwrap_or_default();
    let trait_name = child_by_field_text(ctx, node, "trait");

    let impl_name = match &trait_name {
        Some(t) => format!("{t} for {type_name}"),
        None => type_name.clone(),
    };

    if impl_name.is_empty() {
        return;
    }

    let body_hash = extract_body_hash(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: impl_name.clone(),
        qualified_name: Some(build_qualified_name(scope, &format!("impl {impl_name}"))),
        kind: SymbolKind::Impl,
        visibility: Visibility::Private, // impl blocks don't have visibility
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id: None,
        signature: Some(format!("impl {impl_name}")),
        doc_comment: None,
        body_hash: Some(body_hash),
    });

    // Extract methods inside the impl body.
    let new_scope = extend_scope(scope, &type_name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            extract_node(ctx, child, Some(id), &new_scope, true);
        }
    }
}

// ── Const / Static ──────────────────────────────────────────────────────────

fn extract_const_or_static(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
    kind: SymbolKind,
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_visibility(ctx, node);
    let doc_comment = extract_doc_comment(ctx, node);
    let type_text = child_by_field_text(ctx, node, "type").unwrap_or_default();
    let qualified_name = build_qualified_name(scope, &name);

    let label = if kind == SymbolKind::Constant {
        "const"
    } else {
        "static"
    };

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qualified_name),
        kind,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(format!("{label}: {type_text}")),
        doc_comment,
        body_hash: None,
    });
}

// ── Module ──────────────────────────────────────────────────────────────────

fn extract_module(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_visibility(ctx, node);
    let doc_comment = extract_doc_comment(ctx, node);
    let qualified_name = build_qualified_name(scope, &name);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qualified_name),
        kind: SymbolKind::Module,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(format!("mod {name}")),
        doc_comment,
        body_hash: None,
    });

    // Recurse into module body.
    let new_scope = extend_scope(scope, &name);
    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            extract_node(ctx, child, Some(id), &new_scope, false);
        }
    }
}

// ── Type Alias ──────────────────────────────────────────────────────────────

fn extract_type_alias(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let visibility = extract_visibility(ctx, node);
    let doc_comment = extract_doc_comment(ctx, node);
    let qualified_name = build_qualified_name(scope, &name);
    let full_text = ctx.node_text(node).to_string();

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qualified_name),
        kind: SymbolKind::TypeAlias,
        visibility,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(full_text.trim().to_string()),
        doc_comment,
        body_hash: None,
    });
}

// ── Macro ───────────────────────────────────────────────────────────────────

fn extract_macro(
    ctx: &mut ExtractionContext,
    node: Node,
    parent_id: Option<i64>,
    scope: &[String],
) {
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let doc_comment = extract_doc_comment(ctx, node);
    let qualified_name = build_qualified_name(scope, &name);
    let body_hash = extract_body_hash(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qualified_name),
        kind: SymbolKind::Macro,
        visibility: Visibility::Public, // macros are implicitly pub at crate level
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(format!("macro_rules! {name}")),
        doc_comment,
        body_hash: Some(body_hash),
    });
}

// ── Use / Import ────────────────────────────────────────────────────────────

fn extract_use(ctx: &mut ExtractionContext, node: Node) {
    let full_text = ctx.node_text(node).trim().to_string();
    let line = node.start_position().row as u32 + 1;

    // Parse the use path — strip "use " prefix and ";" suffix.
    let path = full_text
        .strip_prefix("use ")
        .unwrap_or(&full_text)
        .trim_end_matches(';')
        .trim();

    // Handle grouped imports: use std::{io, fs};
    // For simplicity, store the whole path as imported_name and extract the source module.
    let (source_module, imported_name) = if let Some(idx) = path.rfind("::") {
        (
            path[..idx].to_string(),
            path[idx + 2..]
                .trim_matches(|c| c == '{' || c == '}')
                .to_string(),
        )
    } else {
        (String::new(), path.to_string())
    };

    // Check for alias: `use foo::Bar as Baz`
    let (final_name, alias) = if let Some(idx) = imported_name.find(" as ") {
        (
            imported_name[..idx].to_string(),
            Some(imported_name[idx + 4..].to_string()),
        )
    } else {
        (imported_name, None)
    };

    ctx.imports.push(ImportEntry {
        file_id: 0, // set by caller
        imported_name: final_name,
        source_module,
        alias,
        line,
        kind: "use".to_string(),
    });
}

// ── Helper functions ────────────────────────────────────────────────────────

/// Get the text of a named child field.
fn child_by_field_text(ctx: &ExtractionContext, node: Node, field: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| ctx.node_text(n).to_string())
}

/// Extract the visibility modifier from a node.
fn extract_visibility(ctx: &ExtractionContext, node: Node) -> Visibility {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "visibility_modifier" {
            let text = ctx.node_text(child);
            return match text {
                "pub" => Visibility::Public,
                "pub(crate)" => Visibility::PubCrate,
                "pub(super)" => Visibility::PubSuper,
                _ if text.starts_with("pub") => Visibility::Public,
                _ => Visibility::Private,
            };
        }
    }
    Visibility::Private
}

/// Extract doc comments above a node.
///
/// Looks at preceding siblings for `line_comment` nodes starting with `///`
/// or `//!`, and `block_comment` nodes starting with `/**`.
fn extract_doc_comment(ctx: &ExtractionContext, node: Node) -> Option<String> {
    let mut lines = Vec::new();
    let mut sibling = node.prev_sibling();

    while let Some(sib) = sibling {
        match sib.kind() {
            "line_comment" => {
                let text = ctx.node_text(sib);
                if text.starts_with("///") || text.starts_with("//!") {
                    let content = text
                        .trim_start_matches("///")
                        .trim_start_matches("//!")
                        .trim_start();
                    lines.push(content.to_string());
                } else {
                    break; // non-doc comment breaks the chain
                }
            }
            "block_comment" => {
                let text = ctx.node_text(sib);
                if text.starts_with("/**") {
                    let content = text.trim_start_matches("/**").trim_end_matches("*/").trim();
                    lines.push(content.to_string());
                }
                break;
            }
            "attribute_item" | "attribute" => {
                // Skip attributes (e.g. #[derive(...)]) when looking for doc comments
                sibling = sib.prev_sibling();
                continue;
            }
            _ => break,
        }
        sibling = sib.prev_sibling();
    }

    if lines.is_empty() {
        None
    } else {
        lines.reverse(); // they were collected bottom-up
        Some(lines.join("\n"))
    }
}

/// Extract the function signature from a function_item node.
fn extract_function_signature(ctx: &ExtractionContext, node: Node) -> String {
    // Build signature from: visibility + fn + name + parameters + return_type
    let vis = extract_visibility(ctx, node);
    let name = child_by_field_text(ctx, node, "name").unwrap_or_default();
    let params = child_by_field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let return_type = child_by_field_text(ctx, node, "return_type");

    let vis_str = match vis {
        Visibility::Public => "pub ",
        Visibility::PubCrate => "pub(crate) ",
        Visibility::PubSuper => "pub(super) ",
        Visibility::Private => "",
    };

    match return_type {
        Some(rt) => format!("{vis_str}fn {name}{params} -> {rt}"),
        None => format!("{vis_str}fn {name}{params}"),
    }
}

/// Check if a function has a `#[test]` or `#[tokio::test]` attribute.
fn has_test_attribute(ctx: &ExtractionContext, node: Node) -> bool {
    let mut sibling = node.prev_sibling();
    while let Some(sib) = sibling {
        if sib.kind() == "attribute_item" {
            let text = ctx.node_text(sib);
            if text.contains("test") {
                return true;
            }
        } else if sib.kind() != "line_comment" && sib.kind() != "block_comment" {
            break;
        }
        sibling = sib.prev_sibling();
    }
    false
}

/// Build a qualified name from scope segments and a name.
fn build_qualified_name(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", scope.join("::"), name)
    }
}

/// Extend a scope with a new segment.
fn extend_scope(scope: &[String], name: &str) -> Vec<String> {
    let mut new_scope = scope.to_vec();
    new_scope.push(name.to_string());
    new_scope
}

/// Compute a blake3 hash of a node's text for change detection.
fn extract_body_hash(ctx: &ExtractionContext, node: Node) -> String {
    let text = ctx.node_text(node);
    crate::scanner::hash_content(text.as_bytes())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(source: &str) -> ParsedFile {
        let parser = RustParser::new();
        parser.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_simple_function() {
        let parsed = parse_rust("fn hello() { println!(\"hi\"); }");
        assert_eq!(parsed.symbols.len(), 1);
        assert_eq!(parsed.symbols[0].name, "hello");
        assert_eq!(parsed.symbols[0].kind, SymbolKind::Function);
        assert_eq!(parsed.symbols[0].visibility, Visibility::Private);
    }

    #[test]
    fn test_pub_function_with_return() {
        let parsed = parse_rust("pub fn add(a: i32, b: i32) -> i32 { a + b }");
        assert_eq!(parsed.symbols.len(), 1);
        let sym = &parsed.symbols[0];
        assert_eq!(sym.name, "add");
        assert_eq!(sym.visibility, Visibility::Public);
        assert!(sym.signature.as_ref().unwrap().contains("-> i32"));
        assert!(sym.signature.as_ref().unwrap().contains("pub fn add"));
    }

    #[test]
    fn test_struct_with_fields() {
        let source = r#"
pub struct Config {
    pub name: String,
    value: i32,
}
"#;
        let parsed = parse_rust(source);
        let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Config"), "got: {names:?}");
        assert!(names.contains(&"name"), "got: {names:?}");
        assert!(names.contains(&"value"), "got: {names:?}");

        let config = parsed.symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config.kind, SymbolKind::Struct);
        assert_eq!(config.visibility, Visibility::Public);

        let name_field = parsed.symbols.iter().find(|s| s.name == "name").unwrap();
        assert_eq!(name_field.kind, SymbolKind::Field);
        assert_eq!(name_field.visibility, Visibility::Public);
        assert!(name_field.parent_id.is_some());
    }

    #[test]
    fn test_enum_with_variants() {
        let source = r#"
pub enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let parsed = parse_rust(source);
        let names: Vec<&str> = parsed.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Color"));
        assert!(names.contains(&"Red"));
        assert!(names.contains(&"Green"));
        assert!(names.contains(&"Blue"));

        let red = parsed.symbols.iter().find(|s| s.name == "Red").unwrap();
        assert_eq!(red.kind, SymbolKind::EnumVariant);
    }

    #[test]
    fn test_trait_with_methods() {
        let source = r#"
pub trait Greet {
    fn greet(&self) -> String;
}
"#;
        let parsed = parse_rust(source);
        let greet_trait = parsed.symbols.iter().find(|s| s.name == "Greet").unwrap();
        assert_eq!(greet_trait.kind, SymbolKind::Trait);

        let greet_fn = parsed.symbols.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(greet_fn.kind, SymbolKind::Method);
        assert_eq!(greet_fn.parent_id, Some(greet_trait.id));
    }

    #[test]
    fn test_impl_block_with_methods() {
        let source = r#"
struct Foo;

impl Foo {
    pub fn new() -> Self { Foo }
    fn helper(&self) {}
}
"#;
        let parsed = parse_rust(source);
        let impl_sym = parsed
            .symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Impl)
            .unwrap();
        assert!(impl_sym.name.contains("Foo"));

        let new_fn = parsed.symbols.iter().find(|s| s.name == "new").unwrap();
        assert_eq!(new_fn.kind, SymbolKind::Method);
        assert_eq!(new_fn.parent_id, Some(impl_sym.id));
        assert_eq!(new_fn.visibility, Visibility::Public);

        let helper = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(helper.kind, SymbolKind::Method);
        assert_eq!(helper.visibility, Visibility::Private);
    }

    #[test]
    fn test_trait_impl() {
        let source = r#"
struct Foo;

impl Display for Foo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Foo")
    }
}
"#;
        let parsed = parse_rust(source);
        let impl_sym = parsed
            .symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Impl)
            .unwrap();
        assert!(
            impl_sym.name.contains("Display") && impl_sym.name.contains("Foo"),
            "impl name should contain trait and type: {}",
            impl_sym.name
        );
    }

    #[test]
    fn test_const_and_static() {
        let source = r#"
pub const MAX_SIZE: usize = 1024;
static COUNTER: i32 = 0;
"#;
        let parsed = parse_rust(source);
        let max_size = parsed
            .symbols
            .iter()
            .find(|s| s.name == "MAX_SIZE")
            .unwrap();
        assert_eq!(max_size.kind, SymbolKind::Constant);
        assert_eq!(max_size.visibility, Visibility::Public);

        let counter = parsed.symbols.iter().find(|s| s.name == "COUNTER").unwrap();
        assert_eq!(counter.kind, SymbolKind::Static);
    }

    #[test]
    fn test_module() {
        let source = r#"
pub mod utils {
    pub fn helper() {}
}
"#;
        let parsed = parse_rust(source);
        let module = parsed.symbols.iter().find(|s| s.name == "utils").unwrap();
        assert_eq!(module.kind, SymbolKind::Module);

        let helper = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(helper.kind, SymbolKind::Function);
        assert_eq!(helper.parent_id, Some(module.id));
        assert_eq!(helper.qualified_name.as_deref(), Some("utils::helper"));
    }

    #[test]
    fn test_type_alias() {
        let source = "pub type Result<T> = std::result::Result<T, Error>;";
        let parsed = parse_rust(source);
        assert_eq!(parsed.symbols.len(), 1);
        assert_eq!(parsed.symbols[0].name, "Result");
        assert_eq!(parsed.symbols[0].kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_macro_definition() {
        let source = r#"
macro_rules! my_macro {
    () => {};
}
"#;
        let parsed = parse_rust(source);
        let mac = parsed
            .symbols
            .iter()
            .find(|s| s.name == "my_macro")
            .unwrap();
        assert_eq!(mac.kind, SymbolKind::Macro);
    }

    #[test]
    fn test_use_statements() {
        let source = r#"
use std::collections::HashMap;
use std::io::{self, Write};
use crate::config::Config as AppConfig;
"#;
        let parsed = parse_rust(source);
        assert!(
            parsed.imports.len() >= 2,
            "got {} imports",
            parsed.imports.len()
        );

        let hashmap = parsed
            .imports
            .iter()
            .find(|i| i.imported_name.contains("HashMap"))
            .unwrap();
        assert_eq!(hashmap.source_module, "std::collections");
    }

    #[test]
    fn test_test_function() {
        let source = r#"
#[test]
fn test_something() {
    assert!(true);
}

#[tokio::test]
async fn test_async() {
    assert!(true);
}
"#;
        let parsed = parse_rust(source);
        let test1 = parsed
            .symbols
            .iter()
            .find(|s| s.name == "test_something")
            .unwrap();
        assert_eq!(test1.kind, SymbolKind::Test);

        let test2 = parsed
            .symbols
            .iter()
            .find(|s| s.name == "test_async")
            .unwrap();
        assert_eq!(test2.kind, SymbolKind::Test);
    }

    #[test]
    fn test_doc_comments() {
        let source = r#"
/// This is a documented function.
/// It does cool things.
pub fn documented() {}
"#;
        let parsed = parse_rust(source);
        let sym = parsed
            .symbols
            .iter()
            .find(|s| s.name == "documented")
            .unwrap();
        let doc = sym.doc_comment.as_ref().unwrap();
        assert!(doc.contains("documented function"), "doc: {doc}");
        assert!(doc.contains("cool things"), "doc: {doc}");
    }

    #[test]
    fn test_line_numbers() {
        let source = "fn first() {}\nfn second() {}\nfn third() {}\n";
        let parsed = parse_rust(source);

        let first = parsed.symbols.iter().find(|s| s.name == "first").unwrap();
        assert_eq!(first.start_line, 1);

        let second = parsed.symbols.iter().find(|s| s.name == "second").unwrap();
        assert_eq!(second.start_line, 2);

        let third = parsed.symbols.iter().find(|s| s.name == "third").unwrap();
        assert_eq!(third.start_line, 3);
    }

    #[test]
    fn test_qualified_names_nested() {
        let source = r#"
mod outer {
    mod inner {
        fn deep() {}
    }
}
"#;
        let parsed = parse_rust(source);
        let deep = parsed.symbols.iter().find(|s| s.name == "deep").unwrap();
        assert_eq!(deep.qualified_name.as_deref(), Some("outer::inner::deep"));
    }

    #[test]
    fn test_body_hash_changes() {
        let source1 = "fn foo() { 1 + 1 }";
        let source2 = "fn foo() { 2 + 2 }";
        let p1 = parse_rust(source1);
        let p2 = parse_rust(source2);

        assert_ne!(
            p1.symbols[0].body_hash, p2.symbols[0].body_hash,
            "body hash should change when body changes"
        );
    }

    #[test]
    fn test_empty_source() {
        let parsed = parse_rust("");
        assert!(parsed.symbols.is_empty());
        assert!(parsed.imports.is_empty());
    }

    #[test]
    fn test_complex_real_world() {
        let source = r#"
//! Module documentation.

use std::collections::HashMap;
use std::sync::Arc;

/// A configuration manager.
pub struct ConfigManager {
    /// The internal storage.
    pub store: HashMap<String, String>,
    cache: Vec<u8>,
}

impl ConfigManager {
    /// Create a new config manager.
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            cache: Vec::new(),
        }
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.store.get(key)
    }

    fn internal_helper(&self) {}
}

pub enum Status {
    Active,
    Inactive,
    Pending,
}

pub trait Loadable {
    fn load(&self) -> Result<(), String>;
}

impl Loadable for ConfigManager {
    fn load(&self) -> Result<(), String> {
        Ok(())
    }
}

pub const MAX_ITEMS: usize = 100;

pub type Config = ConfigManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let cm = ConfigManager::new();
        assert!(cm.store.is_empty());
    }
}
"#;
        let parsed = parse_rust(source);

        // Count by kind.
        let count = |k: SymbolKind| parsed.symbols.iter().filter(|s| s.kind == k).count();

        assert!(
            count(SymbolKind::Struct) >= 1,
            "should find ConfigManager struct"
        );
        assert!(count(SymbolKind::Enum) >= 1, "should find Status enum");
        assert!(count(SymbolKind::Trait) >= 1, "should find Loadable trait");
        assert!(count(SymbolKind::Impl) >= 2, "should find 2 impl blocks");
        assert!(count(SymbolKind::Constant) >= 1, "should find MAX_ITEMS");
        assert!(
            count(SymbolKind::TypeAlias) >= 1,
            "should find Config type alias"
        );
        assert!(count(SymbolKind::Test) >= 1, "should find test_new");
        assert!(count(SymbolKind::Field) >= 2, "should find struct fields");
        assert!(count(SymbolKind::Module) >= 1, "should find tests module");

        // Verify imports.
        assert!(parsed.imports.len() >= 2, "should have at least 2 imports");
    }
}
