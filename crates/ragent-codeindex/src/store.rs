//! SQLite-backed index storage for files and symbols.
//!
//! [`IndexStore`] manages a local SQLite database that tracks which files
//! have been indexed, their content hashes, and (in later milestones)
//! the symbols extracted from them.

use crate::types::{
    Confidence, FileEntry, GraphEdge, ImportEntry, ScannedFile, StaleDiff, Symbol, SymbolFilter,
    SymbolKind, SymbolRef, Visibility,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

/// Current schema version — bump when migrating.
const SCHEMA_VERSION: i32 = 3;

/// Persistent store for the code index, backed by `SQLite`.
pub struct IndexStore {
    /// The SQLite connection.
    ///
    /// Exposed as `pub(crate)` so that the graph edge-derivation code in
    /// [`crate::graph::edges`] can issue a `ROLLBACK` when a batch insert
    /// fails mid-transaction (the `begin_transaction` / `commit_transaction`
    /// methods only cover the happy path).
    pub(crate) conn: Connection,
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
    ///
    /// Sets performance-oriented pragmas:
    /// - `WAL` journal mode — dramatically faster writes than rollback journal.
    /// - `synchronous = NORMAL` — safe with WAL, avoids fsync on every commit.
    /// - `temp_store = MEMORY` — keeps temp tables/indexes in RAM.
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
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

            CREATE TABLE IF NOT EXISTS graph_edges (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                source_sym    INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
                target_sym    INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
                kind          TEXT NOT NULL,
                confidence    TEXT NOT NULL,
                source_file   INTEGER REFERENCES indexed_files(id) ON DELETE CASCADE,
                line          INTEGER,
                UNIQUE(source_sym, target_sym, kind)
            );

            CREATE INDEX IF NOT EXISTS idx_edges_source ON graph_edges(source_sym);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON graph_edges(target_sym);
            CREATE INDEX IF NOT EXISTS idx_edges_kind ON graph_edges(kind);

            CREATE TABLE IF NOT EXISTS communities (
                sym_id        INTEGER PRIMARY KEY REFERENCES symbols(id) ON DELETE CASCADE,
                community     INTEGER NOT NULL,
                label         TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_communities_community ON communities(community);
            ",
        )?;

        // Seed schema version if missing, or migrate if below current version.
        let current_version: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))?;
        if current_version == 0 {
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
        } else {
            // Update the schema version row if it is below the current version.
            // New tables are created idempotently above via CREATE TABLE IF NOT EXISTS,
            // so the migration is simply bumping the version number. Existing tables
            // are not altered (FR-025).
            let stored_version: i64 =
                self.conn
                    .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))?;
            if stored_version < i64::from(SCHEMA_VERSION) {
                self.conn.execute(
                    "UPDATE schema_version SET version = ?1 WHERE version = ?2",
                    params![SCHEMA_VERSION, stored_version],
                )?;
                debug!(
                    "codeindex schema migrated: v{} -> v{}",
                    stored_version, SCHEMA_VERSION
                );
            }
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
        // last_insert_rowid() returns 0 on UPDATE (no insert happened),
        // so always query for the actual id by path.
        let file_id: i64 = self.conn.query_row(
            "SELECT id FROM indexed_files WHERE path = ?1",
            [&entry.path],
            |row| row.get(0),
        )?;
        Ok(file_id)
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

    /// Get a file entry by its ID.
    pub fn get_file_by_id(&self, file_id: i64) -> Result<Option<FileEntry>> {
        let row = self.conn.query_row(
            "SELECT path, content_hash, byte_size, language, last_indexed, mtime_ns, line_count
             FROM indexed_files WHERE id = ?1",
            [file_id],
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
        );
        match row {
            Ok(r) => Ok(Some(raw_to_file_entry(r)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
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
    // NOTE: intentional duplication — see DUPPLAN.md Milestone J
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
            .filter_map(std::result::Result::ok)
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
                i64::from(sym.start_line),
                i64::from(sym.end_line),
                i64::from(sym.start_col),
                i64::from(sym.end_col),
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
                i64::from(sym.start_line),
                i64::from(sym.end_line),
                i64::from(sym.start_col),
                i64::from(sym.end_col),
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
    ///
    /// Queries directly by `file_id` for efficiency — this avoids loading
    /// all symbols into memory and filtering in Rust (O(N) per call).
    pub fn get_file_symbols(&self, file_id: i64) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.file_id, s.name, s.qualified_name, s.kind, s.visibility,
                    s.start_line, s.end_line, s.start_col, s.end_col,
                    s.parent_id, s.signature, s.doc_comment, s.body_hash
             FROM symbols s
             WHERE s.file_id = ?1
             ORDER BY s.start_line",
        )?;

        let rows = stmt.query_map([file_id], |row| {
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
                i64::from(imp.line),
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
                i64::from(r.line),
                i64::from(r.col),
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

    /// Return all references across all files.
    pub fn query_all_refs(&self) -> Result<Vec<SymbolRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.symbol_name, r.file_id, r.line, r.col, r.kind,
                    COALESCE(f.path, '') as file_path
             FROM symbol_refs r
             LEFT JOIN indexed_files f ON f.id = r.file_id
             ORDER BY f.path, r.line",
        )?;

        let rows = stmt.query_map([], |row| {
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

    /// Return all references for a single file, filtered at the SQL level.
    ///
    /// This avoids loading the entire `symbol_refs` table when only one
    /// file's refs are needed (the incremental `index_file` path).
    pub fn get_file_refs(&self, file_id: i64) -> Result<Vec<SymbolRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.symbol_name, r.file_id, r.line, r.col, r.kind,
                    COALESCE(f.path, '') as file_path
             FROM symbol_refs r
             LEFT JOIN indexed_files f ON f.id = r.file_id
             WHERE r.file_id = ?1
             ORDER BY r.line",
        )?;

        let rows = stmt.query_map([file_id], |row| {
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

    /// Get the dependencies of a source file as `(target_path, kind)` pairs.
    ///
    /// Read-only accessor for the `file_deps` table; used by the graph-layer
    /// read-only verification (spec graphCI, T-030, FR-025).
    pub fn get_file_deps(&self, source_file_id: i64) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT target_path, kind FROM file_deps WHERE source_file_id = ?1
             ORDER BY target_path, kind",
        )?;
        let rows = stmt.query_map([source_file_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Return the current stored schema version.
    ///
    /// Read-only accessor for the `schema_version` table; used by the
    /// graph-layer read-only verification (spec graphCI, T-030, FR-025).
    pub fn schema_version(&self) -> Result<i64> {
        let version: i64 =
            self.conn
                .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))?;
        Ok(version)
    }

    // ── Graph Edges ─────────────────────────────────────────────────────────

    /// Insert or replace a semantic edge between two symbols.
    ///
    /// The `kind` is a string label such as `"calls"`, `"imports"`,
    /// `"inherits"`, `"references"`, `"mixes_in"`, or `"implements"`.
    /// The `confidence` is `"EXTRACTED"` or `"INFERRED"`.
    pub fn upsert_edge(
        &self,
        source_sym: i64,
        target_sym: i64,
        kind: &str,
        confidence: &str,
        source_file: Option<i64>,
        line: Option<u32>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO graph_edges (source_sym, target_sym, kind, confidence, source_file, line)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_sym, target_sym, kind) DO UPDATE SET
                 confidence = excluded.confidence,
                 source_file = excluded.source_file,
                 line = excluded.line",
            params![
                source_sym,
                target_sym,
                kind,
                confidence,
                source_file,
                line.map(i64::from),
            ],
        )?;
        Ok(())
    }

    /// Insert or replace a semantic edge using typed [`EdgeKind`] and
    /// [`Confidence`] values.
    pub fn upsert_edge_typed(&self, edge: &GraphEdge) -> Result<()> {
        self.upsert_edge(
            edge.source_sym,
            edge.target_sym,
            &edge.kind.to_string(),
            &edge.confidence.to_string(),
            edge.source_file,
            edge.line,
        )
    }

    /// Bulk-insert edges using a prepared statement inside an explicit
    /// transaction.
    ///
    /// The caller must call [`begin_transaction`] before this method and
    /// [`commit_transaction`] after it.  This avoids per-edge auto-commit
    /// overhead, which is the dominant cost when persisting thousands of
    /// edges during a full reindex.
    ///
    /// [`begin_transaction`]: IndexStore::begin_transaction
    /// [`commit_transaction`]: IndexStore::commit_transaction
    pub fn upsert_edges_batch(&self, edges: &[GraphEdge]) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO graph_edges (source_sym, target_sym, kind, confidence, source_file, line)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_sym, target_sym, kind) DO UPDATE SET
                 confidence = excluded.confidence,
                 source_file = excluded.source_file,
                 line = excluded.line",
        )?;
        for edge in edges {
            stmt.execute(params![
                edge.source_sym,
                edge.target_sym,
                edge.kind.to_string(),
                edge.confidence.to_string(),
                edge.source_file,
                edge.line.map(i64::from),
            ])?;
        }
        Ok(())
    }

    /// Delete all edges whose source or target symbol belongs to the given
    /// set of symbol IDs.  Used during incremental re-index to refresh edges
    /// for a single file.
    pub fn delete_edges_for_symbols(&self, symbol_ids: &[i64]) -> Result<()> {
        if symbol_ids.is_empty() {
            return Ok(());
        }
        // Build a parameterised IN-clause with unique positional placeholders.
        // We need symbol_ids.len() placeholders for the source IN-clause and
        // another symbol_ids.len() for the target IN-clause, so the total
        // parameter count is 2 * symbol_ids.len().
        let n = symbol_ids.len();
        let source_placeholders: Vec<String> = (1..=n).map(|i| format!("?{i}")).collect();
        let target_placeholders: Vec<String> = (n + 1..=2 * n).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "DELETE FROM graph_edges WHERE source_sym IN ({}) OR target_sym IN ({})",
            source_placeholders.join(", "),
            target_placeholders.join(", "),
        );
        let params: Vec<i64> = symbol_ids
            .iter()
            .chain(symbol_ids.iter())
            .copied()
            .collect();
        let params_ref: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        self.conn.execute(&sql, params_ref.as_slice())?;
        Ok(())
    }

    /// Delete all edges.  Used before re-deriving the full graph.
    pub fn clear_edges(&self) -> Result<()> {
        self.conn.execute("DELETE FROM graph_edges", [])?;
        Ok(())
    }

    /// Return the total number of edges in the `graph_edges` table.
    pub fn edge_count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM graph_edges", [], |r| r.get(0))?;
        Ok(count as u64)
    }

    /// Return the number of edges filtered by confidence tag.
    pub fn edge_count_by_confidence(&self, confidence: &str) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE confidence = ?1",
            [confidence],
            |r| r.get(0),
        )?;
        Ok(count as u64)
    }

    /// Return the number of edges filtered by typed [`Confidence`].
    pub fn edge_count_by_confidence_typed(&self, confidence: Confidence) -> Result<u64> {
        self.edge_count_by_confidence(&confidence.to_string())
    }

    /// Return the number of edges filtered by edge kind (e.g. "calls",
    /// "imports").
    pub fn edge_count_by_kind(&self, kind: &str) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE kind = ?1",
            [kind],
            |r| r.get(0),
        )?;
        Ok(count as u64)
    }

    /// Return the number of distinct symbols that appear as either the source
    /// or target of at least one edge (i.e. the number of nodes in the graph).
    pub fn graph_node_count(&self) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM (
                SELECT source_sym AS s FROM graph_edges
                UNION
                SELECT target_sym AS s FROM graph_edges
            )",
            [],
            |r| r.get(0),
        )?;
        Ok(count as u64)
    }

    /// Query all edges where the given symbol is either the source or the
    /// target.  Returns typed [`GraphEdge`] values.
    pub fn query_edges_for_symbol_typed(&self, symbol_id: i64) -> Result<Vec<GraphEdge>> {
        let rows = self.query_edges_for_symbol(symbol_id)?;
        let mut out = Vec::with_capacity(rows.len());
        for (src, tgt, kind, conf, sf, line) in rows {
            out.push(GraphEdge {
                source_sym: src,
                target_sym: tgt,
                kind: kind.parse()?,
                confidence: conf.parse()?,
                source_file: sf,
                line: line.map(|l| l as u32),
            });
        }
        Ok(out)
    }

    /// Query all edges in the graph.  Returns typed [`GraphEdge`] values.
    pub fn query_all_edges_typed(&self) -> Result<Vec<GraphEdge>> {
        let rows = self.query_all_edges()?;
        let mut out = Vec::with_capacity(rows.len());
        for (src, tgt, kind, conf, sf, line) in rows {
            out.push(GraphEdge {
                source_sym: src,
                target_sym: tgt,
                kind: kind.parse()?,
                confidence: conf.parse()?,
                source_file: sf,
                line: line.map(|l| l as u32),
            });
        }
        Ok(out)
    }

    /// Query all edges where the given symbol is either the source or the
    /// target.  Returns tuples of `(source_sym, target_sym, kind, confidence,
    /// source_file, line)`.
    pub fn query_edges_for_symbol(
        &self,
        symbol_id: i64,
    ) -> Result<Vec<(i64, i64, String, String, Option<i64>, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_sym, target_sym, kind, confidence, source_file, line
             FROM graph_edges
             WHERE source_sym = ?1 OR target_sym = ?1",
        )?;
        let rows = stmt.query_map([symbol_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Query all edges in the graph.  Returns tuples of `(source_sym,
    /// target_sym, kind, confidence, source_file, line)`.
    pub fn query_all_edges(
        &self,
    ) -> Result<Vec<(i64, i64, String, String, Option<i64>, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_sym, target_sym, kind, confidence, source_file, line
             FROM graph_edges",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ── Communities ──────────────────────────────────────────────────────────

    /// Insert or replace a community assignment for a symbol.
    pub fn upsert_community(&self, sym_id: i64, community: i64, label: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO communities (sym_id, community, label)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(sym_id) DO UPDATE SET
                 community = excluded.community,
                 label = excluded.label",
            params![sym_id, community, label],
        )?;
        Ok(())
    }

    /// Delete all community assignments.  Used before re-running community
    /// detection.
    pub fn clear_communities(&self) -> Result<()> {
        self.conn.execute("DELETE FROM communities", [])?;
        Ok(())
    }

    /// Return the number of distinct communities.
    pub fn community_count(&self) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT community) FROM communities",
            [],
            |r| r.get(0),
        )?;
        Ok(count as u64)
    }

    /// Return all community assignments as `(sym_id, community, label)`.
    pub fn query_all_communities(&self) -> Result<Vec<(i64, i64, Option<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT sym_id, community, label FROM communities")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Return all symbols in a given community as `(sym_id, label)`.
    pub fn query_community_members(&self, community: i64) -> Result<Vec<(i64, Option<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT sym_id, label FROM communities WHERE community = ?1")?;
        let rows = stmt.query_map([community], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ── Aggregate stats ─────────────────────────────────────────────────────

    /// Get the `file_id` for a given path.
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

/// Raw row from `SQLite` before type conversion.
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
        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
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

/// Raw symbol row from `SQLite` before type conversion.
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
