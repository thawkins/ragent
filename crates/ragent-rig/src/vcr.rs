//! VCR cassette test harness (T-015 / FR-026).
//!
//! This module provides a deterministic record/playback layer for LLM
//! interactions. It wraps any [`LlmClient`] and stores the resulting
//! [`StreamEvent`]s in a JSON cassette file. In playback mode the same file
//! is read and matching requests replay the recorded event stream without any
//! network calls.
//!
//! The harness is intended for:
//!
//! * Regression tests that need stable provider outputs.
//! * CI environments that should not call live APIs.
//! * Capturing edge-case responses (tool calls, errors, usage events) once
//!   and replaying them across platforms.
//!
//! Cassette files are plain JSON and checked into version control. Sensitive
//! values such as API keys are never recorded because the cassette only stores
//! the [`ChatRequest`] fields used for matching plus the response events.
//!
//! # Example
//!
//! ```rust,ignore
//! use ragent_llm::llm::{ChatRequest, LlmClient};
//! use ragent_rig::vcr::{VcrCassette, VcrClient, VcrMode};
//!
//! # async fn example() {
//! # let inner: Box<dyn LlmClient> = unimplemented!();
//! let client = VcrClient::new(inner, VcrMode::Record("fixtures/echo.json".into())).await.unwrap();
//! let events: Vec<_> = client.chat(ChatRequest::default()).await.unwrap()
//!     .collect().await;
//! # }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use anyhow::{Context, Result, bail};
use futures::{Stream, StreamExt};
use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

/// Cassette file format version.
const CASSETTE_VERSION: u32 = 1;

/// Operational mode for a [`VcrClient`].
#[derive(Clone, Debug)]
pub enum VcrMode {
    /// Record interactions to the given cassette file, overwriting any previous
    /// contents.
    Record(PathBuf),
    /// Replay interactions from the given cassette file. Unknown requests fail
    /// unless `RecordNew` is enabled.
    Playback(PathBuf),
    /// Replay known interactions and record any unmatched request to the same
    /// cassette file. Useful when extending an existing fixture.
    PlaybackRecordNew(PathBuf),
}

impl VcrMode {
    /// Returns the cassette path.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Record(p) | Self::Playback(p) | Self::PlaybackRecordNew(p) => p.as_path(),
        }
    }

    /// Returns `true` when the mode may write to the cassette.
    #[must_use]
    pub fn can_record(&self) -> bool {
        matches!(self, Self::Record(_) | Self::PlaybackRecordNew(_))
    }

    /// Returns `true` when the mode may replay from the cassette.
    #[must_use]
    pub fn can_playback(&self) -> bool {
        matches!(self, Self::Playback(_) | Self::PlaybackRecordNew(_))
    }
}

/// A single recorded request/response pair.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VcrInteraction {
    /// Normalized request key used for matching during playback.
    pub request: VcrRequestKey,
    /// Response events captured from the wrapped client.
    pub response: Vec<StreamEvent>,
}

/// Cassette container written to disk.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VcrCassette {
    /// Format version; bumped when the schema changes incompatibly.
    pub version: u32,
    /// Ordered list of recorded interactions.
    pub interactions: Vec<VcrInteraction>,
}

impl VcrCassette {
    /// Create an empty cassette with the current format version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: CASSETTE_VERSION,
            interactions: Vec::new(),
        }
    }

    /// Find the first interaction whose request matches `key`.
    #[must_use]
    pub fn find(&self, key: &VcrRequestKey) -> Option<&VcrInteraction> {
        self.interactions.iter().find(|i| &i.request == key)
    }
}

impl Default for VcrCassette {
    fn default() -> Self {
        Self::new()
    }
}

/// Subset of a [`ChatRequest`] used to identify matching interactions.
///
/// Fields that are runtime-specific (`session_id`, `request_id`,
/// `stream_timeout_secs`) are intentionally omitted so cassettes remain stable
/// across runs. The `thinking` configuration is included because it may affect
/// provider behavior and outputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VcrRequestKey {
    /// Model identifier.
    pub model: String,
    /// Conversation history.
    pub messages: Vec<ragent_types::llm::ChatMessage>,
    /// Names of the tools advertised to the model.
    ///
    /// Only tool *names* are stored; the full JSON schemas are omitted to keep
    /// cassettes compact and to avoid spurious mismatches when schema wording
    /// changes.
    pub tools: Vec<String>,
    /// Sampling temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Nucleus-sampling cutoff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Maximum tokens to generate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Optional system prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Provider-specific options.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub options: HashMap<String, serde_json::Value>,
    /// Thinking/reasoning configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ragent_types::ThinkingConfig>,
}

impl VcrRequestKey {
    /// Build a stable key from a full [`ChatRequest`].
    #[must_use]
    pub fn from_request(request: &ChatRequest) -> Self {
        Self {
            model: request.model.clone(),
            messages: (*request.messages).clone(),
            tools: request
                .tools
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>(),
            temperature: request.temperature,
            top_p: request.top_p,
            max_tokens: request.max_tokens,
            system: request.system.as_ref().map(|s| (*s).to_string()),
            options: request.options.clone(),
            thinking: request.thinking.clone(),
        }
    }

