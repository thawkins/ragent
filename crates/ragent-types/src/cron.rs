//! Cron scheduling types for the agent cron system.
//!
//! This module defines the core data types used by the agent cron system
//! (spec `agentchron`). A [`CronEvent`] represents a scheduled agent run
//! with a designated agent type and initial prompt. Events can be one-shot
//! (fire once at a specified timestamp) or repeating (fire on a recurring
//! interval).
//!
//! The schedule is described by a [`CronSchedule`], which captures one of
//! three forms:
//!
//! - `OneShot` - fire once at a specific timestamp.
//! - `RepeatFrom` - repeating, with an explicit start timestamp.
//! - `RepeatNow` - repeating, with the start time assumed to be "now".
//!
//! See `specs/agentchron/SPEC.md` for the full specification.

use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Seconds per minute.
const SECS_PER_MIN: i64 = 60;
/// Seconds per hour.
const SECS_PER_HOUR: i64 = 3_600;
/// Seconds per day.
const SECS_PER_DAY: i64 = 86_400;
/// Seconds per week (7 days).
const SECS_PER_WEEK: i64 = 604_800;
/// Seconds per month, approximated as 30 days (per spec NFR).
const SECS_PER_MONTH: i64 = 2_592_000;

/// Returns a map of duration-unit alias -> seconds-per-unit.
///
/// Built once on first access and cached for the process lifetime.
fn unit_map() -> &'static HashMap<&'static str, i64> {
    static MAP: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        // Minutes
        m.insert("m", SECS_PER_MIN);
        m.insert("min", SECS_PER_MIN);
        m.insert("mins", SECS_PER_MIN);
        // Hours
        m.insert("h", SECS_PER_HOUR);
        m.insert("hr", SECS_PER_HOUR);
        m.insert("hrs", SECS_PER_HOUR);
        // Days
        m.insert("d", SECS_PER_DAY);
        m.insert("day", SECS_PER_DAY);
        m.insert("days", SECS_PER_DAY);
        // Weeks
        m.insert("w", SECS_PER_WEEK);
        m.insert("wk", SECS_PER_WEEK);
        m.insert("wks", SECS_PER_WEEK);
        // Months (30-day approximation)
        m.insert("mo", SECS_PER_MONTH);
        m.insert("month", SECS_PER_MONTH);
        m.insert("months", SECS_PER_MONTH);
        m
    })
}

/// Returns the supported unit tokens as a sorted string for error messages.
fn supported_units_str() -> String {
    // Canonical single-letter forms for display, in spec order.
    let canonical = ["m", "h", "d", "w", "mo"];
    format!(
        "minutes (m), hours (h), days (d), weeks (w), months (mo). \
             Supported aliases: {}",
        canonical.join(", ")
    )
}

/// Convert a duration in seconds to a compact human-readable string (FR-015).
///
/// Picks the largest unit that divides the interval evenly, falling back to
/// seconds if none of the canonical units divide evenly.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(duration_to_string(60), "1m");
/// assert_eq!(duration_to_string(1800), "30m");
/// assert_eq!(duration_to_string(3600), "1h");
/// assert_eq!(duration_to_string(86400), "1d");
/// assert_eq!(duration_to_string(604800), "1w");
/// assert_eq!(duration_to_string(2592000), "1mo");
/// ```
fn duration_to_string(secs: i64) -> String {
    // Order: largest unit first so we pick the most natural representation.
    let units: [(&str, i64); 5] = [
        ("mo", SECS_PER_MONTH),
        ("w", SECS_PER_WEEK),
        ("d", SECS_PER_DAY),
        ("h", SECS_PER_HOUR),
        ("m", SECS_PER_MIN),
    ];
    for (label, unit_secs) in &units {
        if secs % unit_secs == 0 {
            return format!("{}{}", secs / unit_secs, label);
        }
    }
    // Fallback: seconds (should not happen for spec-compliant durations).
    format!("{secs}s")
}

