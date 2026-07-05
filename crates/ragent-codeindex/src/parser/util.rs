//! Shared helpers for tree-sitter language parsers.
//!
//! Provides:
//! - [`build_qname`] — build a qualified name from scope segments and a name,
//!   with a configurable separator (`"::"` or `"."`).  Previously this was
//!   copy-pasted as `build_qname` / `build_qualified` / `build_qualified_name`
//!   across 10 parser files (see `DUPPLAN.md` Milestone C).
//! - The [`tree_sitter_parser!`] macro — generates the uniform
//!   `create_parser()` + `parse_tree()` pair used by most language parsers,
//!   eliminating a third boilerplate duplication (DUPPLAN.md Milestone C).

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
/// methods that were copy-pasted across 7–9 language parser structs
/// (DUPPLAN.md Milestone C, `cargo dupes` groups 10 and 16).
///
/// # Expansion
///
/// Expands to two associated functions on the struct:
///
/// ```ignore
/// fn create_parser() -> Result<Parser> {
///     let mut parser = Parser::new();
///     let language = $language;
///     parser.set_language(&language.into()).context($grammar_err)?;
///     Ok(parser)
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
        fn create_parser() -> Result<tree_sitter::Parser> {
            let mut parser = tree_sitter::Parser::new();
            let language = $lang;
            parser
                .set_language(&language.into())
                .context($grammar_err)?;
            Ok(parser)
        }

        /// Parse source code into a tree-sitter Tree.
        fn parse_tree(source: &[u8]) -> Result<tree_sitter::Tree> {
            let mut parser = Self::create_parser()?;
            parser.parse(source, None).context($parse_err)
        }
    };
}

pub(crate) use tree_sitter_parser;
