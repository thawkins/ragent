//! `model_info` — Report metadata about the currently connected LLM.
//!
//! Implements a read-only introspection tool that returns the active
//! provider/model pair plus resolved capabilities, context window, output
//! limits, cost tier, and thinking support. When the active provider is the
//! Model Router, the tool also reports the router's enabled state and notes
//! that the effective downstream model is selected per request.

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;

use super::{Tool, ToolContext, ToolOutput};
use crate::agent::ModelRef;
use crate::provider::{ModelInfo, ProviderRegistry};

/// Read-only tool that reports information and statistics about the currently
/// connected model.
pub struct ModelInfoTool;

/// Normalized metadata returned by [`ModelInfoTool`].
#[derive(Debug, Clone, Serialize)]
struct ModelMetadata {
    provider_id: String,
    provider_name: String,
    model_id: String,
    model_name: String,
    context_window: usize,
    max_output: Option<usize>,
    capabilities: ModelCapabilities,
    cost: ModelCost,
    thinking: ThinkingSummary,
    router: Option<RouterSummary>,
}

/// Capability flags as booleans for easy JSON consumption.
#[derive(Debug, Clone, Serialize)]
struct ModelCapabilities {
    reasoning: bool,
    streaming: bool,
    vision: bool,
    tool_use: bool,
    thinking_levels: Vec<ragent_types::ThinkingLevel>,
}

/// Cost summary in USD per 1M tokens.
#[derive(Debug, Clone, Serialize)]
struct ModelCost {
    input_per_1m: f64,
    output_per_1m: f64,
    request_multiplier: Option<f64>,
}

/// Thinking support summary.
#[derive(Debug, Clone, Serialize)]
struct ThinkingSummary {
    supported: bool,
    levels: Vec<ragent_types::ThinkingLevel>,
    default: Option<String>,
}

/// Router-specific state, present only when the active provider is `router`.
#[derive(Debug, Clone, Serialize)]
struct RouterSummary {
    enabled: bool,
    note: &'static str,
}

impl ModelInfoTool {
    fn parameters_schema_value() -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["text", "json"],
                    "description": "Output format: 'text' (human-readable markdown, default) or 'json' (structured metadata only)"
                }
            },
            "additionalProperties": false
        })
    }
}

#[async_trait::async_trait]
impl Tool for ModelInfoTool {
    fn name(&self) -> &'static str {
        "model_info"
    }

    fn description(&self) -> &'static str {
        "Report information and statistics about the currently connected LLM model. \
         Returns the active provider/model pair, provider display name, capabilities, \
         context window, max output tokens, cost tier, and thinking support. When the \
         Model Router is active, it also reports whether routing is enabled and notes \
         that the effective downstream model is chosen per request. Optional parameter: \
         'format' ('text' or 'json', default 'text')."
    }

    fn parameters_schema(&self) -> Value {
        Self::parameters_schema_value()
    }

    fn permission_category(&self) -> &'static str {
        "model:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let model_ref = ctx
            .active_model
            .as_ref()
            .context("No active model is set for this session.")?;
        let registry = ctx
            .provider_registry
            .as_ref()
            .context("Provider registry is not available in this session.")?;

        let format = input["format"].as_str().unwrap_or("text");
        let meta = build_metadata(model_ref, registry).await?;

        match format {
            "json" => Ok(ToolOutput {
                content: serde_json::to_string_pretty(&meta)?,
                metadata: Some(serde_json::to_value(&meta)?),
            }),
            _ => Ok(ToolOutput {
                content: render_text(&meta),
                metadata: Some(serde_json::to_value(&meta)?),
            }),
        }
    }
}