/// An error produced by [`parse_duration`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurationParseError {
    /// The input was empty or only whitespace.
    #[error("empty duration expression")]
    Empty,
    /// No unit was found after the numeric portion.
    #[error("missing unit in duration expression: {0:?}")]
    MissingUnit(String),
    /// The numeric portion could not be parsed as an integer.
    #[error("invalid number in duration expression: {0:?}")]
    InvalidNumber(String),
    /// The duration value was zero (FR-018).
    #[error("duration must be greater than zero, got 0")]
    Zero,
    /// The duration value was negative (FR-018).
    #[error("duration must be greater than zero, got {0}")]
    Negative(i64),
    /// The unit was not recognised (FR-019).
    #[error("unknown duration unit {0:?}; supported units: {1}")]
    UnknownUnit(String, String),
}

/// Parse a duration expression into seconds.
///
/// A duration is a positive integer followed by a unit, optionally separated
/// by whitespace. Supported units and their aliases (FR-014):
///
/// | Unit    | Aliases           | Seconds  |
/// |---------|-------------------|----------|
/// | `m`     | `min`, `mins`     | 60       |
/// | `h`     | `hr`, `hrs`       | 3,600    |
/// | `d`     | `day`, `days`     | 86,400   |
/// | `w`     | `wk`, `wks`       | 604,800  |
/// | `mo`    | `month`, `months` | 2,592,000 (30 days) |
///
/// # Errors
///
/// - [`DurationParseError::Empty`] - the input is empty or whitespace only.
/// - [`DurationParseError::MissingUnit`] - no unit token found after the number.
/// - [`DurationParseError::InvalidNumber`] - the numeric portion is not an integer.
/// - [`DurationParseError::Zero`] - the value is `0` (FR-018).
/// - [`DurationParseError::Negative`] - the value is negative (FR-018).
/// - [`DurationParseError::UnknownUnit`] - the unit is not in the alias table (FR-019).
///
/// # Examples
///
/// ```
/// use ragent_types::cron::parse_duration;
///
/// assert_eq!(parse_duration("30m").unwrap(), 1800);
/// assert_eq!(parse_duration("2h").unwrap(), 7200);
/// assert_eq!(parse_duration("1 d").unwrap(), 86400);
/// assert_eq!(parse_duration("1month").unwrap(), 2592000);
/// ```
pub fn parse_duration(s: &str) -> Result<i64, DurationParseError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(DurationParseError::Empty);
    }

    // Split into numeric prefix and unit suffix.
    // Walk forward to find the first non-digit, non-minus character.
    let bytes = trimmed.as_bytes();
    let mut split = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if b.is_ascii_digit() || (i == 0 && b == b'-') {
            split = i + 1;
        } else {
            break;
        }
    }

    if split == 0 {
        // No numeric prefix at all.
        return Err(DurationParseError::InvalidNumber(trimmed.to_string()));
    }

    let num_str = &trimmed[..split];
    let unit_str = trimmed[split..].trim();

    let num: i64 = num_str
        .parse()
        .map_err(|_| DurationParseError::InvalidNumber(num_str.to_string()))?;

    if unit_str.is_empty() {
        return Err(DurationParseError::MissingUnit(trimmed.to_string()));
    }

    let unit_lower = unit_str.to_lowercase();
    let secs = unit_map()
        .get(unit_lower.as_str())
        .copied()
        .ok_or_else(|| {
            DurationParseError::UnknownUnit(unit_str.to_string(), supported_units_str())
        })?;

    if num == 0 {
        return Err(DurationParseError::Zero);
    }
    if num < 0 {
        return Err(DurationParseError::Negative(num));
    }

    // SEC-ragent-types-003 (SECTASKS T-035): `num` is an arbitrarily large
    // integer prefix (`from 2020-01-01T00:00:00Z every 999999999999999mo`), and
    // the workspace release profile does not enable `overflow-checks`, so the
    // multiplication wrapped silently in release and panicked in debug. Surface
    // the overflow as a parse error instead.
    num.checked_mul(secs)
        .ok_or_else(|| DurationParseError::InvalidNumber(num_str.to_string()))
}

