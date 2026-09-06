//! Shared rendering helpers for the `plot_*` tool family.
//!
//! These tools render graphs on the ragent message window by drawing with
//! [`ratatui-plt`](https://crates.io/crates/ratatui-plt) — a GPL-3.0
//! scientific-plotting widget set built on ratatui. Each tool builds a widget
//! (line, scatter, bar, histogram, pie, or heatmap) and renders it off-screen
//! to a terminal-friendly plain-text canvas that the message window displays
//! inline, in the same way it renders Mermaid diagrams.
//!
//! Rendering is fully local — no network requests are made, and the widgets
//! are rendered into an off-screen ratatui [`Buffer`] then flattened to text
//! via [`render_to_buffer`] + [`buffer_to_text`].
//!
//! # Licensing note
//!
//! `ratatui-plt` is GPL-3.0. This module (and the whole
//! `ragent-tools-extended` crate) MIT-links against it. The dependency was
//! explicitly accepted by the project owner (2026-09-06) and added to the
//! cargo-deny license allow list. It is pulled in with
//! `default-features = false` so only the pure off-screen widgets are used,
//! never the crossterm terminal features.

pub mod plot_bar;
pub mod plot_heatmap;
pub mod plot_histogram;
pub mod plot_line;
pub mod plot_pie;
pub mod plot_scatter;

use anyhow::{Result, bail};
use ratatui::widgets::Widget;
use ratatui_plt::export::{buffer_to_ansi, buffer_to_text, render_to_buffer};
use ratatui_plt::prelude::{Color, GridData, Series};
use serde_json::Value;

use crate::ToolOutput;

/// Default canvas width (message-window columns) when `width` is omitted.
pub const DEFAULT_WIDTH: u16 = 80;

/// Default canvas height (message-window rows) when `height` is omitted.
pub const DEFAULT_HEIGHT: u16 = 20;

/// The maximum canvas width we are willing to produce, to keep tool output
/// bounded and readable inside the message window.
pub const MAX_WIDTH: u16 = 220;

/// The maximum canvas height we are willing to produce.
pub const MAX_HEIGHT: u16 = 80;

/// Render any ratatui widget to a plain-text canvas.
///
/// The widget is rendered into an off-screen buffer of the requested
/// dimensions and flattened to text, with trailing whitespace trimmed per
/// line by `ratatui_plt`.
pub fn render_text<W: Widget>(widget: W, width: u16, height: u16) -> String {
    let buf = render_to_buffer(widget, width, height);
    buffer_to_text(&buf)
}

/// Render any ratatui widget to an ANSI-coloured canvas.
///
/// Same off-screen render as [`render_text`], but the flattened output keeps
/// the per-cell foreground/background colours as SGR escape sequences. The
/// TUI parses these back into styled spans so multi-colour plots (palette
/// series, pie slices, heatmaps) display with their real colours instead of a
/// single flat colour.
pub fn render_ansi<W: Widget>(widget: W, width: u16, height: u16) -> String {
    let buf = render_to_buffer(widget, width, height);
    buffer_to_ansi(&buf)
}

/// Read an optional unsigned integer parameter, bounded to a sane canvas
/// range so a malicious or accidental `width`/`height` cannot produce
/// unbounded output.
pub fn canvas_dimension(input: &Value, key: &str, default: u16, max: u16) -> Result<u16> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Number(n)) => {
            let v = n.as_u64().unwrap_or(default as u64);
            if v == 0 {
                Ok(default)
            } else {
                Ok(v.min(max as u64) as u16)
            }
        }
        Some(other) => bail!("parameter '{key}' must be an integer, got {other}"),
    }
}

/// Read an optional string parameter.
pub fn opt_string<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

