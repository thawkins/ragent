//! CMake language parser using tree-sitter.
//!
//! Extracts function definitions, macro definitions, block definitions,
//! foreach/while loops, if-conditions, and normal commands from CMake
//! listfiles (`.cmake` and `CMakeLists.txt`).
//!
//! CMake's language is command-based. The tree-sitter grammar represents
//! top-level constructs as `function_def`, `macro_def`, `block_def`,
//! `foreach_loop`, `while_loop`, `if_condition`, and `normal_command`.
//! Each `*_command` child contains an `identifier` (the command name) and
//! an `argument_list`. Definition nodes (`function_def`, `macro_def`)
//! contain a `body` child with nested statements.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

/// Tree-sitter parser for the CMake build language.
pub struct CmakeParser {
    _private: (),
}

impl CmakeParser {
    /// Create a new CMake parser.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a tree-sitter parser configured for CMake.
    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        let language = tree_sitter_cmake::LANGUAGE;
        parser
            .set_language(&language.into())
            .context("failed to load CMake grammar")?;
        Ok(parser)
    }

    /// Parse source code into a tree-sitter Tree.
    fn parse_tree(source: &[u8]) -> Result<Tree> {
        let mut parser = Self::create_parser()?;
        parser
            .parse(source, None)
            .context("tree-sitter parse returned None for CMake source")
    }
}

impl LanguageParser for CmakeParser {
    fn language_id(&self) -> &'static str {
        "cmake"
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

        walk(&mut ctx, root, None);

        Ok(ParsedFile {
            symbols: ctx.symbols,
            imports: ctx.imports,
            references: ctx.references,
            tree: Some(tree),
        })
    }
}

// ── Extraction context ────��─────────────────────────────────────────────────

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

/// Walk a tree-sitter node, extracting CMake symbols.
fn walk(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    match node.kind() {
        "function_def" => extract_function_def(ctx, node, parent_id),
        "macro_def" => extract_macro_def(ctx, node, parent_id),
        "block_def" => extract_block_def(ctx, node, parent_id),
        "foreach_loop" => extract_foreach_loop(ctx, node, parent_id),
        "while_loop" => extract_while_loop(ctx, node, parent_id),
        "if_condition" => extract_if_condition(ctx, node, parent_id),
        "normal_command" => extract_normal_command(ctx, node, parent_id),
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent_id);
            }
        }
    }
}

// ── Function definition (`function(name …) … endfunction()`) ────────────────

/// Extract a CMake `function()` definition.
///
/// A `function_def` contains a `function_command` child whose first
/// argument is the function name, and a `body` child with nested statements.
fn extract_function_def(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    let cmd = find_child(node, "function_command");
    let name = cmd
        .and_then(|c| first_argument_text(ctx, c))
        .unwrap_or_default();

    if name.is_empty() {
        return;
    }

    let args = cmd
        .and_then(|c| argument_list_text(ctx, c))
        .unwrap_or_default();
    let sig = format!("function({args})");
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: None,
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

    // Recurse into the body.
    if let Some(body) = find_child(node, "body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id));
        }
    }
}

// ── Macro definition (`macro(name …) … endmacro()`) ─────────────────────────

/// Extract a CMake `macro()` definition.
fn extract_macro_def(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    let cmd = find_child(node, "macro_command");
    let name = cmd
        .and_then(|c| first_argument_text(ctx, c))
        .unwrap_or_default();

    if name.is_empty() {
        return;
    }

    let args = cmd
        .and_then(|c| argument_list_text(ctx, c))
        .unwrap_or_default();
    let sig = format!("macro({args})");
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name,
        qualified_name: None,
        kind: SymbolKind::Macro,
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

    if let Some(body) = find_child(node, "body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id));
        }
    }
}

// ── Block definition (`block() … endblock()`) ───────────────────────────────

/// Extract a CMake `block()` scope definition (CMake 3.25+).
fn extract_block_def(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    let cmd = find_child(node, "block_command");
    let args = cmd
        .and_then(|c| argument_list_text(ctx, c))
        .unwrap_or_default();
    let sig = format!("block({args})");
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: "block".to_string(),
        qualified_name: None,
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

    if let Some(body) = find_child(node, "body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id));
        }
    }
}

// ── Foreach loop ───────────────────────────────────────────────────────────

/// Extract a CMake `foreach()` loop.
fn extract_foreach_loop(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    let cmd = find_child(node, "foreach_command");
    let args = cmd
        .and_then(|c| argument_list_text(ctx, c))
        .unwrap_or_default();
    let sig = format!("foreach({args})");
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: "foreach".to_string(),
        qualified_name: None,
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

    if let Some(body) = find_child(node, "body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id));
        }
    }
}

// ── While loop ─────────────────────────────────────────────────────────────

/// Extract a CMake `while()` loop.
fn extract_while_loop(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    let cmd = find_child(node, "while_command");
    let args = cmd
        .and_then(|c| argument_list_text(ctx, c))
        .unwrap_or_default();
    let sig = format!("while({args})");
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: "while".to_string(),
        qualified_name: None,
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

    if let Some(body) = find_child(node, "body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id));
        }
    }
}