/// The scheduling form of a [`CronEvent`].
///
/// Maps directly to the three schedule grammar forms defined in the spec:
///
/// | Form                        | `CronForm`       |
/// |-----------------------------|------------------|
/// | `at <timestamp>`            | `OneShot`        |
/// | `from <timestamp> every <d>`| `RepeatFrom`     |
/// | `every <duration>`          | `RepeatNow`      |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronForm {
    /// One-shot: fires exactly once at the specified timestamp.
    OneShot,
    /// Repeating with an explicit start timestamp (`from <ts> every <d>`).
    RepeatFrom,
    /// Repeating with no explicit start - the start is assumed to be "now".
    RepeatNow,
}

/// A parsed cron schedule.
///
/// The schedule is one of three forms, distinguished by [`CronForm`].
/// For repeating schedules, `duration_secs` holds the interval between
/// firings. A "month" duration is approximated as 30 days (2,592,000 seconds)
/// per the spec's non-functional requirement.
///
/// # Fields
///
/// - `form` - which of the three schedule grammar forms this schedule uses.
/// - `start_at` - the explicit start timestamp. `None` for `RepeatNow`
///   (until the first fire sets it). Always `Some` for `OneShot` and
///   `RepeatFrom`.
/// - `duration_secs` - the repeat interval in seconds. `None` for one-shot
///   events. For months, this is 2,592,000 (30 x 24 x 3600).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronSchedule {
    /// Which schedule grammar form this schedule uses.
    pub form: CronForm,
    /// Explicit start timestamp, if any. `None` for `RepeatNow`.
    pub start_at: Option<DateTime<Utc>>,
    /// Repeat interval in seconds. `None` for one-shot events.
    /// Months are approximated as 30 days (2,592,000 s).
    pub duration_secs: Option<i64>,
}

impl CronSchedule {
    /// Create a one-shot schedule firing at the given timestamp.
    ///
    /// # Arguments
    ///
    /// - `at` - the timestamp at which the event should fire exactly once.
    #[must_use]
    pub fn one_shot(at: DateTime<Utc>) -> Self {
        Self {
            form: CronForm::OneShot,
            start_at: Some(at),
            duration_secs: None,
        }
    }

    /// Create a repeating schedule with an explicit start timestamp.
    ///
    /// # Arguments
    ///
    /// - `start_at` - the timestamp of the first execution.
    /// - `duration_secs` - the repeat interval in seconds.
    #[must_use]
    pub fn repeat_from(start_at: DateTime<Utc>, duration_secs: i64) -> Self {
        Self {
            form: CronForm::RepeatFrom,
            start_at: Some(start_at),
            duration_secs: Some(duration_secs),
        }
    }

    /// Create a repeating schedule with no explicit start - the start is
    /// assumed to be "now", so the first fire is `duration_secs` from now.
    ///
    /// # Arguments
    ///
    /// - `duration_secs` - the repeat interval in seconds.
    #[must_use]
    pub fn repeat_now(duration_secs: i64) -> Self {
        Self {
            form: CronForm::RepeatNow,
            start_at: None,
            duration_secs: Some(duration_secs),
        }
    }

    /// Returns `true` if this schedule is repeating (fires more than once).
    #[must_use]
    pub fn is_repeating(&self) -> bool {
        matches!(self.form, CronForm::RepeatFrom | CronForm::RepeatNow)
    }

    /// Returns `true` if this schedule is a one-shot (fires exactly once).
    #[must_use]
    pub fn is_one_shot(&self) -> bool {
        self.form == CronForm::OneShot
    }

