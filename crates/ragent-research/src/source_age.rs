//! Publication-age cut-off for web sources (`/research create --age <span>`).
//!
//! A run may reject web sources whose publication date is older than a
//! caller-supplied span, so a report can be restricted to recent material.
//! The span is written with a single unit suffix:
//!
//! - `[n]d` - days
//! - `[n]m` - months (30 days each)
//! - `[n]y` - years (365 days each)
//!
//! Months and years are converted to a fixed number of days rather than a
//! calendar duration so the same span always covers the same window; a
//! research report compares dates, not billing periods.
//!
//! When a cut-off is active, a web source with **no** publication date is
//! rejected as well: the caller asked for recent sources only, and an
//! undated page cannot be shown to satisfy that.

use std::fmt;

use chrono::{DateTime, Utc};

/// Days covered by one month in an [`AgeSpan`] (`--age <n>m`).
pub const DAYS_PER_MONTH: u64 = 30;

/// Days covered by one year in an [`AgeSpan`] (`--age <n>y`).
pub const DAYS_PER_YEAR: u64 = 365;

/// The unit of an [`AgeSpan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeUnit {
    /// `d` - days.
    Days,
    /// `m` - months (30 days).
    Months,
    /// `y` - years (365 days).
    Years,
}

impl AgeUnit {
    /// Days covered by one unit of this kind.
    #[must_use]
    pub const fn days_per_unit(self) -> u64 {
        match self {
            Self::Days => 1,
            Self::Months => DAYS_PER_MONTH,
            Self::Years => DAYS_PER_YEAR,
        }
    }

    /// The single-letter suffix that spells this unit on the command line.
    #[must_use]
    pub const fn suffix(self) -> char {
        match self {
            Self::Days => 'd',
            Self::Months => 'm',
            Self::Years => 'y',
        }
    }
}

impl fmt::Display for AgeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Days => "day(s)",
            Self::Months => "month(s)",
            Self::Years => "year(s)",
        })
    }
}

/// A publication-age cut-off for web sources, parsed from `--age <span>`.
///
/// Build one with [`AgeSpan::parse`] and apply it with
/// [`AgeSpan::admits`] or [`AgeSpan::rejection_reason`].
///
/// The serde representation is the command-line spelling (`"6m"`), so a JSON
/// request body and the recorded invocation frontmatter carry the same form the
/// caller would type, and deserialising an invalid span fails instead of
/// silently disabling the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct AgeSpan {
    /// Magnitude as written by the caller (never zero for a parsed span).
    amount: u32,
    /// Unit of [`Self::amount`].
    unit: AgeUnit,
}

