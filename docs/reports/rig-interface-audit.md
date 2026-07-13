# Rig Integration Interface Audit (T-002)

**Spec:** `rig`  
**Task:** T-002 — Audit `ragent-llm`, `ragent-agent`, `ragent-codeindex`,
`ragent-research`, and `ragent-storage` interfaces  
**Requirements covered:** FR-004, FR-011, FR-015, FR-016  
**Status:** Reference document (read-only audit — no code changes)

---

## Purpose

This document inventories the **exact public types, traits, and function
signatures** that the `ragent-rig` adapter must map onto or augment. It is the
empirical basis for T-003 (internal adapter traits), T-004 (Rig
`CompletionModel` adapter), T-007 (embedding adapter), T-008 (vector-store
adapter), T-009/T-010 (codeindex/memory wiring), and T-012 (research semantic
retrieval).

Each section lists the canonical home crate/module, the relevant signatures,
and a one-line note on how Rig maps onto or augments it.

---

## 1. `ragent-llm` — completion requests, responses, and streaming

The provider abstraction lives in two places:

- `ragent-types::llm` owns the **primitive** request/response/streaming types
  (no `futures`/`anyhow` dependency).
- `ragent-llm::llm` re-exports those primitives and defines the
  **streaming-client trait** `LlmClient`.

### 1.1 Primitive types — `crates/ragent-types/src/llm.rs`

```rust
// line 33
pub enum StreamEvent {
    ReasoningStart,
    ReasoningDelta { text: String },
    ReasoningEnd,
    TextDelta    { text: String },
    ToolCallStart  { id: String, name: String },
    ToolCallDelta  { id: String, args_json: String },
    ToolCallEnd    { id: String },
    Usage          { input_tokens: u64, output_tokens: u64 },
    RateLimit      { requests_used_pct: Option<f32>, tokens_used_pct: Option<f32> },
    Error          { message: String },
    Finish         { reason: FinishReason },
}

// line 98
pub struct ChatRequest {
    pub model: String,
    pub messages: Arc<Vec<ChatMessage>>,
    pub tools: Arc<Vec<ToolDefinition>>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub system: Option<Arc<str>>,
    pub options: HashMap<String, serde_json::Value>,
    pub session_id: Option<String>,      // #[serde(skip)]
    pub request_id: Option<String>,     // #[serde(skip)]
    pub stream_timeout_secs: Option<u64>, // #[serde(skip)]
    pub thinking: Option<ThinkingConfig>,
}

// line 145
pub struct ChatMessage { pub role: String, pub content: ChatContent }

// line 158  (untagged)
pub enum ChatContent { Text(String), Parts(Vec<ContentPart>) }

// line 170  (tag = "type", snake_case)
pub enum ContentPart {
    Text     { text: String },
    ToolUse  { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: Arc<str> },
    ImageUrl { url: String },
}

// line 205
pub struct ToolDefinition { pub name: String, pub description: String, pub parameters: serde_json::Value }

// line 17 (re-exported as LlmFinishReason)
pub enum FinishReason { Stop, ToolUse, /* … */ }
```

> **Rig mapping (FR-004/FR-005):** The adapter converts
> `ChatRequest` + `Vec<ChatMessage>`/`ChatContent` into Rig's
> `CompletionRequest`/`Message` types, and converts Rig's
> `CompletionResponse` back into `ChatContent::Parts` / `ContentPart::ToolUse`.
> Streaming Rig chunks are mapped onto `StreamEvent::TextDelta` /
> `ToolCallDelta` / `Usage` / `Finish`.

### 1.2 Streaming client trait — `crates/ragent-llm/src/llm.rs:32`

```rust
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>>;
}
```

> **Rig mapping (FR-012):** A Rig-backed provider implements `LlmClient` by
> delegating to Rig's `CompletionModel::stream(...)`, mapping each Rig
> streaming chunk into a `StreamEvent`.

### 1.3 Provider trait + registry — `crates/ragent-llm/src/providers/mod.rs:93`