    /// Return a human-readable description of this schedule (FR-015).
    ///
    /// Produces a compact, user-facing string summarising the schedule form:
    ///
    /// | Form          | Example output                            |
    /// |---------------|-------------------------------------------|
    /// | `OneShot`     | `at 2025-01-15T09:00:00Z`                 |
    /// | `RepeatFrom`  | `every 30m from 2025-01-15T09:00:00Z`     |
    /// | `RepeatNow`   | `every 30m`                               |
    ///
    /// The duration is rendered using the largest unit that divides the
    /// interval evenly (minutes -> hours -> days -> weeks -> months).
    ///
    /// # Panics
    ///
    /// Panics if `start_at` is `None` for `OneShot` or `RepeatFrom`, or if
    /// `duration_secs` is `None` for a repeating form - these are structural
    /// invariant violations.
    #[must_use]
    pub fn human_readable(&self) -> String {
        // SEC-ragent-types-003 (SECTASKS T-035): `CronSchedule` derives
        // `Deserialize` and is rebuilt from persisted rows and from archive
        // import, so a `form`/`start_at`/`duration_secs` combination that
        // violates the structural invariant is reachable input. Render it
        // instead of panicking the caller that is displaying it.
        match self.form {
            CronForm::OneShot => match self.start_at {
                Some(ts) => format!("at {}", ts.to_rfc3339()),
                None => "at <unspecified time> (invalid schedule)".to_string(),
            },
            CronForm::RepeatFrom => {
                let start = self
                    .start_at
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_else(|| "<unspecified time>".to_string());
                let dur = match self.positive_duration_secs() {
                    Some(d) => duration_to_string(d),
                    None => "<unspecified interval>".to_string(),
                };
                format!("every {dur} from {start}")
            }
            CronForm::RepeatNow => match self.positive_duration_secs() {
                Some(d) => format!("every {}", duration_to_string(d)),
                None => "every <unspecified interval> (invalid schedule)".to_string(),
            },
        }
    }

    /// The schedule's duration, but only when it is a positive value.
    ///
    /// SEC-ragent-types-003 (SECTASKS T-035): `duration_secs` can be absent
    /// (invariant violation) or non-positive (which trips the `assert!` in
    /// [`Self::advance_next_due`]). Both mean "this schedule cannot advance",
    /// which callers now handle instead of panicking.
    #[must_use]
    pub fn positive_duration_secs(&self) -> Option<i64> {
        self.duration_secs.filter(|d| *d > 0)
    }

    /// Compute the next-due timestamp after a fire (FR-004, FR-005).
    ///
    /// For **one-shot** events, returns `None` - the event fires once and is
    /// done (FR-005).
    ///
    /// For **repeating** events, advances `next_due` by one duration interval
    /// from the current `next_due`. If the computed time is still in the past
    /// relative to `now`, advances by whole intervals until it is in the future
    /// (FR-004). This handles the case where the scheduler was offline for
    /// multiple intervals.
    ///
    /// # Arguments
    ///
    /// - `current_next_due` - the event's current `next_due` timestamp (the
    ///   one that just fired or was skipped).
    /// - `now` - the reference "now" timestamp.
    ///
    /// # Returns
    ///
    /// - `Some(DateTime)` for repeating events - the next fire time.
    /// - `None` for one-shot events - no further fires.
    ///
    /// # Panics
    ///
    /// Panics if called on a repeating schedule with `duration_secs == None`,
    /// which is a structural invariant violation (should never happen for
    /// well-constructed schedules).
    #[must_use]
    pub fn advance_next_due(
        &self,
        current_next_due: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        match self.form {
            CronForm::OneShot => None,
            CronForm::RepeatFrom | CronForm::RepeatNow => {
                // SEC-ragent-types-003 (SECTASKS T-035): no `expect`/`assert!`.
                // A repeating schedule whose `duration_secs` is absent or
                // non-positive cannot advance - return `None` so the caller's
                // existing "disable the event" branch handles it (the TUI
                // already has that branch), rather than panicking the tick.
                let duration = self.positive_duration_secs()?;
                let mut next = current_next_due + chrono::Duration::seconds(duration);
                // If we're behind (e.g. scheduler was down), skip ahead to the
                // next future interval so we don't fire a burst of catch-up runs.
                while next <= now {
                    next += chrono::Duration::seconds(duration);
                }
                Some(next)
            }
        }
    }

