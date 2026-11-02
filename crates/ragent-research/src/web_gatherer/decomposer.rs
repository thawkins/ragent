//! Query decomposition - break a research topic into focused sub-queries for
//! parallel web search.
//!
//! These helpers were previously inline in `web_gatherer.rs`.

use async_trait::async_trait;
use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::provider::ProviderRegistry;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

use crate::web_gatherer::MAX_DECOMPOSED_QUERIES;

/// Default cap on the number of search queries a decomposition pass may
/// produce. A per-run `--max-queries` (or `research.max_queries`) overrides it;
/// `0` means "no cap beyond the model's own output".
pub const DEFAULT_MAX_QUERIES: usize = 20;

/// Trait abstracting the decomposition of a research topic into focused
/// sub-queries.  A decomposer may be heuristic (cheap, no LLM) or LLM-backed
/// (higher quality, costs one call).  When no decomposer is configured the
/// gatherer falls back to searching the raw topic as a single query.
#[async_trait]
pub trait QueryDecomposer: Send + Sync {
    /// Break `topic` into a list of search queries.  The gatherer runs each
    /// query in parallel, deduplicates results by URL, and then fetches up
    /// to the caller's `max_results` unique pages.
    async fn decompose(&self, topic: &str) -> anyhow::Result<Vec<String>>;

    /// The cap this decomposer applies to the queries it returns.
    ///
    /// `None` means "no cap is applied" (`--max-queries 0`) and the gatherer
    /// searches every query the decomposer produced.
    fn query_limit(&self) -> Option<usize> {
        Some(MAX_DECOMPOSED_QUERIES)
    }
}

/// Simple heuristic decomposer that splits a topic on conjunctions and
/// commas, then also includes the original topic as a catch-all query.
///
/// Cheap and deterministic; requires no network calls.  Kept as a fallback
/// for the LLM-backed decomposer and for callers that intentionally want
/// heuristic splitting.
///
/// The number of queries it yields is capped by [`Self::max_queries`], which
/// mirrors the per-run `--max-queries` flag (default
/// [`DEFAULT_MAX_QUERIES`]; `0` = no cap).
#[derive(Debug, Clone, Copy)]
pub struct HeuristicQueryDecomposer {
    max_queries: Option<usize>,
}

impl Default for HeuristicQueryDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicQueryDecomposer {
    /// Build a decomposer using the default query cap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_queries: Some(DEFAULT_MAX_QUERIES),
        }
    }

    /// Cap the number of queries returned. `None` means "no cap".
    #[must_use]
    pub fn with_max_queries(mut self, max_queries: Option<usize>) -> Self {
        self.max_queries = max_queries;
        self
    }
}

#[async_trait]
impl QueryDecomposer for HeuristicQueryDecomposer {
    async fn decompose(&self, topic: &str) -> anyhow::Result<Vec<String>> {
        let trimmed = topic.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Split on sentence boundaries first. Long prose topics often
        //    contain commas inside a single sentence; splitting on those commas
        //    first produces nonsensical fragments.
        let mut queries: Vec<String> = Vec::new();
        for sentence in split_into_sentence_chunks(trimmed) {
            // 2. Within each sentence, split on explicit conjunctions of short
            //    noun phrases (e.g. "Rust async and Tokio runtime"). Only split
            //    when every resulting chunk is short enough to be a focused query.
            let mut sentence_queries = split_short_conjunctions(&sentence);
            queries.append(&mut sentence_queries);
        }

        // 3. If the whole topic is a short comma-separated list (no sentence
        //    punctuation), treat the comma-separated items as distinct queries.
        if queries.len() == 1
            && let Some(list_queries) = split_comma_list(trimmed)
        {
            queries = list_queries;
        }

        // Deduplicate preserving order; keep the full topic last so it acts
        // as a catch-all when earlier sub-queries returned nothing.
        let mut seen = HashSet::new();
        let mut deduped: Vec<String> = Vec::new();
        for q in queries {
            let normalized = collapse_whitespace(&q);
            if normalized.is_empty() {
                continue;
            }
            let lower = normalized.to_lowercase();
            if seen.insert(lower) {
                deduped.push(normalized);
            }
        }
        let full_lower = trimmed.to_lowercase();
        if seen.insert(full_lower) {
            deduped.push(trimmed.to_string());
        }

        // Cap the number of sub-queries to avoid hammering the search
        // provider while still giving broad topics enough coverage.
        if let Some(limit) = self.max_queries {
            deduped.truncate(limit);
        }
        Ok(deduped)
    }

    fn query_limit(&self) -> Option<usize> {
        self.max_queries
    }
}

/// Split a topic on sentence boundaries, keeping parenthesised text intact.
fn split_into_sentence_chunks(topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;
    let chars: Vec<char> = topic.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        current.push(c);
        match c {
            '(' | '[' | '{' => paren_depth += 1,
            ')' | ']' | '}' if paren_depth > 0 => paren_depth -= 1,
            _ => {}
        }
        if paren_depth == 0 && matches!(c, '.' | '?' | '!') {
            // End of a sentence only if followed by whitespace or end of text.
            if i + 1 == chars.len() || chars[i + 1].is_whitespace() {
                let chunk = current.trim().to_string();
                if !chunk.is_empty() {
                    out.push(chunk);
                }
                current.clear();
            }
        }
        i += 1;
    }
    let remainder = current.trim().to_string();
    if !remainder.is_empty() {
        out.push(remainder);
    }
    if out.is_empty() {
        out.push(topic.to_string());
    }
    out
}

