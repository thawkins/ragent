//! Shared tree-sitter extraction context.
//!
//! Consolidates the `struct Ctx` accumulator and its `alloc_id` / `text`
//! accessors that were copy-pasted into every language parser
//! (see `ANTIPAT.md` M3.4/M3.5).

use crate::types::{ImportEntry, Symbol, SymbolRef};
use tree_sitter::Node;

/// Accumulator threaded through a parser's tree walk.
///
/// Holds the source bytes plus the symbol/import/reference vectors being
/// collected, and a monotonic id counter for synthetic symbol ids.
pub struct Ctx<'a> {
    /// The source bytes the parse tree was built from.
    pub source: &'a [u8],
    /// Symbols extracted so far.
    pub symbols: Vec<Symbol>,
    /// Imports / use statements extracted so far.
    pub imports: Vec<ImportEntry>,
    /// Cross-module symbol references extracted so far.
    pub references: Vec<SymbolRef>,
    /// Next synthetic symbol id to hand out.
    pub next_id: i64,
    /// C++-only flag selecting the C++ extraction arms in the C/C++ parser.
    pub is_cpp: bool,
}

impl Ctx<'_> {
    /// Create an empty context over `source`.
    #[must_use]
    pub fn new(source: &[u8]) -> Ctx<'_> {
        Ctx {
            source,
            symbols: Vec::new(),
            imports: Vec::new(),
            references: Vec::new(),
            next_id: 0,
            is_cpp: false,
        }
    }

    /// Allocate the next synthetic symbol id.
    pub const fn alloc_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// UTF-8 text of `node`, or `""` when the node is not valid UTF-8.
    #[must_use]
    pub fn text(&self, node: Node) -> &str {
        node.utf8_text(self.source).unwrap_or("")
    }
}