    /// Compute the initial `next_due` for this schedule (FR-008, FR-009).
    ///
    /// - **OneShot**: `next_due` = `start_at` (the scheduled timestamp).
    /// - **RepeatFrom**: `next_due` = `start_at`, advanced to the next future
    ///   multiple if `start_at` is in the past.
    /// - **RepeatNow**: `next_due` = `now` + `duration_secs`.
    ///
    /// # Arguments
    ///
    /// - `now` - the reference "now" timestamp.
    ///
    /// # Panics
    ///
    /// Panics if `start_at` is `None` for `OneShot` or `RepeatFrom`, or if
    /// `duration_secs` is `None` for `RepeatNow` - these are structural
    /// invariant violations.
    #[must_use]
    pub fn initial_next_due(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        // SEC-ragent-types-003 (SECTASKS T-035): no `expect`. An invalid
        // schedule has no meaningful next due time, so fall back to `now`
        // (the event is immediately due and the caller disables it when it
        // cannot advance).
        match self.form {
            CronForm::OneShot => self.start_at.unwrap_or(now),
            CronForm::RepeatFrom => {
                let Some(start) = self.start_at else {
                    return now;
                };
                let Some(duration) = self.positive_duration_secs() else {
                    return now;
                };
                advance_to_future(start, duration, now)
            }
            CronForm::RepeatNow => {
                let Some(duration) = self.positive_duration_secs() else {
                    return now;
                };
                now + chrono::Duration::seconds(duration)
            }
        }
    }
}

/// A scheduled agent run.
///
/// Combines an agent type, an initial prompt, and a [`CronSchedule`]. The
/// scheduler evaluates the `next_due` timestamp and fires the event when it
/// has passed. Events persist in SQLite via the `Storage` layer (FR-001) and
/// carry the full state described in FR-002.
///
/// # Fields
///
/// - `id` - unique event identifier (e.g. `cron-<timestamp>-<rand>`).
/// - `agent_type` - built-in or custom agent name to run.
/// - `prompt` - the initial prompt passed to the agent.
/// - `schedule` - the parsed schedule (form, start, duration).
/// - `schedule_raw` - the original schedule expression string (e.g.
///   `every 30m`), preserved for display and `/cron list`.
/// - `enabled` - whether the scheduler should fire this event. `false`
///   means the scheduler skips it and logs `"skipped"` (FR-007).
/// - `next_due` - the computed timestamp of the next execution.
/// - `created_at` - when the event was created.
/// - `last_fired` - when the event last fired, or `None` if never.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronEvent {
    /// Unique event identifier.
    pub id: String,
    /// Built-in or custom agent name to run when this event fires.
    pub agent_type: String,
    /// The initial prompt passed to the spawned agent.
    pub prompt: String,
    /// The parsed schedule.
    pub schedule: CronSchedule,
    /// The original schedule expression string, for display.
    pub schedule_raw: String,
    /// Whether the scheduler should fire this event.
    pub enabled: bool,
    /// The computed timestamp of the next execution.
    pub next_due: DateTime<Utc>,
    /// When the event was created.
    pub created_at: DateTime<Utc>,
    /// When the event last fired, or `None` if it has never fired.
    pub last_fired: Option<DateTime<Utc>>,
    /// Whether this event runs in stateful loop mode (FR-004).
    ///
    /// When `true`, the scheduler maintains a cross-run state file and
    /// parses `<loop-state>` / `<inbox>` output protocol tags from the
    /// sub-agent's response.
    #[serde(default)]
    pub stateful: bool,
}

