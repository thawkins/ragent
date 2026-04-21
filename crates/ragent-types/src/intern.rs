//! String interning utilities for memory optimization.
//!
//! This module provides centralized string interning for commonly repeated values
//! like tool names, session IDs, and error messages. By interning these strings,
//! we reduce memory allocations and allow O(1) equality comparisons.
//!
//! The interner uses a thread-safe global instance backed by `StringInterner`.

use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use string_interner::{DefaultBackend, StringInterner};

/// The type of symbols returned by the interner.
pub type Symbol = string_interner::DefaultSymbol;

/// Global thread-safe string interner for tool names and other common strings.
///
/// Uses a `Mutex<StringInterner>` to allow thread-safe access. The interner
/// is lazily initialized on first use.
///
/// Note: Strings are never deallocated from the interner — this is a tradeoff
/// for simple thread-safety and performance. For long-running processes with
/// many unique strings, consider resetting or using a bounded interner.
static INTERNER: Lazy<Mutex<StringInterner<DefaultBackend>>> =
    Lazy::new(|| Mutex::new(StringInterner::new()));

/// Intern a string and return its symbol handle.
///
/// If the string is already interned, returns the existing symbol.
/// If not, adds it to the interner and returns the new symbol.
///
/// # Examples
///
/// ```
/// use ragent_types::intern::intern;
///
/// let sym1 = intern("read");
/// let sym2 = intern("read");
/// // Both symbols point to the same interned string
/// assert_eq!(sym1, sym2);
/// ```
#[must_use]
pub fn intern(name: &str) -> Symbol {
    INTERNER
        .lock()
        .expect("interner poisoned")
        .get_or_intern(name)
}

/// Resolve a symbol back to its string value.
///
/// Returns `None` if the symbol is no longer in the interner (should never
/// happen for valid symbols obtained from `intern`).
///
/// # Examples
///
/// ```
/// use ragent_types::intern::{intern, resolve};
///
/// let sym = intern("read");
/// assert_eq!(resolve(sym), Some("read".to_string()));
/// ```
#[must_use]
pub fn resolve(symbol: Symbol) -> Option<String> {
    INTERNER
        .lock()
        .expect("interner poisoned")
        .resolve(symbol)
        .map(ToString::to_string)
}

/// Get the number of unique strings currently interned.
///
/// Useful for monitoring memory usage and debugging.
#[must_use]
pub fn len() -> usize {
    INTERNER.lock().expect("interner poisoned").len()
}

/// Check if the interner is empty.
#[must_use]
pub fn is_empty() -> bool {
    INTERNER.lock().expect("interner poisoned").is_empty()
}

/// An interned string that owns its symbol handle.
///
/// This type provides a convenient way to store interned strings while
/// keeping the original string accessible. The `Arc<String>` allows
/// cheap cloning and sharing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InternedString {
    /// The symbol handle in the interner.
    pub symbol: Symbol,
    /// The resolved string value (cached for quick access).
    value: Arc<String>,
}

impl InternedString {
    /// Create a new interned string from a value.
    ///
    /// The value is interned immediately. The resolved string is cached
    /// in the struct for O(1) access without lock contention.
    pub fn new(name: &str) -> Self {
        let symbol = intern(name);
        // Cache the resolved value to avoid locking the interner on every access
        let value = Arc::new(resolve(symbol).unwrap_or_else(|| name.to_string()));
        Self { symbol, value }
    }

    /// Get the interned string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }
}

impl std::fmt::Display for InternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl AsRef<str> for InternedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl serde::Serialize for InternedString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for InternedString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::new(&value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_dedup() {
        let s1 = intern("read");
        let s2 = intern("read");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_intern_different() {
        let s1 = intern("read");
        let s2 = intern("write");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_resolve() {
        let sym = intern("test_value");
        assert_eq!(resolve(sym), Some("test_value".to_string()));
    }

    #[test]
    fn test_interned_string() {
        let s1 = InternedString::new("read_file");
        let s2 = InternedString::new("read_file");

        // Symbols should be equal
        assert_eq!(s1.symbol, s2.symbol);
        // Values should be equal
        assert_eq!(s1.as_str(), s2.as_str());
        // Display should work
        assert_eq!(s1.to_string(), "read_file");
    }

    #[test]
    fn test_serialize_deserialize() {
        let original = InternedString::new("test_tool");
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"test_tool\"");

        let deserialized: InternedString = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.as_str(), "test_tool");
    }
}
