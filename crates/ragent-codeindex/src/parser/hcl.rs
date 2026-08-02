//! HCL / Terraform language parser using tree-sitter.
//!
//! Extracts resource blocks, data block, module calls, variables, locals,
//! outputs, provider blocks, and terraform settings blocks from HCL-based
//! Terraform configuration files (`.tf` / `.tfvars`).
//!
//! Terraform files use HCL (`HashiCorp` Configuration Language). The tree-sitter
//! HCL grammar produces a `body` → `block` / `attribute` tree. Each `block`
//! has the form:
//!
//! ```text
//! <identifier> <string_lit|identifier> … <block_start> <body>? <block_end>
//! ```
//!
//! The first `identifier` child is the block type (e.g. `resource`, `data`,
//! `module`, `variable`). Subsequent `string_lit` / `identifier` children are
//! the block labels. The `body` child contains `attribute` and nested `block`
//! nodes.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::Node;

/// Tree-sitter parser for HCL / Terraform configurations.
pub struct HclParser {
    _private: (),
}

impl HclParser {
    /// Create a new HCL parser.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }

    super::util::tree_sitter_parser!(
        tree_sitter_hcl::LANGUAGE,
        "failed to load HCL grammar",
        "tree-sitter parse returned None for HCL source"
    );
}

impl LanguageParser for HclParser {
    fn language_id(&self) -> &'static str {
        "terraform"
    }

    fn parse(&self, source: &[u8]) -> Result<ParsedFile> {
        let tree = Self::parse_tree(source)?;
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

// ── Extraction context ──────────────────────────────────────────────────────

/// Mutable context threaded through recursive extraction.
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

// ── HCL block types and their mapping to SymbolKinds ────────────────────────

/// Map an HCL block type string to the appropriate [`SymbolKind`].
fn block_kind(block_type: &str) -> SymbolKind {
    match block_type {
        "resource" => SymbolKind::Class,
        "data" => SymbolKind::Class,
        "module" => SymbolKind::Module,
        "variable" => SymbolKind::Constant,
        "locals" => SymbolKind::Module,
        "output" => SymbolKind::Field,
        "provider" => SymbolKind::Module,
        "terraform" => SymbolKind::Module,
        "backend" => SymbolKind::Module,
        "moved" => SymbolKind::Field,
        "removed" => SymbolKind::Field,
        "lifecycle" => SymbolKind::Module,
        "import" => SymbolKind::Import,
        _ => SymbolKind::Unknown,
    }
}

// ── Recursive walk ─────────────────────────────────────────────────────────

/// Walk a tree-sitter node, extracting HCL/Terraform symbols.
fn walk(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    match node.kind() {
        "block" => extract_block(ctx, node, parent_id, scope),
        "attribute" => extract_attribute(ctx, node, parent_id, scope),
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent_id, scope);
            }
        }
    }
}

// ── Block extraction ────────────────────────────────────────────────────────

/// Extract an HCL block.
///
/// HCL block structure in tree-sitter:
///
/// ```text
/// block → identifier (string_lit|identifier)* block_start body? block_end
/// ```
///
/// The first `identifier` is the block type. Subsequent `string_lit` and
/// `identifier` nodes are labels. The `body` child holds attributes and
/// nested blocks.
fn extract_block(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let cursor = &mut node.walk();
    let children: Vec<Node> = node.children(cursor).collect();

    // First identifier child is the block type.
    let block_type = children
        .iter()
        .find(|c| c.kind() == "identifier")
        .map(|c| ctx.text(*c).to_string())
        .unwrap_or_default();

    if block_type.is_empty() {
        return;
    }

    // Collect label texts: string_lit and subsequent identifier children.
    let mut labels: Vec<String> = Vec::new();
    let mut seen_type = false;
    for child in &children {
        if child.kind() == "identifier" {
            if seen_type {
                // Additional identifier labels (rare but valid in HCL).
                labels.push(ctx.text(*child).to_string());
            } else {
                // This is the block type — skip.
                seen_type = true;
            }
        } else if child.kind() == "string_lit" {
            // Strip surrounding quotes from string_lit labels.
            let raw = ctx.text(*child);
            let cleaned = raw.trim_matches('"').to_string();
            labels.push(cleaned);
        }
    }

    // Build the display name.
    // e.g. resource "aws_instance" "web" → resource.aws_instance.web
    let name = if labels.is_empty() {
        block_type.clone()
    } else {
        format!("{}.{}", block_type, labels.join("."))
    };

    let kind = block_kind(&block_type);

    // Build a signature: block_type "label1" "label2"
    let sig = if labels.is_empty() {
        block_type.clone()
    } else {
        let label_str: Vec<String> = labels.iter().map(|l| format!("\"{l}\"")).collect();
        format!("{} {}", block_type, label_str.join(" "))
    };

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });

    // Recurse into the block body for nested blocks and attributes.
    let inner_scope = ext_scope(scope, &block_type);
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, Some(id), &inner_scope);
    }
}

// ── Attribute extraction (`key = value`) ────────────────────────────────────

/// Extract an HCL attribute.
///
/// An `attribute` node has children: `identifier` followed by `expression`.
/// Attributes inside `locals` blocks are treated as constants; all others
/// are treated as fields.
fn extract_attribute(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = first_child_by_kind(ctx, node, "identifier").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let kind = if is_inside_locals_block(node, ctx.source) {
        SymbolKind::Constant
    } else {
        SymbolKind::Field
    };

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);
    let sig = ctx.text(node).lines().next().unwrap_or("").to_string();

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind,
        visibility: Visibility::Public,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Check whether a node is inside a `locals` block by walking ancestors.
fn is_inside_locals_block(node: Node, source: &[u8]) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "block" {
            let cursor = &mut parent.walk();
            for child in parent.children(cursor) {
                if child.kind() == "identifier" {
                    let text = child.utf8_text(source).unwrap_or("");
                    return text == "locals";
                }
            }
        }
        current = parent.parent();
    }
    false
}

/// Get the text of the first child node matching `kind`.
fn first_child_by_kind(ctx: &Ctx, node: Node, kind: &str) -> Option<String> {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == kind {
            return Some(ctx.text(child).to_string());
        }
    }
    None
}

/// Build a qualified name from the current scope and a local name.
///
/// Delegates to [`super::util::build_qname`] with the `:` separator.
#[inline]
fn build_qname(scope: &[String], name: &str) -> String {
    super::util::build_qname(scope, name, ":")
}

/// Extend the scope with one more level.
fn ext_scope(scope: &[String], name: &str) -> Vec<String> {
    let mut v = scope.to_vec();
    v.push(name.to_string());
    v
}

/// Compute a blake3 hash of the node's text for change detection.
fn hash_node(ctx: &Ctx, node: Node) -> String {
    let text = ctx.text(node);
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

// ── Tests ────────────────────────────────────────────���──────────────────────
