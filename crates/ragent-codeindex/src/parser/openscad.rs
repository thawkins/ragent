//! OpenSCAD language parser using tree-sitter.
//!
//! Extracts modules, functions, variable declarations, include/use statements,
//! and parameter lists from OpenSCAD (`.scad`) source files.
//!
//! OpenSCAD is a declarative CAD scripting language. Its main code units are
//! **modules** (imperative geometry generators) and **functions** (pure
//! expressions that return values). Both can accept parameters. Variables are
//! declared with `=`, and files are pulled in via `include <…>` or `use <…>`.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

/// Tree-sitter parser for the OpenSCAD language.
pub struct OpenScadParser {
    _private: (),
}

impl OpenScadParser {
    /// Create a new OpenSCAD parser.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a tree-sitter parser configured for OpenSCAD.
    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        let language = tree_sitter_openscad::LANGUAGE;
        parser
            .set_language(&language.into())
            .context("failed to load OpenSCAD grammar")?;
        Ok(parser)
    }

    /// Parse source code into a tree-sitter Tree.
    fn parse_tree(source: &[u8]) -> Result<Tree> {
        let mut parser = Self::create_parser()?;
        parser
            .parse(source, None)
            .context("tree-sitter parse returned None for OpenSCAD source")
    }
}

impl LanguageParser for OpenScadParser {
    fn language_id(&self) -> &'static str {
        "openscad"
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

// ── Extraction context ────────────────────────────────────���─────────────────

/// Mutable context threaded through recursive extraction.
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

    fn text(&self, node: Node) -> &str {
        node.utf8_text(self.source).unwrap_or("")
    }
}

// ── Recursive walk ──────────────────────────────────────────────────────────

/// Walk a tree-sitter node, extracting OpenSCAD symbols and imports.
fn walk(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    match node.kind() {
        "module_item" => extract_module_item(ctx, node, parent_id, scope),
        "function_item" => extract_function_item(ctx, node, parent_id, scope),
        "module_call" => extract_module_call_ref(ctx, node),
        "function_call" => extract_function_call_ref(ctx, node),
        "var_declaration" => extract_var_declaration(ctx, node, parent_id, scope),
        "include_statement" | "use_statement" => extract_include_use(ctx, node),
        "assignment" => extract_assignment(ctx, node, parent_id, scope),
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent_id, scope);
            }
        }
    }
}

// ── Module item (`module <name>(…) { … }`) ──────────────────────────────────

/// Extract an OpenSCAD module definition.
///
/// Tree-sitter node fields: `name` (identifier), `parameters`, `body` (statement).
fn extract_module_item(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_child_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let params = field_child_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let sig = format!("module {name}{params}");
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

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
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });

    // Recurse into the module body for nested definitions.
    let inner_scope = ext_scope(scope, &name);
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, Some(id), &inner_scope);
    }
}

// ── Function item (`function <name>(…) = …;`) ──────────────────────────────

/// Extract an OpenSCAD function definition.
///
/// Tree-sitter node fields: `name` (identifier), `parameters`.
fn extract_function_item(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_child_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let params = field_child_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let sig = format!("function {name}{params}");
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
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });
}

// ── Variable declaration (`var_declaration` wrapping `assignment`) ──────────

/// Extract an OpenSCAD top-level variable declaration.
///
/// A `var_declaration` contains a single `assignment` child, which has fields
/// `name` (identifier or special_variable) and `value` (expression).
fn extract_var_declaration(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    // The var_declaration node wraps an assignment — delegate to assignment
    // extraction by finding the assignment child.
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "assignment" {
            extract_assignment(ctx, child, parent_id, scope);
            return;
        }
    }
}

// ── Assignment (`name = expr;`) ────────────────────────────────────────────

/// Extract an OpenSCAD assignment as a constant symbol.
///
/// Tree-sitter node fields: `name` (identifier or special_variable), `value` (expression).
fn extract_assignment(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_child_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    // OpenSCAD special variables ($fn, $fa, $fs, etc.) start with '$' — skip.
    if name.starts_with('$') {
        return;
    }

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);
    let sig = ctx.text(node).lines().next().unwrap_or("").to_string();

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: Some(qname),
        kind: SymbolKind::Constant,
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

// ── Include / use statements ───────────────────────────────────────────────

/// Extract an OpenSCAD include or use statement.
///
/// Both `include_statement` and `use_statement` contain an `include_path` child.
fn extract_include_use(ctx: &mut Ctx, node: Node) {
    let path_text = child_text_by_kind(ctx, node, "include_path").unwrap_or_default();

    if path_text.is_empty() {
        return;
    }

    // Strip surrounding angle brackets or quotes if present.
    let clean_path = path_text
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_matches('"');

    let kind = if node.kind() == "include_statement" {
        "include"
    } else {
        "use"
    };

    ctx.imports.push(ImportEntry {
        file_id: 0,
        imported_name: clean_path.to_string(),
        source_module: clean_path.to_string(),
        alias: None,
        line: node.start_position().row as u32 + 1,
        kind: kind.to_string(),
    });
}

