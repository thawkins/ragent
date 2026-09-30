//! Shared helpers for tree-sitter language parsers.
//!
//! Provides:
//! - [`build_qname`](crate::parser::util::build_qname) - build a qualified name from scope
//!   segments and a name, with a configurable separator (`"::"` or `"."`).
//!   Previously this was copy-pasted as `build_qname` / `build_qualified` /
//!   `build_qualified_name` across 10 parser files (see `DUPPLAN.md`
//!   Milestone C).
//! - [`extend_scope`](crate::parser::util::extend_scope) - append a name to a scope chain
//!   (10 parser files).
//! - [`node_hash`](crate::parser::util::node_hash) - content hash of a tree-sitter node,
//!   routed through the single [`crate::scanner::hash_content`] implementation
//!   (12 parser files).
//! - [`field_text`](crate::parser::util::field_text) /
//!   [`first_child_by_kind`](crate::parser::util::first_child_by_kind) /
//!   [`find_child`](crate::parser::util::find_child) - small tree-walking accessors duplicated
//!   across the parsers.
//! - `tree_sitter_parser!` - generates the uniform `create_parser()` +
//!   `parse_tree()` pair used by most language parsers, eliminating a third
//!   boilerplate duplication (DUPPLAN.md Milestone C).

use tree_sitter::Node;

/// Maximum tree-sitter recursion depth before a parser stops descending.
///
/// Each parser's `walk` / `extract_node` recursion follows the parse tree,
/// which tree-sitter bounds by file size (itself capped by `max_file_size`),
/// so a depth guard is defence-in-depth for a pathologically deep generated
/// file (ANTIPAT M6.4 / audit 4.4). The limit is far deeper than any
/// hand-written or machine-generated source nests in practice, so it never
/// truncates real extraction; past it the walk simply stops descending.
pub const MAX_TREE_DEPTH: u32 = 512;

thread_local! {
    /// Current tree-walk recursion depth on this thread.
    static WALK_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// RAII depth guard for a parser tree walk.
///
/// Held at the top of every parser `walk` / `extract_node`. Entering increments
/// the per-thread depth counter and dropping decrements it, so the counter
/// tracks the walk's live recursion depth without threading a `depth`
/// parameter through every recursive helper (ANTIPAT M6.4 / audit 4.4). When
/// the depth reaches [`MAX_TREE_DEPTH`], [`TreeDepthGuard::enter`] returns
/// `None` and the caller stops descending; the crossing is logged once per
/// process so a wide node boundary cannot spam the log.
pub struct TreeDepthGuard;

impl TreeDepthGuard {
    /// Enter one level of tree recursion, or `None` past [`MAX_TREE_DEPTH`].
    ///
    /// Callers use the result as `let Some(_guard) = ... else { return; };`.
    #[must_use]
    pub fn enter(node: Node<'_>) -> Option<Self> {
        WALK_DEPTH.with(|depth| {
            let current = depth.get();
            if current >= MAX_TREE_DEPTH {
                static WARN_ONCE: std::sync::Once = std::sync::Once::new();
                WARN_ONCE.call_once(|| {
                    tracing::warn!(
                        depth = current,
                        kind = node.kind(),
                        "codeindex: parser tree walk hit MAX_TREE_DEPTH; not descending further"
                    );
                });
                return None;
            }
            depth.set(current + 1);
            Some(Self)
        })
    }
}

impl Drop for TreeDepthGuard {
    fn drop(&mut self) {
        WALK_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Append `name` to a scope chain, returning the extended scope.
///
/// Previously copy-pasted as `ext_scope` / `extend_scope` across 10 parser
/// files (see `ANTIPAT.md` M3.4).
#[must_use]
pub fn extend_scope(scope: &[String], name: &str) -> Vec<String> {
    let mut extended = scope.to_vec();
    extended.push(name.to_string());
    extended
}

/// Compute the content hash of a tree-sitter node.
///
/// Always routes through [`crate::scanner::hash_content`] so every parser
/// hashes identically; six parsers previously inlined a raw `blake3` call
/// (see `ANTIPAT.md` M3.4).
#[must_use]
pub fn node_hash(source: &[u8], node: Node<'_>) -> String {
    crate::scanner::hash_content(node_text(source, node).as_bytes())
}

/// UTF-8 text of a node, or `""` when the node is not valid UTF-8.
#[must_use]
pub fn node_text<'a>(source: &'a [u8], node: Node<'_>) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// Text of a node's named field child, if present.
#[must_use]
pub fn field_text(source: &[u8], node: Node<'_>, field: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(node_text(source, child).to_string())
}

/// Text of the first child whose `kind()` equals `kind`.
#[must_use]
pub fn first_child_by_kind(source: &[u8], node: Node<'_>, kind: &str) -> Option<String> {
    find_child(node, kind).map(|child| node_text(source, child).to_string())
}

/// The first child node whose `kind()` equals `kind`.
#[must_use]
pub fn find_child<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let cursor = &mut node.walk();
    node.children(cursor).find(|child| child.kind() == kind)
}

/// Build a qualified name from scope segments and a leaf name.
///
/// If `scope` is empty, returns `name` as-is.  Otherwise, joins the scope
/// segments with `sep` and appends `name`.
///
/// # Arguments
///
/// * `scope` - The enclosing scope segments (e.g. `["std", "collections"]`).
/// * `name` - The leaf symbol name.
/// * `sep`  - The separator between segments (`"::"` for Rust/Python/Java,
///   `"."` for Go).
///
/// # Returns
///
/// The fully qualified name (e.g. `"std::collections::HashMap"`).
#[must_use]
pub fn build_qname(scope: &[String], name: &str, sep: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}{sep}{}", scope.join(sep), name)
    }
}

