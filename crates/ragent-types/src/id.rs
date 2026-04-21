//! Typed newtype wrappers for identifiers used throughout ragent.
//!
//! Each wrapper ensures type safety — a [`SessionId`] cannot be accidentally
//! used where a [`MessageId`] is expected, even though both are strings.
//!
//! # Examples
//!
//! Creating a new random identifier:
//!
//! ```
//! use ragent_types::id::SessionId;
//!
//! let id = SessionId::new();
//! assert!(!id.as_str().is_empty());
//! ```
//!
//! Converting from a known string:
//!
//! ```
//! use ragent_types::id::MessageId;
//!
//! let id = MessageId::from("msg-42");
//! assert_eq!(id.as_str(), "msg-42");
//! ```

use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl $name {
            /// Creates a new random ID.
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }
            /// Returns the ID as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

define_id!(SessionId, "A unique session identifier.");
define_id!(MessageId, "A unique message identifier.");
define_id!(ProviderId, "A unique provider identifier.");
define_id!(ToolCallId, "A unique tool call identifier.");
