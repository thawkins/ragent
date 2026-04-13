//! Core data types for the code index.
//!
//! These types represent the entities tracked by the indexing pipeline:
//! files, symbols, imports, references, and configuration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

// ── Symbol Kind ─────────────────────────────────────────────────────────────

/// The kind of a code symbol extracted by tree-sitter parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    /// A standalone function.
    Function,
    /// A method on a type (inside an impl block).
    Method,
    /// A struct definition.
    Struct,
    /// A class definition (Python, TypeScript, Java, etc.).
    Class,
    /// An enum definition.
    Enum,
    /// A variant of an enum.
    EnumVariant,
    /// A trait definition (Rust).
    Trait,
    /// An interface definition (TypeScript, Go, Java).
    Interface,
    /// An impl block (Rust).
    Impl,
    /// A module declaration.
    Module,
    /// A constant binding.
    Constant,
    /// A static variable.
    Static,
    /// A type alias.
    TypeAlias,
    /// A struct/class field.
    Field,
    /// An import / use statement.
    Import,
    /// A macro definition.
    Macro,
    /// A test function.
    Test,
    /// An unrecognised symbol kind.
    Unknown,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Class => "class",
            Self::Enum => "enum",
            Self::EnumVariant => "enum_variant",
            Self::Trait => "trait",
            Self::Interface => "interface",
            Self::Impl => "impl",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Static => "static",
            Self::TypeAlias => "type_alias",
            Self::Field => "field",
            Self::Import => "import",
            Self::Macro => "macro",
            Self::Test => "test",
            Self::Unknown => "unknown",
        };
        write!(f, "{label}")
    }
}

// ── Visibility ──────────────────────────────────────────────────────────────

/// Parse a `SymbolKind` from its database string representation.
impl FromStr for SymbolKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "method" => Ok(Self::Method),
            "struct" => Ok(Self::Struct),
            "class" => Ok(Self::Class),
            "enum" => Ok(Self::Enum),
            "enum_variant" => Ok(Self::EnumVariant),
            "trait" => Ok(Self::Trait),
            "interface" => Ok(Self::Interface),
            "impl" => Ok(Self::Impl),
            "module" => Ok(Self::Module),
            "constant" => Ok(Self::Constant),
            "static" => Ok(Self::Static),
            "type_alias" => Ok(Self::TypeAlias),
            "field" => Ok(Self::Field),
            "import" => Ok(Self::Import),
            "macro" => Ok(Self::Macro),
            "test" => Ok(Self::Test),
            "unknown" => Ok(Self::Unknown),
            other => anyhow::bail!("unknown SymbolKind: {other}"),
        }
    }
}

/// Visibility of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// `pub` — visible everywhere.
    Public,
    /// `pub(crate)` — visible within the crate.
    PubCrate,
    /// `pub(super)` — visible to the parent module.
    PubSuper,
    /// No visibility modifier — private to the containing module.
    Private,
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Public => "pub",
            Self::PubCrate => "pub(crate)",
            Self::PubSuper => "pub(super)",
            Self::Private => "private",
        };
        write!(f, "{label}")
    }
}

impl FromStr for Visibility {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pub" => Ok(Self::Public),
            "pub(crate)" => Ok(Self::PubCrate),
            "pub(super)" => Ok(Self::PubSuper),
            "private" => Ok(Self::Private),
            other => anyhow::bail!("unknown Visibility: {other}"),
        }
    }
}

// ── File Entry ──────────────────────────────────────────────────────────────

/// A tracked file in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative path from the project root.
    pub path: String,
    /// Blake3 content hash (hex-encoded).
    pub content_hash: String,
    /// File size in bytes.
    pub byte_size: u64,
    /// Detected language (e.g. "rust", "python"), or `None` if unknown.
    pub language: Option<String>,
    /// When this file was last indexed.
    pub last_indexed: DateTime<Utc>,
    /// File modification time as nanoseconds since the Unix epoch.
    pub mtime_ns: i64,
    /// Number of lines in the file.
    pub line_count: u64,
}

// ── Symbol ──────────────────────────────────────────────────────────────────