/// Split a sentence on " and ", " & ", " + " and "; " only when every
/// resulting chunk is short enough to be a useful focused query. This keeps
/// long prose sentences intact while expanding short conjunctions like
/// "Rust async and Tokio runtime".
fn split_short_conjunctions(sentence: &str) -> Vec<String> {
    const MAX_CHUNK_WORDS: usize = 8;
    let separators = [" and ", " & ", " + ", "; "];

    // First try splitting on each separator.
    let mut best: Option<Vec<String>> = None;
    for sep in &separators {
        let parts: Vec<String> = sentence
            .split(sep)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(collapse_whitespace)
            .collect();
        if parts.len() > 1
            && parts
                .iter()
                .all(|p| p.split_whitespace().count() <= MAX_CHUNK_WORDS)
            && best.as_ref().is_none_or(|b| parts.len() > b.len())
        {
            best = Some(parts);
        }
    }

    if let Some(parts) = best {
        return parts;
    }
    vec![collapse_whitespace(sentence)]
}

/// If `topic` looks like a short comma-separated list of distinct phrases,
/// return those phrases. Returns `None` for long prose or single-sentence
/// topics so they are not over-split.
fn split_comma_list(topic: &str) -> Option<Vec<String>> {
    let comma_chunks: Vec<&str> = topic
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if comma_chunks.len() < 2 || comma_chunks.len() > 5 {
        return None;
    }
    if topic.contains('.') || topic.contains('?') || topic.contains('!') || topic.contains(';') {
        return None;
    }
    let total_words: usize = comma_chunks
        .iter()
        .map(|s| s.split_whitespace().count())
        .sum();
    if total_words > 25 {
        return None;
    }
    Some(comma_chunks.into_iter().map(collapse_whitespace).collect())
}

/// Collapse runs of whitespace into a single space and trim.
fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// LLM-backed query decomposer.
///
/// Sends the topic to the configured provider/model and asks it to return a
/// JSON array of focused web-search queries (bounded by the per-run
/// `--max-queries` cap, default [`DEFAULT_MAX_QUERIES`]). The first query should
/// be the most specific; the last query can be a broader catch-all. If the model
/// response cannot be parsed, or the provider is unavailable, the decomposer
/// falls back to the heuristic splitter so research always makes progress.
#[derive(Clone)]
pub struct LlmQueryDecomposer {
    provider_registry: Arc<ProviderRegistry>,
    provider_id: String,
    model_id: String,
    api_key: Option<String>,
    base_url: Option<String>,
    fallback: HeuristicQueryDecomposer,
    max_queries: Option<usize>,
}

impl std::fmt::Debug for LlmQueryDecomposer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmQueryDecomposer")
            .field("provider_id", &self.provider_id)
            .field("model_id", &self.model_id)
            .field("has_api_key", &self.api_key.is_some())
            .finish_non_exhaustive()
    }
}

impl LlmQueryDecomposer {
    /// Build a new LLM decomposer.
    pub fn new(
        provider_registry: Arc<ProviderRegistry>,
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        let max_queries = Some(DEFAULT_MAX_QUERIES);
        Self {
            provider_registry,
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            api_key: None,
            base_url: None,
            fallback: HeuristicQueryDecomposer::new().with_max_queries(max_queries),
            max_queries,
        }
    }

    /// Provide an API key for the provider.
    #[must_use]
    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Override the API base URL.
    #[must_use]
    pub fn with_base_url(mut self, base_url: Option<String>) -> Self {
        self.base_url = base_url;
        self
    }

    /// Cap the number of queries this decomposer produces (and searches).
    ///
    /// `None` means "no cap": the model is not told a maximum and every query
    /// it returns is used. The cap also bounds the heuristic fallback, so a
    /// degraded run honours the same limit as a model-backed one.
    #[must_use]
    pub fn with_max_queries(mut self, max_queries: Option<usize>) -> Self {
        self.max_queries = max_queries;
        self.fallback = self.fallback.with_max_queries(max_queries);
        self
    }

    /// The query count the model is asked for, clamped for the prompt.
    ///
    /// The prompt already hard-caps the request at
    /// [`MAX_DECOMPOSED_QUERIES`]; a larger `--max-queries` cannot make the
    /// model emit more than that, so the asked-for count is clamped rather than
    /// promising a number the parser would then truncate.
    fn prompt_query_cap(&self) -> Option<usize> {
        self.max_queries.map(|n| n.min(MAX_DECOMPOSED_QUERIES))
    }

