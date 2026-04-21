//! Gradle Groovy DSL language parser using tree-sitter.
//!
//! Extracts classes, methods, function definitions, imports, closures,
//! method invocations, and variable declarations from Gradle Groovy DSL
//! build scripts (`.gradle`).
//!
//! The tree-sitter Groovy grammar is very similar to Java's. Build scripts
//! use a DSL that is essentially Groovy with Gradle-specific method calls
//! like `plugins {}`, `dependencies {}`, `android {}`, etc. These appear
//! as `method_invocation` or `juxt_function_call` nodes in the AST.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

/// Tree-sitter parser for the Gradle Groovy DSL.
pub struct GradleParser {
    _private: (),
}

impl GradleParser {
    /// Create a new Gradle (Groovy DSL) parser.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a tree-sitter parser configured for Groovy.
    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        let language = tree_sitter_groovy::LANGUAGE;
        parser
            .set_language(&language.into())
            .context("failed to load Groovy grammar")?;
        Ok(parser)
    }

    /// Parse source code into a tree-sitter Tree.
    fn parse_tree(source: &[u8]) -> Result<Tree> {
        let mut parser = Self::create_parser()?;
        parser
            .parse(source, None)
            .context("tree-sitter parse returned None for Groovy source")
    }
}

impl LanguageParser for GradleParser {
    fn language_id(&self) -> &'static str {
        "gradle"
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
    fn alloc_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn text(&self, node: Node) -> &str {
        node.utf8_text(self.source).unwrap_or("")
    }
}

// ── Recursive walk ────────────────────────────────────────────��─────────────

/// Walk a tree-sitter node, extracting Groovy/Gradle symbols.
fn walk(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    match node.kind() {
        "class_declaration" => extract_class(ctx, node, parent_id, scope),
        "interface_declaration" => extract_interface(ctx, node, parent_id, scope),
        "enum_declaration" => extract_enum(ctx, node, parent_id, scope),
        "method_declaration" => extract_method(ctx, node, parent_id, scope),
        "constructor_declaration" => extract_constructor(ctx, node, parent_id, scope),
        "function_definition" => extract_function(ctx, node, parent_id, scope),
        "import_declaration" => extract_import(ctx, node),
        "local_variable_declaration" | "constant_declaration" | "field_declaration" => {
            extract_variable(ctx, node, parent_id, scope);
        }
        "juxt_function_call" => extract_juxt_call(ctx, node),
        "method_invocation" => extract_method_invocation(ctx, node),
        "marker_annotation" | "annotation" => extract_annotation(ctx, node),
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent_id, scope);
            }
        }
    }
}

// ── Class declaration ───────────────────────────────────────────────────────

/// Extract a Groovy class declaration.
///
/// Fields: `name` (identifier), `body` (class_body), `superclass`, `interfaces`.
fn extract_class(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);
    let sig = build_class_sig(ctx, node, &name);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Class,
        visibility: extract_visibility(ctx, node),
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });

    let inner_scope = ext_scope(scope, &name);
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, Some(id), &inner_scope);
    }
}

// ── Interface declaration ───────────────────────────────────────────────────

/// Extract a Groovy interface declaration.
fn extract_interface(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);
    let sig = format!("interface {name}");

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Interface,
        visibility: extract_visibility(ctx, node),
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });

    let inner_scope = ext_scope(scope, &name);
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, Some(id), &inner_scope);
    }
}

// ── Enum declaration ────────────────────────────────────────────────────────

/// Extract a Groovy enum declaration.
fn extract_enum(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);
    let sig = format!("enum {name}");

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Enum,
        visibility: extract_visibility(ctx, node),
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });

    let inner_scope = ext_scope(scope, &name);
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, Some(id), &inner_scope);
    }
}

// ── Method declaration ─────────────────────────────────────────────────────

/// Extract a Groovy method declaration.
///
/// Fields: `name` (identifier), `type`, `parameters`, `body` (block).
fn extract_method(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let params = field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let ret_type = field_text(ctx, node, "type").unwrap_or_default();
    let sig = if ret_type.is_empty() {
        format!("def {name}{params}")
    } else {
        format!("{ret_type} {name}{params}")
    };

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Method,
        visibility: extract_visibility(ctx, node),
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

// ── Constructor declaration ──────────────────────────────────────────────────

/// Extract a Groovy constructor declaration.
fn extract_constructor(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let params = field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let sig = format!("{name}{params}");
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(build_qname(scope, &name)),
        kind: SymbolKind::Method,
        visibility: extract_visibility(ctx, node),
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

// ── Function definition (Groovy `def foo() { … }`) ──────────────────────────

/// Extract a standalone Groovy function definition (outside a class).
///
/// Fields: `name` (identifier), `type`, `parameters`, `body` (closure).
fn extract_function(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let params = field_text(ctx, node, "parameters").unwrap_or_else(|| "()".to_string());
    let ret_type = field_text(ctx, node, "type").unwrap_or_default();
    let sig = if ret_type.is_empty() {
        format!("def {name}{params}")
    } else {
        format!("{ret_type} {name}{params}")
    };

    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Function,
        visibility: extract_visibility(ctx, node),
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_col: node.start_position().column as u32,
        end_col: node.end_position().column as u32,
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });

    // Recurse into body.
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, Some(id), scope);
    }
}

// ── Import declaration ──────────────────────────────────────────────��───────

