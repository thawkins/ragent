# Performance Improvement Plan for ragent‑core

**Goal**: Reduce latency and CPU usage of the core runtime, especially the orchestrator, tool execution, snapshot handling, storage, and team coordination. The plan follows a phased approach: profiling → bottleneck reduction → refactoring → verification.

---

## 1. Baseline Profiling & Benchmarking

| Step | Action | Tool | Expected Outcome |
|------|--------|------|-----------------|
| 1.1 | Add a lightweight `tracing` span around `Coordinator::start_job_sync` and `Router::dispatch`. | `tracing`/`tracing-subscriber` | Ability to measure per‑job overhead and identify hot paths.
| 1.2 | Run `cargo bench` (existing bench `bench_file_ops.rs`) and add new benches for:
|   - Orchestrator job submission (`bench_orchestrator.rs`)
|   - Tool execution (grep, read, bash) (`bench_tools.rs`)
|   - Snapshot creation (`bench_snapshot.rs`)
|   - Team message routing (`bench_team.rs`) | `criterion` | Quantitative baseline numbers (ns/μs per operation).
| 1.3 | Use `perf` or `cargo flamegraph` to capture stack traces for real‑world workloads (e.g., running a typical session with a few agents). | `perf`, `flamegraph` | Visual representation of where CPU time is spent.

All results should be stored in `docs/plan/perf-baseline.md` for later comparison.

---

## 2. Reduce Lock Contention in the Orchestrator

### 2.1 Replace `RwLock<HashMap<JobEntry>>` with `dashmap`
* `dashmap` provides sharded lock‑free access, reducing contention when many jobs are started concurrently.
* Update `src/orchestrator/coordinator.rs` to use `DashMap<JobId, JobEntry>`.
* Add unit tests to verify behaviour parity.

### 2.2 Batch Job Events
* Currently each state change emits an individual `JobEvent` via a broadcast channel. Create a small buffer (`EventBatcher`) that coalesces events occurring within a 5 ms window.
* Modify `src/orchestrator/event.rs` to support `batch_send`.
* Benchmark impact on event throughput.

---

## 3. Optimize Tool Execution Path

### 3.1 File‑I/O Caching
* Introduce a simple in‑memory LRU cache (`lru::LruCache<PathBuf, Arc<String>>`) for `read` and `grep` results.
* Cache key = `(file_path, start_line, end_line)` for `read` and `(file_path, pattern)` for `grep`.
* Cache size configurable via `ragent.json` (default 10 MiB).

### 3.2 Parallelise Heavy Tools
* `grep` and `glob` can be parallelised across CPU cores using `rayon`.
* Update `src/tool/grep.rs` and `src/tool/glob.rs` to use `rayon::par_bridge` when the file count exceeds a threshold (e.g., >1000 files).

### 3.3 Reduce Subprocess Overhead for `bash`
* Reuse a single persistent `sh` process for short commands using `std::process::Stdio::piped` and keep the child alive across calls (similar to a REPL).
* Fallback to spawning a new process for long‑running commands.

---

## 4. Snapshot & Storage Improvements

### 4.1 Incremental Snapshots
* Current snapshots copy the whole state directory. Implement delta‑based snapshots using `similar` crate to store only changed files.
* Add a `snapshot::incremental_save` function and modify `src/snapshot/mod.rs`.

### 4.2 Asynchronous Persistence
* Convert `storage::write` operations to use `tokio::fs::File` with async writes, avoiding blocking the orchestrator thread.
* Ensure tests cover race conditions.

---

## 5. Team Coordination Optimisation

### 5.1 Message Queue Back‑pressure
* Introduce bounded `tokio::sync::mpsc` channels with configurable capacity (default 100) for team messages.
* When full, apply back‑pressure by awaiting `send` instead of dropping messages.

### 5.2 Reduce Serialization Overhead
* `transport.rs` currently uses `serde_json`. Switch to `bincode` for internal in‑process messages (no need for human‑readable format). Keep JSON for external API.

---

## 6. Verification & CI Integration

| Phase | CI Step |
|-------|---------|
| 6.1 | Add new benchmark jobs to GitHub Actions using `criterion` and `cargo flamegraph`. Fail build if regression > 10 % over baseline.
| 6.2 | Run `cargo clippy` and `cargo fmt --check` to keep code quality.
| 6.3 | Extend existing test suite with performance regression tests using `assert_eq!` on benchmark timings (allow small epsilon).

---

## 7. Documentation & Knowledge Transfer

* Update `docs/plan/update-core.md` with this plan (the current file).
* Add a `docs/performance/benchmark‑guide.md` describing how to run the benchmarks locally.
* Record a short video walkthrough and place a link in `README.md` under a new **Performance** section.

---

## 8. Timeline (estimated effort)

| Milestone | Duration |
|-----------|----------|
| Profiling & Baseline | 1 week |
| Orchestrator lock refactor | 1 week |
| Tool execution cache & parallelism | 2 weeks |
| Snapshot & storage async changes | 1 week |
| Team coordination tweaks | 1 week |
| CI & verification integration | 1 week |
| Documentation & cleanup | 0.5 week |

**Total**: ~7.5 weeks (≈ 1.5 months).

---

### Success Criteria
* Median job submission latency reduced by **≥ 30 %**.
* CPU usage for a standard session (5 agents, 20 tool calls) reduced by **≥ 20 %**.
* No new race conditions; all existing tests pass and new performance tests succeed.
* Updated documentation and CI checks are merged.

---

*Prepared by the Rust Agent following the project’s AGENTS.md guidelines.*
