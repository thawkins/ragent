//! `plot_pie` tool — render a pie/donut chart on the message window.

use anyhow::Result;
use ratatui_plt::prelude::{Color, Theme};
use ratatui_plt::widgets::pie_chart::{PieChart, PieSlice};
use serde_json::{Value, json};

use super::{
    DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_HEIGHT, MAX_WIDTH, canvas_dimension, error_output,
    parse_bool, parse_color, parse_title, render_ansi, render_text, success_output,
};
use crate::{Tool, ToolContext, ToolOutput};

struct SliceSpec {
    label: String,
    value: f64,
    color: Option<Color>,
}

fn parse_slices(input: &Value) -> Result<Vec<SliceSpec>> {
    let input = &super::coerce_json(input);
    let arr = input
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("'slices' must be an array of objects, got {input}"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let obj = item
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("expected a slice object, got {item}"))?;
        let label = obj
            .get("label")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("slice is missing required 'label'"))?
            .to_string();
        let value = obj
            .get("value")
            .and_then(Value::as_f64)
            .or_else(|| obj.get("value").and_then(Value::as_u64).map(|u| u as f64))
            .ok_or_else(|| anyhow::anyhow!("slice '{label}' is missing numeric 'value'"))?;
        let color = parse_color(
            obj.get("color")
                .and_then(Value::as_str)
                .or_else(|| obj.get("colour").and_then(Value::as_str)),
        );
        out.push(SliceSpec {
            label,
            value,
            color,
        });
    }
    Ok(out)
}

#[async_trait::async_trait]
impl Tool for PlotPieTool {
    fn name(&self) -> &'static str {
        "plot_pie"
    }

    fn description(&self) -> &'static str {
        "Render a pie (or donut) chart on the message window. Required: \
         'slices' — an array of {label, value, color?}. Optional: 'title', \
         'donut' (bool, default false), 'radius' (donut hole ratio \
         [0,1), default 0.35), 'percentages' (bool, default true), \
         'labels' (bool, default true), 'width', 'height'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "slices": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": { "type": "string" },
                            "value": { "type": "number" },
                            "color": { "type": "string" }
                        },
                        "required": ["label", "value"],
                        "additionalProperties": false
                    }
                },
                "title": { "type": "string" },
                "donut": { "type": "boolean" },
                "radius": { "type": "number", "minimum": 0, "maximum": 0.95 },
                "percentages": { "type": "boolean" },
                "labels": { "type": "boolean" },
                "width": { "type": "integer", "minimum": 1, "maximum": 220 },
                "height": { "type": "integer", "minimum": 1, "maximum": 80 }
            },
            "required": ["slices"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "system"
    }

    /// # Errors
    ///
    /// Returns an error output (never a session crash) when arguments are
    /// missing or malformed.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let slices_val = match input.get("slices") {
            Some(v) => super::coerce_json(v),
            None => return Ok(error_output("missing required parameter 'slices'", None)),
        };
        let slices = match parse_slices(&slices_val) {
            Ok(s) => s,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        if slices.is_empty() {
            return Ok(error_output("'slices' contains no slices", None));
        }

        let width = match canvas_dimension(&input, "width", DEFAULT_WIDTH, MAX_WIDTH) {
            Ok(w) => w,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let height = match canvas_dimension(&input, "height", DEFAULT_HEIGHT, MAX_HEIGHT) {
            Ok(h) => h,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let donut = match parse_bool(&input, "donut", false) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let percentages = match parse_bool(&input, "percentages", true) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let labels = match parse_bool(&input, "labels", true) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let radius = input
            .get("radius")
            .and_then(Value::as_f64)
            .unwrap_or(0.35)
            .clamp(0.0, 0.95);

        let mut chart = PieChart::new()
            .theme(Theme::dark())
            .show_labels(labels)
            .show_percentages(percentages);
        if donut {
            chart = chart.donut_ratio(radius);
        }
        if let Some(t) = parse_title(&input) {
            chart = chart.title(t);
        }
        for (i, slice) in slices.iter().enumerate() {
            let color = slice.color.unwrap_or_else(|| palette_color(i));
            chart = chart.slice(PieSlice::new(slice.label.clone(), slice.value).color(color));
        }

        let content = render_text(&chart, width, height);
        let ansi = render_ansi(&chart, width, height);
        let meta = json!({
            "width": width,
            "height": height,
            "slices": slices.len(),
            "donut": donut,
            "plot_ansi": ansi,
        });
        Ok(success_output(content, "pie", meta))
    }
}

/// A small fixed palette so uncoloured slices are visually distinct.
fn palette_color(i: usize) -> Color {
    const PALETTE: [Color; 8] = [
        Color::Cyan,
        Color::Yellow,
        Color::Magenta,
        Color::Green,
        Color::Red,
        Color::Blue,
        Color::LightCyan,
        Color::LightMagenta,
    ];
    PALETTE[i % PALETTE.len()]
}

/// Render a pie chart from a `slices` argument.
pub struct PlotPieTool;