```rust
pub struct ModelInfo { /* id, provider_id, name, cost, capabilities, context_window, max_output, request_multiplier, thinking_config */ }
pub struct ProviderInfo { pub id: String, pub name: String, pub models: Vec<ModelInfo> }
pub struct UsageInfo { pub plan: Option<String>, pub percent: Option<f32> }

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn default_models(&self) -> Vec<ModelInfo>;
    fn set_event_bus(&self, _event_bus: Option<Arc<EventBus>>) {}
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> { Ok(self.default_models()) }
    async fn create_client(
        &self, api_key: &str, base_url: Option<&str>, options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>>;
    async fn fetch_usage(&self, api_key: &str) -> Option<UsageInfo> { None }
}

pub struct ProviderRegistry { providers: HashMap<String, Box<dyn Provider>> }
impl ProviderRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, provider: Box<dyn Provider>);
    pub fn set_event_bus_all(&self, event_bus: Option<Arc<EventBus>>);
    pub fn get(&self, id: &str) -> Option<&dyn Provider>;
    pub fn list(&self) -> Vec<ProviderInfo>;
    pub fn resolve_model(&self, provider_id: &str, model_id: &str) -> Option<ModelInfo>;
}
pub fn create_default_registry() -> ProviderRegistry;
```

> **Rig mapping (FR-003/FR-012):** A `RigProvider` struct implements `Provider`,
> returning a `RigLlmClient` from `create_client`. It registers itself in
> `ProviderRegistry` alongside the native providers so routing is transparent.
> `default_models()` can surface Rig-known models for the configured providers.

### 1.4 Shared request handle — `crates/ragent-llm/src/shared_request.rs:21`

```rust
pub struct SharedChatRequest {
    pub messages: Arc<Vec<ChatMessage>>,
    pub tools: Arc<Vec<ToolDefinition>>,
}
impl SharedChatRequest {
    pub fn new(messages: Vec<ChatMessage>, tools: Vec<ToolDefinition>) -> Self;
    pub fn from_arc(messages: Arc<Vec<ChatMessage>>, tools: Arc<Vec<ToolDefinition>>) -> Self;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

> **Note:** Cheap-clone handle sharing request body with the cancellation
> guard. The Rig adapter receives a `ChatRequest` (which already owns
> `Arc<Vec<...>>`), so no new sharing primitive is required — Rig consumes the
> borrowed view.

### 1.5 Existing mock client — `crates/ragent-llm/src/providers/mock_llm_client.rs`

```rust
pub enum MockScenario { SimpleTextReply, SingleToolCall, MultiStepLoop, Empty }
pub struct MockLlmClient { /* … */ }
impl LlmClient for MockLlmClient { /* deterministic scenario playback */ }
```

> **Rig mapping (FR-025/NFR-005):** Rig's mock models/VCR cassettes can either
> replace or complement this. The existing `MockLlmClient` remains valid for
> non-Rig paths; the Rig mock harness (T-014/T-015) is additive.

---

## 2. `ragent-agent` — session, message, memory, and compression

### 2.1 Conversation message — `crates/ragent-types/src/message/mod.rs`

```rust
// line 15
pub enum Role { /* User, Assistant, System, Tool */ }
// line 39
pub enum ToolCallStatus { /* … */ }
pub struct ToolCallState { /* tool, call_id, state */ }
// line 122 (tag = "type", snake_case)
pub enum MessagePart {
    Text { text: String },
    ToolCall { tool: String, call_id: String, state: ToolCallState },
    Reasoning { text: String },
    Image(Box<ImageData>),
}
// line 158
pub struct Message {
    pub id: String, pub session_id: String, pub role: Role,
    pub parts: Vec<MessagePart>, pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
}
```

> **Rig mapping (FR-004):** `Message`/`MessagePart` is the *session-persisted*
> form; `ChatMessage`/`ChatContent` is the *LLM-wire* form. The adapter converts
> `Vec<Message>` → `Vec<ChatMessage>` at request-build time (already done by
> the agent loop). Rig receives the `ChatMessage` view, so no new conversion is
> needed — the adapter only translates `ChatMessage` ⇄ Rig `Message`.

### 2.2 Session manager — `crates/ragent-agent/src/session/mod.rs`

```rust
pub struct Session { /* id, directory, … */ }
pub struct SessionSummary { /* … */ }
pub struct SessionManager { /* owns Storage */ }
impl SessionManager {
    pub fn session_state_cache(&self, session_id: &str) -> Arc<Mutex<SessionState>>;
    pub fn create_session(&self, directory: PathBuf) -> anyhow::Result<Session>;
    pub fn get_session(&self, id: &str) -> anyhow::Result<Option<Session>>;
    pub fn list_sessions(&self) -> anyhow::Result<Vec<Session>>;
    pub fn archive_session(&self, id: &str) -> anyhow::Result<()>;
    pub fn get_messages(&self, session_id: &str) -> anyhow::Result<Vec<Message>>;
}
```

### 2.3 History trimming — `crates/ragent-agent/src/session/history.rs`

```rust
pub fn estimate_request_bytes(request: &ChatRequest) -> u64;
pub fn estimate_tool_definition_bytes(tools: &[ToolDefinition]) -> u64;
pub fn chat_request_payload_bytes(request: &ChatRequest) -> u64;
pub fn is_token_overflow_error_message(error_msg: &str) -> bool;
pub fn should_compress_with_reported(/* … */) -> bool;
pub fn emergency_compress_chat_messages(/* … */) -> /* … */;
pub fn is_permanent_llm_api_error(error_msg: &str) -> bool;
pub fn stream_has_meaningful_partial_output(/* … */) -> bool;
pub fn should_retry_stream_error(/* … */) -> bool;
```

> **Rig mapping (FR-014):** The context-window trimming is currently ad-hoc.
> A Rig `rig-memory` policy can be wired as an *alternative* trimming backend:
> when `memory.rig.enabled`, the session loop delegates history compaction to
> the configured Rig policy (sliding window / token budget / compaction)
> instead of `emergency_compress_chat_messages`. The trigger point
> (`should_compress_with_reported`) and the byte/token estimation helpers
> remain unchanged.

### 2.4 Compression pipeline — `crates/ragent-agent/src/compression/pipeline.rs`

```rust
pub enum ContentType { /* … */ }
pub fn detect_content_type(part: &MessagePart) -> ContentType;
pub fn count_tokens(messages: &[Message]) -> usize;
pub fn should_compress(/* … */) -> bool;
pub struct CompressionStats { /* … */ }
pub struct ChatCompressionResult { /* … */ }
pub fn compress_chat_messages(/* … */) -> ChatCompressionResult;
pub fn should_compress_chat_messages(/* … */) -> bool;
pub enum CompressionMode { /* … */ }
pub struct CompressionResult { /* … */ }
pub fn compress_history(/* … */) -> CompressionResult;
pub fn compress_history_with_mode(/* … */) -> CompressionResult;
```

> Config: `ragent-config::compression::CompressionConfig` (`enabled`,
> `auto_threshold`, `ccr`, `compressors`, `relevance`, `tokenizer`).
>
> **Rig mapping (FR-014/FR-020):** Rig memory policies layer *underneath* the
> existing compression pipeline. They are an optional pre-filter that trims
> before the Headroom compressor runs; the compressor remains the authoritative
> compaction step.

### 2.5 Memory subsystem — `crates/ragent-agent/src/memory/`

```rust
// store.rs
pub struct StructuredMemory { /* content, category, confidence, source, project, session_id, tags */ }
impl StructuredMemory { pub fn new(/* … */) -> Self; /* with_* builders, validators */ }
pub enum ForgetFilter { /* … */ }