/// A code symbol extracted from a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Unique ID (assigned by the store).
    pub id: i64,
    /// Foreign key to the file that contains this symbol.
    pub file_id: i64,
    /// Simple name (e.g. `parse_config`).
    pub name: String,
    /// Fully-qualified name (e.g. `crate::config::parse_config`).
    pub qualified_name: Option<String>,
    /// What kind of symbol this is.
    pub kind: SymbolKind,
    /// Visibility modifier.
    pub visibility: Visibility,
    /// First line of the symbol (1-based).
    pub start_line: u32,
    /// Last line of the symbol (1-based).
    pub end_line: u32,
    /// Start column (0-based byte offset).
    pub start_col: u32,
    /// End column (0-based byte offset).
    pub end_col: u32,
    /// Parent symbol ID (e.g. the impl block that contains a method).
    pub parent_id: Option<i64>,
    /// Signature string (e.g. `fn parse_config(path: &Path) -> Result<Config>`).
    pub signature: Option<String>,
    /// Documentation comment text.
    pub doc_comment: Option<String>,
    /// Blake3 hash of the symbol body for change detection.
    pub body_hash: Option<String>,
}

// ── Import Entry ────────────────────────────────────────────────────────────

/// An import/use statement in a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEntry {
    /// Foreign key to the file that contains this import.
    pub file_id: i64,
    /// The imported name (e.g. `HashMap`).
    pub imported_name: String,
    /// Source module path (e.g. `std::collections`).
    pub source_module: String,
    /// Optional alias (e.g. `use Foo as Bar` → alias = `Bar`).
    pub alias: Option<String>,
    /// Line number of the import statement.
    pub line: u32,
    /// Kind of import: "use", "mod", "extern crate", etc.
    pub kind: String,
}

// ── Symbol Reference ────────────────────────────────────────────────────────

/// A reference to a symbol found in source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    /// Name of the referenced symbol.
    pub symbol_name: String,
    /// Foreign key to the file containing the reference.
    pub file_id: i64,
    /// Line number of the reference.
    pub line: u32,
    /// Column of the reference.
    pub col: u32,
    /// Kind of reference (e.g. "call", "type", "field_access").
    pub kind: String,
}

// ── Index Stats ─────────────────────────────────────────────────────────────

/// Summary statistics for the code index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    /// Total number of indexed files.
    pub files_indexed: u64,
    /// Total number of extracted symbols.
    pub total_symbols: u64,
    /// Total size of all indexed files in bytes.
    pub total_bytes: u64,
    /// Breakdown of files per language.
    pub languages: Vec<(String, u64)>,
    /// When the last full index completed.
    pub last_full_index: Option<DateTime<Utc>>,
    /// When the last incremental update completed.
    pub last_incremental_update: Option<DateTime<Utc>>,
    /// Size of the index database on disk in bytes.
    pub index_size_bytes: u64,
}

impl fmt::Display for IndexStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Files indexed: {}", self.files_indexed)?;
        writeln!(f, "Total symbols: {}", self.total_symbols)?;
        writeln!(
            f,
            "Total size:    {:.1} KB",
            self.total_bytes as f64 / 1024.0
        )?;
        if !self.languages.is_empty() {
            write!(f, "Languages:     ")?;
            for (i, (lang, count)) in self.languages.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{lang} ({count})")?;
            }
            writeln!(f)?;
        }
        if let Some(ts) = &self.last_full_index {
            writeln!(f, "Last full:     {ts}")?;
        }
        if let Some(ts) = &self.last_incremental_update {
            writeln!(f, "Last incr:     {ts}")?;
        }
        writeln!(
            f,
            "Index size:    {:.1} KB",
            self.index_size_bytes as f64 / 1024.0
        )?;
        Ok(())
    }
}

// ── Symbol Filter ───────────────────────────────────────────────────────────

/// Filter criteria for querying symbols from the index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SymbolFilter {
    /// Filter by symbol name (substring match, case-insensitive).
    pub name: Option<String>,
    /// Filter by exact symbol kind.
    pub kind: Option<SymbolKind>,
    /// Filter by file path (substring match).
    pub file_path: Option<String>,
    /// Filter by language.
    pub language: Option<String>,
    /// Filter by visibility.
    pub visibility: Option<Visibility>,
    /// Maximum number of results.
    pub limit: Option<u32>,
}