    async fn decompose_with_llm(&self, topic: &str) -> anyhow::Result<Vec<String>> {
        let provider = self
            .provider_registry
            .get(&self.provider_id)
            .ok_or_else(|| anyhow::anyhow!("unknown provider '{}'", self.provider_id))?;

        let api_key = self.api_key.clone().unwrap_or_default();
        let client = provider
            .create_client(
                &api_key,
                self.base_url.as_deref(),
                &std::collections::HashMap::new(),
            )
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to create LLM client for {}/{}: {e}",
                    self.provider_id,
                    self.model_id
                )
            })?;

        // The model has no clock: left to itself it writes the year it last saw
        // in training data into every query ("... 2025"), which then steers the
        // search engines - and relevance scoring, which treats the year as a
        // topic term - towards stale material. State today's date and forbid
        // years other than the current one.
        let today = chrono::Utc::now().format("%Y-%m-%d");
        let current_year = chrono::Utc::now().format("%Y");
        // `--max-queries` sets the array size the model is asked for. When the
        // cap is disabled (`--max-queries 0`) the prompt falls back to the
        // hard upper bound so the response stays a bounded JSON array.
        let count_phrase = match self.prompt_query_cap() {
            Some(n) => format!("an array of 1 to {n} short search-engine queries"),
            None => format!(
                "an array of short search-engine queries (at most {MAX_DECOMPOSED_QUERIES})"
            ),
        };
        let prompt = format!(
            "You are decomposing a research topic into focused web-search queries.\n\nTopic: {topic}\n\nToday's date is {today}.\n\nReturn a JSON object with exactly one key, \"queries\", whose value is {count_phrase} that together cover the topic. Put the most specific query first and a broader catch-all query last. Each query must be a plain string with no markdown or explanation.\n\nDo not put a year in a query. The single exception is when the topic names a specific year; then use only {current_year}, never an earlier year. Do not write date ranges or relative phrases such as \"recent\" or \"latest\" - the caller applies its own publication-date filter.\n\nExample response:\n{{\"queries\":[\"Rust async runtime internals\", \"Tokio runtime scheduling\", \"Rust async and Tokio runtime\"]}}\n\nNow produce only the JSON object:"
        );

        let request = ChatRequest {
            model: self.model_id.clone(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(prompt),
            }]),
            tools: Arc::new(vec![]),
            temperature: Some(0.2),
            top_p: Some(1.0),
            max_tokens: Some(512),
            system: Some(std::sync::Arc::from(
                "You are a precise research assistant that returns only valid JSON.",
            )),
            options: std::collections::HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: Some(120),
            thinking: None,
        };

        let mut stream = client.chat(request).await?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text: delta } => text.push_str(&delta),
                StreamEvent::Error { message } => anyhow::bail!("provider error: {message}"),
                StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }

        parse_query_decomposition(&text, self.max_queries)
    }
}

#[async_trait]
impl QueryDecomposer for LlmQueryDecomposer {
    async fn decompose(&self, topic: &str) -> anyhow::Result<Vec<String>> {
        match self.decompose_with_llm(topic).await {
            Ok(qs) if !qs.is_empty() => Ok(qs),
            Ok(_) => {
                tracing::warn!(
                    topic,
                    "research: LLM decomposer returned empty queries; falling back to heuristic"
                );
                self.fallback.decompose(topic).await
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    topic,
                    "research: LLM query decomposition failed; falling back to heuristic"
                );
                self.fallback.decompose(topic).await
            }
        }
    }

    fn query_limit(&self) -> Option<usize> {
        self.max_queries
    }
}

/// Parse the model's JSON response into a list of queries.
///
/// Accepts `{ "queries": [...] }`, markdown-fenced JSON, and strips trailing
/// commas before delegating to `serde_json`. `limit` is the effective
/// `--max-queries` cap; `None` keeps every query the model returned (the hard
/// [`MAX_DECOMPOSED_QUERIES`] bound still applies so a runaway response cannot
/// fan out into unbounded searches).
fn parse_query_decomposition(raw: &str, limit: Option<usize>) -> anyhow::Result<Vec<String>> {
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    let cleaned = remove_trailing_commas(json_str);

    #[derive(Deserialize)]
    struct DecompResponse {
        queries: Vec<String>,
    }

    let parsed: DecompResponse = serde_json::from_str(&cleaned).map_err(|e| {
        anyhow::anyhow!("failed to parse decomposition JSON: {e}\n\nRaw response:\n{raw}")
    })?;

    let queries: Vec<String> = parsed
        .queries
        .into_iter()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty())
        .collect();

    if queries.is_empty() {
        anyhow::bail!("LLM decomposer returned no usable queries");
    }

    // Enforce the per-run `--max-queries` cap. The hard
    // `MAX_DECOMPOSED_QUERIES` bound always applies first, so an unbounded
    // (`--max-queries 0`) run cannot fan out past what the prompt asked for.
    let cap = limit.unwrap_or(MAX_DECOMPOSED_QUERIES);
    Ok(queries
        .into_iter()
        .take(cap.min(MAX_DECOMPOSED_QUERIES))
        .collect())
}

/// Remove trailing commas before `}` or `]` in JSON.
fn remove_trailing_commas(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for i in 0..len {
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                continue;
            }
        }
        result.push(chars[i]);
    }
    result
}
