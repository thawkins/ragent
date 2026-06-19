//! URL-safe identifier for a research item.
//!
//! A `ResearchName` is the validated, unique identifier used as the directory
//! name under `research/`. It enforces FR-002 from the research-system spec:
//!
//! - Only lowercase ASCII letters, digits, and hyphens
//! - Must start with a letter (not a digit or hyphen)
//! - Length between 3 and 64 characters (inclusive)
//! - Rejects path traversal sequences (`.`, `..`, `/`, `\`)
//!
//! `ResearchName::new` returns `Option<Self>` so callers can surface validation
//! failures without throwing. The richer `ResearchName::try_new` constructor
//! returns a `Result` that includes the specific violation reason, suitable
//! for surfacing in the TUI permission dialog or HTTP error responses.
//!
//! Example:
//!
//! ```
//! use ragent_research::ResearchName;
//! assert!(ResearchName::new("rust-async").is_some());
//! assert!(ResearchName::new("Ru").is_none());      // too short
//! assert!(ResearchName::new("1abc").is_none());    // starts with digit
//! assert!(ResearchName::new("a..b").is_none());     // contains '.'
//! assert!(ResearchName::new("../etc").is_none());   // path traversal
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Errors that can occur while validating a research name.
///
/// Returned by [`ResearchName::try_new`]. Each variant identifies a specific
/// FR-002 violation so the UI can report the exact rule that was broken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchNameError {
    /// The supplied name was empty.
    Empty,
    /// The name is shorter than [`MIN_LEN`] characters.
    TooShort {
        /// The actual length of the rejected input.
        length: usize,
    },
    /// The name is longer than [`MAX_LEN`] characters.
    TooLong {
        /// The actual length of the rejected input.
        length: usize,
    },
    /// The first character is not an ASCII lowercase letter.
    InvalidStart {
        /// The offending first character.
        ch: char,
    },
    /// The name contains a character outside `[a-z0-9-]`.
    InvalidCharacter {
        /// The first offending character encountered.
        ch: char,
    },
}

impl fmt::Display for ResearchNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(
                f,
                "research name must not be empty (minimum {MIN_LEN} characters)",
            ),
            Self::TooShort { length } => write!(
                f,
                "research name is too short ({length} chars; minimum {MIN_LEN})",
            ),
            Self::TooLong { length } => write!(
                f,
                "research name is too long ({length} chars; maximum {MAX_LEN})",
            ),
            Self::InvalidStart { ch } => write!(
                f,
                "research name must start with a lowercase ASCII letter (got '{ch}')",
            ),
            Self::InvalidCharacter { ch } => write!(
                f,
                "research name may only contain lowercase ASCII letters, digits, and hyphens (got '{ch}')",
            ),
        }
    }
}

impl std::error::Error for ResearchNameError {}

/// Minimum allowed length of a research name.
pub const MIN_LEN: usize = 3;
/// Maximum allowed length of a research name.
pub const MAX_LEN: usize = 64;

/// Validated, URL-safe identifier for a research item.
///
/// Constructed via [`ResearchName::new`] (returns `Option<Self>`) or
/// [`ResearchName::try_new`] (returns `Result<Self, ResearchNameError>`).
/// Once constructed, every `ResearchName` is guaranteed to satisfy FR-002.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ResearchName(String);

impl ResearchName {
    /// Construct a `ResearchName`, returning `None` on any validation failure.
    ///
    /// This is the cheap constructor used by hot paths where a single
    /// "is this valid?" check suffices. Use [`ResearchName::try_new`] when
    /// the caller needs to know *which* FR-002 rule was violated.
    pub fn new(name: impl Into<String>) -> Option<Self> {
        Self::try_new(name).ok()
    }

    /// Construct a `ResearchName`, returning the specific violation reason on
    /// failure. The returned `Err` is suitable for surfacing to end users.
    ///
    /// Validation rules (all must hold):
    ///
    /// 1. Length is between [`MIN_LEN`] and [`MAX_LEN`] inclusive.
    /// 2. The first character is an ASCII lowercase letter.
    /// 3. Every character is an ASCII lowercase letter, digit, or hyphen.
    pub fn try_new(name: impl Into<String>) -> Result<Self, ResearchNameError> {
        let name = name.into();

        if name.is_empty() {
            return Err(ResearchNameError::Empty);
        }
        if name.len() < MIN_LEN {
            return Err(ResearchNameError::TooShort { length: name.len() });
        }
        if name.len() > MAX_LEN {
            return Err(ResearchNameError::TooLong { length: name.len() });
        }

        let mut chars = name.chars();

        // Rule 2: first character must be a lowercase letter.
        let first = chars.next().expect("non-empty checked above");
        if !first.is_ascii_lowercase() {
            return Err(ResearchNameError::InvalidStart { ch: first });
        }

        // Rule 3: every subsequent character must be [a-z0-9-].
        for ch in chars {
            if !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
                return Err(ResearchNameError::InvalidCharacter { ch });
            }
        }

