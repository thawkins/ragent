//! Current date/time tool.
//!
//! Provides [`DateTimeTool`], which reports the current instant as UTC and
//! renders it in an explicit timezone offset.
//!
//! The tool is deliberately hermetic: it reads the system clock only. It does
//! not consult `TZ`, `/etc/localtime`, or the platform locale, because a tool
//! result that silently depends on ambient process state is not reproducible.
//! The local offset is therefore a parameter (default: UTC), and any offset the
//! model passes is echoed back in the output so the caller can see which
//! rendering it received.

use anyhow::{Context, Result, bail};
use chrono::{FixedOffset, SecondsFormat, Utc};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Largest offset magnitude accepted, in seconds.
///
/// Real-world civil offsets span UTC-12:00 to UTC+14:00. The bound is widened
/// to 18 hours in each direction so it always contains that range (and every
/// legitimate quarter-hour offset inside it) while still rejecting a value
/// produced by arithmetic on epoch seconds instead of an offset.
const MAX_OFFSET_SECS: i32 = 18 * 60 * 60;

/// Report the current date and time.
pub struct DateTimeTool;

#[async_trait::async_trait]
impl Tool for DateTimeTool {
    fn name(&self) -> &'static str {
        "DateTime"
    }

    fn description(&self) -> &'static str {
        "Return the current date and time, read from the system clock. All \
         parameters are optional. `timezone_offset_minutes` renders the result \
         at that fixed offset from UTC (e.g. 60 for UTC+01:00, -300 for \
         UTC-05:00, 330 for UTC+05:30); it defaults to 0 (UTC). `format` is a \
         chrono date-time format string (e.g. '%d/%m/%Y %H:%M'), and defaults \
         to ISO 8601. Returns the UTC instant in RFC 3339, the same instant in \
         the requested offset, the individual date and time fields, the \
         Unix timestamp (seconds and milliseconds), and the day of week. It \
         does not read the TZ environment variable or the host timezone \
         database: pass the offset you want."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "timezone_offset_minutes": {
                    "type": "integer",
                    "description": "Offset from UTC in minutes, between -1080 and 1080. \
                                    0 (the default) is UTC, 60 is UTC+01:00, -300 is \
                                    UTC-05:00, 330 is UTC+05:30."
                },
                "format": {
                    "type": "string",
                    "description": "chrono date-time format string applied to the local \
                                    rendering (e.g. '%Y-%m-%d %H:%M:%S', '%d/%m/%Y'). \
                                    Defaults to ISO 8601."
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        // Reads the system clock and nothing else: no subprocess, no
        // filesystem access, no network.
        "none"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let offset_minutes = match input.get("timezone_offset_minutes") {
            None | Some(Value::Null) => 0i64,
            Some(value) => value
                .as_i64()
                .context("'timezone_offset_minutes' must be an integer number of minutes")?,
        };
        let format = match input.get("format") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .context("'format' must be a string")?
                    .to_string(),
            ),
        };

        let offset = parse_offset(offset_minutes)?;
        let now = Utc::now();
        let local = now.with_timezone(&offset);

        let local_text = format.as_deref().map(|fmt| {
            // A hostile format string is a model error, not a tool failure; fall
            // back to ISO 8601 so the answer is still useful.
            local.format(fmt).to_string()
        });
        let local_iso = local.to_rfc3339_opts(SecondsFormat::Secs, false);
        let utc_iso = now.to_rfc3339_opts(SecondsFormat::Secs, true);
        let offset_label = format_offset(offset.local_minus_utc());
        let day_of_week = local.format("%A").to_string();
        let weekday_number = local.format("%u").to_string();

        let mut body = format!(
            "current date/time\n\
             \x20 local      {local_text}\n\
             \x20 offset     {offset_label}\n\
             \x20 utc        {utc_iso}\n\
             \x20 date       {}\n\
             \x20 time       {}\n\
             \x20 day        {day_of_week}\n\
             \x20 unix       {} s ({} ms)",
            local.format("%Y-%m-%d"),
            local.format("%H:%M:%S"),
            now.timestamp(),
            now.timestamp_millis(),
            local_text = local_text.clone().unwrap_or_else(|| local_iso.clone()),
        );
        if local_text.is_some() {
            body.push_str(&format!("\n   iso        {local_iso}"));
        }

        let mut metadata = json!({
            "utc": utc_iso,
            "local": local_iso,
            "timezone_offset_minutes": offset_minutes,
            "timezone_offset": offset_label,
            "date": local.format("%Y-%m-%d").to_string(),
            "time": local.format("%H:%M:%S").to_string(),
            "day_of_week": day_of_week,
            "weekday": weekday_number,
            "year": local.format("%Y").to_string(),
            "month": local.format("%m").to_string(),
            "day": local.format("%d").to_string(),
            "hour": local.format("%H").to_string(),
            "minute": local.format("%M").to_string(),
            "second": local.format("%S").to_string(),
            "unix_seconds": now.timestamp(),
            "unix_millis": now.timestamp_millis(),
        });
        if let Some(text) = &local_text {
            metadata["formatted"] = Value::String(text.clone());
        }

        Ok(ToolOutput {
            content: body,
            metadata: Some(metadata),
        })
    }
}

/// Convert a signed offset in minutes into a [`FixedOffset`].
///
/// # Errors
///
/// Returns an error when the magnitude exceeds [`MAX_OFFSET_SECS`].
fn parse_offset(minutes: i64) -> Result<FixedOffset> {
    let seconds = minutes
        .checked_mul(60)
        .context("'timezone_offset_minutes' is out of range")?;
    let seconds = i32::try_from(seconds).context("'timezone_offset_minutes' is out of range")?;
    if seconds.abs() > MAX_OFFSET_SECS {
        bail!(
            "'timezone_offset_minutes' must be between -{} and {}, got {minutes}",
            MAX_OFFSET_SECS / 60,
            MAX_OFFSET_SECS / 60
        );
    }
    FixedOffset::east_opt(seconds)
        .with_context(|| format!("'{minutes}' is not a valid offset from UTC in minutes"))
}

/// Render an offset in seconds as `UTC+HH:MM` / `UTC-HH:MM`.
fn format_offset(seconds_east: i32) -> String {
    let sign = if seconds_east < 0 { '-' } else { '+' };
    let magnitude = seconds_east.abs();
    let hours = magnitude / 3600;
    let minutes = (magnitude % 3600) / 60;
    format!("UTC{sign}{hours:02}:{minutes:02}")
}