/// Resolve provider name and model metadata, falling back gracefully when
/// discovery is unavailable.
async fn build_metadata(
    model_ref: &ModelRef,
    registry: &Arc<ProviderRegistry>,
) -> Result<ModelMetadata> {
    let provider_name = registry
        .get(&model_ref.provider_id)
        .map(|p| p.name().to_string())
        .unwrap_or_else(|| model_ref.provider_id.clone());

    let (model, router): (Option<ModelInfo>, Option<RouterSummary>) = if model_ref.provider_id
        == "router"
    {
        let router_summary = registry
                .get_as_any("router")
                .and_then(|any| any.downcast_ref::<crate::provider::router::RouterProvider>())
                .map(|rp| RouterSummary {
                    enabled: rp.is_enabled(),
                    note: "The router selects a concrete downstream provider/model for each request; there is no single fixed effective model.",
                });
        (registry.resolve_model("router", "router"), router_summary)
    } else {
        let discovered = registry
            .resolve_model_async(&model_ref.provider_id, &model_ref.model_id)
            .await;
        (discovered, None)
    };

    let model = model.unwrap_or_else(|| ModelInfo {
        id: model_ref.model_id.clone(),
        provider_id: model_ref.provider_id.clone(),
        name: model_ref.model_id.clone(),
        cost: ragent_config::Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: ragent_config::Capabilities::default(),
        context_window: 0,
        max_output: None,
        request_multiplier: None,
        thinking_config: None,
    });

    Ok(ModelMetadata {
        provider_id: model_ref.provider_id.clone(),
        provider_name,
        model_id: model.id.clone(),
        model_name: model.name.clone(),
        context_window: model.context_window,
        max_output: model.max_output,
        capabilities: ModelCapabilities {
            reasoning: model.capabilities.reasoning,
            streaming: model.capabilities.streaming,
            vision: model.capabilities.vision,
            tool_use: model.capabilities.tool_use,
            thinking_levels: model.capabilities.thinking_levels.clone(),
        },
        cost: ModelCost {
            input_per_1m: model.cost.input,
            output_per_1m: model.cost.output,
            request_multiplier: model.request_multiplier,
        },
        thinking: ThinkingSummary {
            supported: !model.capabilities.thinking_levels.is_empty(),
            levels: model.capabilities.thinking_levels.clone(),
            default: model.thinking_config.as_ref().map(|tc| format!("{tc:?}")),
        },
        router,
    })
}

/// Render a human-readable markdown report.
fn render_text(meta: &ModelMetadata) -> String {
    let mut lines = vec![
        "## Currently Connected Model".to_string(),
        String::new(),
        format!(
            "- **Provider**: {} (`{}`)",
            meta.provider_name, meta.provider_id
        ),
        format!("- **Model**: {} (`{}`)", meta.model_name, meta.model_id),
    ];

    if meta.context_window > 0 {
        lines.push(format!(
            "- **Context window**: {} tokens",
            meta.context_window
        ));
    } else {
        lines.push("- **Context window**: (unknown)".to_string());
    }

    if let Some(max_out) = meta.max_output {
        lines.push(format!("- **Max output**: {max_out} tokens"));
    } else {
        lines.push("- **Max output**: (unlimited / provider-defined)".to_string());
    }

    lines.push(format!(
        "- **Capabilities**: streaming={}, tool_use={}, vision={}, reasoning={}",
        meta.capabilities.streaming,
        meta.capabilities.tool_use,
        meta.capabilities.vision,
        meta.capabilities.reasoning
    ));

    if meta.capabilities.thinking_levels.is_empty() {
        lines.push("- **Thinking support**: none".to_string());
    } else {
        let levels: Vec<String> = meta
            .capabilities
            .thinking_levels
            .iter()
            .map(|l| format!("{l:?}"))
            .collect();
        lines.push(format!("- **Thinking support**: {}", levels.join(", ")));
    }

    lines.push(format!(
        "- **Cost**: ${:.2}/1M input, ${:.2}/1M output",
        meta.cost.input_per_1m, meta.cost.output_per_1m
    ));

    if let Some(multiplier) = meta.cost.request_multiplier {
        lines.push(format!("- **Request multiplier**: {multiplier}"));
    }

    if let Some(router) = &meta.router {
        lines.push(String::new());
        lines.push("### Model Router".to_string());
        lines.push(format!("- **Enabled**: {}", router.enabled));
        lines.push(format!("- **Note**: {}", router.note));
    }

    lines.join("\n")
}
