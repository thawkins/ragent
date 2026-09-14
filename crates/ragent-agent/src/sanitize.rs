//! Secret redaction facade.
//!
//! Re-exports the single process-wide sanitisation implementation from
//! [`ragent_types::sanitize`]. A `sanitize` module in this crate previously held
//! a byte-identical second copy with its own private secret registry — so a
//! credential registered through `ragent_agent::sanitize::register_secret`
//! (or `ragent_storage`'s re-export) was invisible to
//! `ragent_server::sse::redact_secrets` and vice versa. Re-exporting the
//! `ragent_types` module makes one registry serve every subsystem (PERF-055).
//!
//! Callers continue to use `crate::sanitize::…` / `ragent_agent::sanitize::…`
//! unchanged.

pub use ragent_types::sanitize::*;