        Ok(Self(name))
    }

    /// Borrow the underlying string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the directory name for this research item.
    ///
    /// Equivalent to `self.as_str()` — research names are used as-is for the
    /// `research/<name>/` directory path because the FR-002 character set is
    /// already filesystem-safe on every supported platform.
    pub fn dir_name(&self) -> &str {
        &self.0
    }

    /// Return the length of the research name in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` if the research name is empty (never true for a constructed
    /// `ResearchName`, but provided for parity with `str::is_empty`).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ResearchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ResearchName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ResearchName> for String {
    fn from(value: ResearchName) -> Self {
        value.0
    }
}

impl TryFrom<String> for ResearchName {
    type Error = ResearchNameError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for ResearchName {
    type Error = ResearchNameError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_lowercase_name() {
        assert!(ResearchName::new("rust").is_some());
    }

    #[test]
    fn accepts_letters_digits_and_hyphens() {
        assert!(ResearchName::new("rust-2024").is_some());
        assert!(ResearchName::new("a-b-c").is_some());
        assert!(ResearchName::new("abc123").is_some());
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(ResearchName::try_new(""), Err(ResearchNameError::Empty));
    }

    #[test]
    fn rejects_too_short() {
        let err = ResearchName::try_new("ab").unwrap_err();
        assert_eq!(err, ResearchNameError::TooShort { length: 2 });
    }

    #[test]
    fn accepts_min_length() {
        assert!(ResearchName::new("abc").is_some());
    }

    #[test]
    fn rejects_too_long() {
        let too_long = "a".repeat(MAX_LEN + 1);
        let err = ResearchName::try_new(too_long.clone()).unwrap_err();
        assert_eq!(err, ResearchNameError::TooLong { length: MAX_LEN + 1 });
    }

    #[test]
    fn accepts_max_length() {
        let name = "a".repeat(MAX_LEN);
        assert!(ResearchName::new(name).is_some());
    }

    #[test]
    fn rejects_uppercase() {
        assert_eq!(
            ResearchName::try_new("Rust"),
            Err(ResearchNameError::InvalidStart { ch: 'R' }),
        );
        assert_eq!(
            ResearchName::try_new("rustAsync"),
            Err(ResearchNameError::InvalidCharacter { ch: 'A' }),
        );
    }

    #[test]
    fn rejects_starting_with_digit() {
        assert_eq!(
            ResearchName::try_new("1abc"),
            Err(ResearchNameError::InvalidStart { ch: '1' }),
        );
    }

    #[test]
    fn rejects_starting_with_hyphen() {
        assert_eq!(
            ResearchName::try_new("-abc"),
            Err(ResearchNameError::InvalidStart { ch: '-' }),
        );
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(ResearchName::new("../etc").is_none());
        assert!(ResearchName::new("foo/bar").is_none());
        assert!(ResearchName::new("..").is_none());
        assert!(ResearchName::new(".hidden").is_none());
    }

    #[test]
    fn rejects_whitespace_and_unicode() {
        assert!(ResearchName::new("foo bar").is_none());
        assert!(ResearchName::new("foo\nbar").is_none());
        assert!(ResearchName::new("café").is_none());
    }

    #[test]
    fn dir_name_matches_as_str() {
        let name = ResearchName::new("rust-async").unwrap();
        assert_eq!(name.dir_name(), "rust-async");
        assert_eq!(name.as_str(), "rust-async");
        assert_eq!(name.to_string(), "rust-async");
    }

    #[test]
    fn try_from_string_succeeds() {
        let name: ResearchName = "foo-bar".to_string().try_into().unwrap();
        assert_eq!(name.as_str(), "foo-bar");
    }

    #[test]
    fn try_from_string_fails() {
        let result: Result<ResearchName, _> = "AB".to_string().try_into();
        assert!(result.is_err());
    }

    #[test]
    fn equality_and_hash() {
        let a = ResearchName::new("rust-lang").unwrap();
        let b = ResearchName::new("rust-lang").unwrap();
        let c = ResearchName::new("go-lang").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }
}
