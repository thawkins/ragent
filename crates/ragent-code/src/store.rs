//! SQLite-backed index storage for files and symbols.
//!
//! [`IndexStore`] manages a local SQLite database that tracks which files
//! have been indexed, their content hashes, and (in later milestones)
//! the symbols extracted from them.

use crate::types::{
    FileEntry, ImportEntry, ScannedFile, StaleDiff, Symbol, SymbolFilter, SymbolKind, SymbolRef,
    Visibility,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

/// Current schema version — bump when migrating.
const SCHEMA_VERSION: i32 = 2;

/// Persistent store for the code index, backed by SQLite.
pub struct IndexStore {
    conn: Connection,
}

impl IndexStore {
    /// Open (or create) an index database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create directory: {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("cannot open index db: {}", path.display()))?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("cannot open in-memory db")?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Create or migrate the database schema.
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS indexed_files (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                path        TEXT    NOT NULL UNIQUE,
                content_hash TEXT   NOT NULL,
                byte_size   INTEGER NOT NULL,
                language    TEXT,
                last_indexed TEXT   NOT NULL,
                mtime_ns    INTEGER NOT NULL,
                line_count  INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_files_path ON indexed_files(path);
            CREATE INDEX IF NOT EXISTS idx_files_language ON indexed_files(language);

            CREATE TABLE IF NOT EXISTS symbols (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id         INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
                name            TEXT NOT NULL,
                qualified_name  TEXT,
                kind            TEXT NOT NULL,
                visibility      TEXT,
                start_line      INTEGER NOT NULL,
                end_line        INTEGER NOT NULL,
                start_col       INTEGER NOT NULL,
                end_col         INTEGER NOT NULL,
                parent_id       INTEGER,
                signature       TEXT,
                doc_comment     TEXT,
                body_hash       TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
            CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
            CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
            CREATE INDEX IF NOT EXISTS idx_symbols_parent ON symbols(parent_id);
            CREATE INDEX IF NOT EXISTS idx_symbols_qualified ON symbols(qualified_name);

            CREATE TABLE IF NOT EXISTS imports (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id         INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
                imported_name   TEXT NOT NULL,
                source_module   TEXT,
                alias           TEXT,
                line            INTEGER NOT NULL,
                kind            TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file_id);
            CREATE INDEX IF NOT EXISTS idx_imports_name ON imports(imported_name);
            CREATE INDEX IF NOT EXISTS idx_imports_source ON imports(source_module);

            CREATE TABLE IF NOT EXISTS symbol_refs (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol_name TEXT NOT NULL,
                file_id     INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
                line        INTEGER NOT NULL,
                col         INTEGER NOT NULL,
                kind        TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_refs_symbol ON symbol_refs(symbol_name);
            CREATE INDEX IF NOT EXISTS idx_refs_file ON symbol_refs(file_id);

            CREATE TABLE IF NOT EXISTS file_deps (
                source_file_id  INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
                target_path     TEXT NOT NULL,
                kind            TEXT NOT NULL,
                PRIMARY KEY (source_file_id, target_path, kind)
            );

            CREATE INDEX IF NOT EXISTS idx_deps_source ON file_deps(source_file_id);
            CREATE INDEX IF NOT EXISTS idx_deps_target ON file_deps(target_path);
            ",
        )?;

        // Seed schema version if missing.
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))?;
        if count == 0 {
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
        }

        Ok(())
    }

    // ── File CRUD ───────────────────────────────────────────────────────────

    /// Insert or update a file entry. Returns the row ID.
    pub fn upsert_file(&self, entry: &FileEntry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO indexed_files (path, content_hash, byte_size, language, last_indexed, mtime_ns, line_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                content_hash = excluded.content_hash,
                byte_size    = excluded.byte_size,
                language     = excluded.language,
                last_indexed = excluded.last_indexed,
                mtime_ns     = excluded.mtime_ns,
                line_count   = excluded.line_count",
            params![
                entry.path,
                entry.content_hash,
                entry.byte_size as i64,
                entry.language,
                entry.last_indexed.to_rfc3339(),
                entry.mtime_ns,
                entry.line_count as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a file entry by its relative path.
    pub fn get_file(&self, path: &str) -> Result<Option<FileEntry>> {
        let row = self
            .conn
            .query_row(
                "SELECT path, content_hash, byte_size, language, last_indexed, mtime_ns, line_count
                 FROM indexed_files WHERE path = ?1",
                [path],
                |row| {
                    Ok(RawFileRow {
                        path: row.get(0)?,
                        content_hash: row.get(1)?,
                        byte_size: row.get::<_, i64>(2)?,
                        language: row.get(3)?,
                        last_indexed: row.get::<_, String>(4)?,
                        mtime_ns: row.get(5)?,
                        line_count: row.get::<_, i64>(6)?,
                    })
                },
            )
            .optional()?;

        match row {
            Some(r) => Ok(Some(raw_to_file_entry(r)?)),
            None => Ok(None),
        }
    }

    /// List all indexed files.
    pub fn list_files(&self) -> Result<Vec<FileEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, content_hash, byte_size, language, last_indexed, mtime_ns, line_count
             FROM indexed_files ORDER BY path",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(RawFileRow {
                path: row.get(0)?,
                content_hash: row.get(1)?,
                byte_size: row.get::<_, i64>(2)?,
                language: row.get(3)?,
                last_indexed: row.get::<_, String>(4)?,
                mtime_ns: row.get(5)?,
                line_count: row.get::<_, i64>(6)?,
            })
        })?;

        let mut files = Vec::new();
        for r in rows {
            files.push(raw_to_file_entry(r?)?);
        }
        Ok(files)
    }

    /// Delete a file entry by path.
    pub fn delete_file(&self, path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM indexed_files WHERE path = ?1", [path])?;
        Ok(())
    }

    /// Count total indexed files.
    pub fn file_count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM indexed_files", [], |r| r.get(0))?;
        Ok(count as u64)
    }

    /// Total byte size of all indexed files.
    pub fn total_bytes(&self) -> Result<u64> {
        let total: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(byte_size), 0) FROM indexed_files",
            [],
            |r| r.get(0),
        )?;
        Ok(total as u64)
    }

    /// Count of files per language.
    pub fn language_counts(&self) -> Result<Vec<(String, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(language, 'unknown'), COUNT(*)
             FROM indexed_files
             GROUP BY language
             ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ── Stale detection ─────────────────────────────────────────────────────

    /// Compare scanned files against the stored index and return the diff.
    ///
    /// Identifies files to add (new on disk), update (hash changed), and
    /// remove (no longer on disk).
    pub fn get_stale_files(&self, scanned: &[ScannedFile]) -> Result<StaleDiff> {
        // Build a map of currently indexed files: path → hash.
        let mut stmt = self
            .conn
            .prepare("SELECT path, content_hash FROM indexed_files")?;
        let indexed: HashMap<String, String> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Build a set of scanned paths for removal detection.
        let scanned_paths: std::collections::HashSet<String> = scanned
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect();

        let mut diff = StaleDiff::default();

        for file in scanned {
            let path_str = file.path.to_string_lossy().to_string();
            match indexed.get(&path_str) {
                None => {
                    // New file — not yet indexed.
                    diff.to_add.push(file.clone());
                }
                Some(old_hash) if *old_hash != file.hash => {
                    // Hash changed — needs re-indexing.
                    diff.to_update.push(file.clone());
                }
                _ => {
                    // Unchanged.
                }
            }
        }

        // Files in the index that are no longer on disk.
        for indexed_path in indexed.keys() {
            if !scanned_paths.contains(indexed_path) {
                diff.to_remove.push(indexed_path.clone());
            }
        }

        debug!(
            "stale diff: {} to add, {} to update, {} to remove",
            diff.to_add.len(),
            diff.to_update.len(),
            diff.to_remove.len()
        );

        Ok(diff)
    }

    /// Apply a batch of scanned files to the index in a single transaction.
    ///
    /// Upserts new/changed files and deletes removed files.
    pub fn apply_diff(&self, diff: &StaleDiff) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        for file in diff.to_add.iter().chain(diff.to_update.iter()) {
            let entry = scanned_to_entry(file);
            tx.execute(
                "INSERT INTO indexed_files (path, content_hash, byte_size, language, last_indexed, mtime_ns, line_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(path) DO UPDATE SET
                    content_hash = excluded.content_hash,
                    byte_size    = excluded.byte_size,
                    language     = excluded.language,
                    last_indexed = excluded.last_indexed,
                    mtime_ns     = excluded.mtime_ns,
                    line_count   = excluded.line_count",
                params![
                    entry.path,
                    entry.content_hash,
                    entry.byte_size as i64,
                    entry.language,
                    entry.last_indexed.to_rfc3339(),
                    entry.mtime_ns,
                    entry.line_count as i64,
                ],
            )?;
        }

        for path in &diff.to_remove {
            tx.execute("DELETE FROM indexed_files WHERE path = ?1", [path])?;
        }

        tx.commit()?;
        Ok(())
    }

    // ── Symbol CRUD ─────────────────────────────────────────────────────────

    /// Begin an explicit transaction.
    ///
    /// All subsequent `upsert_symbols`, `upsert_imports`, and `upsert_refs`
    /// calls will run within this transaction until `commit_transaction()`.
    /// This avoids per-statement auto-commit, dramatically reducing disk I/O
    /// when indexing many files in a batch.
    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute_batch("BEGIN")?;
        Ok(())
    }

    /// Commit an explicit transaction started by `begin_transaction()`.
    pub fn commit_transaction(&self) -> Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Insert symbols for a file, replacing any existing symbols for that file.
    ///
    /// The `file_id` field on each `Symbol` must be set correctly before calling.
    /// Returns the number of symbols inserted.
    pub fn upsert_symbols(&self, file_id: i64, symbols: &[Symbol]) -> Result<usize> {
        // Delete existing symbols for this file first.
        self.conn
            .execute("DELETE FROM symbols WHERE file_id = ?1", [file_id])?;

        let mut stmt = self.conn.prepare(
            "INSERT INTO symbols (file_id, name, qualified_name, kind, visibility,
                start_line, end_line, start_col, end_col, parent_id,
                signature, doc_comment, body_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;

        // Build a map from temporary IDs to real (SQLite-assigned) IDs.
        let mut id_map: HashMap<i64, i64> = HashMap::new();
        let mut count = 0;

        // First pass: insert symbols without parent_id.
        for sym in symbols {
            if sym.parent_id.is_some() {
                continue;
            }
            stmt.execute(params![
                file_id,
                sym.name,
                sym.qualified_name,
                sym.kind.to_string(),
                sym.visibility.to_string(),
                sym.start_line as i64,
                sym.end_line as i64,
                sym.start_col as i64,
                sym.end_col as i64,
                Option::<i64>::None,
                sym.signature,
                sym.doc_comment,
                sym.body_hash,
            ])?;
            let real_id = self.conn.last_insert_rowid();
            id_map.insert(sym.id, real_id);
            count += 1;
        }

        // Second pass: insert symbols that have parents.
        for sym in symbols {
            if sym.parent_id.is_none() {
                continue;
            }
            let real_parent_id = sym.parent_id.and_then(|pid| id_map.get(&pid).copied());
            stmt.execute(params![
                file_id,
                sym.name,
                sym.qualified_name,
                sym.kind.to_string(),
                sym.visibility.to_string(),
                sym.start_line as i64,
                sym.end_line as i64,
                sym.start_col as i64,
                sym.end_col as i64,
                real_parent_id,
                sym.signature,
                sym.doc_comment,
                sym.body_hash,
            ])?;
            let real_id = self.conn.last_insert_rowid();
            id_map.insert(sym.id, real_id);
            count += 1;
        }

        Ok(count)
    }

    /// Query symbols with optional filters.
    pub fn query_symbols(&self, filter: &SymbolFilter) -> Result<Vec<Symbol>> {
        let mut sql = String::from(
            "SELECT s.id, s.file_id, s.name, s.qualified_name, s.kind, s.visibility,
                    s.start_line, s.end_line, s.start_col, s.end_col,
                    s.parent_id, s.signature, s.doc_comment, s.body_hash
             FROM symbols s",
        );
        let mut conditions: Vec<String> = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();

        if filter.file_path.is_some() || filter.language.is_some() {
            sql.push_str(" JOIN indexed_files f ON s.file_id = f.id");
        }

        if let Some(ref name) = filter.name {
            conditions.push(format!(
                "s.name LIKE '%' || ?{} || '%' COLLATE NOCASE",
                bind_values.len() + 1
            ));
            bind_values.push(name.clone());
        }
        if let Some(kind) = filter.kind {
            conditions.push(format!("s.kind = ?{}", bind_values.len() + 1));
            bind_values.push(kind.to_string());
        }
        if let Some(ref vis) = filter.visibility {
            conditions.push(format!("s.visibility = ?{}", bind_values.len() + 1));
            bind_values.push(vis.to_string());
        }
        if let Some(ref fp) = filter.file_path {
            conditions.push(format!(
                "f.path LIKE '%' || ?{} || '%'",
                bind_values.len() + 1
            ));
            bind_values.push(fp.clone());
        }
        if let Some(ref lang) = filter.language {
            conditions.push(format!("f.language = ?{}", bind_values.len() + 1));
            bind_values.push(lang.clone());
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY s.name");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let bind_refs: Vec<&dyn rusqlite::types::ToSql> = bind_values
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(bind_refs.as_slice(), |row| {
            Ok(RawSymbolRow {
                id: row.get(0)?,
                file_id: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                kind: row.get(4)?,
                visibility: row.get(5)?,
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_col: row.get(8)?,
                end_col: row.get(9)?,
                parent_id: row.get(10)?,
                signature: row.get(11)?,
                doc_comment: row.get(12)?,
                body_hash: row.get(13)?,
            })
        })?;

        let mut symbols = Vec::new();
        for r in rows {
            symbols.push(raw_to_symbol(r?)?);
        }
        Ok(symbols)
    }

    /// Get all symbols for a specific file.
    pub fn get_file_symbols(&self, file_id: i64) -> Result<Vec<Symbol>> {
        self.query_symbols(&SymbolFilter {
            file_path: None,
            ..Default::default()
        })
        .map(|syms| syms.into_iter().filter(|s| s.file_id == file_id).collect())
    }

    /// Count total symbols in the index.
    pub fn symbol_count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))?;
        Ok(count as u64)
    }

    /// Count total symbol references in the index.
    pub fn reference_count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbol_refs", [], |r| r.get(0))?;
        Ok(count as u64)
    }

    // ── Import CRUD ─────────────────────────────────────────────────────────

    /// Insert imports for a file, replacing any existing imports for that file.
    pub fn upsert_imports(&self, file_id: i64, imports: &[ImportEntry]) -> Result<usize> {
        self.conn
            .execute("DELETE FROM imports WHERE file_id = ?1", [file_id])?;

        let mut stmt = self.conn.prepare(
            "INSERT INTO imports (file_id, imported_name, source_module, alias, line, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;

        for imp in imports {
            stmt.execute(params![
                file_id,
                imp.imported_name,
                imp.source_module,
                imp.alias,
                imp.line as i64,
                imp.kind,
            ])?;
        }

        Ok(imports.len())
    }

    /// Get all imports for a specific file.
    pub fn get_file_imports(&self, file_id: i64) -> Result<Vec<ImportEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_id, imported_name, source_module, alias, line, kind
             FROM imports WHERE file_id = ?1 ORDER BY line",
        )?;

        let rows = stmt.query_map([file_id], |row| {
            Ok(ImportEntry {
                file_id: row.get(0)?,
                imported_name: row.get(1)?,
                source_module: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                alias: row.get(3)?,
                line: row.get::<_, i64>(4)? as u32,
                kind: row.get(5)?,
            })
        })?;

        let mut imports = Vec::new();
        for r in rows {
            imports.push(r?);
        }
        Ok(imports)
    }

    /// Search imports by imported name.
    pub fn query_imports(&self, name_substring: &str) -> Result<Vec<ImportEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_id, imported_name, source_module, alias, line, kind
             FROM imports
             WHERE imported_name LIKE '%' || ?1 || '%' COLLATE NOCASE
             ORDER BY imported_name",
        )?;

        let rows = stmt.query_map([name_substring], |row| {
            Ok(ImportEntry {
                file_id: row.get(0)?,
                imported_name: row.get(1)?,
                source_module: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                alias: row.get(3)?,
                line: row.get::<_, i64>(4)? as u32,
                kind: row.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ── Symbol Ref CRUD ─────────────────────────────────────────────────────

    /// Insert symbol references for a file, replacing existing ones.
    pub fn upsert_refs(&self, file_id: i64, refs: &[SymbolRef]) -> Result<usize> {
        self.conn
            .execute("DELETE FROM symbol_refs WHERE file_id = ?1", [file_id])?;

        let mut stmt = self.conn.prepare(
            "INSERT INTO symbol_refs (symbol_name, file_id, line, col, kind)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;

        for r in refs {
            stmt.execute(params![
                r.symbol_name,
                file_id,
                r.line as i64,
                r.col as i64,
                r.kind,
            ])?;
        }

        Ok(refs.len())
    }

    /// Find all references to a symbol by name.
    pub fn find_references(&self, symbol_name: &str) -> Result<Vec<SymbolRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.symbol_name, r.file_id, r.line, r.col, r.kind,
                    COALESCE(f.path, '') as file_path
             FROM symbol_refs r
             LEFT JOIN indexed_files f ON f.id = r.file_id
             WHERE r.symbol_name = ?1
             ORDER BY f.path, r.line",
        )?;

        let rows = stmt.query_map([symbol_name], |row| {
            Ok(SymbolRef {
                symbol_name: row.get(0)?,
                file_id: row.get(1)?,
                file_path: row.get(5)?,
                line: row.get::<_, i64>(2)? as u32,
                col: row.get::<_, i64>(3)? as u32,
                kind: row.get(4)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ── File Dependencies ───────────────────────────────────────────────────

    /// Set file dependencies for a source file, replacing existing ones.
    pub fn set_file_deps(
        &self,
        source_file_id: i64,
        deps: &[(String, String)], // (target_path, kind)
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM file_deps WHERE source_file_id = ?1",
            [source_file_id],
        )?;

        let mut stmt = self.conn.prepare(
            "INSERT OR IGNORE INTO file_deps (source_file_id, target_path, kind)
             VALUES (?1, ?2, ?3)",
        )?;

        for (target, kind) in deps {
            stmt.execute(params![source_file_id, target, kind])?;
        }

        Ok(())
    }

    /// Get the file IDs of files that depend on the given path.
    pub fn get_dependents(&self, target_path: &str) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT source_file_id FROM file_deps WHERE target_path = ?1")?;
        let rows = stmt.query_map([target_path], |row| row.get(0))?;
        let mut ids = Vec::new();
        for r in rows {
            ids.push(r?);
        }
        Ok(ids)
    }

    // ── Aggregate stats ─────────────────────────────────────────────────────

    /// Get the file_id for a given path.
    pub fn get_file_id(&self, path: &str) -> Result<Option<i64>> {
        self.conn
            .query_row(
                "SELECT id FROM indexed_files WHERE path = ?1",
                [path],
                |row| row.get(0),
            )
            .optional()
            .context("query file id")
    }

    /// Get comprehensive index statistics.
    pub fn get_stats(&self) -> Result<crate::types::IndexStats> {
        let files_indexed = self.file_count()?;
        let total_symbols = self.symbol_count()?;
        let total_bytes = self.total_bytes()?;
        let total_references = self.reference_count()?;
        let languages = self.language_counts()?;

        Ok(crate::types::IndexStats {
            files_indexed,
            total_symbols,
            total_bytes,
            languages,
            last_full_index: None,
            last_incremental_update: None,
            index_size_bytes: 0,
            fts_doc_count: 0, // set by CodeIndex::status()
            total_references,
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Raw row from SQLite before type conversion.
struct RawFileRow {
    path: String,
    content_hash: String,
    byte_size: i64,
    language: Option<String>,
    last_indexed: String,
    mtime_ns: i64,
    line_count: i64,
}

/// Convert a raw database row to a [`FileEntry`].
fn raw_to_file_entry(r: RawFileRow) -> Result<FileEntry> {
    let last_indexed: DateTime<Utc> = DateTime::parse_from_rfc3339(&r.last_indexed)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    Ok(FileEntry {
        path: r.path,
        content_hash: r.content_hash,
        byte_size: r.byte_size as u64,
        language: r.language,
        last_indexed,
        mtime_ns: r.mtime_ns,
        line_count: r.line_count as u64,
    })
}

/// Convert a [`ScannedFile`] into a [`FileEntry`] for storage.
fn scanned_to_entry(file: &ScannedFile) -> FileEntry {
    FileEntry {
        path: file.path.to_string_lossy().to_string(),
        content_hash: file.hash.clone(),
        byte_size: file.size,
        language: file.language.clone(),
        last_indexed: Utc::now(),
        mtime_ns: file.mtime_ns,
        line_count: file.line_count,
    }
}

/// Raw symbol row from SQLite before type conversion.
struct RawSymbolRow {
    id: i64,
    file_id: i64,
    name: String,
    qualified_name: Option<String>,
    kind: String,
    visibility: Option<String>,
    start_line: i64,
    end_line: i64,
    start_col: i64,
    end_col: i64,
    parent_id: Option<i64>,
    signature: Option<String>,
    doc_comment: Option<String>,
    body_hash: Option<String>,
}

/// Convert a raw symbol row into a [`Symbol`].
fn raw_to_symbol(r: RawSymbolRow) -> Result<Symbol> {
    let kind: SymbolKind = r.kind.parse()?;
    let visibility: Visibility = r.visibility.as_deref().unwrap_or("private").parse()?;

    Ok(Symbol {
        id: r.id,
        file_id: r.file_id,
        name: r.name,
        qualified_name: r.qualified_name,
        kind,
        visibility,
        start_line: r.start_line as u32,
        end_line: r.end_line as u32,
        start_col: r.start_col as u32,
        end_col: r.end_col as u32,
        parent_id: r.parent_id,
        signature: r.signature,
        doc_comment: r.doc_comment,
        body_hash: r.body_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(path: &str, hash: &str) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            content_hash: hash.to_string(),
            byte_size: 100,
            language: Some("rust".to_string()),
            last_indexed: Utc::now(),
            mtime_ns: 1_000_000_000,
            line_count: 10,
        }
    }

    #[test]
    fn test_open_in_memory() {
        let store = IndexStore::open_in_memory().unwrap();
        assert_eq!(store.file_count().unwrap(), 0);
    }

    #[test]
    fn test_upsert_and_get() {
        let store = IndexStore::open_in_memory().unwrap();
        let entry = make_entry("src/main.rs", "abc123");
        store.upsert_file(&entry).unwrap();

        let got = store.get_file("src/main.rs").unwrap().unwrap();
        assert_eq!(got.path, "src/main.rs");
        assert_eq!(got.content_hash, "abc123");
        assert_eq!(got.byte_size, 100);
    }

    #[test]
    fn test_upsert_updates_existing() {
        let store = IndexStore::open_in_memory().unwrap();
        let entry1 = make_entry("src/main.rs", "hash_v1");
        store.upsert_file(&entry1).unwrap();

        let entry2 = make_entry("src/main.rs", "hash_v2");
        store.upsert_file(&entry2).unwrap();

        assert_eq!(store.file_count().unwrap(), 1);
        let got = store.get_file("src/main.rs").unwrap().unwrap();
        assert_eq!(got.content_hash, "hash_v2");
    }

    #[test]
    fn test_list_files() {
        let store = IndexStore::open_in_memory().unwrap();
        store.upsert_file(&make_entry("b.rs", "h2")).unwrap();
        store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

        let files = store.list_files().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "a.rs"); // sorted by path
        assert_eq!(files[1].path, "b.rs");
    }

    #[test]
    fn test_delete_file() {
        let store = IndexStore::open_in_memory().unwrap();
        store
            .upsert_file(&make_entry("src/main.rs", "abc"))
            .unwrap();
        assert_eq!(store.file_count().unwrap(), 1);

        store.delete_file("src/main.rs").unwrap();
        assert_eq!(store.file_count().unwrap(), 0);
        assert!(store.get_file("src/main.rs").unwrap().is_none());
    }

    #[test]
    fn test_stale_detection() {
        let store = IndexStore::open_in_memory().unwrap();

        // Index two files.
        store.upsert_file(&make_entry("a.rs", "hash_a")).unwrap();
        store.upsert_file(&make_entry("b.rs", "hash_b")).unwrap();

        // Simulate a re-scan: a.rs unchanged, b.rs changed, c.rs is new, a.rs still present.
        // d.rs was deleted (not in scanned).
        // Actually let's index d.rs too so we can test removal.
        store.upsert_file(&make_entry("d.rs", "hash_d")).unwrap();

        let scanned = vec![
            ScannedFile {
                path: "a.rs".into(),
                hash: "hash_a".to_string(), // unchanged
                size: 100,
                language: Some("rust".to_string()),
                mtime_ns: 1_000_000_000,
                line_count: 10,
            },
            ScannedFile {
                path: "b.rs".into(),
                hash: "hash_b_v2".to_string(), // changed
                size: 200,
                language: Some("rust".to_string()),
                mtime_ns: 2_000_000_000,
                line_count: 20,
            },
            ScannedFile {
                path: "c.rs".into(),
                hash: "hash_c".to_string(), // new
                size: 50,
                language: Some("rust".to_string()),
                mtime_ns: 3_000_000_000,
                line_count: 5,
            },
        ];

        let diff = store.get_stale_files(&scanned).unwrap();

        assert_eq!(diff.to_add.len(), 1);
        assert_eq!(diff.to_add[0].path.to_string_lossy(), "c.rs");

        assert_eq!(diff.to_update.len(), 1);
        assert_eq!(diff.to_update[0].path.to_string_lossy(), "b.rs");

        assert_eq!(diff.to_remove.len(), 1);
        assert_eq!(diff.to_remove[0], "d.rs");
    }

    #[test]
    fn test_apply_diff() {
        let store = IndexStore::open_in_memory().unwrap();
        store
            .upsert_file(&make_entry("old.rs", "hash_old"))
            .unwrap();

        let diff = StaleDiff {
            to_add: vec![ScannedFile {
                path: "new.rs".into(),
                hash: "hash_new".to_string(),
                size: 100,
                language: Some("rust".to_string()),
                mtime_ns: 1_000_000_000,
                line_count: 10,
            }],
            to_update: vec![],
            to_remove: vec!["old.rs".to_string()],
        };

        store.apply_diff(&diff).unwrap();

        assert!(store.get_file("old.rs").unwrap().is_none());
        assert!(store.get_file("new.rs").unwrap().is_some());
        assert_eq!(store.file_count().unwrap(), 1);
    }

    #[test]
    fn test_language_counts() {
        let store = IndexStore::open_in_memory().unwrap();
        store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
        store.upsert_file(&make_entry("b.rs", "h2")).unwrap();

        let mut entry_py = make_entry("c.py", "h3");
        entry_py.language = Some("python".to_string());
        store.upsert_file(&entry_py).unwrap();

        let counts = store.language_counts().unwrap();
        assert_eq!(counts.len(), 2);
        // rust=2 should come first (sorted by count DESC)
        assert_eq!(counts[0], ("rust".to_string(), 2));
        assert_eq!(counts[1], ("python".to_string(), 1));
    }

    fn make_symbol(name: &str, kind: SymbolKind, parent_id: Option<i64>, temp_id: i64) -> Symbol {
        Symbol {
            id: temp_id,
            file_id: 0,
            name: name.to_string(),
            qualified_name: Some(name.to_string()),
            kind,
            visibility: Visibility::Public,
            start_line: 1,
            end_line: 10,
            start_col: 0,
            end_col: 0,
            parent_id,
            signature: Some(format!("fn {name}()")),
            doc_comment: None,
            body_hash: Some("hash123".to_string()),
        }
    }

    #[test]
    fn test_upsert_and_query_symbols() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store
            .upsert_file(&make_entry("src/main.rs", "abc"))
            .unwrap();

        let symbols = vec![
            make_symbol("main", SymbolKind::Function, None, 0),
            make_symbol("Config", SymbolKind::Struct, None, 1),
            make_symbol("new", SymbolKind::Method, Some(1), 2),
        ];

        let count = store.upsert_symbols(file_id, &symbols).unwrap();
        assert_eq!(count, 3);
        assert_eq!(store.symbol_count().unwrap(), 3);

        // Query all
        let all = store.query_symbols(&SymbolFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        // Query by name
        let by_name = store
            .query_symbols(&SymbolFilter {
                name: Some("main".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].name, "main");

        // Query by kind
        let methods = store
            .query_symbols(&SymbolFilter {
                kind: Some(SymbolKind::Method),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "new");
    }

    #[test]
    fn test_symbol_parent_id_mapping() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store.upsert_file(&make_entry("src/lib.rs", "def")).unwrap();

        let symbols = vec![
            make_symbol("Foo", SymbolKind::Struct, None, 0),
            make_symbol("bar", SymbolKind::Method, Some(0), 1),
        ];

        store.upsert_symbols(file_id, &symbols).unwrap();

        let all = store.query_symbols(&SymbolFilter::default()).unwrap();
        let foo = all.iter().find(|s| s.name == "Foo").unwrap();
        let bar = all.iter().find(|s| s.name == "bar").unwrap();

        // bar's parent_id should be the real DB id of Foo
        assert_eq!(bar.parent_id, Some(foo.id));
    }

    #[test]
    fn test_upsert_symbols_replaces() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

        let v1 = vec![make_symbol("old_fn", SymbolKind::Function, None, 0)];
        store.upsert_symbols(file_id, &v1).unwrap();
        assert_eq!(store.symbol_count().unwrap(), 1);

        let v2 = vec![
            make_symbol("new_fn", SymbolKind::Function, None, 0),
            make_symbol("another", SymbolKind::Function, None, 1),
        ];
        store.upsert_symbols(file_id, &v2).unwrap();
        assert_eq!(store.symbol_count().unwrap(), 2);

        let all = store.query_symbols(&SymbolFilter::default()).unwrap();
        let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"new_fn"));
        assert!(names.contains(&"another"));
        assert!(!names.contains(&"old_fn"));
    }

    #[test]
    fn test_upsert_and_query_imports() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store
            .upsert_file(&make_entry("src/main.rs", "abc"))
            .unwrap();

        let imports = vec![
            ImportEntry {
                file_id,
                imported_name: "HashMap".to_string(),
                source_module: "std::collections".to_string(),
                alias: None,
                line: 1,
                kind: "use".to_string(),
            },
            ImportEntry {
                file_id,
                imported_name: "Result".to_string(),
                source_module: "anyhow".to_string(),
                alias: None,
                line: 2,
                kind: "use".to_string(),
            },
        ];

        let count = store.upsert_imports(file_id, &imports).unwrap();
        assert_eq!(count, 2);

        let got = store.get_file_imports(file_id).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].imported_name, "HashMap");

        let searched = store.query_imports("Hash").unwrap();
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].imported_name, "HashMap");
    }

    #[test]
    fn test_upsert_and_find_refs() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store
            .upsert_file(&make_entry("src/main.rs", "abc"))
            .unwrap();

        let refs = vec![
            SymbolRef {
                symbol_name: "Config".to_string(),
                file_id,
                file_path: String::new(),
                line: 10,
                col: 5,
                kind: "type_ref".to_string(),
            },
            SymbolRef {
                symbol_name: "Config".to_string(),
                file_id,
                file_path: String::new(),
                line: 20,
                col: 8,
                kind: "call".to_string(),
            },
        ];

        store.upsert_refs(file_id, &refs).unwrap();

        let found = store.find_references("Config").unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].line, 10);
        assert_eq!(found[1].line, 20);
    }

    #[test]
    fn test_file_deps() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store
            .upsert_file(&make_entry("src/main.rs", "abc"))
            .unwrap();

        let deps = vec![
            ("src/config.rs".to_string(), "use".to_string()),
            ("src/utils.rs".to_string(), "mod".to_string()),
        ];

        store.set_file_deps(file_id, &deps).unwrap();

        let dependents = store.get_dependents("src/config.rs").unwrap();
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0], file_id);

        let no_deps = store.get_dependents("nonexistent.rs").unwrap();
        assert!(no_deps.is_empty());
    }

    #[test]
    fn test_get_stats() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
        let symbols = vec![make_symbol("foo", SymbolKind::Function, None, 0)];
        store.upsert_symbols(file_id, &symbols).unwrap();

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.files_indexed, 1);
        assert_eq!(stats.total_symbols, 1);
        assert_eq!(stats.total_bytes, 100);
    }

    #[test]
    fn test_cascade_delete() {
        let store = IndexStore::open_in_memory().unwrap();
        let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

        let symbols = vec![make_symbol("foo", SymbolKind::Function, None, 0)];
        store.upsert_symbols(file_id, &symbols).unwrap();

        let imports = vec![ImportEntry {
            file_id,
            imported_name: "Bar".to_string(),
            source_module: "baz".to_string(),
            alias: None,
            line: 1,
            kind: "use".to_string(),
        }];
        store.upsert_imports(file_id, &imports).unwrap();

        assert_eq!(store.symbol_count().unwrap(), 1);

        // Deleting the file should cascade-delete symbols and imports.
        store.delete_file("a.rs").unwrap();
        assert_eq!(store.symbol_count().unwrap(), 0);
        assert!(store.get_file_imports(file_id).unwrap().is_empty());
    }

    #[test]
    fn test_get_file_id() {
        let store = IndexStore::open_in_memory().unwrap();
        assert!(store.get_file_id("nonexistent.rs").unwrap().is_none());

        store.upsert_file(&make_entry("src/lib.rs", "h1")).unwrap();
        let id = store.get_file_id("src/lib.rs").unwrap();
        assert!(id.is_some());
    }
}