// ── Module call reference ───────────────────────────────────────────────────

/// Record a module call as a symbol reference (e.g. `sphere(r=10);`).
///
/// Tree-sitter node fields: `name` (identifier), `arguments`.
fn extract_module_call_ref(ctx: &mut Ctx, node: Node) {
    let name = field_child_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    ctx.references.push(SymbolRef {
        symbol_name: name,
        file_id: 0,
        file_path: String::new(),
        line: node.start_position().row as u32 + 1,
        col: node.start_position().column as u32,
        kind: "call".to_string(),
    });

    // Recurse into the module call body (children may contain more calls).
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, None, &[]);
    }
}

// ── Function call reference ────────────────────────────────────────────────

/// Record a function call as a symbol reference (e.g. `sin(45)`).
///
/// Tree-sitter node fields: `name` (expression), `arguments`.
/// Note: the `name` field can be an expression, so we extract the first
/// identifier child from it.
fn extract_function_call_ref(ctx: &mut Ctx, node: Node) {
    // The name field may be an expression node, so we look for the
    // first identifier within it.
    let name_node = node.child_by_field_name("name");
    let name = if let Some(n) = name_node {
        if n.kind() == "identifier" {
            ctx.text(n).to_string()
        } else {
            // Expression node — find the first identifier child.
            child_text_by_kind(ctx, n, "identifier").unwrap_or_default()
        }
    } else {
        String::new()
    };

    if name.is_empty() {
        return;
    }

    ctx.references.push(SymbolRef {
        symbol_name: name,
        file_id: 0,
        file_path: String::new(),
        line: node.start_position().row as u32 + 1,
        col: node.start_position().column as u32,
        kind: "call".to_string(),
    });
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Get the text of a named field child from a tree-sitter node.
fn field_child_text(ctx: &Ctx, node: Node, field: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(ctx.text(child).to_string())
}

/// Get the text of the first child node matching `kind`.
fn child_text_by_kind(ctx: &Ctx, node: Node, kind: &str) -> Option<String> {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == kind {
            return Some(ctx.text(child).to_string());
        }
    }
    None
}

/// Build a qualified name from the current scope and a local name.
fn build_qname(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", scope.join("::"), name)
    }
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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_scad(source: &str) -> ParsedFile {
        let parser = OpenScadParser::new();
        parser.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_module_item() {
        let src = "module bracket(size=10) { cube(size); }";
        let pf = parse_scad(src);
        // Should have the module definition plus the cube call inside creates
        // a module_call reference, but may also produce an assignment symbol.
        assert!(!pf.symbols.is_empty());
        let module = pf.symbols.iter().find(|s| s.name == "bracket").unwrap();
        assert_eq!(module.kind, SymbolKind::Module);
        assert_eq!(module.start_line, 1);
    }

    #[test]
    fn test_function_item() {
        let src = "function deg2rad(d) = d * 180 / PI;";
        let pf = parse_scad(src);
        assert_eq!(pf.symbols.len(), 1);
        let s = &pf.symbols[0];
        assert_eq!(s.name, "deg2rad");
        assert_eq!(s.kind, SymbolKind::Function);
    }

    #[test]
    fn test_var_declaration() {
        let src = "tolerance = 0.2;";
        let pf = parse_scad(src);
        assert_eq!(pf.symbols.len(), 1);
        let s = &pf.symbols[0];
        assert_eq!(s.name, "tolerance");
        assert_eq!(s.kind, SymbolKind::Constant);
    }

    #[test]
    fn test_include_use() {
        let src = r#"include <MCAD/stepper.scad>
use <utils.scad>"#;
        let pf = parse_scad(src);
        assert_eq!(pf.imports.len(), 2);
        assert_eq!(pf.imports[0].kind, "include");
        assert_eq!(pf.imports[1].kind, "use");
    }

    #[test]
    fn test_module_call_reference() {
        let src = "sphere(r=10);";
        let pf = parse_scad(src);
        assert!(!pf.references.is_empty());
        assert_eq!(pf.references[0].symbol_name, "sphere");
        assert_eq!(pf.references[0].kind, "call");
    }

    #[test]
    fn test_nested_module() {
        let src = r#"
module housing() {
    difference() {
        cube(20, center=true);
        sphere(r=9);
    }
}
"#;
        let pf = parse_scad(src);
        // housing module + references for difference, cube, sphere
        assert!(!pf.symbols.is_empty());
        assert_eq!(pf.symbols[0].name, "housing");
        assert!(!pf.references.is_empty());
    }

    #[test]
    fn test_special_variable_skipped() {
        // $fn, $fa, $fs are OpenSCAD special variables — should not be
        // extracted as user-defined constants.
        let src = "$fn = 64;";
        let pf = parse_scad(src);
        assert!(pf.symbols.is_empty());
    }

    #[test]
    fn test_empty_source() {
        let pf = parse_scad("");
        assert!(pf.symbols.is_empty());
        assert!(pf.imports.is_empty());
    }
}