impl CronEvent {
    /// Create a new cron event with the given fields, computing `created_at`
    /// as the current time and `last_fired` as `None`.
    ///
    /// # Arguments
    ///
    /// - `id` - unique event identifier.
    /// - `agent_type` - agent name to run.
    /// - `prompt` - initial prompt.
    /// - `schedule` - parsed schedule.
    /// - `schedule_raw` - raw schedule expression string.
    /// - `next_due` - computed next-due timestamp.
    #[must_use]
    pub fn new(
        id: String,
        agent_type: String,
        prompt: String,
        schedule: CronSchedule,
        schedule_raw: String,
        next_due: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            agent_type,
            prompt,
            schedule,
            schedule_raw,
            enabled: true,
            next_due,
            created_at: Utc::now(),
            last_fired: None,
            stateful: false,
        }
    }
}

/// An error produced by [`parse_schedule`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleParseError {
    /// The input was empty or only whitespace.
    #[error("empty schedule expression")]
    Empty,
    /// The schedule keyword was not recognised.
    #[error("unknown schedule keyword {0:?}; expected 'at', 'from', or 'every'")]
    UnknownKeyword(String),
    /// A timestamp failed ISO-8601 parsing.
    #[error("invalid timestamp {0:?}: {1}")]
    InvalidTimestamp(String, String),
    /// The `from <ts> every <d>` form was missing the `every` keyword.
    #[error("missing 'every' keyword in 'from' schedule expression: {0:?}")]
    MissingEvery(String),
    /// The `from <ts> every <d>` form was missing the duration.
    #[error("missing duration after 'every' in schedule expression: {0:?}")]
    MissingDuration(String),
    /// The duration component was invalid.
    #[error(transparent)]
    Duration(#[from] DurationParseError),
}

/// The result of parsing a schedule expression: the parsed [`CronSchedule`]
/// plus the computed initial `next_due` timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSchedule {
    /// The parsed schedule.
    pub schedule: CronSchedule,
    /// The computed next-due timestamp.
    pub next_due: DateTime<Utc>,
}

/// Parse a cron schedule expression into a [`ParsedSchedule`].
///
/// Supports the three schedule grammar forms defined in the spec:
///
/// | Form                            | `CronForm`  | `next_due` computation                     |
/// |---------------------------------|-------------|--------------------------------------------|
/// | `at <ISO-8601>`                 | `OneShot`   | = the given timestamp                      |
/// | `from <ISO-8601> every <dur>`   | `RepeatFrom`| = start (or advanced to next future multiple if past) |
/// | `every <duration>`             | `RepeatNow` | = now + duration                           |
///
/// # Arguments
///
/// - `expr` - the raw schedule expression string.
/// - `now` - the reference "now" timestamp for `RepeatNow` computation and
///   past-start advancement.
///
/// # Errors
///
/// Returns [`ScheduleParseError`] for unrecognised keywords, invalid
/// timestamps, missing components, or invalid durations.
///
/// # Examples
///
/// ```
/// use ragent_types::cron::{parse_schedule, CronForm};
/// use chrono::Utc;
///
/// let now = Utc::now();
///
/// // One-shot
/// let parsed = parse_schedule("at 2025-01-15T09:00:00Z", now).unwrap();
/// assert_eq!(parsed.schedule.form, CronForm::OneShot);
///
/// // Repeat from now
/// let parsed = parse_schedule("every 30m", now).unwrap();
/// assert_eq!(parsed.schedule.form, CronForm::RepeatNow);
///
/// // Repeat from explicit start
/// let parsed = parse_schedule("from 2025-01-15T09:00:00Z every 1h", now).unwrap();
/// assert_eq!(parsed.schedule.form, CronForm::RepeatFrom);
/// ```
pub fn parse_schedule(
    expr: &str,
    now: DateTime<Utc>,
) -> Result<ParsedSchedule, ScheduleParseError> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(ScheduleParseError::Empty);
    }

    // Find the first word (keyword).
    let keyword_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let keyword = trimmed[..keyword_end].to_lowercase();
    let rest = trimmed[keyword_end..].trim();

    match keyword.as_str() {
        "at" => parse_at(rest),
        "from" => parse_from(rest, now),
        "every" => parse_every(rest, now),
        _ => Err(ScheduleParseError::UnknownKeyword(keyword)),
    }
}

