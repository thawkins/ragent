//! Gradle Kotlin DSL language parser using tree-sitter.
//!
//! Extracts classes, functions, properties, imports, object declarations,
//! and call expressions from Gradle Kotlin DSL build scripts (`.gradle.kts`).
//!
//! Gradle Kotlin DSL scripts use standard Kotlin syntax with Gradle-specific
//! function calls like `plugins {}`, `dependencies {}`, `android {}`, etc.
//! These appear as `call_expression` nodes in the tree-sitter Kotlin AST.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

/// Tree-sitter parser for the Gradle Kotlin DSL.
pub struct GradleKtsParser {
    _private: (),
}

impl GradleKtsParser {
    /// Create a new Gradle Kotlin DSL parser.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a tree-sitter parser configured for Kotlin.
    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        let language = tree_sitter_kotlin_ng::LANGUAGE;
        parser
            .set_language(&language.into())
            .context("failed to load Kotlin grammar")?;
        Ok(parser)
    }

    /// Parse source code into a tree-sitter Tree.
    fn parse_tree(source: &[u8]) -> Result<Tree> {
        let mut parser = Self::create_parser()?;
        parser
            .parse(source, None)
            .context("tree-sitter parse returned None for Kotlin source")
    }
}

impl LanguageParser for GradleKtsParser {
    fn language_id(&self) -> &'static str {
        "gradle_kts"
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

// ── Recursive walk ──────────────────────────────────────────────────────────

/// Walk a tree-sitter node, extracting Kotlin/Gradle KTS symbols.
fn walk(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    match node.kind() {
        "class_declaration" => extract_class(ctx, node, parent_id, scope),
        "object_declaration" => extract_object(ctx, node, parent_id, scope),
        "function_declaration" => extract_function(ctx, node, parent_id, scope),
        "property_declaration" => extract_property(ctx, node, parent_id, scope),
        "type_alias" => extract_type_alias(ctx, node, parent_id, scope),
        "import" => extract_import(ctx, node),
        "call_expression" => extract_call(ctx, node),
        "companion_object" => extract_companion(ctx, node, parent_id, scope),
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent_id, scope);
            }
        }
    }
}

// ── Class declaration ───────────────────────────────────────────────────────

/// Extract a Kotlin class declaration.
///
/// Field: `name` (identifier). Children: modifiers, class_body,
/// type_parameters, delegation_specifiers, primary_constructor.
fn extract_class(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let modifiers = extract_modifiers(ctx, node);
    let vis = modifiers_to_visibility(&modifiers);
    let sig = build_class_sig(ctx, node, &name, &modifiers);
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Class,
        visibility: vis,
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

// ── Object declaration ───────────────────────────────���─────────────────────

/// Extract a Kotlin object declaration.
fn extract_object(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let modifiers = extract_modifiers(ctx, node);
    let vis = modifiers_to_visibility(&modifiers);
    let sig = format!("object {name}");
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::Class,
        visibility: vis,
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

// ── Companion object ───��────────────────────────────────────────────────────

/// Extract a Kotlin companion object.
fn extract_companion(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or("Companion".to_string());
    let sig = format!("companion object {name}");
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
        parent_id,
        signature: Some(sig),
        doc_comment: None,
        body_hash: Some(hash),
    });

    let inner_scope = ext_scope(scope, "Companion");
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, Some(id), &inner_scope);
    }
}

// ── Function declaration ─────────��──────────────────────────────────────────

/// Extract a Kotlin function declaration.
///
/// Field: `name` (identifier). Children: function_value_parameters, type,
/// function_body, modifiers, type_parameters.
fn extract_function(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "name").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let modifiers = extract_modifiers(ctx, node);
    let vis = modifiers_to_visibility(&modifiers);

    // Build signature: fun <name>(<params>): <return_type>
    let params = extract_function_params(ctx, node);
    let ret_type = extract_return_type(ctx, node);
    let sig = if ret_type.is_empty() {
        format!("fun {name}({params})")
    } else {
        format!("fun {name}({params}): {ret_type}")
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
        visibility: vis,
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

// ── Property declaration ────────────────────────────────────────────────────

/// Extract a Kotlin property declaration.
fn extract_property(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    // Properties contain variable_declaration or multi_variable_declaration children.
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "variable_declaration" {
            extract_single_property(ctx, child, parent_id, scope);
        } else if child.kind() == "multi_variable_declaration" {
            let multi_cursor = &mut child.walk();
            for vc in child.children(multi_cursor) {
                if vc.kind() == "variable_declaration" {
                    extract_single_property(ctx, vc, parent_id, scope);
                }
            }
        }
    }
}