    /// Normalise the key to a [`serde_json::Value`] for stable comparison.
    ///
    /// Floating-point fields are serialised via `to_bits()` so NaN values can
    /// be compared deterministically.
    fn to_compare_value(&self) -> Result<serde_json::Value> {
        use serde_json::{Number, Value};
        let messages =
            serde_json::to_value(&self.messages).context("serialising messages for comparison")?;
        let tools =
            serde_json::to_value(&self.tools).context("serialising tools for comparison")?;
        let options =
            serde_json::to_value(&self.options).context("serialising options for comparison")?;
        let f32_bits = |v: Option<f32>| v.map(|f| f.to_bits());
        Ok(Value::Object({
            let mut map = serde_json::Map::new();
            map.insert("model".to_owned(), Value::String(self.model.clone()));
            map.insert("messages".to_owned(), messages);
            map.insert("tools".to_owned(), tools);
            map.insert(
                "temperature".to_owned(),
                f32_bits(self.temperature)
                    .map(|b| Value::Number(Number::from(b)))
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "top_p".to_owned(),
                f32_bits(self.top_p)
                    .map(|b| Value::Number(Number::from(b)))
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "max_tokens".to_owned(),
                self.max_tokens
                    .map(|b| Value::Number(Number::from(b)))
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "system".to_owned(),
                self.system
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            map.insert("options".to_owned(), options);
            map.insert(
                "thinking".to_owned(),
                self.thinking
                    .as_ref()
                    .map(|t| serde_json::to_value(t).ok())
                    .unwrap_or(None)
                    .unwrap_or(Value::Null),
            );
            map
        }))
    }
}

impl PartialEq for VcrRequestKey {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_compare_value(), other.to_compare_value()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for VcrRequestKey {}

/// VCR-aware [`LlmClient`] that records or replays interactions.
///
/// Construct with [`VcrClient::new`]. In `Record` mode every call to [`chat`]
/// forwards to the wrapped client, captures the resulting stream events, and
/// appends them to the cassette file. In `Playback` mode matching requests
/// return the recorded events; non-matching requests return an error. In
/// `PlaybackRecordNew` mode matching requests replay and non-matching ones
/// are forwarded to the inner client and then appended to the cassette.
pub struct VcrClient {
    /// The real or mock client used when an interaction is not in the cassette.
    inner: Box<dyn LlmClient>,
    /// Operational mode and cassette path.
    mode: VcrMode,
    /// In-memory cassette loaded from disk (playback) or built during the
    /// test (record).
    cassette: std::sync::Mutex<VcrCassette>,
    /// Counter used to generate unique tool-call ids when the cassette has
    /// been edited to remove ids. Currently unused; kept for forward
    /// compatibility.
    _sequence: std::sync::atomic::AtomicUsize,
}

impl std::fmt::Debug for VcrClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VcrClient")
            .field("mode", &self.mode)
            .field(
                "interactions",
                &self.cassette.lock().unwrap().interactions.len(),
            )
            .finish_non_exhaustive()
    }
}

impl VcrClient {
    /// Wrap an existing client in a VCR layer.
    ///
    /// In playback modes the cassette file is loaded immediately. If the file
    /// does not exist, `Playback` fails fast; `PlaybackRecordNew` and `Record`
    /// start with an empty in-memory cassette.
    ///
    /// # Errors
    ///
    /// Returns an error if the mode is `Playback` and the cassette file cannot
    /// be read or parsed.
    pub async fn new(inner: Box<dyn LlmClient>, mode: VcrMode) -> Result<Self> {
        let cassette = match &mode {
            VcrMode::Playback(path) => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .with_context(|| format!("reading cassette from {}", path.display()))?;
                serde_json::from_str(&content)
                    .with_context(|| format!("parsing cassette from {}", path.display()))?
            }
            VcrMode::PlaybackRecordNew(path) | VcrMode::Record(path) => {
                if path.exists() {
                    let content = tokio::fs::read_to_string(path)
                        .await
                        .with_context(|| format!("reading cassette from {}", path.display()))?;
                    serde_json::from_str(&content)
                        .with_context(|| format!("parsing cassette from {}", path.display()))?
                } else {
                    VcrCassette::new()
                }
            }
        };
        Ok(Self {
            inner,
            mode,
            cassette: std::sync::Mutex::new(cassette),
            _sequence: std::sync::atomic::AtomicUsize::new(0),
        })
    }
    /// Persist the current cassette to the path specified by [`VcrMode`].
    ///
    /// Call this at the end of a recording test to ensure the fixture is
    /// written to disk. It is a no-op in pure `Playback` mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the cassette cannot be serialized or written.
    pub async fn save(&self) -> Result<()> {
        if !self.mode.can_record() {
            return Ok(());
        }
        let cassette = self.cassette.lock().unwrap().clone();
        let json =
            serde_json::to_string_pretty(&cassette).context("serializing cassette to JSON")?;
        let path = self.mode.path().to_path_buf();
        tokio::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))
            .await
            .with_context(|| format!("creating cassette directory for {}", path.display()))?;
        let mut file = tokio::fs::File::create(&path)
            .await
            .with_context(|| format!("creating cassette file {}", path.display()))?;
        file.write_all(json.as_bytes())
            .await
            .with_context(|| format!("writing cassette file {}", path.display()))?;
        file.flush().await.context("flushing cassette file")?;
        Ok(())
    }

    /// Return the number of interactions currently in the cassette.
    #[must_use]
    pub fn interaction_count(&self) -> usize {
        self.cassette.lock().unwrap().interactions.len()
    }
}