// embedding.rs re-exports ragent_tools_extended::memory::embedding:
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> { /* default seq */ }
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str;
    fn is_available(&self) -> bool { self.dimensions() > 0 }
}
pub struct NoOpEmbedding;
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32;
pub fn serialise_embedding(vec: &[f32]) -> Vec<u8>;
pub fn deserialise_embedding(blob: &[u8], dimensions: usize) -> Result<Vec<f32>>;
pub struct SimilarityResult { /* … */ }
```

> **Rig mapping (FR-007/FR-010/FR-014/FR-020):** A `RigEmbeddingProvider`
> implements `EmbeddingProvider` by delegating to Rig's `EmbeddingModel::embed`.
> This slots directly into the existing memory semantic-search path
> (`Storage::search_memories_by_embedding`). A Rig memory-policy adapter
> implements a trimming strategy consumed by the session loop (T-011).

### 2.6 Research adapter — `crates/ragent-agent/src/research_adapter.rs`

```rust
pub fn build_research_session(
    registry: &Arc<ToolRegistry>,
    manager: ResearchManager,
    session_id: String,
    working_dir: PathBuf,
    event_bus: Arc<EventBus>,
    storage: Option<Arc<Storage>>,
    config: Option<Arc<Config>>,
    provider_registry: Option<Arc<ProviderRegistry>>,
    active_model: Option<ModelRef>,
) -> ResearchSession;
```

> **Rig mapping (FR-016):** The research session is built here. Semantic
> retrieval (T-012) is injected by passing a Rig-backed embedding/vector-store
> handle into the `ResearchSession` (likely via a new builder method or a
> config flag), so the gatherer can embed fetched documents and query prior
> findings by similarity.

---

## 3. `ragent-codeindex` — lexical index, search, and worker

### 3.1 Entry point — `crates/ragent-codeindex/src/lib.rs:82`

```rust
pub struct CodeIndex {
    store: Mutex<IndexStore>,
    fts: Mutex<FtsIndex>,
    tree_cache: Mutex<TreeCache>,
    parsers: ParserRegistry,
    project_root: PathBuf,
    config: CodeIndexConfig,
    reindex_total: AtomicU32, reindex_done: AtomicU32,
}
impl CodeIndex {
    pub fn open(config: &CodeIndexConfig) -> Result<Self>;
    pub fn open_in_memory(config: &CodeIndexConfig) -> Result<Self>;
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;
    pub fn try_search(&self, query: &SearchQuery) -> Result<Option<Vec<SearchResult>>>;
    pub fn symbols(&self, filter: &SymbolFilter) -> Result<Vec<Symbol>>;
    pub fn try_symbols(&self, filter: &SymbolFilter) -> Result<Option<Vec<Symbol>>>;
    pub fn references(&self, symbol_name: &str, limit: usize) -> Result<Vec<SymbolRef>>;
    pub fn try_references(&self, /* … */) -> Result<Option<Vec<SymbolRef>>>;
    pub fn dependencies(&self, path: &str, direction: DepDirection) -> Result<Vec<String>>;
    pub fn try_dependencies(&self, /* … */) -> Result<Option<Vec<String>>>;
}
```

### 3.2 SQLite store — `crates/ragent-codeindex/src/store.rs`

```rust
pub struct IndexStore { /* rusqlite connection */ }
impl IndexStore {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn open_in_memory() -> Result<Self>;
    pub fn upsert_file(&self, entry: &FileEntry) -> Result<i64>;
    pub fn get_file(&self, path: &str) -> Result<Option<FileEntry>>;
    pub fn list_files(&self) -> Result<Vec<FileEntry>>;
    pub fn delete_file(&self, path: &str) -> Result<()>;
    pub fn upsert_symbols(&self, file_id: i64, symbols: &[Symbol]) -> Result<usize>;
    pub fn query_symbols(&self, filter: &SymbolFilter) -> Result<Vec<Symbol>>;
    pub fn get_file_symbols(&self, file_id: i64) -> Result<Vec<Symbol>>;
    pub fn upsert_imports(&self, file_id: i64, imports: &[ImportEntry]) -> Result<usize>;
    pub fn query_imports(&self, name_substring: &str) -> Result<Vec<ImportEntry>>;
    pub fn upsert_refs(&self, file_id: i64, refs: &[SymbolRef]) -> Result<usize>;
    pub fn find_references(&self, symbol_name: &str) -> Result<Vec<SymbolRef>>;
    pub fn set_file_deps(&self, /* … */);
    pub fn get_dependents(&self, target_path: &str) -> Result<Vec<i64>>;
    pub fn get_file_id(&self, path: &str) -> Result<Option<i64>>;
    pub fn get_stats(&self) -> Result<IndexStats>;
    /* + transaction helpers, language_counts, file_count, total_bytes, stale diff */
}
```

### 3.3 Tantivy FTS — `crates/ragent-codeindex/src/search.rs`

```rust
pub struct SearchResult {
    pub symbol_name: String, pub qualified_name: String, pub kind: String,
    pub file_path: String, pub line: u32, pub end_line: u32, pub score: f32,
    pub signature: String, pub doc_snippet: String,
}
pub struct FtsIndex { /* tantivy index */ }
impl FtsIndex {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn open_in_memory() -> Result<Self>;
    pub fn add_symbols(&self, symbols: &[FtsSymbol<'_>]) -> Result<()>;
    pub fn remove_file(&self, file_path: &str) -> Result<()>;
    pub fn batch_update(&self, remove_paths: &[&str], symbols: &[FtsSymbol<'_>]) -> Result<()>;
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    pub fn doc_count(&self) -> Result<u64>;
}
pub struct FtsSymbol<'a> {
    pub name: &'a str, pub qualified_name: Option<&'a str>, pub kind: &'a str,
    pub file_path: &'a str, pub signature: Option<&'a str>,
    pub doc_comment: Option<&'a str>, pub body_snippet: Option<&'a str>,
    pub start_line: u32,
}
```

### 3.4 Types — `crates/ragent-codeindex/src/types.rs`

```rust
pub enum SymbolKind { /* … */ }
pub enum Visibility { /* … */ }
pub struct FileEntry { /* path, language, hash, mtime, size */ }
pub struct Symbol { /* name, qualified_name, kind, file_id, line, end_line, visibility, signature, doc, body_snippet */ }
pub struct ImportEntry { /* … */ }
pub struct SymbolRef { /* … */ }
pub struct IndexStats { /* … */ }
pub struct SymbolFilter { /* … */ }
pub struct SearchQuery { /* query, kind, language, file_pattern, max_results */ }
pub struct IndexResult { /* … */ }
pub enum DepDirection { Imports, Dependents }
pub struct CodeIndexConfig { pub enabled: bool, pub project_root: PathBuf, pub index_dir: PathBuf, pub scan_config: ScanConfig }
```

### 3.5 Worker — `crates/ragent-codeindex/src/worker.rs`

```rust
pub struct IndexWorker;
impl IndexWorker {
    pub fn start(index: Arc<CodeIndex>, event_rx: mpsc::Receiver<WatchEvent>, config: WorkerConfig) -> IndexWorkerHandle;
}
pub struct IndexWorkerHandle { /* stop_flag, thread, stats, manual_tx */ }
impl IndexWorkerHandle {
    pub fn stop(&mut self);
    pub fn queue_reindex(&self, path: PathBuf);
    pub fn queue_full_reindex(&self);
    pub fn stats(&self) -> WorkerStats;
}
```

> **Rig mapping (FR-010/FR-015/FR-021):** Semantic indexing augments — not
> replaces — the lexical index. The integration point is the **worker**: when
> a file is re-indexed (`IndexStore::upsert_file` + `FtsIndex::add_symbols`),
> also generate a Rig embedding for the symbol/doc/body and upsert it into the
> configured vector store. `CodeIndex::search` is extended to issue a parallel
> vector query and merge results by combined score. A new
> `SemanticSearchHandle` (Rig `VectorStoreIndex` wrapper) is stored alongside
> `store`/`fts` in `CodeIndex`.

---

## 4. `ragent-research` — engine, session, gatherers, sources

### 4.1 Research item — `crates/ragent-research/src/item.rs`

```rust
pub struct ResearchItem {
    pub name: ResearchName, pub title: String, pub topic: String,
    pub status: ResearchStatus, pub created_at: DateTime<Utc>, pub modified_at: DateTime<Utc>,
    pub sources: Vec<Source>, pub queries: Vec<String>, pub output_format: Option<String>,
}
impl ResearchItem {
    pub fn new(name: ResearchName, title: impl Into<String>, topic: impl Into<String>) -> Self;
    pub fn add_source(&mut self, source: Source) -> &mut Self;
    pub fn source_count(&self) -> usize;
    /* + set_queries, set_status, set_title, touch, frontmatter round-trip */
}
```

### 4.2 Source — `crates/ragent-research/src/source.rs:64`

```rust
pub enum Source {
    Web   { url, title, captured_at, published_at, body_path, body, relevance },
    Local { path, kind, captured_at, body_path, relevance, body },
    Spec  { spec_id, captured_at, relevance },
    Other { label, captured_at, body_path, body },
}
impl Source {
    pub fn type_str(&self) -> &'static str;
    pub fn title(&self) -> &str;
    pub fn path_or_url(&self) -> &str;
    pub fn body(&self) -> Option<&str>;
    pub fn has_body(&self) -> bool;
    pub fn relevance(&self) -> Option<&str>;
    pub fn published_at(&self) -> Option<DateTime<Utc>>;
}
```

> **Rig mapping (FR-016/FR-017):** Each `Source` with a body can be chunked and
> embedded into the vector store. Prior `ResearchItem`s become a searchable
> corpus; `/research create` queries them by similarity to the topic before
> web/local gathering, surfacing semantically related prior findings.

### 4.3 Session + config — `crates/ragent-research/src/session.rs`

```rust
pub struct SessionConfig {
    pub topic: String, pub sources_dir: Option<PathBuf>, pub template: Option<String>,
    pub max_web_results: usize, pub max_local_sources: usize,
    pub disable_local: bool, pub disable_specs: bool,
    pub from_url: Option<String>, pub fetch_concurrency: usize,
    pub depth: Option<Depth>, pub iterations: Option<u32>, pub output_format: OutputFormat,
}
impl SessionConfig {
    pub fn engine_config(&self) -> EngineConfig;
    pub fn budget_web_results(&self) -> usize;
    pub fn budget_local_sources(&self) -> usize;
}

pub struct ResearchSession {
    manager: ResearchManager,
    web: Option<WebGatherer>,
    local: Option<LocalGatherer>,
    analysis: Arc<dyn AnalysisEngine>,
    planner: Option<Arc<dyn Planner>>,
    critic: Option<Arc<dyn Critic>>,
}
impl ResearchSession {
    pub fn new(/* … */) -> Self;
    pub fn with_planner(mut self, planner: Arc<dyn Planner>) -> Self;
    pub fn with_critic(mut self, critic: Arc<dyn Critic>) -> Self;
    pub fn with_local_tool(/* … */) -> Self;
    pub async fn run(&self, name_str: &str, title: &str, config: &SessionConfig, observer: Arc<dyn SessionObserver>) -> Result<RunOutcome>;
}
pub trait SessionObserver: Send + Sync { /* on_event */ }
pub enum SessionEvent { /* Phase, WebCaptured, … */ }
pub enum SessionPhase { /* Setup, Gather, Synthesize, Persist, Complete */ }
pub struct RunOutcome { /* … */ }
```

### 4.4 Engine + critic — `crates/ragent-research/src/engine.rs`

```rust
pub struct EngineConfig { /* iterations, … */ }
pub struct IterationResult { /* … */ }
pub trait Critic: Send + Sync { /* evaluate */ }
pub struct SimpleCritic;
pub struct IterativeEngine { /* … */ }
impl IterativeEngine { pub fn new(/* … */) -> Self; }
```

### 4.5 Analysis — `crates/ragent-research/src/analysis.rs`

```rust
pub struct SourceBody { pub index, kind, title, path_or_url, relevance, body, published_at }
pub struct AnalysisResult { pub summary, findings, cross_references, open_questions }
#[async_trait]
pub trait AnalysisEngine: Send + Sync {
    async fn analyze(&self, topic: &str, sources: &[SourceBody]) -> anyhow::Result<AnalysisResult>;
    fn is_noop_marker(&self) -> bool { false }
    async fn analyze_with_outcome(&self, topic: &str, sources: &[SourceBody])
        -> anyhow::Result<(AnalysisResult, AnalysisOutcome)>;
}
pub struct NoopAnalysisEngine;
pub struct LlmAnalysisEngine { /* … */ }
pub enum AnalysisOutcome { Llm, FallbackEmpty, FallbackError }
```

### 4.6 Web gatherer — `crates/ragent-research/src/web_gatherer.rs`

```rust
pub struct GatherResult { pub queries: Vec<String>, pub sources: Vec<Source> }
pub struct WebSearchHit { pub url, title, snippet, matched_query }
pub struct WebFetchedPage { pub url, title, body, published_at }
#[async_trait] pub trait WebSearchTool: Send + Sync { async fn search(&self, query: &str, max_results: usize) -> Result<Vec<WebSearchHit>>; }
#[async_trait] pub trait WebFetchTool:  Send + Sync { async fn fetch(&self, url: &str) -> Result<WebFetchedPage>; }
pub struct WebGatherer { /* … */ }
impl WebGatherer { pub fn new(search: Arc<dyn WebSearchTool>, fetch: Arc<dyn WebFetchTool>) -> Self; }
```

### 4.7 Local gatherer — `crates/ragent-research/src/local_gatherer.rs`

```rust
pub trait LocalTool: Send + Sync { /* … */ }
pub struct GrepMatch { /* … */ }
pub struct LocalGatherConfig { /* … */ }
pub struct LocalGatherer { /* … */ }
impl LocalGatherer { pub fn new(tool: Arc<dyn LocalTool>) -> Self; pub async fn gather(/* … */) -> /* … */; }
```

### 4.8 Planner — `crates/ragent-research/src/planner.rs`

```rust
#[async_trait] pub trait Planner: Send + Sync { async fn plan(&self, topic: &str) -> anyhow::Result<ResearchPlan>; }
pub struct HeuristicPlanner;
```

### 4.9 Manager — `crates/ragent-research/src/manager.rs`

```rust
pub enum ResearchError { /* … */ }
pub struct SearchHit { /* … */ }
pub struct ResearchManager { /* research_root */ }
impl ResearchManager {
    pub fn new(research_root: impl Into<PathBuf>) -> Self;
    pub fn root(&self) -> &Path;
}
```

> **Rig mapping (FR-016/FR-017/FR-022):** Semantic retrieval is injected into
> `ResearchSession` via a new optional field (e.g.
> `semantic: Option<Arc<SemanticRetriever>>`). Before/after the web+local
> gather, the session queries the vector store for prior findings/sources
> similar to `config.topic` and merges them into the `Source` list. Fetched
> `WebFetchedPage`/`Source::body` text is chunked + embedded (FR-017). The
> `AnalysisEngine` prompt can be augmented with the top-k semantically similar
> prior items.

---

## 5. `ragent-storage` — SQLite-backed session, message, memory, embeddings

### 5.1 Storage — `crates/ragent-storage/src/storage.rs:206`

```rust
pub struct Storage { conn: Mutex<Connection>, has_format_version: AtomicBool }
impl Storage {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn open_in_memory() -> Result<Self>;

    // Sessions
    pub fn create_session(&self, id: &str, directory: &str) -> Result<()>;
    pub fn get_session(&self, id: &str) -> Result<Option<SessionRow>>;
    pub fn list_sessions(&self) -> Result<Vec<SessionRow>>;
    pub fn update_session(&self, id: &str, title: &str) -> Result<()>;
    pub fn archive_session(&self, id: &str) -> Result<()>;

    // Messages
    pub fn create_message(&self, msg: &Message) -> Result<()>;
    pub fn get_messages(&self, session_id: &str) -> Result<Vec<Message>>;
    pub fn update_message(&self, msg: &Message) -> Result<()>;
    pub fn delete_messages(&self, session_id: &str) -> Result<usize>;
    pub fn has_assistant_messages(&self, session_id: &str) -> Result<bool>;

    // Auth + settings + discovered models
    pub fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> Result<()>;
    pub fn delete_provider_auth(&self, provider_id: &str) -> Result<()>;
    pub fn get_provider_auth(&self, provider_id: &str) -> Result<Option<String>>;
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()>;
    pub fn get_setting(&self, key: &str) -> Result<Option<String>>;
    pub fn delete_setting(&self, key: &str) -> Result<()>;
    pub fn set_discovered_models(&self, provider_id: &str, models_json: &str) -> Result<()>;
    pub fn get_discovered_models(&self, provider_id: &str) -> Result<Option<String>>;
    pub fn delete_discovered_models(&self, provider_id: &str) -> Result<()>;

    // TODOs
    pub fn create_todo(&self, /* … */) -> Result<()>;
    pub fn get_todos(&self, session_id: &str, status_filter: Option<&str>) -> Result<Vec<TodoRow>>;
    pub fn update_todo(&self, /* … */) -> Result<()>;
    pub fn delete_todo(&self, id: &str, session_id: &str) -> Result<bool>;
    pub fn clear_todos(&self, session_id: &str) -> Result<usize>;

    // Structured memories
    pub fn create_memory(&self, /* … */) -> Result<i64>;
    pub fn get_memory(&self, id: i64) -> Result<Option<MemoryRow>>;
    pub fn get_memory_tags(&self, memory_id: i64) -> Result<Vec<String>>;
    pub fn search_memories(&self, /* … */) -> Result<Vec<MemoryRow>>;
    pub fn list_memories(&self, project: &str, limit: usize) -> Result<Vec<MemoryRow>>;
    pub fn delete_memory(&self, id: i64) -> Result<bool>;
    pub fn delete_memories_by_filter(&self, /* … */) -> Result<usize>;
    pub fn update_memory_confidence(&self, id: i64, confidence: f64) -> Result<bool>;
    pub fn increment_memory_access(&self, id: i64) -> Result<bool>;
    pub fn count_memories(&self) -> Result<u64>;
    pub fn update_memory_content(&self, id: i64, content: &str) -> Result<bool>;
    pub fn set_memory_tags(&self, memory_id: i64, tags: &[String]) -> Result<()>;

    // Embeddings (brute-force cosine over stored blobs)
    pub fn store_memory_embedding(&self, id: i64, embedding_blob: &[u8]) -> Result<bool>;
    pub fn list_memory_embeddings(&self) -> Result<Vec<(i64, Vec<u8>)>>;
    pub fn search_memories_by_embedding<F>(
        &self, query_embedding: &[f32], dimensions: usize, limit: usize, min_similarity: f32, similarity: F,
    ) -> Result<Vec<EmbeddingMatch>>
    where F: Fn(&[f32], &[f32]) -> f32;

    // Knowledge graph
    pub fn upsert_entity(&self, /* … */) -> Result<i64>;
    pub fn create_relationship(&self, /* … */) -> Result<()>;
    pub fn list_entities(&self) -> Result<Vec<KgEntityRow>>;
    pub fn list_relationships(&self) -> Result<Vec<KgRelationshipRow>>;
    pub fn query_entity_neighbours(&self, /* … */) -> /* … */;
}

pub struct SessionRow { /* id, title, project_id, directory, parent_id, version, format_version, created_at, updated_at, archived_at, summary */ }
pub struct TodoRow { /* id, session_id, title, status, description, created_at, updated_at */ }
pub struct MemoryRow { /* id, content, category, source, confidence, project, session_id, created_at, updated_at, access_count, last_accessed */ }
pub struct KgEntityRow { /* … */ }
pub struct KgRelationshipRow { /* … */ }
pub struct EmbeddingMatch { pub row_id: i64, pub score: f32 }
```

> **Rig mapping (FR-008/FR-009/FR-010/FR-014/FR-020):**
> - **Embeddings:** `store_memory_embedding` + `search_memories_by_embedding`
>   already provide a vector store surface. A Rig `VectorStoreIndex` backend
>   can *replace* the brute-force path for large corpora (FR-008/FR-009), while
>   the storage API remains the read/write boundary.
> - **Session history:** `get_messages`/`create_message` persist the
>   conversation; a Rig memory policy reads this history, trims it per
>   `memory.rig` config, and the loop re-sends the trimmed slice (FR-014).
> - **Knowledge graph:** untouched by Rig in phase 1.

---

## 6. Event bus — `crates/ragent-types/src/event/mod.rs`

```rust
pub enum Event {
    SessionCreated { session_id }, SessionUpdated { session_id },
    MessageStart { session_id, message_id },
    TextDelta { session_id, text },            // ← FR-013 target
    ReasoningDelta { session_id, text },
    ToolCallStart { session_id, call_id, tool }, ToolCallEnd { session_id, call_id, tool, error, duration_ms },
    MessageEnd { session_id, message_id, reason: FinishReason },
    PermissionRequested { /* … */ },
    TokenUsage { session_id, input_tokens, output_tokens },
    RequestStarted { session_id, outbound_bytes }, ToolsSent { session_id, tools },
    ModelResponse { /* … */ },
    /* … */
}
pub struct EventBus { /* broadcast channel */ }
impl EventBus { pub fn publish(&self, event: Event); pub fn subscribe(&self) -> broadcast::Receiver<Event>; }
```

> **Note (FR-013):** The spec text mentions `Event::ChatStreamDelta`, but the
> actual event variant is `Event::TextDelta`. Rig streaming chunks map onto
> `Event::TextDelta` (and `ReasoningDelta`/`ToolCallStart`/`ToolCallDelta`/
> `ToolCallEnd`/`TokenUsage`/`MessageEnd`) so the TUI/server consume them
> uniformly — **no new event variant is required**. The adapter emits the same
> events the native providers already emit.

---

## 7. Summary of integration seams

| Concern | ragent seam | Rig side | Phase |
|---|---|---|---|
| Completion request → provider | `LlmClient::chat(ChatRequest)` | `CompletionModel::stream` | T-004 |
| Response → ragent | `StreamEvent` variants | Rig completion chunks | T-005 |
| Provider registration | `Provider` trait + `ProviderRegistry` | `RigProvider` impl | T-006 |
| Streaming events | `Event::TextDelta` et al. (no `ChatStreamDelta`) | Rig stream chunks | T-005 |
| Embeddings | `EmbeddingProvider` trait | Rig `EmbeddingModel` | T-007 |
| Vector store | `Storage::search_memories_by_embedding` + new `SemanticSearchHandle` | Rig `VectorStoreIndex` | T-008/T-009/T-010 |
| Code index augmentation | `CodeIndex::search` + `IndexWorker` | Embed-on-index + parallel vector query | T-009 |
| Memory trimming | `session::history` + `compression::pipeline` | Rig `rig-memory` policies | T-011 |
| Research semantic retrieval | `ResearchSession::run` + `Source`/`AnalysisEngine` | Embed sources + query prior findings | T-012 |
| Config | `ragent-config` (provider/permissions/memory/compression) | New `provider.rig.*` schema | T-006/T-019 |

### Key findings for downstream tasks

1. **`Event::ChatStreamDelta` does not exist** — the real variant is
   `Event::TextDelta`. T-005 maps Rig chunks onto the existing event set; no new
   event variant is needed.
2. **`ChatRequest` already uses `Arc<Vec<ChatMessage>>`** — the adapter borrows
   the conversation cheaply; no new shared handle is required.
3. **`EmbeddingProvider` is the single embedding trait** to implement for Rig
   (T-007); it is re-exported from `ragent-agent::memory::embedding` but
   canonically lives in `ragent-tools-extended::memory::embedding`.
4. **`Storage` already has a vector-store surface** (embedding store/search).
   T-008 can either back this with a Rig `VectorStoreIndex` or layer a new
   `SemanticSearchHandle` above it.
5. **Research has clean trait seams** (`Planner`, `Critic`, `AnalysisEngine`,
   `WebSearchTool`, `WebFetchTool`, `LocalTool`) — semantic retrieval is added
   as an optional handle on `ResearchSession`, not by replacing any trait.
6. **Code index worker is the embedding hook point** (FR-015): the worker
   already owns file reindex events; embedding generation rides alongside
   `FtsIndex::add_symbols`.
7. **Native providers stay untouched** (FR-003): the Rig adapter is a new
   `Provider`/`LlmClient` implementation registered alongside the existing
   ones; nothing is removed.