// ── If condition ───────────────────────────────────────────────────────────

/// Extract a CMake `if()` conditional.
fn extract_if_condition(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    let cmd = find_child(node, "if_command");
    let args = cmd
        .and_then(|c| argument_list_text(ctx, c))
        .unwrap_or_default();
    let sig = format!("if({args})");
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: "if".to_string(),
        qualified_name: None,
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

    if let Some(body) = find_child(node, "body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            walk(ctx, child, Some(id));
        }
    }
}

// ── Normal command ─────────────────────────────────────────────────────────

/// Extract a CMake normal command as a call reference.
///
/// Commands like `add_library`, `target_link_libraries`, `find_package`,
/// `include()`, `add_subdirectory()` are recorded as references. The
/// `include()` and `add_subdirectory()` commands are also treated as imports.
fn extract_normal_command(ctx: &mut Ctx, node: Node, _parent_id: Option<i64>) {
    let ident = find_child(node, "identifier");
    let cmd_name = ident.map(|n| ctx.text(n).to_string()).unwrap_or_default();

    if cmd_name.is_empty() {
        return;
    }

    // Record as a call reference.
    ctx.references.push(SymbolRef {
        symbol_name: cmd_name.clone(),
        file_id: 0,
        file_path: String::new(),
        line: node.start_position().row as u32 + 1,
        col: node.start_position().column as u32,
        kind: "call".to_string(),
    });

    // `include()` and `add_subdirectory()` are import-like.
    if cmd_name == "include" || cmd_name == "add_subdirectory" {
        let arg = first_argument_text(ctx, node).unwrap_or_default();
        if !arg.is_empty() {
            ctx.imports.push(ImportEntry {
                file_id: 0,
                imported_name: arg.clone(),
                source_module: arg,
                alias: None,
                line: node.start_position().row as u32 + 1,
                kind: cmd_name,
            });
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Find the first child of a node matching the given kind.
fn find_child<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let cursor = &mut node.walk();
    node.children(cursor).find(|&child| child.kind() == kind)
}

/// Get the text of the first argument in a command's argument_list.
fn first_argument_text(ctx: &Ctx, cmd_node: Node<'_>) -> Option<String> {
    let args = find_child(cmd_node, "argument_list")?;
    let cursor = &mut args.walk();
    for child in args.children(cursor) {
        // The first named child in an argument_list is the first argument.
        if child.is_named() && child.kind() != "bracket_comment" {
            return Some(ctx.text(child).to_string());
        }
    }
    None
}

/// Get the full text of a command's argument_list (without parentheses).
fn argument_list_text(ctx: &Ctx, cmd_node: Node) -> Option<String> {
    let args = find_child(cmd_node, "argument_list")?;
    Some(ctx.text(args).to_string())
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

    fn parse_cmake(source: &str) -> ParsedFile {
        let parser = CmakeParser::new();
        parser.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_function_def() {
        let src = r#"
function(add_custom target)
  message(STATUS "Building ${target}")
endfunction()
"#;
        let pf = parse_cmake(src);
        let funcs: Vec<_> = pf
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "add_custom");
    }

    #[test]
    fn test_macro_def() {
        let src = r#"
macro(assert_test name)
  message("Testing ${name}")
endmacro()
"#;
        let pf = parse_cmake(src);
        let macros: Vec<_> = pf
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Macro)
            .collect();
        assert_eq!(macros.len(), 1);
        assert_eq!(macros[0].name, "assert_test");
    }

    #[test]
    fn test_normal_command_reference() {
        let src = "add_library(mylib STATIC src.cpp)";
        let pf = parse_cmake(src);
        assert!(!pf.references.is_empty());
        assert_eq!(pf.references[0].symbol_name, "add_library");
        assert_eq!(pf.references[0].kind, "call");
    }

    #[test]
    fn test_include_import() {
        let src = "include(GNUInstallDirs)";
        let pf = parse_cmake(src);
        assert!(!pf.imports.is_empty());
        assert_eq!(pf.imports[0].kind, "include");
    }

    #[test]
    fn test_add_subdirectory_import() {
        let src = "add_subdirectory(libs)";
        let pf = parse_cmake(src);
        assert!(!pf.imports.is_empty());
        assert_eq!(pf.imports[0].kind, "add_subdirectory");
    }

    #[test]
    fn test_foreach_loop() {
        let src = r#"
foreach(item IN ITEMS a b c)
  message(${item})
endforeach()
"#;
        let pf = parse_cmake(src);
        let loops: Vec<_> = pf.symbols.iter().filter(|s| s.name == "foreach").collect();
        assert_eq!(loops.len(), 1);
    }

    #[test]
    fn test_empty_source() {
        let pf = parse_cmake("");
        assert!(pf.symbols.is_empty());
        assert!(pf.imports.is_empty());
    }
}
