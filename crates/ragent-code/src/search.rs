//! Full-text search index backed by tantivy.
//!
//! [`FtsIndex`] manages a tantivy index that provides fast full-text
//! search over extracted code symbols — names, signatures, doc comments,
//! and body snippets.

use anyhow::{Context, Result};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, NumericOptions, Schema, TextFieldIndexing, TextOptions, Value,
};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

/// Maximum characters to keep from a symbol body for the FTS index.
const BODY_SNIPPET_LEN: usize = 500;

/// A single search result returned by [`FtsIndex::search`].
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Simple symbol name.
    pub symbol_name: String,
    /// Fully qualified name, if available.
    pub qualified_name: String,
    /// Symbol kind (e.g. "function", "struct").
    pub kind: String,
    /// Relative file path.
    pub file_path: String,
    /// Start line in the file.
    pub line: u32,
    /// End line in the file.
    pub end_line: u32,
    /// Tantivy relevance score.
    pub score: f32,
    /// Signature string, if available.
    pub signature: String,
    /// Doc comment snippet, if available.
    pub doc_snippet: String,
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            // Detailed mode: {:#}
            writeln!(
                f,
                "{} {} — {}:{}",
                self.kind, self.symbol_name, self.file_path, self.line
            )?;
            if !self.qualified_name.is_empty() {
                writeln!(f, "  qualified: {}", self.qualified_name)?;
            }
            if !self.signature.is_empty() {
                writeln!(f, "  signature: {}", self.signature)?;
            }
            if !self.doc_snippet.is_empty() {
                writeln!(f, "  doc: {}", self.doc_snippet)?;
            }
            write!(f, "  score: {:.3}", self.score)
        } else {
            // Compact mode: {}
            write!(
                f,
                "{} {} — {}:{}",
                self.kind, self.symbol_name, self.file_path, self.line
            )
        }
    }
}

/// Field identifiers for the tantivy schema.
struct FtsFields {
    name: Field,
    qualified_name: Field,
    kind: Field,
    file_path: Field,
    signature: Field,
    doc_comment: Field,
    body_snippet: Field,
    start_line: Field,
    end_line: Field,
}

/// Full-text search index backed by tantivy.
pub struct FtsIndex {
    index: Index,
    reader: IndexReader,
    fields: FtsFields,
}

