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

    /// Build a document from a [`FtsSymbol`] for the writer.
    fn document_from_symbol(&self, sym: &FtsSymbol<'_>) -> TantivyDocument {
        let mut doc = TantivyDocument::default();
        doc.add_text(self.fields.name, sym.name);
        doc.add_text(self.fields.qualified_name, sym.qualified_name.unwrap_or(""));
        doc.add_text(self.fields.kind, sym.kind);
        doc.add_text(self.fields.file_path, sym.file_path);
        doc.add_text(self.fields.signature, sym.signature.unwrap_or(""));
        doc.add_text(self.fields.doc_comment, sym.doc_comment.unwrap_or(""));
        let snippet = sym.body_snippet.unwrap_or("");
        // Truncate on a UTF-8 char boundary so a multi-byte sequence at the
        // cut point cannot panic with "byte index is not a char boundary".
        let end = {
            let mut i = snippet.len().min(BODY_SNIPPET_LEN);
            while i > 0 && !snippet.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        doc.add_text(self.fields.body_snippet, &snippet[..end]);
        doc.add_i64(self.fields.start_line, i64::from(sym.start_line));
        doc.add_i64(self.fields.end_line, i64::from(sym.end_line));
        doc
    }

    /// Add symbols to the FTS index. Each element is `(Symbol-like fields, file_path)`.
    ///
    /// Call `commit()` afterwards to make them searchable.
    pub fn add_symbols(&self, symbols: &[FtsSymbol<'_>]) -> Result<()> {
        let mut writer = self.writer()?;
        for sym in symbols {
            writer.add_document(self.document_from_symbol(sym))?;
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
            writer.add_document(self.document_from_symbol(sym))?;
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
    /// Fields are boosted: name 10×, `qualified_name` 5×, signature 3×,
    /// `doc_comment` 2×, `body_snippet` 1×.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.reader.reload()?;
        let searcher = self.reader.searcher();
        let total_docs = searcher.num_docs();
        let sanitized = Self::sanitize_query(query);
        tracing::debug!(
            query = %query,
            sanitized = %sanitized,
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
            .parse_query(&sanitized)
            .with_context(|| format!("cannot parse FTS query: {sanitized}"))?;
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

    /// Escape Tantivy query-parser special characters so that raw
    /// code identifiers such as `Widget::new`, `std::io`, or `foo<T>`
    /// are treated as literal search terms.  Language scope operators
    /// (`::`) are replaced with spaces so each path segment becomes a
    /// separate search term.
    pub fn sanitize_query(raw: &str) -> String {
        // Replace :: with space first — the default tokenizer splits on
        // punctuation anyway, so the index never contains literal colons.
        let replaced = raw.replace("::", " ");
        let mut out = String::with_capacity(replaced.len() * 2);
        for ch in replaced.chars() {
            if matches!(
                ch,
                '+' | '-'
                    | '&'
                    | '|'
                    | '!'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '^'
                    | '"'
                    | '~'
                    | '*'
                    | '?'
                    | ':'
                    | '\\'
                    | '/'
            ) {
                out.push('\\');
            }
            out.push(ch);
        }
        out
    }

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
        builder.add_text_field("doc_comment", text_opts);

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
        let idx = match Index::open(dir) {
            Ok(idx) => idx,
            Err(e) => {
                // Preserve the real failure reason (corruption, version
                // mismatch, mmap error) before falling into the create branch.
                tracing::warn!(
                    "existing FTS index at {} unreadable ({e}); recreating",
                    path.display()
                );
                // Clear the directory and recreate.
                for entry in std::fs::read_dir(path)?.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
                let dir2 = tantivy::directory::MmapDirectory::open(path)
                    .with_context(|| format!("cannot reopen tantivy dir: {}", path.display()))?;
                return Index::create(dir2, schema.clone(), Default::default())
                    .context("cannot create tantivy index");
            }
        };
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
            let dir2 = tantivy::directory::MmapDirectory::open(path)
                .with_context(|| format!("cannot reopen tantivy dir: {}", path.display()))?;
            return Index::create(dir2, schema.clone(), Default::default())
                .context("cannot create tantivy index");
        }
        Ok(idx)
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
