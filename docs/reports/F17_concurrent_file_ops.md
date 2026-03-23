F17 — Concurrent File Operations: Design & API Proposal

Status: Draft (Milestone M1 deliverable)

Overview
--------
Provide a safe, ergonomic API + implementation in ragent-core for performing parallel file reads and staged edits across many files. This enables faster multi-file workflows such as bulk refactors, repository-wide code-mods, and analysis passes while preserving repository integrity via atomic writes, conflict detection and recoverable rollbacks.

Survey of existing utilities (hotspots)
--------------------------------------
The following modules already perform file I/O and must be considered when integrating concurrent operations:

- crates/ragent-core/src/tool/read.rs — high-level file reading helper used by tools and agents. It provides logic for large-file summarisation and section detection.
- crates/ragent-core/src/tool/write.rs — low-level write tool used by agents to write files.
- crates/ragent-core/src/tool/edit.rs and multiedit.rs — tools that implement single- and multi-file edits. Their semantics (validation, patch application) are important to reuse.
- crates/ragent-core/src/tool/patch.rs — contains complex edit/patch application logic.

These modules provide behaviour we should reuse (section detection, permission checks, metadata, and tool interfaces) rather than reimplementing from scratch.

Design decisions
----------------
1) Runtime model: Tokio-based async IO for primary implementation
- Rationale: File operations are IO-bound. The repository already depends on tokio (see ragent-core Cargo.toml) and many tools use async functions. Building the first implementation on tokio::fs and async tasks keeps integration straightforward.
- For CPU-intensive transformations (e.g., large text diffs, AST transforms), provide an ergonomic path to offload to Rayon via spawning blocking tasks (tokio::task::spawn_blocking) or by offering sync helper APIs that library consumers can call inside a Rayon threadpool.

2) Concurrency primitives and limits
- Use an internal bounded task semaphore (tokio::sync::Semaphore) to limit concurrency to a configurable number (default: num_cpus::get()).
- Provide a high-level function that accepts a concurrency limit, permit count, and per-file timeout.

3) Locking & atomic writes
- Writes must be atomic: write to a temp file (tempfile crate), fsync the temp file (where available), then rename over the destination atomically when possible.
- For conflict detection and cooperative multi-process safety, provide optional advisory file locking using the fs2 crate (or a thin platform-specific abstraction). Advisory locks are optional and behind a feature flag; atomic write + checksum is the primary defense against races.

4) Conflict detection and merge strategy
- Use optimistic concurrency: read original contents and compute a stable checksum (blake3 or sha256). Before committing an edit, re-read the file and compare checksum; if changed, mark as conflict.
- Provide an automatic 3-way merge hook for text content (optional). Prefer to surface conflicts to callers and allow the caller to supply a merge function.
- Support a dry-run mode where edits are validated but not written.

5) Staging model
- Introduce EditStaging: in-memory representation of proposed changes. Staging holds original checksum, proposed content, and metadata (path, timestamp).
- A staging session can validate all edits, perform preflight checks (permission, path existence), and then commit changes concurrently.
- Provide an apply/commit step that writes all staged edits atomically per-file. If any file fails to commit, the system will attempt best-effort rollback for already-written files (restore from original content backups) and return a detailed error describing failures and conflicts.

6) Safety & recovery
- Before overwriting any file, create a backup copy in a work directory (target/temp/concurrent_edits/<uuid>/backup/) so we can restore on failure.
- Keep dry-run and verbose logging to allow operators to inspect planned edits.

Public API (proposal)
---------------------
The following API surface will be added to crates/ragent-core. Names and signatures are review proposals and meant to be ergonomic for internal callers (tools and higher-level agents).

Public types and functions

- pub struct ConcurrentFileReader { /* config (concurrency, timeout) */ }
  - impl ConcurrentFileReader {
      pub async fn read_paths(&self, paths: impl IntoIterator<Item=PathBuf>) -> anyhow::Result<Vec<FileReadResult>>;
      pub fn with_concurrency(self, n: usize) -> Self; // builder
    }

- pub struct FileReadResult {
    pub path: PathBuf,
    pub content: Option<String>, // None if failed
    pub err: Option<anyhow::Error>,
    pub lines: Option<usize>,
  }

- pub struct EditStaging {
    pub edits: Vec<StagedEdit>,
    pub dry_run: bool,
    pub backup_dir: PathBuf,
  }

- pub struct StagedEdit {
    pub path: PathBuf,
    pub original_checksum: String,
    pub original_content_preview: String,
    pub proposed_content: String,
  }

- impl EditStaging {
    pub fn new(dry_run: bool) -> Self;
    pub async fn stage_edit(&mut self, path: impl AsRef<Path>, new_content: String) -> anyhow::Result<()>;
    pub fn validate(&self) -> anyhow::Result<()>; // local checks
    pub async fn commit_all(&self, concurrency_limit: usize) -> anyhow::Result<CommitResult>;
  }

- pub struct CommitResult {
    pub applied: Vec<PathBuf>,
    pub conflicts: Vec<Conflict>,
    pub errors: Vec<(PathBuf, anyhow::Error)>,
  }

- pub struct Conflict { pub path: PathBuf, pub reason: String }

Planned feature flags
---------------------
- feature "file-locks": enable advisory locking via fs2 for cooperative multi-process writes.
- feature "blake3": use blake3 crate for checksums (optional; fallback sha256 in std).

Integration points
------------------
- Update tools/multiedit.rs and tools/edit.rs to use EditStaging for multi-file edits behind a feature flag.
- Update /simplify skill (or wrapper) to call ConcurrentFileReader::read_paths and EditStaging::commit_all when making batch edits.
- Add examples/parallel_edit.rs demonstrating read->modify->stage->commit flow.

Testing approach (M1->M2 handoff)
---------------------------------
- Unit tests for checksum and staging validation.
- Integration tests that create temporary repo fixtures (tests/fixtures/) and run concurrent edits with deterministic conflict injection.
- Benchmarks comparing serial edits vs concurrent edits on a fixture repo with hundreds of small files.

Open questions for review
------------------------
- Should we build the staging representation as full in-memory strings (simpler) or as diff deltas to reduce memory pressure for very large files?
- Which hash algorithm should be default (blake3 recommended for speed) and should it be a compile-time feature?
- Want to depend on fs2 for locks? It introduces native code. Alternative: advisory locks only for platforms where supported and gate behind feature flag.

Next steps (implementation M2)
------------------------------
- Choose concrete crates (blake3, fs2) and add to Cargo.toml behind features.
- Implement ConcurrentFileReader and EditStaging with Tokio-based parallelism and semaphore limiting.
- Implement atomic write strategy (tempfile + fsync + rename) and backup/rollback.
- Wire into multiedit tool behind feature gate and add example + tests.

Appendix: Example usage
-----------------------

// Pseudocode
let mut staging = EditStaging::new(false);
for path in repo_files {
    let content = reader.read_path(&path).await?.content.unwrap();
    let new_content = apply_codemod(&content);
    staging.stage_edit(&path, new_content).await?;
}
let res = staging.commit_all(num_cpus::get()).await?;
if !res.conflicts.is_empty() {
    // handle conflicts
}


Contact
-------
Design authored by: ragent core contributors (TBD)