impl AgeSpan {
    /// Parse a `--age` span such as `7d`, `6m`, or `2y`.
    ///
    /// The value is one or more ASCII digits followed by a single unit
    /// suffix (`d`, `m`, or `y`, case-insensitive). The magnitude must be
    /// greater than zero.
    ///
    /// # Errors
    ///
    /// Returns [`AgeSpanError`] when the span is empty, lacks a unit suffix,
    /// carries an unknown suffix, has no digits, or is zero.
    pub fn parse(value: &str) -> Result<Self, AgeSpanError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AgeSpanError::Empty);
        }
        let (digits, suffix) = trimmed.split_at(trimmed.len() - 1);
        let unit = match suffix.chars().next() {
            Some('d' | 'D') => AgeUnit::Days,
            Some('m' | 'M') => AgeUnit::Months,
            Some('y' | 'Y') => AgeUnit::Years,
            Some(other) => {
                return Err(AgeSpanError::UnknownUnit {
                    unit: other,
                    value: trimmed.to_string(),
                });
            }
            // `split_at` cannot land past the end of a non-empty string, and
            // `trimmed` is non-empty, so the suffix always has one char.
            None => return Err(AgeSpanError::Empty),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AgeSpanError::MissingMagnitude {
                value: trimmed.to_string(),
            });
        }
        let amount: u32 = digits
            .parse()
            .map_err(|_| AgeSpanError::MagnitudeTooLarge {
                value: trimmed.to_string(),
            })?;
        if amount == 0 {
            return Err(AgeSpanError::ZeroSpan {
                value: trimmed.to_string(),
            });
        }
        Ok(Self { amount, unit })
    }

    /// Magnitude as written by the caller.
    #[must_use]
    pub const fn amount(self) -> u32 {
        self.amount
    }

    /// Unit of the magnitude.
    #[must_use]
    pub const fn unit(self) -> AgeUnit {
        self.unit
    }

    /// Total length of the span in whole days.
    #[must_use]
    pub fn days(self) -> u64 {
        u64::from(self.amount).saturating_mul(self.unit.days_per_unit())
    }

    /// The oldest publication date a source may carry and still be admitted,
    /// relative to `now`.
    ///
    /// A span far beyond the representable calendar (a caller may write
    /// `999999999y`) saturates at [`DateTime::<Utc>::MIN_UTC`] instead of
    /// panicking. Both `Duration::days` and `DateTime::sub` panic on an
    /// out-of-range result, and the span comes straight from user input.
    #[must_use]
    pub fn oldest_allowed(self, now: DateTime<Utc>) -> DateTime<Utc> {
        let days = i64::try_from(self.days()).unwrap_or(i64::MAX);
        chrono::Duration::try_days(days)
            .and_then(|span| now.checked_sub_signed(span))
            .unwrap_or(DateTime::<Utc>::MIN_UTC)
    }

    /// The oldest publication date a source may carry and still be admitted,
    /// as a calendar date (`YYYY-MM-DD`) relative to `now`.
    ///
    /// This is the same deadline as [`AgeSpan::oldest_allowed`] with the time
    /// of day removed. It is the form appended to every decomposed web-search
    /// query so the search engine can steer towards material inside the
    /// `--age` window, rather than only the post-fetch filter rejecting what
    /// falls outside it.
    #[must_use]
    pub fn oldest_allowed_date(self, now: DateTime<Utc>) -> String {
        self.oldest_allowed(now).format("%Y-%m-%d").to_string()
    }

    /// Whether a web source published at `published_at` is admitted by this
    /// cut-off.
    ///
    /// An undated source (`published_at` is `None`) is rejected: when a span
    /// is active only sources that can be shown to be recent are kept.
    #[must_use]
    pub fn admits(self, published_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
        self.rejection_reason(published_at, now).is_none()
    }

    /// The reason a web source published at `published_at` is rejected, or
    /// `None` when it is admitted.
    ///
    /// The text is human-readable and is surfaced in the gatherer's exclusion
    /// diagnostics (log line, gather log, and progress event).
    #[must_use]
    pub fn rejection_reason(
        self,
        published_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Option<String> {
        match published_at {
            None => Some(format!(
                "no publication date; --age {self} keeps only dated sources"
            )),
            Some(dt) if dt < self.oldest_allowed(now) => Some(format!(
                "published {} is older than --age {self}",
                dt.format("%Y-%m-%d")
            )),
            Some(_) => None,
        }
    }
}

impl fmt::Display for AgeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.unit.suffix())
    }
}

impl TryFrom<String> for AgeSpan {
    type Error = AgeSpanError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

/// Why a `--age` span could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgeSpanError {
    /// The value was empty or whitespace only.
    Empty,
    /// The value carried a unit suffix that is not `d`, `m`, or `y`.
    UnknownUnit {
        /// The offending suffix character.
        unit: char,
        /// The whole value as written.
        value: String,
    },
    /// The value had no digits before the unit suffix.
    MissingMagnitude {
        /// The whole value as written.
        value: String,
    },
    /// The magnitude did not fit in a `u32`.
    MagnitudeTooLarge {
        /// The whole value as written.
        value: String,
    },
    /// The magnitude was zero, which would reject every dated source.
    ZeroSpan {
        /// The whole value as written.
        value: String,
    },
}

impl fmt::Display for AgeSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("expected a span such as `30d`, `6m`, or `2y`"),
            Self::UnknownUnit { unit, .. } => write!(
                f,
                "unknown unit `{unit}`: expected `d` (days), `m` (months), or `y` (years)"
            ),
            Self::MissingMagnitude { value } => write!(
                f,
                "`{value}` has no number before its unit: expected a span such as `30d`"
            ),
            Self::MagnitudeTooLarge { value } => {
                write!(f, "`{value}` is too large: expected at most 4294967295")
            }
            Self::ZeroSpan { value } => {
                write!(f, "`{value}` is zero: a span must cover at least one day")
            }
        }
    }
}

impl std::error::Error for AgeSpanError {}

#[cfg(test)]
#[path = "../tests/inline/source_age_tests.rs"]
mod tests;