impl FtsIndex {
    /// Open (or create) a tantivy index on disk.
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("cannot create FTS directory: {}", path.display()))?;
        let schema = Self::build_schema();
        let index = Self::open_or_create(path, &schema)?;
        Self::from_index(index, schema)
    }

    /// Open an in-memory index (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let schema = Self::build_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::from_index(index, schema)
    }

    /// Add symbols to the FTS index. Each element is `(Symbol-like fields, file_path)`.
    ///
    /// Call `commit()` afterwards to make them searchable.
    pub fn add_symbols(&self, symbols: &[FtsSymbol<'_>]) -> Result<()> {
        let mut writer = self.writer()?;
        for sym in symbols {
            let mut doc = TantivyDocument::default();
            doc.add_text(self.fields.name, sym.name);
            doc.add_text(self.fields.qualified_name, sym.qualified_name.unwrap_or(""));
            doc.add_text(self.fields.kind, sym.kind);
            doc.add_text(self.fields.file_path, sym.file_path);
            doc.add_text(self.fields.signature, sym.signature.unwrap_or(""));
            doc.add_text(self.fields.doc_comment, sym.doc_comment.unwrap_or(""));
            let snippet = sym.body_snippet.unwrap_or("");
            let truncated = &snippet[..snippet.len().min(BODY_SNIPPET_LEN)];
            doc.add_text(self.fields.body_snippet, truncated);
            doc.add_i64(self.fields.start_line, sym.start_line as i64);
            doc.add_i64(self.fields.end_line, sym.end_line as i64);
            writer.add_document(doc)?;
        }
        writer.commit()?;
        Ok(())
    }

    /// Remove all entries for a given file path.
    pub fn remove_file(&self, file_path: &str) -> Result<()> {
        let mut writer = self.writer()?;
        let term = tantivy::Term::from_field_text(self.fields.file_path, file_path);
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }

    /// Batch-update the FTS index: remove old entries for the given files,
    /// then add new symbols, using a single writer and commit.
    ///
    /// Much faster than calling `remove_file()` + `add_symbols()` per file
    /// because it avoids per-file writer allocation and commit overhead.
    pub fn batch_update(&self, remove_paths: &[&str], symbols: &[FtsSymbol<'_>]) -> Result<()> {
        let mut writer = self.writer()?;

        for path in remove_paths {
            let term = tantivy::Term::from_field_text(self.fields.file_path, path);
            writer.delete_term(term);
        }

        for sym in symbols {
            let mut doc = TantivyDocument::default();
            doc.add_text(self.fields.name, sym.name);
            doc.add_text(self.fields.qualified_name, sym.qualified_name.unwrap_or(""));
            doc.add_text(self.fields.kind, sym.kind);
            doc.add_text(self.fields.file_path, sym.file_path);
            doc.add_text(self.fields.signature, sym.signature.unwrap_or(""));
            doc.add_text(self.fields.doc_comment, sym.doc_comment.unwrap_or(""));
            let snippet = sym.body_snippet.unwrap_or("");
            let truncated = &snippet[..snippet.len().min(BODY_SNIPPET_LEN)];
            doc.add_text(self.fields.body_snippet, truncated);
            doc.add_i64(self.fields.start_line, sym.start_line as i64);
            doc.add_i64(self.fields.end_line, sym.end_line as i64);
            writer.add_document(doc)?;
        }

        writer.commit()?;
        Ok(())
    }

    /// Delete all documents from the FTS index.
    pub fn clear(&self) -> Result<()> {
        let mut writer = self.writer()?;
        writer.delete_all_documents()?;
        writer.commit()?;
        Ok(())
    }

    /// Search the FTS index with the given query string.
    ///
    /// Fields are boosted: name 10×, qualified_name 5×, signature 3×,
    /// doc_comment 2×, body_snippet 1×.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.reader.reload()?;
        let searcher = self.reader.searcher();
        let total_docs = searcher.num_docs();
        tracing::debug!(
            query = %query,
            limit = limit,
            docs_in_index = total_docs,
            "FTS search starting"
        );

        let mut parser = QueryParser::for_index(
            &self.index,
            vec![
                self.fields.name,
                self.fields.qualified_name,
                self.fields.signature,
                self.fields.doc_comment,
                self.fields.body_snippet,
            ],
        );
        parser.set_field_boost(self.fields.name, 10.0);
        parser.set_field_boost(self.fields.qualified_name, 5.0);
        parser.set_field_boost(self.fields.signature, 3.0);
        parser.set_field_boost(self.fields.doc_comment, 2.0);
        parser.set_field_boost(self.fields.body_snippet, 1.0);

        let parsed_query = parser
            .parse_query(query)
            .with_context(|| "cannot parse FTS query")?;

        let top_docs = searcher
            .search(&parsed_query, &TopDocs::with_limit(limit))
            .context("FTS search failed")?;

        let mut results = Vec::with_capacity(top_docs.len());
        for (score, addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(addr).context("cannot retrieve doc")?;
            results.push(SearchResult {
                symbol_name: self.get_text(&doc, self.fields.name),
                qualified_name: self.get_text(&doc, self.fields.qualified_name),
                kind: self.get_text(&doc, self.fields.kind),
                file_path: self.get_text(&doc, self.fields.file_path),
                line: self.get_i64(&doc, self.fields.start_line) as u32,
                end_line: self.get_i64(&doc, self.fields.end_line) as u32,
                score,
                signature: self.get_text(&doc, self.fields.signature),
                doc_snippet: self.get_text(&doc, self.fields.doc_comment),
            });
        }
        tracing::debug!(
            query = %query,
            results = results.len(),
            "FTS search complete"
        );
        Ok(results)
    }

    /// Return the total number of documents in the index.
    pub fn doc_count(&self) -> Result<u64> {
        self.reader.reload()?;
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs())
    }

    // ── Private helpers ─────────────────────────────────────────────────

    fn build_schema() -> Schema {
        let mut builder = Schema::builder();

        // TEXT fields — tokenized and searchable, stored for retrieval
        let text_opts = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        builder.add_text_field("name", text_opts.clone());
        builder.add_text_field("qualified_name", text_opts.clone());
        builder.add_text_field("signature", text_opts.clone());
        builder.add_text_field("doc_comment", text_opts.clone());

        // body_snippet: tokenized but NOT stored (too large)
        let body_opts = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("default")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
        builder.add_text_field("body_snippet", body_opts);

        // STRING fields — stored, not tokenized (exact match / filters)
        let string_opts = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            )
            .set_stored();

        builder.add_text_field("kind", string_opts.clone());
        builder.add_text_field("file_path", string_opts);

        // Numeric fields
        let i64_opts = NumericOptions::default().set_indexed().set_stored();
        builder.add_i64_field("start_line", i64_opts.clone());
        builder.add_i64_field("end_line", i64_opts);

        builder.build()
    }

    fn open_or_create(path: &Path, schema: &Schema) -> Result<Index> {
        let dir = tantivy::directory::MmapDirectory::open(path)
            .with_context(|| format!("cannot open tantivy dir: {}", path.display()))?;
        match Index::open(dir) {
            Ok(idx) => {
                // Validate that the on-disk schema matches our expected schema.
                // If field count differs, the index was created by a different code version;
                // delete and recreate to avoid silent field-ID mismatches.
                let disk_schema = idx.schema();
                let expected_field_count = schema.fields().count();
                let actual_field_count = disk_schema.fields().count();
                if actual_field_count != expected_field_count {
                    tracing::warn!(
                        "FTS schema mismatch: expected {} fields, found {}; recreating index",
                        expected_field_count,
                        actual_field_count,
                    );
                    drop(idx);
                    // Clear the directory and recreate.
                    for entry in std::fs::read_dir(path)?.flatten() {
                        let _ = std::fs::remove_file(entry.path());
                    }
                    let dir2 =
                        tantivy::directory::MmapDirectory::open(path).with_context(|| {
                            format!("cannot reopen tantivy dir: {}", path.display())
                        })?;
                    return Index::create(dir2, schema.clone(), Default::default())
                        .context("cannot create tantivy index");
                }
                Ok(idx)
            }
            Err(_) => {
                let dir2 = tantivy::directory::MmapDirectory::open(path)
                    .with_context(|| format!("cannot reopen tantivy dir: {}", path.display()))?;
                Index::create(dir2, schema.clone(), Default::default())
                    .context("cannot create tantivy index")
            }
        }
    }

    fn from_index(index: Index, schema: Schema) -> Result<Self> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .context("cannot build index reader")?;

        // Use the index's own schema for field lookups to ensure field IDs
        // match what's on disk, even if field insertion order differed.
        let idx_schema = index.schema();
        let fields = FtsFields {
            name: idx_schema
                .get_field("name")
                .or_else(|_| schema.get_field("name"))
                .unwrap(),
            qualified_name: idx_schema
                .get_field("qualified_name")
                .or_else(|_| schema.get_field("qualified_name"))
                .unwrap(),
            kind: idx_schema
                .get_field("kind")
                .or_else(|_| schema.get_field("kind"))
                .unwrap(),
            file_path: idx_schema
                .get_field("file_path")
                .or_else(|_| schema.get_field("file_path"))
                .unwrap(),
            signature: idx_schema
                .get_field("signature")
                .or_else(|_| schema.get_field("signature"))
                .unwrap(),
            doc_comment: idx_schema
                .get_field("doc_comment")
                .or_else(|_| schema.get_field("doc_comment"))
                .unwrap(),
            body_snippet: idx_schema
                .get_field("body_snippet")
                .or_else(|_| schema.get_field("body_snippet"))
                .unwrap(),
            start_line: idx_schema
                .get_field("start_line")
                .or_else(|_| schema.get_field("start_line"))
                .unwrap(),
            end_line: idx_schema
                .get_field("end_line")
                .or_else(|_| schema.get_field("end_line"))
                .unwrap(),
        };

        Ok(Self {
            index,
            reader,
            fields,
        })
    }

    fn writer(&self) -> Result<IndexWriter> {
        // 15 MB heap for the writer — small but sufficient for incremental updates
        self.index
            .writer(15_000_000)
            .context("cannot create index writer")
    }

    fn get_text(&self, doc: &TantivyDocument, field: Field) -> String {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_i64(&self, doc: &TantivyDocument, field: Field) -> i64 {
        doc.get_first(field).and_then(|v| v.as_i64()).unwrap_or(0)
    }
}

