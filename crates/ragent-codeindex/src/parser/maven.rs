//! Maven POM (XML) language parser using tree-sitter.
//!
//! Extracts Maven-specific structure from `pom.xml` files: the project
//! coordinates (groupId, artifactId, version), modules, dependencies,
//! plugins, profiles, properties, and repositories.
//!
//! Maven POM files are XML documents. The tree-sitter XML grammar produces
//! `document` → `element` → `STag`/`ETag`/`content` nodes. Each XML element
//! is represented by its tag name (`Name` child inside `STag`), its
//! attributes (`Attribute` children), and its `content` child holding
//! character data and nested elements.

use super::{LanguageParser, ParsedFile};
use crate::types::{ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility};
use anyhow::{Context, Result};
use tree_sitter::Node;

/// Tree-sitter parser for Maven POM (XML) files.
pub struct MavenParser {
    _private: (),
}

impl MavenParser {
    /// Create a new Maven POM parser.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }

    super::util::tree_sitter_parser!(
        tree_sitter_xml::LANGUAGE_XML,
        "failed to load XML grammar",
        "tree-sitter parse returned None for XML source"
    );
}

impl LanguageParser for MavenParser {
    fn language_id(&self) -> &'static str {
        "maven"
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

// ── Maven-specific tag classification ────────────────────────────��───────────

/// Maven POM tags that represent important structural elements.
const MAVEN_SYMBOL_TAGS: &[&str] = &[
    "project",
    "parent",
    "modules",
    "module",
    "dependencies",
    "dependency",
    "dependencyManagement",
    "plugins",
    "plugin",
    "build",
    "profiles",
    "profile",
    "properties",
    "repositories",
    "repository",
    "pluginRepositories",
    "pluginRepository",
    "reporting",
    "distributionManagement",
    "ciManagement",
    "scm",
    "issueManagement",
    "organization",
    "developers",
    "developer",
    "contributors",
    "contributor",
    "licenses",
    "license",
    "mailingLists",
    "mailingList",
    "description",
];

/// Maven coordinate tags used to identify a POM artifact.
const MAVEN_COORD_TAGS: &[&str] = &["groupId", "artifactId", "version", "packaging"];

// ── Recursive walk ──────────────────────────────────────────────────────────

/// Walk a tree-sitter node, extracting Maven POM symbols.
fn walk(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    match node.kind() {
        "document" => {
            // The document has a root element child.
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent_id);
            }
        }
        "element" => extract_element(ctx, node, parent_id),
        _ => {
            let cursor = &mut node.walk();
            for child in node.children(cursor) {
                walk(ctx, child, parent_id);
            }
        }
    }
}

// ── Element extraction ──────────────────────────────────────────────────────