/// Parse an optional `title` string parameter into an owned `Option<String>`.
pub fn parse_title(input: &Value) -> Option<String> {
    opt_string(input, "title")
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// Parse an optional bool parameter with a default.
pub fn parse_bool(input: &Value, key: &str, default: bool) -> Result<bool> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(other) => bail!("parameter '{key}' must be a boolean, got {other}"),
    }
}

/// Map a small set of common colour names (plus hex `#rrggbb`) to a ratatui
/// [`Color`]. Returns `None` when the caller does not provide a colour (so
/// the theme colour cycle can assign one automatically).
pub fn parse_color(name: Option<&str>) -> Option<Color> {
    let name = name?.trim();
    if name.is_empty() {
        return None;
    }
    match name.to_ascii_lowercase().as_str() {
        "reset" => Some(Color::Reset),
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "white" => Some(Color::White),
        "darkgray" | "dark_gray" => Some(Color::DarkGray),
        "lightred" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        hex if hex.starts_with('#') && hex.len() == 7 => {
            let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
            let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
            let b = u8::from_str_radix(&hex[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

/// Parse a list of `[x, y]` pairs (or `{x, y}` objects) into a vector of
/// (f64, f64) points.
pub fn parse_pairs(v: &Value) -> Result<Vec<(f64, f64)>> {
    let arr = v
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected an array of [x, y] points, got {v}"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let (x, y) = match item {
            Value::Array(pair) if pair.len() == 2 => {
                let x = pair[0]
                    .as_f64()
                    .or_else(|| pair[0].as_u64().map(|u| u as f64));
                let y = pair[1]
                    .as_f64()
                    .or_else(|| pair[1].as_u64().map(|u| u as f64));
                match (x, y) {
                    (Some(x), Some(y)) => (x, y),
                    _ => bail!("expected numeric [x, y] point, got {item}"),
                }
            }
            Value::Object(map) => {
                let x = map
                    .get("x")
                    .and_then(Value::as_f64)
                    .or_else(|| map.get("x").and_then(Value::as_u64).map(|u| u as f64));
                let y = map
                    .get("y")
                    .and_then(Value::as_f64)
                    .or_else(|| map.get("y").and_then(Value::as_u64).map(|u| u as f64));
                match (x, y) {
                    (Some(x), Some(y)) => (x, y),
                    _ => bail!("expected numeric {{x, y}} point, got {item}"),
                }
            }
            _ => bail!("expected an [x, y] point, got {item}"),
        };
        out.push((x, y));
    }
    Ok(out)
}

/// Coerce a JSON-string-encoded argument value into its parsed form.
///
/// Some LLM providers and tool bridges serialize complex array-or-object
/// arguments as JSON strings instead of structured values. When `v` is a
/// string whose content parses as JSON, return the parsed value; otherwise
/// return `v` unchanged so the caller's normal validation produces the
/// appropriate type error.
#[must_use]
pub fn coerce_json(v: &Value) -> Value {
    match v {
        Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| v.clone()),
        other => other.clone(),
    }
}

/// Parse a list of finite floats.
pub fn parse_floats(v: &Value) -> Result<Vec<f64>> {
    let v = &coerce_json(v);
    let arr = v
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected an array of numbers, got {v}"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let f = item
            .as_f64()
            .or_else(|| item.as_u64().map(|u| u as f64))
            .ok_or_else(|| anyhow::anyhow!("expected a number, got {item}"))?;
        out.push(f);
    }
    Ok(out)
}

/// Parse a list of label strings.
pub fn parse_labels(v: &Value) -> Result<Vec<String>> {
    let arr = v
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected an array of strings, got {v}"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("expected a string label, got {item}"))?;
        out.push(s.to_string());
    }
    Ok(out)
}

/// A single named data series parsed from a tool argument object.
#[derive(Debug)]
pub struct SeriesSpec {
    /// Display name (used in the legend).
    pub name: String,
    /// (x, y) data points.
    pub data: Vec<(f64, f64)>,
    /// Optional explicit colour.
    pub color: Option<Color>,
}