/// Lightweight struct for passing symbol data into the FTS index.
///
/// Borrows strings to avoid unnecessary allocation when converting
/// from `Symbol` + file path.
#[derive(Debug)]
pub struct FtsSymbol<'a> {
    /// Simple name.
    pub name: &'a str,
    /// Fully-qualified name.
    pub qualified_name: Option<&'a str>,
    /// Kind string (e.g. "function").
    pub kind: &'a str,
    /// Relative file path.
    pub file_path: &'a str,
    /// Signature string.
    pub signature: Option<&'a str>,
    /// Doc comment.
    pub doc_comment: Option<&'a str>,
    /// Body snippet (first N chars of the symbol body).
    pub body_snippet: Option<&'a str>,
    /// Start line.
    pub start_line: u32,
    /// End line.
    pub end_line: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_symbols() -> Vec<FtsSymbol<'static>> {
        vec![
            FtsSymbol {
                name: "parse_config",
                qualified_name: Some("crate::config::parse_config"),
                kind: "function",
                file_path: "src/config.rs",
                signature: Some("fn parse_config(path: &Path) -> Result<Config>"),
                doc_comment: Some("Parse the configuration file from disk."),
                body_snippet: Some("let content = fs::read_to_string(path)?;"),
                start_line: 10,
                end_line: 25,
            },
            FtsSymbol {
                name: "Config",
                qualified_name: Some("crate::config::Config"),
                kind: "struct",
                file_path: "src/config.rs",
                signature: Some("pub struct Config"),
                doc_comment: Some("Application configuration loaded from TOML."),
                body_snippet: Some("name: String, port: u16, debug: bool"),
                start_line: 1,
                end_line: 8,
            },
            FtsSymbol {
                name: "serve",
                qualified_name: Some("crate::server::serve"),
                kind: "function",
                file_path: "src/server.rs",
                signature: Some("pub async fn serve(config: &Config) -> Result<()>"),
                doc_comment: Some("Start the HTTP server."),
                body_snippet: Some("let listener = TcpListener::bind(config.addr()).await?;"),
                start_line: 15,
                end_line: 45,
            },
        ]
    }

    #[test]
    fn test_open_in_memory() {
        let fts = FtsIndex::open_in_memory().unwrap();
        assert_eq!(fts.doc_count().unwrap(), 0);
    }

    #[test]
    fn test_add_and_search_by_name() {
        let fts = FtsIndex::open_in_memory().unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();

        let results = fts.search("parse_config", 10).unwrap();
        assert!(!results.is_empty(), "should find parse_config");
        assert_eq!(results[0].symbol_name, "parse_config");
        assert_eq!(results[0].file_path, "src/config.rs");
        assert_eq!(results[0].line, 10);
    }

    #[test]
    fn test_search_by_doc_comment() {
        let fts = FtsIndex::open_in_memory().unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();

        let results = fts.search("HTTP server", 10).unwrap();
        assert!(!results.is_empty(), "should find by doc comment");
        assert_eq!(results[0].symbol_name, "serve");
    }

    #[test]
    fn test_search_by_body_snippet() {
        let fts = FtsIndex::open_in_memory().unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();

        let results = fts.search("read_to_string", 10).unwrap();
        assert!(!results.is_empty(), "should find by body snippet");
        assert_eq!(results[0].symbol_name, "parse_config");
    }

    #[test]
    fn test_search_by_signature() {
        let fts = FtsIndex::open_in_memory().unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();

        let results = fts.search("Result<Config>", 10).unwrap();
        assert!(!results.is_empty(), "should find by signature");
        assert_eq!(results[0].symbol_name, "parse_config");
    }

    #[test]
    fn test_remove_file() {
        let fts = FtsIndex::open_in_memory().unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();
        assert_eq!(fts.doc_count().unwrap(), 3);

        fts.remove_file("src/config.rs").unwrap();
        // After remove + commit, docs from config.rs should be gone.
        // Note: tantivy soft-deletes, so num_docs may still report them
        // until a merge; but search should not return them.
        let results = fts.search("parse_config", 10).unwrap();
        assert!(
            results.is_empty(),
            "parse_config should be gone after remove_file"
        );
    }

    #[test]
    fn test_search_limit() {
        let fts = FtsIndex::open_in_memory().unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();

        // Search for something matching all (a common term)
        let results = fts.search("config", 1).unwrap();
        assert!(results.len() <= 1, "limit should cap results");
    }

    #[test]
    fn test_name_boost_over_body() {
        let fts = FtsIndex::open_in_memory().unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();

        // "Config" is a name hit AND appears in body/doc of other symbols.
        // The name-match should rank highest.
        let results = fts.search("Config", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(
            results[0].symbol_name, "Config",
            "name match should rank first"
        );
    }

    #[test]
    fn test_display_compact() {
        let r = SearchResult {
            symbol_name: "foo".into(),
            qualified_name: "crate::foo".into(),
            kind: "function".into(),
            file_path: "src/lib.rs".into(),
            line: 42,
            end_line: 50,
            score: 1.5,
            signature: "fn foo()".into(),
            doc_snippet: "Does something.".into(),
        };
        let compact = format!("{r}");
        assert!(compact.contains("function foo"));
        assert!(compact.contains("src/lib.rs:42"));
    }

    #[test]
    fn test_display_detailed() {
        let r = SearchResult {
            symbol_name: "foo".into(),
            qualified_name: "crate::foo".into(),
            kind: "function".into(),
            file_path: "src/lib.rs".into(),
            line: 42,
            end_line: 50,
            score: 1.5,
            signature: "fn foo()".into(),
            doc_snippet: "Does something.".into(),
        };
        let detailed = format!("{r:#}");
        assert!(detailed.contains("qualified: crate::foo"));
        assert!(detailed.contains("signature: fn foo()"));
        assert!(detailed.contains("doc: Does something."));
    }

    #[test]
    fn test_open_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let fts_path = dir.path().join("fts");
        let fts = FtsIndex::open(&fts_path).unwrap();
        fts.add_symbols(&sample_symbols()).unwrap();
        assert_eq!(fts.doc_count().unwrap(), 3);
        drop(fts);

        // Re-open and verify data persisted
        let fts2 = FtsIndex::open(&fts_path).unwrap();
        let results = fts2.search("parse_config", 10).unwrap();
        assert!(!results.is_empty());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Mimic how full_reindex adds symbols: remove_file + add_symbols per file
    #[test]
    fn test_incremental_add_many_files() {
        let dir = tempfile::tempdir().unwrap();
        let fts_path = dir.path().join("fts");
        let fts = FtsIndex::open(&fts_path).unwrap();

        // Simulate 50 files, each with a few symbols
        for i in 0..50 {
            let file_path = format!("src/file_{i}.rs");
            fts.remove_file(&file_path).unwrap();
            let name = format!("func_{i}");
            let qname = format!("crate::mod_{i}::func_{i}");
            let syms = vec![FtsSymbol {
                name: &name,
                qualified_name: Some(&qname),
                kind: "function",
                file_path: &file_path,
                signature: Some("fn func() -> bool"),
                doc_comment: Some("A test function."),
                body_snippet: Some("let x = 42; return true;"),
                start_line: 10,
                end_line: 20,
            }];
            fts.add_symbols(&syms).unwrap();
        }

        let count = fts.doc_count().unwrap();
        eprintln!("doc_count after 50 files: {count}");
        assert_eq!(count, 50, "should have 50 docs");

        let results = fts.search("func_25", 10).unwrap();
        eprintln!("search for func_25: {} results", results.len());
        assert!(!results.is_empty(), "should find func_25");
        assert_eq!(results[0].symbol_name, "func_25");

        // Now drop and reopen to test persistence
        drop(fts);
        let fts2 = FtsIndex::open(&fts_path).unwrap();
        let count2 = fts2.doc_count().unwrap();
        eprintln!("doc_count after reopen: {count2}");
        assert_eq!(count2, 50, "should still have 50 docs after reopen");

        let results2 = fts2.search("func_25", 10).unwrap();
        eprintln!("search after reopen: {} results", results2.len());
        assert!(!results2.is_empty(), "should find func_25 after reopen");
    }
}
