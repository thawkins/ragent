//! `plot_bar` tool — render a bar chart on the message window.

use anyhow::{Result, bail};
use ratatui_plt::prelude::{Color, Theme};
use ratatui_plt::widgets::bar_chart::{BarChart, BarDataset, BarMode, Orientation};
use serde_json::{Value, json};

use super::{
    DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_HEIGHT, MAX_WIDTH, canvas_dimension, error_output,
    parse_bool, parse_color, parse_floats, parse_labels, parse_title, render_ansi, render_text,
    success_output,
};
use crate::{Tool, ToolContext, ToolOutput};

/// A single bar dataset parsed from a tool argument.
struct BarSpec {
    name: String,
    values: Vec<f64>,
    color: Option<Color>,
}

fn parse_datasets(input: &Value) -> Result<Vec<BarSpec>> {
    let input = &super::coerce_json(input);
    let arr = match input {
        Value::Array(a) => a.clone(),
        Value::Object(_) => vec![input.clone()],
        other => bail!("expected a dataset or array of datasets, got {other}"),
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let obj = item
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("expected a dataset object, got {item}"))?;
        let values_val = obj
            .get("data")
            .or_else(|| obj.get("values"))
            .ok_or_else(|| anyhow::anyhow!("dataset {item:?} is missing required 'data'"))?;
        let values = parse_floats(values_val)?;
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("series")
            .to_string();
        let color = parse_color(
            obj.get("color")
                .and_then(Value::as_str)
                .or_else(|| obj.get("colour").and_then(Value::as_str)),
        );
        out.push(BarSpec {
            name,
            values,
            color,
        });
    }
    Ok(out)
}

#[async_trait::async_trait]
impl Tool for PlotBarTool {
    fn name(&self) -> &'static str {
        "plot_bar"
    }

    fn description(&self) -> &'static str {
        "Render a bar chart on the message window. Required: 'categories' — \
         an array of category labels; 'datasets' — one object \
         {name?, data: [values], color?} or an array of such objects. \
         Optional: 'title', 'x_label', 'y_label', 'width', 'height', \
         'horizontal' (bool, default false), 'stacked' (bool, default false)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "categories": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "datasets": {
                    "oneOf": [
                        { "type": "array", "items": { "$ref": "#/$defs/dataset" } },
                        { "$ref": "#/$defs/dataset" }
                    ]
                },
                "title": { "type": "string" },
                "x_label": { "type": "string" },
                "y_label": { "type": "string" },
                "width": { "type": "integer", "minimum": 1, "maximum": 220 },
                "height": { "type": "integer", "minimum": 1, "maximum": 80 },
                "horizontal": { "type": "boolean" },
                "stacked": { "type": "boolean" }
            },
            "$defs": {
                "dataset": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "data": {
                            "type": "array",
                            "items": { "type": "number" }
                        },
                        "color": { "type": "string" }
                    },
                    "required": ["data"],
                    "additionalProperties": false
                }
            },
            "required": ["categories", "datasets"],
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
        let cats_val = match input.get("categories") {
            Some(v) => super::coerce_json(v),
            None => {
                return Ok(error_output(
                    "missing required parameter 'categories'",
                    None,
                ));
            }
        };
        let categories = match parse_labels(&cats_val) {
            Ok(c) => c,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let ds_val = match input.get("datasets") {
            Some(v) => super::coerce_json(v),
            None => return Ok(error_output("missing required parameter 'datasets'", None)),
        };
        let datasets = match parse_datasets(&ds_val) {
            Ok(d) => d,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        if datasets.is_empty() {
            return Ok(error_output("'datasets' contains no datasets", None));
        }

        let width = match canvas_dimension(&input, "width", DEFAULT_WIDTH, MAX_WIDTH) {
            Ok(w) => w,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let height = match canvas_dimension(&input, "height", DEFAULT_HEIGHT, MAX_HEIGHT) {
            Ok(h) => h,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let horizontal = match parse_bool(&input, "horizontal", false) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };
        let stacked = match parse_bool(&input, "stacked", false) {
            Ok(b) => b,
            Err(e) => return Ok(error_output(e.to_string(), None)),
        };

        let mut chart = BarChart::new()
            .theme(Theme::dark())
            .categories(categories.clone())
            .mode(if stacked {
                BarMode::Stacked
            } else {
                BarMode::Grouped
            })
            .orientation(if horizontal {
                Orientation::Horizontal
            } else {
                Orientation::Vertical
            });
        if let Some(t) = parse_title(&input) {
            chart = chart.title(t);
        }
        for (i, ds) in datasets.iter().enumerate() {
            let color = ds.color.unwrap_or_else(|| palette_color(i));
            chart = chart.dataset(BarDataset::new(ds.name.clone(), ds.values.clone(), color));
        }

        let content = render_text(&chart, width, height);
        let ansi = render_ansi(&chart, width, height);
        let meta = json!({
            "width": width,
            "height": height,
            "categories": categories.len(),
            "datasets": datasets.len(),
            "horizontal": horizontal,
            "stacked": stacked,
            "plot_ansi": ansi,
        });
        Ok(success_output(content, "bar", meta))
    }
}

/// A small fixed palette so uncoloured datasets are visually distinct.
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

/// Render a bar chart from `categories` + `datasets` arguments.
pub struct PlotBarTool;