/// Parse the `at <ISO-8601>` form.
fn parse_at(rest: &str) -> Result<ParsedSchedule, ScheduleParseError> {
    let ts = parse_timestamp(rest)?;
    Ok(ParsedSchedule {
        schedule: CronSchedule::one_shot(ts),
        next_due: ts,
    })
}

/// Parse the `from <ISO-8601> every <duration>` form.
fn parse_from(rest: &str, now: DateTime<Utc>) -> Result<ParsedSchedule, ScheduleParseError> {
    // Split on "every" keyword.
    let lower = rest.to_lowercase();
    let every_pos = lower
        .find("every")
        .ok_or_else(|| ScheduleParseError::MissingEvery(rest.to_string()))?;

    let ts_str = rest[..every_pos].trim();
    // Skip past the "every" keyword (5 chars) and trim.
    let dur_str = rest[every_pos + 5..].trim();

    if dur_str.is_empty() {
        return Err(ScheduleParseError::MissingDuration(rest.to_string()));
    }

    let start = parse_timestamp(ts_str)?;
    let duration_secs = parse_duration(dur_str)?;

    // Advance next_due to the next future multiple if start is in the past.
    let next_due = advance_to_future(start, duration_secs, now);

    Ok(ParsedSchedule {
        schedule: CronSchedule::repeat_from(start, duration_secs),
        next_due,
    })
}

/// Parse the `every <duration>` form (start = now).
fn parse_every(rest: &str, now: DateTime<Utc>) -> Result<ParsedSchedule, ScheduleParseError> {
    let duration_secs = parse_duration(rest)?;
    let next_due = now + chrono::Duration::seconds(duration_secs);

    Ok(ParsedSchedule {
        schedule: CronSchedule::repeat_now(duration_secs),
        next_due,
    })
}

/// Parse a timestamp, accepting ISO-8601 or natural-language shortcuts.
///
/// In addition to full RFC-3339 / ISO-8601 timestamps, this function
/// recognises human-friendly time shortcuts resolved against the user's
/// local timezone:
///
/// - `5pm` - next 5pm (today if not yet passed, else tomorrow)
/// - `5:30pm` / `5:30 pm` - next 5:30pm
/// - `17:00` - next 17:00 (24-hour clock)
/// - `5am tomorrow` - 5am on the following day
///
/// The result is returned as a UTC `DateTime`.
fn parse_timestamp(s: &str) -> Result<DateTime<Utc>, ScheduleParseError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ScheduleParseError::InvalidTimestamp(
            s.to_string(),
            "empty timestamp".to_string(),
        ));
    }

    // Try parsing as a full DateTime with timezone.
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Fallback: try without timezone (assume UTC).
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Ok(ndt.and_utc());
    }

    // Fallback: try natural-language time shortcut.
    if let Ok(dt) = parse_natural_time(trimmed) {
        return Ok(dt);
    }

    Err(ScheduleParseError::InvalidTimestamp(
        trimmed.to_string(),
        "not a valid ISO-8601 or natural-language timestamp".to_string(),
    ))
}