/// Extract a single variable_declaration within a property_declaration.
fn extract_single_property(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    // variable_declaration children: identifier, type, expression
    let name = first_child_by_kind(ctx, node, "identifier").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let kind = if parent_id.is_some() {
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

// ── Type alias ──────────────────────────────────────────────────────────────

/// Extract a Kotlin type alias.
fn extract_type_alias(ctx: &mut Ctx, node: Node, parent_id: Option<i64>, scope: &[String]) {
    let name = field_text(ctx, node, "type").unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let sig = format!("typealias {name}");
    let qname = build_qname(scope, &name);
    let hash = hash_node(ctx, node);

    let id = ctx.alloc_id();
    ctx.symbols.push(Symbol {
        id,
        file_id: 0,
        name: name.clone(),
        qualified_name: Some(qname),
        kind: SymbolKind::TypeAlias,
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

// ── Import ──────────────────────────────────────────────────────────────────

/// Extract a Kotlin import statement.
fn extract_import(ctx: &mut Ctx, node: Node) {
    // import children: identifier, qualified_identifier
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "qualified_identifier" || child.kind() == "identifier" {
            let path = ctx.text(child).to_string();
            if path.is_empty() {
                continue;
            }

            let imported_name = path.rsplit('.').next().unwrap_or("*").to_string();

            ctx.imports.push(ImportEntry {
                file_id: 0,
                imported_name,
                source_module: path,
                alias: None,
                line: node.start_position().row as u32 + 1,
                kind: "import".to_string(),
            });
            return;
        }
    }
}

// ── Call expression (Gradle DSL blocks) ─────────────────────────────────────

/// Extract a call expression as a reference.
///
/// In Gradle Kotlin DSL, blocks like `plugins { }`, `dependencies { }`,
/// `tasks { }` appear as call_expression nodes.
fn extract_call(ctx: &mut Ctx, node: Node) {
    // The first identifier-like child is the callee name.
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "identifier" || child.kind() == "simple_identifier" {
            let name = ctx.text(child).to_string();
            if !name.is_empty() {
                ctx.references.push(SymbolRef {
                    symbol_name: name,
                    file_id: 0,
                    file_path: String::new(),
                    line: child.start_position().row as u32 + 1,
                    col: child.start_position().column as u32,
                    kind: "call".to_string(),
                });
            }
            break;
        }
        // navigation_expression like `project.tasks`
        if child.kind() == "navigation_expression" {
            let nav_cursor = &mut child.walk();
            for nc in child.children(nav_cursor) {
                if nc.kind() == "simple_identifier" || nc.kind() == "identifier" {
                    let name = ctx.text(nc).to_string();
                    if !name.is_empty() {
                        ctx.references.push(SymbolRef {
                            symbol_name: name,
                            file_id: 0,
                            file_path: String::new(),
                            line: nc.start_position().row as u32 + 1,
                            col: nc.start_position().column as u32,
                            kind: "call".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Recurse into the call's children for nested calls (closures).
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk(ctx, child, None, &[]);
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Get the text of a named field child from a tree-sitter node.
fn field_text(ctx: &Ctx, node: Node, field: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(ctx.text(child).to_string())
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

/// Extract modifier keywords from a modifiers child.
fn extract_modifiers(ctx: &Ctx, node: Node) -> Vec<String> {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "modifiers" {
            let mut mods = Vec::new();
            let mod_cursor = &mut child.walk();
            for mc in child.children(mod_cursor) {
                if mc.is_named() {
                    mods.push(ctx.text(mc).to_string());
                }
            }
            return mods;
        }
    }
    Vec::new()
}

/// Convert modifier list to Visibility.
fn modifiers_to_visibility(mods: &[String]) -> Visibility {
    for m in mods {
        match m.as_str() {
            "public" => return Visibility::Public,
            "private" => return Visibility::Private,
            "protected" | "internal" => return Visibility::Private,
            _ => {}
        }
    }
    Visibility::Public
}

/// Extract function parameters as a string.
fn extract_function_params(ctx: &Ctx, node: Node) -> String {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "function_value_parameters" {
            return ctx.text(child).to_string();
        }
    }
    String::new()
}

/// Extract the return type from a function declaration.
fn extract_return_type(ctx: &Ctx, node: Node) -> String {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "type" || child.kind() == "user_type" || child.kind() == "nullable_type"
        {
            // In tree-sitter-kotlin, the return type appears as a `type` child
            // after the parameters. Check if it's actually a return type by
            // looking for a colon in the source near this node.
            let text = ctx.text(child).to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

/// Build a class signature from its node.
fn build_class_sig(ctx: &Ctx, node: Node, name: &str, modifiers: &[String]) -> String {
    let mod_str = modifiers.join(" ");
    let prefix = if mod_str.is_empty() {
        "class".to_string()
    } else {
        format!("{mod_str} class")
    };

    let mut parts = vec![format!("{prefix} {name}")];

    // Look for delegation_specifiers (extends/implements).
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "delegation_specifiers" {
            let text = ctx.text(child).to_string();
            if !text.is_empty() {
                parts.push(text);
            }
        }
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

    fn parse_kts(source: &str) -> ParsedFile {
        let parser = GradleKtsParser::new();
        parser.parse(source.as_bytes()).unwrap()
    }

    #[test]
    fn test_plugins_block() {
        let src = r#"
plugins {
    java
    id("org.springframework.boot") version "3.0.0"
}
"#;
        let pf = parse_kts(src);
        assert!(!pf.references.is_empty());
    }

    #[test]
    fn test_dependencies_block() {
        let src = r#"
dependencies {
    implementation("com.example:lib:1.0")
}
"#;
        let pf = parse_kts(src);
        assert!(!pf.references.is_empty());
    }

    #[test]
    fn test_class_declaration() {
        let src = r#"
open class MyPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        project.tasks.register("myTask")
    }
}
"#;
        let pf = parse_kts(src);
        let classes: Vec<_> = pf
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty());
    }

    #[test]
    fn test_function_declaration() {
        let src = r#"
fun customTask(project: Project) {
    println("Hello")
}
"#;
        let pf = parse_kts(src);
        let funcs: Vec<_> = pf
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        assert!(!funcs.is_empty());
    }

    #[test]
    fn test_import() {
        let src = "import org.gradle.api.Plugin";
        let pf = parse_kts(src);
        assert!(!pf.imports.is_empty());
        assert_eq!(pf.imports[0].imported_name, "Plugin");
    }

    #[test]
    fn test_property_declaration() {
        let src = r#"val version = "1.0""#;
        let pf = parse_kts(src);
        let consts: Vec<_> = pf
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Constant)
            .collect();
        assert!(!consts.is_empty());
    }

    #[test]
    fn test_empty_source() {
        let pf = parse_kts("");
        assert!(pf.symbols.is_empty());
        assert!(pf.imports.is_empty());
    }
}