#[async_trait::async_trait]
impl LlmClient for VcrClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        let key = VcrRequestKey::from_request(&request);

        // Playback path: if a matching interaction exists, return its events
        // as a stream without touching the network.
        if self.mode.can_playback() {
            let maybe_events = {
                let cassette = self.cassette.lock().unwrap();
                cassette
                    .find(&key)
                    .map(|interaction| interaction.response.clone())
            };
            if let Some(events) = maybe_events {
                return Ok(Box::pin(futures::stream::iter(events)));
            }

            if !self.mode.can_record() {
                bail!(
                    "No cassette interaction matched request for model {} ({} messages); \
                     run in Record or PlaybackRecordNew mode to capture it",
                    key.model,
                    key.messages.len()
                );
            }
        }

        // Record / PlaybackRecordNew path: run the real client, capture the
        // events, append to the in-memory cassette, and also return them.
        let mut stream = self.inner.chat(request).await?;
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        {
            let mut cassette = self.cassette.lock().unwrap();
            cassette.interactions.push(VcrInteraction {
                request: key,
                response: events.clone(),
            });
        }

        // Persist eagerly so a panic or abort in a test still leaves the
        // recorded fixture on disk.
        if self.mode.can_record() {
            self.save().await?;
        }

        Ok(Box::pin(futures::stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use futures::StreamExt;
    use ragent_types::llm::{ChatContent, ChatMessage};

    use super::*;

    fn text_request(text: &str) -> ChatRequest {
        ChatRequest {
            model: "rig-mock".to_owned(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_owned(),
                content: ChatContent::Text(text.to_owned()),
            }]),
            tools: Arc::new(Vec::new()),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: Some("s-1".to_owned()),
            request_id: Some("r-1".to_owned()),
            stream_timeout_secs: Some(42),
            thinking: None,
        }
    }

    struct StaticClient {
        events: Vec<StreamEvent>,
    }

    #[async_trait::async_trait]
    impl LlmClient for StaticClient {
        async fn chat(
            &self,
            _request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
            Ok(Box::pin(futures::stream::iter(self.events.clone())))
        }
    }

    fn static_client(events: Vec<StreamEvent>) -> Box<dyn LlmClient> {
        Box::new(StaticClient { events })
    }

    #[tokio::test]
    async fn vcr_records_and_replays_interaction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vcr.json");
        let events = vec![
            StreamEvent::TextDelta {
                text: "hi".to_owned(),
            },
            StreamEvent::Finish {
                reason: ragent_types::event::FinishReason::Stop,
            },
        ];

        // Record
        let recorder = VcrClient::new(static_client(events.clone()), VcrMode::Record(path.clone()))
            .await
            .unwrap();
        let mut collected = Vec::new();
        let mut stream = recorder.chat(text_request("hello")).await.unwrap();
        while let Some(ev) = stream.next().await {
            collected.push(ev);
        }
        assert_eq!(collected.len(), events.len());
        for (a, b) in collected.iter().zip(events.iter()) {
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
        assert_eq!(recorder.interaction_count(), 1);
        recorder.save().await.unwrap();
        assert!(path.exists());

        // Replay
        let player = VcrClient::new(
            Box::new(StaticClient { events: Vec::new() }),
            VcrMode::Playback(path.clone()),
        )
        .await
        .unwrap();
        let mut replayed = Vec::new();
        let mut stream = player.chat(text_request("hello")).await.unwrap();
        while let Some(ev) = stream.next().await {
            replayed.push(ev);
        }
        assert_eq!(replayed.len(), events.len());
        for (a, b) in replayed.iter().zip(events.iter()) {
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
    }

    #[tokio::test]
    async fn vcr_playback_fails_when_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.json");
        tokio::fs::write(&path, b"{\"version\":1,\"interactions\":[]}")
            .await
            .unwrap();
        let player = VcrClient::new(
            Box::new(StaticClient { events: Vec::new() }),
            VcrMode::Playback(path.clone()),
        )
        .await
        .unwrap();
        let result = player.chat(text_request("unknown")).await;
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("No cassette interaction matched")),
            Ok(_) => panic!("expected no-match error"),
        }
    }
}