/// Extract an XML element as a Maven symbol if it's a relevant tag.
fn extract_element(ctx: &mut Ctx, node: Node, parent_id: Option<i64>) {
    let tag_name = get_tag_name(ctx, node);
    if tag_name.is_empty() {
        return;
    }

    let is_maven_symbol = MAVEN_SYMBOL_TAGS.contains(&tag_name.as_str());
    let is_coord = MAVEN_COORD_TAGS.contains(&tag_name.as_str());

    if is_maven_symbol || is_coord {
        let kind = map_tag_to_kind(&tag_name);
        let name = build_element_name(ctx, node, &tag_name);
        let sig = build_element_sig(ctx, node, &tag_name);
        let hash = hash_node(ctx, node);

        let id = ctx.alloc_id();
        ctx.symbols.push(Symbol {
            id,
            file_id: 0,
            name,
            qualified_name: None,
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

        // Dependencies and modules are import-like.
        if tag_name == "dependency" {
            extract_dependency_import(ctx, node);
        } else if tag_name == "module" {
            extract_module_import(ctx, node);
        } else if tag_name == "parent" {
            extract_parent_import(ctx, node);
        }

        // Recurse into child elements.
        let cursor = &mut node.walk();
        for child in node.children(cursor) {
            walk(ctx, child, Some(id));
        }
    } else {
        // Recurse even for non-symbol elements to find nested Maven elements.
        let cursor = &mut node.walk();
        for child in node.children(cursor) {
            walk(ctx, child, parent_id);
        }
    }
}

// ── Dependency import extraction ────────────────────────────────────────────

/// Extract a Maven dependency as an import entry.
fn extract_dependency_import(ctx: &mut Ctx, node: Node) {
    let (group_id, artifact_id, version) = extract_pom_coordinates(ctx, node);
    let coords = if version.is_empty() {
        format!("{group_id}:{artifact_id}")
    } else {
        format!("{group_id}:{artifact_id}:{version}")
    };

    if !artifact_id.is_empty() {
        ctx.imports.push(ImportEntry {
            file_id: 0,
            imported_name: artifact_id,
            source_module: coords,
            alias: None,
            line: node.start_position().row as u32 + 1,
            kind: "dependency".to_string(),
        });
    }
}

/// Extract a Maven module as an import entry.
fn extract_module_import(ctx: &mut Ctx, node: Node) {
    let text = get_element_text(ctx, node);
    if !text.is_empty() {
        ctx.imports.push(ImportEntry {
            file_id: 0,
            imported_name: text.clone(),
            source_module: text,
            alias: None,
            line: node.start_position().row as u32 + 1,
            kind: "module".to_string(),
        });
    }
}

/// Extract a Maven parent POM as an import entry.
fn extract_parent_import(ctx: &mut Ctx, node: Node) {
    let (group_id, artifact_id, version) = extract_pom_coordinates(ctx, node);
    let coords = if version.is_empty() {
        format!("{group_id}:{artifact_id}")
    } else {
        format!("{group_id}:{artifact_id}:{version}")
    };

    if !artifact_id.is_empty() {
        ctx.imports.push(ImportEntry {
            file_id: 0,
            imported_name: artifact_id,
            source_module: coords,
            alias: None,
            line: node.start_position().row as u32 + 1,
            kind: "parent".to_string(),
        });
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Get the tag name from an element node by looking at its `STag` or `EmptyElemTag` child.
fn get_tag_name(ctx: &Ctx, node: Node) -> String {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "STag" || child.kind() == "EmptyElemTag" {
            let inner = &mut child.walk();
            for c in child.children(inner) {
                if c.kind() == "Name" {
                    return ctx.text(c).to_string();
                }
            }
        }
    }
    String::new()
}

/// Get the direct text content of an element (no nested element children).
fn get_element_text(ctx: &Ctx, node: Node) -> String {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "content" {
            let inner = &mut child.walk();
            let mut text = String::new();
            for c in child.children(inner) {
                if c.kind() == "CharData" {
                    text.push_str(ctx.text(c));
                }
            }
            return text.trim().to_string();
        }
    }
    String::new()
}

/// Find a child element by tag name and return its text content.
fn find_child_element_text(ctx: &Ctx, node: Node, tag: &str) -> String {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "content" {
            let inner = &mut child.walk();
            for c in child.children(inner) {
                if c.kind() == "element" {
                    let name = get_tag_name(ctx, c);
                    if name == tag {
                        return get_element_text(ctx, c);
                    }
                }
            }
        }
    }
    String::new()
}

/// Extract Maven coordinates (groupId, artifactId, version) from an element.
fn extract_pom_coordinates(ctx: &Ctx, node: Node) -> (String, String, String) {
    let group_id = find_child_element_text(ctx, node, "groupId");
    let artifact_id = find_child_element_text(ctx, node, "artifactId");
    let version = find_child_element_text(ctx, node, "version");
    (group_id, artifact_id, version)
}

/// Build a display name for a Maven element.
fn build_element_name(ctx: &Ctx, node: Node, tag: &str) -> String {
    match tag {
        "project" => {
            // Try to use groupId:artifactId from the project element.
            let gid = find_child_element_text(ctx, node, "groupId");
            let aid = find_child_element_text(ctx, node, "artifactId");
            if aid.is_empty() {
                "project".to_string()
            } else if gid.is_empty() {
                format!("project:{aid}")
            } else {
                format!("project:{gid}:{aid}")
            }
        }
        "dependency" => {
            let (_, aid, ver) = extract_pom_coordinates(ctx, node);
            if ver.is_empty() {
                format!("dep:{aid}")
            } else {
                format!("dep:{aid}:{ver}")
            }
        }
        "plugin" => {
            let (_, aid, ver) = extract_pom_coordinates(ctx, node);
            if ver.is_empty() {
                format!("plugin:{aid}")
            } else {
                format!("plugin:{aid}:{ver}")
            }
        }
        "module" => {
            let text = get_element_text(ctx, node);
            if text.is_empty() {
                "module".to_string()
            } else {
                format!("module:{text}")
            }
        }
        "profile" => {
            let id = find_child_element_text(ctx, node, "id");
            if id.is_empty() {
                "profile".to_string()
            } else {
                format!("profile:{id}")
            }
        }
        "property" | "properties" => tag.to_string(),
        _ => tag.to_string(),
    }
}

/// Build a signature string for a Maven element.
fn build_element_sig(ctx: &Ctx, node: Node, tag: &str) -> String {
    match tag {
        "dependency" | "plugin" | "parent" => {
            let (gid, aid, ver) = extract_pom_coordinates(ctx, node);
            let scope = find_child_element_text(ctx, node, "scope");
            if scope.is_empty() {
                format!("{gid}:{aid}:{ver}")
            } else {
                format!("{gid}:{aid}:{ver} ({scope})")
            }
        }
        _ => {
            let text = get_element_text(ctx, node);
            if text.is_empty() {
                format!("<{tag}>")
            } else {
                let truncated = if text.len() > 60 {
                    let mut end = 57;
                    while end > 0 && !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}…", &text[..end])
                } else {
                    text
                };
                format!("<{tag}>{truncated}</{tag}>")
            }
        }
    }
}

/// Map a Maven tag name to a `SymbolKind`.
fn map_tag_to_kind(tag: &str) -> SymbolKind {
    match tag {
        "project" => SymbolKind::Module,
        "parent" => SymbolKind::Import,
        "module" | "modules" => SymbolKind::Module,
        "dependency" | "dependencies" | "dependencyManagement" => SymbolKind::Import,
        "plugin" | "plugins" => SymbolKind::Module,
        "build" => SymbolKind::Module,
        "profile" | "profiles" => SymbolKind::Module,
        "properties" => SymbolKind::Module,
        "groupId" | "artifactId" | "version" | "packaging" => SymbolKind::Field,
        "repository" | "repositories" => SymbolKind::Module,
        _ => SymbolKind::Unknown,
    }
}

/// Compute a blake3 hash of the node's text for change detection.
fn hash_node(ctx: &Ctx, node: Node) -> String {
    let text = ctx.text(node);
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

// ── Tests ───────────────────────────────────────────────────────────────────