/// Parse a `series` argument into a list of [`SeriesSpec`] values.
///
/// The value is either an array of objects `{name?, data: [[x,y],...], color?}`
/// or (for convenience) a single such object.
pub fn parse_series(input: &Value) -> Result<Vec<SeriesSpec>> {
    let input = &coerce_json(input);
    let arr = match input {
        Value::Array(a) => a.clone(),
        Value::Object(_) => vec![input.clone()],
        other => bail!("expected a series or array of series, got {other}"),
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let obj = item
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("expected a series object, got {item}"))?;
        let data_val = obj
            .get("data")
            .ok_or_else(|| anyhow::anyhow!("series {item:?} is missing required 'data'"))?;
        let data = parse_pairs(data_val)?;
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
        out.push(SeriesSpec { name, data, color });
    }
    Ok(out)
}

/// Build a [`Series`] from a [`SeriesSpec`], applying an optional marker
/// shape (used by the scatter tool).
pub fn build_series(
    spec: &SeriesSpec,
    marker: Option<ratatui_plt::prelude::MarkerShape>,
) -> Series {
    let mut s = Series::new(spec.name.clone()).data(spec.data.clone());
    if let Some(c) = spec.color {
        s = s.color(c);
    }
    if let Some(m) = marker {
        s = s.marker(m);
    }
    s
}

/// Parse a 2D grid `{x: [...], y: [...], values: [[...]]}` into [`GridData`].
pub fn parse_grid(input: &Value) -> Result<GridData> {
    let input = &coerce_json(input);
    let obj = input
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("expected a grid object, got {input}"))?;
    let x_val = obj
        .get("x")
        .ok_or_else(|| anyhow::anyhow!("grid missing required 'x'"))?;
    let y_val = obj
        .get("y")
        .ok_or_else(|| anyhow::anyhow!("grid missing required 'y'"))?;
    let values_val = obj
        .get("values")
        .ok_or_else(|| anyhow::anyhow!("grid missing required 'values'"))?;
    let x = parse_floats(x_val)?;
    let y = parse_floats(y_val)?;
    let values_arr = values_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("'values' must be a 2D array of numbers"))?;
    let mut values = Vec::with_capacity(values_arr.len());
    for row in values_arr {
        values.push(parse_floats(row)?);
    }
    Ok(GridData::new(x, y, values))
}

/// A helper used by the individual tools to build an error-bearing
/// [`ToolOutput`] that still returns `Ok`, so the agent sees the message
/// without the session crashing.
pub fn error_output(message: impl Into<String>, detail: Option<Value>) -> ToolOutput {
    let message = message.into();
    let mut meta = serde_json::json!({ "status": "error", "error": message.clone() });
    if let Some(d) = detail {
        meta["detail"] = d;
    }
    ToolOutput {
        content: format!("Failed to render plot: {message}"),
        metadata: Some(meta),
    }
}

/// A helper for the individual tools to build a successful [`ToolOutput`].
///
/// The rendered plot is mirrored into the metadata under the `plot` key so the
/// TUI can display it in the message window: the event bus only carries a
/// short preview of `content`, but `update_tool_call_output` stores the full
/// metadata on the tool-call state, which the message renderers read.
///
/// The plain-text canvas is stored under `plot` (used by non-TUI consumers and
/// as a fallback). The ANSI-coloured canvas is stored under `plot_ansi`; the
/// TUI prefers it and parses the SGR escapes back into styled spans so
/// multi-colour plots render with their real colours.
pub fn success_output(content: String, kind: &str, metadata: Value) -> ToolOutput {
    let mut meta = serde_json::json!({
        "status": "success",
        "plot_kind": kind,
        "plot": content.clone(),
    });
    if let Some(obj) = metadata.as_object() {
        for (k, v) in obj {
            meta[k] = v.clone();
        }
    }
    ToolOutput {
        content,
        metadata: Some(meta),
    }
}