// ── Scan Config ─────────────────────────────────────────────────────────────

/// Configuration for the file scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// Maximum file size in bytes (files larger than this are skipped).
    pub max_file_size: u64,
    /// Additional directory names to exclude (beyond hardcoded defaults).
    pub extra_exclude_dirs: Vec<String>,
    /// Additional glob patterns to exclude.
    pub extra_exclude_patterns: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_file_size: 1_048_576, // 1 MB
            extra_exclude_dirs: Vec::new(),
            extra_exclude_patterns: Vec::new(),
        }
    }
}

// ── Scanned File ────────────────────────────────────────────────────────────

/// A file discovered and fingerprinted by the scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFile {
    /// Relative path from the project root.
    pub path: PathBuf,
    /// Blake3 content hash (hex-encoded).
    pub hash: String,
    /// File size in bytes.
    pub size: u64,
    /// Detected language, if known.
    pub language: Option<String>,
    /// Modification time as nanoseconds since the Unix epoch.
    pub mtime_ns: i64,
    /// Number of lines in the file.
    pub line_count: u64,
}

// ── Stale Diff ──────────────────────────────────────────────────────────────

/// The result of comparing scanned files against the index.
#[derive(Debug, Clone, Default)]
pub struct StaleDiff {
    /// Files that exist on disk but not in the index.
    pub to_add: Vec<ScannedFile>,
    /// Files whose content hash has changed since last index.
    pub to_update: Vec<ScannedFile>,
    /// Paths of files in the index that no longer exist on disk.
    pub to_remove: Vec<String>,
}

impl StaleDiff {
    /// Returns `true` if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.to_add.is_empty() && self.to_update.is_empty() && self.to_remove.is_empty()
    }

    /// Total number of changes.
    pub fn total(&self) -> usize {
        self.to_add.len() + self.to_update.len() + self.to_remove.len()
    }
}

// ── Search Query ────────────────────────────────────────────────────────────

/// A search request for the code index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Free-text query string (searched via FTS).
    pub query: String,
    /// Optional filter by symbol kind.
    pub kind: Option<SymbolKind>,
    /// Optional filter by language.
    pub language: Option<String>,
    /// Optional filter by file path glob pattern.
    pub file_pattern: Option<String>,
    /// Maximum number of results to return (default: 20).
    pub max_results: usize,
    /// Whether to include body snippets in results.
    pub include_body: bool,
}

impl SearchQuery {
    /// Create a new query with just a search string and default settings.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            max_results: 20,
            ..Default::default()
        }
    }
}

// ── Index Result ────────────────────────────────────────────────────────────

/// Summary of an indexing operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexResult {
    /// Number of files added to the index.
    pub files_added: usize,
    /// Number of files updated in the index.
    pub files_updated: usize,
    /// Number of files removed from the index.
    pub files_removed: usize,
    /// Number of symbols extracted.
    pub symbols_extracted: usize,
    /// Total elapsed time in milliseconds.
    pub elapsed_ms: u64,
}

impl fmt::Display for IndexResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Indexed: +{} ~{} -{} files, {} symbols in {}ms",
            self.files_added,
            self.files_updated,
            self.files_removed,
            self.symbols_extracted,
            self.elapsed_ms,
        )
    }
}

// ── Dependency Direction ────────────────────────────────────────────────────

/// Direction for file dependency queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepDirection {
    /// Files that this file imports/depends on.
    Imports,
    /// Files that depend on / import this file.
    Dependents,
}

// ── Code Index Config ───────────────────────────────────────────────────────

/// Configuration for the code index system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIndexConfig {
    /// Whether the code index is enabled.
    pub enabled: bool,
    /// Root directory of the project to index.
    pub project_root: PathBuf,
    /// Path to the index storage directory (default: `.ragent/codeindex`).
    pub index_dir: PathBuf,
    /// Scanner configuration.
    pub scan_config: ScanConfig,
}

impl Default for CodeIndexConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            project_root: PathBuf::from("."),
            index_dir: PathBuf::from(".ragent/codeindex"),
            scan_config: ScanConfig::default(),
        }
    }
}