/// Declare the uniform `create_parser()` + `parse_tree()` pair for a
/// tree-sitter language parser.
///
/// This macro eliminates the boilerplate `create_parser` / `parse_tree`
/// methods that were copy-pasted across 7-9 language parser structs
/// (DUPPLAN.md Milestone C, `cargo dupes` groups 10 and 16).
///
/// # Expansion
///
/// Expands to two associated functions on the struct. The `create_parser`
/// body runs once per process inside the `OnceLock` initializer; a grammar
/// load failure panics there because every subsequent parse would fail
/// identically (the error is inherent to the linked grammar, not to user
/// input), while the returned guard and parse results flow through `Result`.
///
/// ```ignore
/// fn create_parser() -> Result<MutexGuard<'static, Parser>> {
///     static CACHE: OnceLock<Mutex<Parser>> = OnceLock::new();
///     let mutex = CACHE.get_or_init(|| {
///         let mut parser = Parser::new();
///         let language = $language;
///         parser.set_language(&language.into()).expect($grammar_err);
///         std::sync::Mutex::new(parser)
///     });
///     Ok(mutex.lock().unwrap_or_else(PoisonError::into_inner))
/// }
///
/// fn parse_tree(source: &[u8]) -> Result<Tree> {
///     let mut parser = Self::create_parser()?;
///     parser.parse(source, None).context($parse_err)
/// }
/// ```
///
/// # Arguments
///
/// * `$lang`       - The tree-sitter `LANGUAGE` constant (e.g.
///   `tree_sitter_rust::LANGUAGE`).
/// * `$grammar_err` - The error-context string for `set_language` failures
///   (e.g. `"failed to load Rust grammar"`).
/// * `$parse_err`  - The error-context string for `parse` returning `None`
///   (e.g. `"tree-sitter parse returned None"`).
///
/// # Example
///
/// ```ignore
/// pub struct RustParser { _private: () }
///
/// impl RustParser {
///     pub fn new() -> Self { Self { _private: () } }
///     tree_sitter_parser!(tree_sitter_rust::LANGUAGE, "failed to load Rust grammar", "tree-sitter parse returned None");
/// }
/// ```
macro_rules! tree_sitter_parser {
    ($lang:expr, $grammar_err:expr, $parse_err:expr) => {
        /// Create a tree-sitter parser configured for this language.
        ///
        /// M-027: the parser is cached in a per-language `OnceLock<Mutex<Parser>>`
        /// so re-parsing a file does not construct a fresh parser (and re-set the
        /// grammar) every time. `tree_sitter::Parser` is cheap to reuse across
        /// parses (only the tree is per-file), so a single cached instance per
        /// language avoids repeated setup.
        fn create_parser() -> Result<std::sync::MutexGuard<'static, tree_sitter::Parser>> {
            static CACHE: std::sync::OnceLock<std::sync::Mutex<tree_sitter::Parser>> =
                std::sync::OnceLock::new();
            let mutex = CACHE.get_or_init(|| {
                let mut parser = tree_sitter::Parser::new();
                let language = $lang;
                parser.set_language(&language.into()).expect($grammar_err);
                std::sync::Mutex::new(parser)
            });
            Ok(mutex
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner))
        }

        /// Parse source code into a tree-sitter Tree.
        fn parse_tree(source: &[u8]) -> Result<tree_sitter::Tree> {
            let mut parser = Self::create_parser()?;
            parser.parse(source, None).context($parse_err)
        }
    };
}

pub(crate) use tree_sitter_parser;