/// Extract a Groovy import declaration.
fn extract_import(ctx: &mut Ctx, node: Node) {
    let full_text = ctx.text(node).to_string();
    // import com.example.Foo  OR  import static com.example.Bar.method
    let is_static = full_text.starts_with("import static");
    let path = full_text
        .trim_start_matches("import static")
        .trim_start_matches("import")
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_string();

    if path.is_empty() {
        return;
    }

    let kind = if is_static { "import_static" } else { "import" };

    let imported_name = path.rsplit('.').next().unwrap_or("*").to_string();

    ctx.imports.push(ImportEntry {
        file_id: 0,
        imported_name,
        source_module: path,
        alias: None,
        line: node.start_position().row as u32 + 1,
        kind: kind.to_string(),
    });
}

// ── Variable / field / constant declaration ─────────────────────────────────

/// Extract a Groovy variable, field, or constant declaration.
fn extract_variable(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    // The declarator children hold the names.
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "variable_declarator" {
            let name = field_text(ctx, child, "name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            let kind = if node.kind() == "constant_declaration" {
                SymbolKind::Constant
            } else if parent_id.is_some() {
                SymbolKind::Field
            } else {
                SymbolKind::Constant
            };

            let sig = ctx.text(node).lines().next().unwrap_or("").to_string();
            let qname = build_qname(scope, &name);
            let hash = hash_node(ctx, node);

            let id = ctx.alloc_id();
            ctx.symbols.push(Symbol {
                id,
                file_id: 0,
                name: name.clone(),
                qualified_name: Some(qname),
                kind,
                visibility: extract_visibility(ctx, node),
                start_line: child.start_position().row as u32 + 1,
                end_line: child.end_position().row as u32 + 1,
                start_col: child.start_position().column as u32,
                end_col: child.end_position().column as u32,
                parent_id,
                signature: Some(sig),
                doc_comment: None,
                body_hash: Some(hash),
            });
        }
    }
}

// ── Juxtaposed function call (Gradle DSL style: `plugins { id("java") }`) ──

/// Extract a Groovy juxtaposed function call as a reference.
///
/// In Gradle, DSL blocks like `plugins { … }`, `dependencies { … }`,
/// `android { … }` are parsed as `juxt_function_call` nodes where the
/// `name` field is the block name and `args` is the closure argument.
fn extract_juxt_call(ctx: &mut Ctx, node: Node) {
    let name_node = node.child_by_field_name("name");
    let name = if let Some(n) = name_node {
        ctx.text(n).to_string()
    } else {
        String::new()
    };

    if name.is_empty() {
        return;
    }

    ctx.references.push(SymbolRef {
        symbol_name: name.clone(),
        file_id: 0,
        file_path: String::new(),
        line: node.start_position().row as u32 + 1,
        col: node.start_position().column as u32,
        kind: "call".to_string(),
    });
}

// ── Method invocation ───────────────────────────────────────────────────────

/// Extract a Groovy method invocation as a reference.
fn extract_method_invocation(ctx: &mut Ctx, node: Node) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
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

// ── Annotation ──────────────────────────────────────────────────────────────

/// Extract a Groovy annotation as a reference.
fn extract_annotation(ctx: &mut Ctx, node: Node) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    ctx.references.push(SymbolRef {
        symbol_name: name,
        file_id: 0,
        file_path: String::new(),
        line: node.start_position().row as u32 + 1,
        col: node.start_position().column as u32,
        kind: "type".to_string(),
    });
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Get the text of a named field child from a tree-sitter node.
fn field_text(ctx: &Ctx, node: Node, field: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(ctx.text(child).to_string())
}

/// Extract visibility from a modifiers child node.
fn extract_visibility(ctx: &Ctx, node: Node) -> Visibility {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "modifiers" {
            let mod_cursor = &mut child.walk();
            for mod_child in child.children(mod_cursor) {
                let text = ctx.text(mod_child);
                match text {
                    "public" => return Visibility::Public,
                    "private" => return Visibility::Private,
                    "protected" | "internal" => return Visibility::Private,
                    _ => {}
                }
            }
        }
    }
    Visibility::Public
}

/// Build a class signature from its node.
fn build_class_sig(ctx: &Ctx, node: Node, name: &str) -> String {
    let mut parts = vec![format!("class {name}")];
    if let Some(sc) = field_text(ctx, node, "superclass") {
        parts.push(format!("extends {sc}"));
    }
    if let Some(ifaces) = field_text(ctx, node, "interfaces") {
        parts.push(format!("implements {ifaces}"));
    }
    parts.join(" ")
}

/// Build a qualified name from the current scope and a local name.
fn build_qname(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", scope.join("."), name)
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

    fn parse_gradle(source: &str) -> ParsedFile {
        let parser = GradleParser::new();
        parser.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_gradle_plugins_block() {
        let src = r#"
plugins {
    id("java")
    id("org.springframework.boot") version "3.0.0"
}
"#;
        let pf = parse_gradle(src);
        // plugins block should appear as a juxt_function_call reference
        assert!(!pf.references.is_empty());
    }

    #[test]
    fn test_gradle_dependencies() {
        let src = r#"
dependencies {
    implementation("com.example:lib:1.0")
}
"#;
        let pf = parse_gradle(src);
        assert!(!pf.references.is_empty());
    }

    #[test]
    fn test_class_declaration() {
        let src = r#"
class MyPlugin implements Plugin<Project> {
    void apply(Project project) {
        project.tasks.create("myTask") {}
    }
}
"#;
        let pf = parse_gradle(src);
        let classes: Vec<_> = pf
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "MyPlugin");
    }

    #[test]
    fn test_import() {
        let src = "import org.gradle.api.Plugin;";
        let pf = parse_gradle(src);
        assert!(!pf.imports.is_empty());
        assert_eq!(pf.imports[0].imported_name, "Plugin");
    }

    #[test]
    fn test_empty_source() {
        let pf = parse_gradle("");
        assert!(pf.symbols.is_empty());
        assert!(pf.imports.is_empty());
    }
}