/// Parse a natural-language time expression and resolve it to the next
/// occurrence in the user's local timezone, returning a UTC `DateTime`.
///
/// Accepted forms (case-insensitive):
///
/// - `5pm`, `5:30pm`, `5:30 pm` - 12-hour clock with am/pm
/// - `17:00`, `17:00:00` - 24-hour clock
/// - `5am`, `5pm tomorrow` - optional `today` / `tomorrow` suffix
///
/// When no day is specified, the next upcoming occurrence is used: if the
/// time today has not yet passed, today is used; otherwise tomorrow.
fn parse_natural_time(s: &str) -> Result<DateTime<Utc>, ()> {
    use chrono::{Local, NaiveTime, TimeZone};

    let lower = s.trim().to_lowercase();

    // Check for a "today" or "tomorrow" suffix.
    let (time_part, day_offset) = if let Some(t) = lower.strip_suffix(" tomorrow") {
        (t.trim(), 1)
    } else if let Some(t) = lower.strip_suffix(" today") {
        (t.trim(), 0)
    } else {
        (lower.as_str(), -1) // -1 = auto (today or tomorrow)
    };

    if time_part.is_empty() {
        return Err(());
    }

    // Detect optional am/pm suffix.
    let (time_core, is_pm) = if let Some(t) = time_part.strip_suffix("pm") {
        (t.trim(), Some(true))
    } else if let Some(t) = time_part.strip_suffix("am") {
        (t.trim(), Some(false))
    } else {
        (time_part, None)
    };

    // Parse the numeric time portion: "H", "H:M", or "H:M:S".
    let parts: Vec<&str> = time_core.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return Err(());
    }

    let hour: u32 = parts[0].parse().map_err(|_| ())?;
    let minute: u32 = if parts.len() > 1 {
        parts[1].parse().map_err(|_| ())?
    } else {
        0
    };
    let second: u32 = if parts.len() > 2 {
        parts[2].parse().map_err(|_| ())?
    } else {
        0
    };

    // Convert 12-hour to 24-hour.
    let hour24 = match is_pm {
        Some(true) => {
            if hour == 12 {
                12 // 12pm = noon
            } else if (1..=11).contains(&hour) {
                hour + 12
            } else {
                return Err(()); // e.g. "13pm" is invalid
            }
        }
        Some(false) => {
            if hour == 12 {
                0 // 12am = midnight
            } else if (1..=11).contains(&hour) {
                hour
            } else {
                return Err(()); // e.g. "13am" is invalid
            }
        }
        None => {
            // 24-hour clock: 0-23
            if hour > 23 {
                return Err(());
            }
            hour
        }
    };

    if minute > 59 || second > 59 {
        return Err(());
    }

    let local_now = Local::now();
    let today = local_now.date_naive();

    // Determine the target date.
    let target_date = match day_offset {
        0 => today,                             // explicit "today"
        1 => today + chrono::Duration::days(1), // explicit "tomorrow"
        _ => {
            // Auto: use today if the time hasn't passed yet, else tomorrow.
            let now_time = local_now.time();
            let target_time = NaiveTime::from_hms_opt(hour24, minute, second).ok_or(())?;
            if target_time <= now_time {
                today + chrono::Duration::days(1)
            } else {
                today
            }
        }
    };

    let target_naive =
        target_date.and_time(NaiveTime::from_hms_opt(hour24, minute, second).ok_or(())?);

    Ok(Local
        .from_local_datetime(&target_naive)
        .single()
        .ok_or(())?
        .with_timezone(&Utc))
}

/// Upper bound on how far [`advance_to_future`] will skip ahead in one step.
///
/// SEC-ragent-types-003 (SECTASKS T-035): roughly 100 years of seconds. Without
/// a bound, a corrupt `duration_secs` could produce a `chrono::Duration` that
/// panics when added to a date near the representable range.
const MAX_ADVANCE_SECONDS: i64 = 3_153_600_000;

/// Advance a past start timestamp to the next future multiple of duration.
///
/// If `start >= now`, returns `start` unchanged. Otherwise, adds duration
/// intervals until the result is strictly in the future.
fn advance_to_future(
    start: DateTime<Utc>,
    duration_secs: i64,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    if start >= now {
        return start;
    }

    if duration_secs <= 0 {
        // SEC-ragent-types-003 (SECTASKS T-035): a non-positive duration cannot
        // advance anything; surface the unmodified start rather than dividing
        // by zero / multiplying into a wrap.
        return start;
    }
    let diff = (now - start).num_seconds();
    let intervals = diff / duration_secs;
    // SEC-ragent-types-003 (SECTASKS T-035): `(intervals + 1) * duration_secs`
    // wrapped silently in release. Saturate instead of overflowing.
    let advance_secs = intervals
        .saturating_add(1)
        .checked_mul(duration_secs)
        .unwrap_or(i64::MAX)
        .min(MAX_ADVANCE_SECONDS);
    start + chrono::Duration::seconds(advance_secs)
}
