//! Topic-to-sub-question planner for the research extensions (T-003, FR-001).
//!
//! A [`Planner`] turns a user-supplied research topic into a focused
//! [`ResearchPlan`] of sub-questions. Sub-questions are the unit of work for
//! the iterative research loop (T-004) and drive both retrieval and
//! synthesis.
//!
//! Two implementations are provided:
//!
//! - [`HeuristicPlanner`] — deterministic, no network calls; splits the topic
//!   into generic but focused questions using punctuation, conjunctions, and
//!   key phrases.
//! - [`LlmPlanner`] — LLM-backed; asks a configured model to emit a JSON
//!   array of sub-questions. Falls back to the heuristic planner when the LLM
//!   is unavailable or returns empty content.

use crate::state::{ResearchPlan, SubQuestion};
use async_trait::async_trait;
use futures::StreamExt;
use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_llm::provider::ProviderRegistry;
use regex::Regex;
use std::sync::Arc;

/// Abstraction over topic planners.
#[async_trait]
pub trait Planner: Send + Sync {
    /// Decompose `topic` into a focused [`ResearchPlan`].
    async fn plan(&self, topic: &str) -> anyhow::Result<ResearchPlan>;
}

/// Deterministic planner that needs no network access.
///
/// `HeuristicPlanner` creates between one and five sub-questions:
///
/// 1. An overview question about the topic as a whole.
/// 2. A "how it works" question.
/// 3. A "key benefits / trade-offs" question.
/// 4. One question per clause extracted from the topic (up to two).
///
/// This is intentionally simple; the LLM-backed planner can be swapped in
/// when higher-quality decomposition is required.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicPlanner;

impl HeuristicPlanner {
    /// Create a new heuristic planner.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Extract short clauses from a topic by splitting on common separators.
    fn clauses(topic: &str) -> Vec<String> {
        let separators = [", ", "; ", " and ", " plus ", " vs ", " versus "];
        let mut parts: Vec<String> = vec![topic.to_string()];
        for sep in &separators {
            let mut next = Vec::new();
            for part in &parts {
                for chunk in part.split(sep) {
                    let trimmed = chunk.trim();
                    if !trimmed.is_empty() {
                        next.push(trimmed.to_string());
                    }
                }
            }
            parts = next;
        }
        parts
            .into_iter()
            .filter(|p| !p.is_empty() && p.len() > 3)
            .collect()
    }

    /// Build the stable sub-question id from the topic and index.
    fn id(topic: &str, idx: usize) -> String {
        let slug = topic
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-")
            .split('-')
            .filter(|s| !s.is_empty())
            .take(4)
            .collect::<Vec<_>>()
            .join("-");
        format!("{slug}-{idx:02}")
    }
}

#[async_trait]
impl Planner for HeuristicPlanner {
    async fn plan(&self, topic: &str) -> anyhow::Result<ResearchPlan> {
        let trimmed = topic.trim();
        if trimmed.is_empty() {
            anyhow::bail!("topic is empty");
        }
        let mut plan = ResearchPlan::new(trimmed);
        let mut i = 1;

        plan.sub_questions.push(SubQuestion {
            id: Self::id(trimmed, i),
            question: format!("What is the current state of '{trimmed}'?"),
            status: crate::state::SubQuestionStatus::Pending,
            priority: 10,
        });
        i += 1;

        plan.sub_questions.push(SubQuestion {
            id: Self::id(trimmed, i),
            question: format!("How does '{trimmed}' work in practice?"),
            status: crate::state::SubQuestionStatus::Pending,
            priority: 9,
        });
        i += 1;

        plan.sub_questions.push(SubQuestion {
            id: Self::id(trimmed, i),
            question: format!("What are the main benefits and trade-offs of '{trimmed}'?"),
            status: crate::state::SubQuestionStatus::Pending,
            priority: 8,
        });
        i += 1;

        let clauses = Self::clauses(trimmed);
        for (offset, clause) in clauses.iter().take(2).enumerate() {
            if clause != trimmed {
                plan.sub_questions.push(SubQuestion {
                    id: Self::id(trimmed, i + offset),
                    question: format!("What is the role of '{clause}' in this topic?"),
                    status: crate::state::SubQuestionStatus::Pending,
                    priority: 7,
                });
            }
        }

        Ok(plan)
    }
}

/// LLM-backed planner that asks a provider/model for focused sub-questions.
///
/// If the provider is missing, the model refuses, or the response cannot be
/// parsed, the planner falls back to [`HeuristicPlanner`].
#[derive(Clone)]
pub struct LlmPlanner {
    registry: Arc<ProviderRegistry>,
    provider_id: String,
    model_id: String,
    api_key: Option<String>,
    base_url: Option<String>,
    fallback: HeuristicPlanner,
}

impl std::fmt::Debug for LlmPlanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmPlanner")
            .field("provider_id", &self.provider_id)
            .field("model_id", &self.model_id)
            .field("has_api_key", &self.api_key.is_some())
            .finish_non_exhaustive()
    }
}

impl LlmPlanner {
    /// Build a new LLM planner. The provider/model are resolved at plan time.
    pub fn new(
        registry: Arc<ProviderRegistry>,
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            registry,
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            api_key: None,
            base_url: None,
            fallback: HeuristicPlanner::new(),
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
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LlmSubQuestion {
    question: String,
    #[serde(default)]
    priority: u8,
}

#[async_trait]
impl Planner for LlmPlanner {
    async fn plan(&self, topic: &str) -> anyhow::Result<ResearchPlan> {
        let trimmed = topic.trim();
        if trimmed.is_empty() {
            anyhow::bail!("topic is empty");
        }

        let provider = match self.registry.get(&self.provider_id) {
            Some(p) => p,
            None => return self.fallback.plan(topic).await,
        };

        let client = match provider
            .create_client(
                self.api_key.clone().unwrap_or_default().as_str(),
                self.base_url.as_deref(),
                &std::collections::HashMap::new(),
            )
            .await
        {
            Ok(c) => c,
            Err(_) => return self.fallback.plan(topic).await,
        };

        let prompt = format!(
            "You are a research planner. Break the following topic into 3-5 focused sub-questions. \
             Return ONLY a JSON array of objects with fields 'question' and 'priority' (1-10).\n\nTopic: {trimmed}"
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
            max_tokens: Some(1024),
            system: Some(std::sync::Arc::from("Return only valid JSON. No prose.")),
            options: std::collections::HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: Some(120),
            thinking: None,
        };

        let mut stream = match client.chat(request).await {
            Ok(s) => s,
            Err(_) => return self.fallback.plan(topic).await,
        };

        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text: delta } => text.push_str(&delta),
                StreamEvent::Error { .. } | StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }

        let questions = match parse_llm_questions(&text, trimmed) {
            Some(qs) if !qs.is_empty() => qs,
            _ => return self.fallback.plan(topic).await,
        };

        let mut plan = ResearchPlan::new(trimmed);
        for (idx, q) in questions.into_iter().enumerate() {
            plan.sub_questions.push(SubQuestion {
                id: HeuristicPlanner::id(trimmed, idx + 1),
                question: q.question,
                status: crate::state::SubQuestionStatus::Pending,
                priority: q.priority.clamp(1, 10),
            });
        }
        Ok(plan)
    }
}

/// Extract a JSON array of sub-questions from raw LLM text. Tolerates a small
/// amount of surrounding markdown fencing.
fn parse_llm_questions(text: &str, topic: &str) -> Option<Vec<LlmSubQuestion>> {
    let text = text.trim();
    let code_fence = Regex::new(r"```(?:json)?\s*([\s\S]*?)```").ok()?;
    let inner = code_fence
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
        .unwrap_or(text);
    let mut questions: Vec<LlmSubQuestion> = serde_json::from_str(inner).ok()?;
    for q in &mut questions {
        if q.question.is_empty() {
            q.question = format!("What is the role of '{topic}' in this topic?");
        }
        if q.priority == 0 {
            q.priority = 5;
        }
    }
    Some(questions)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
    use super::*;

    #[tokio::test]
    async fn heuristic_plan_returns_sub_questions() {
        let planner = HeuristicPlanner::new();
        let plan = planner.plan("Rust async runtimes").await.unwrap();
        assert!(!plan.sub_questions.is_empty());
        assert_eq!(plan.topic, "Rust async runtimes");
        assert!(
            plan.sub_questions
                .iter()
                .any(|sq| sq.question.contains("Rust async runtimes"))
        );
    }

    #[tokio::test]
    async fn heuristic_plan_empty_topic_fails() {
        let planner = HeuristicPlanner::new();
        assert!(planner.plan("   ").await.is_err());
    }

    #[tokio::test]
    async fn heuristic_plan_splits_clauses() {
        let planner = HeuristicPlanner::new();
        let plan = planner
            .plan("tokio, async-std, and smol async runtimes")
            .await
            .unwrap();
        let ids: Vec<_> = plan.sub_questions.iter().map(|sq| &sq.id).collect();
        let unique: std::collections::HashSet<_> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn parse_llm_questions_extracts_json_array() {
        let json = r#"[{"question":"What is X?","priority":9},{"question":"How does Y work?","priority":7}]"#;
        let qs = parse_llm_questions(json, "topic").unwrap();
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[0].question, "What is X?");
        assert_eq!(qs[0].priority, 9);
    }

    #[test]
    fn parse_llm_questions_handles_markdown_fence() {
        let text = "```json\n[{\"question\":\"Q1\",\"priority\":5}]\n```";
        let qs = parse_llm_questions(text, "topic").unwrap();
        assert_eq!(qs.len(), 1);
    }

    #[test]
    fn parse_llm_questions_defaults_empty_priority() {
        let json = r#"[{"question":"Q1"}]"#;
        let qs = parse_llm_questions(json, "topic").unwrap();
        assert_eq!(qs[0].priority, 5);
    }
}
