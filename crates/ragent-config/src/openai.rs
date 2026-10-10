//! OpenAI-compatible inbound surface configuration (spec `openhands` FR-012,
//! FR-024, FR-032).
//!
//! ragent exposes an OpenAI-compatible HTTP adapter (`GET /v1/models`,
//! `POST /v1/chat/completions`) over its existing [`SessionProcessor`] agent
//! loop. This section gates that surface and carries the bearer token it must
//! present.
//!
//! ## Fail-closed token requirement
//!
//! The surface is **not** served unauthenticated. When it is explicitly enabled
//! (FR-032) it must have a bearer token configured here (or in the ambient
//! `RAGENT_TOKEN` environment variable); if no token is available the server
//! refuses to start the surface rather than exposing it open. The token is the
//! same one the native REST API uses (FR-024) - ragent runs one bearer token for
//! the whole server.
//!
//! ```jsonc
//! {
//!   "openai": {
//!     "enabled": true,
//!     "token": "sk-ragent-local"
//!   }
//! }
//! ```
//!
//! Opt-in: the section is absent from the default config, which leaves the
//! surface's gating inert (the endpoints remain reachable under the native
//! bearer-token check, exactly as before this section existed).

use serde::{Deserialize, Serialize};

/// The `openai` section of `ragent.json` (FR-012, FR-024, FR-032).
///
/// ```jsonc
/// {
///   "openai": {
///     "enabled": true,
///     "token": "sk-ragent-local"
///   }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenAiConfig {
    /// Whether the OpenAI-compatible surface is an explicitly enabled capability.
    ///
    /// Default `false`. The endpoints are always mounted behind the native
    /// bearer-token check; this flag additionally arms the FR-032 startup gate,
    /// which refuses to serve the surface when it is enabled without a
    /// configured token.
    #[serde(default)]
    pub enabled: bool,
    /// Bearer token required for every request to the surface (FR-024).
    ///
    /// When `None` the ambient `RAGENT_TOKEN` environment variable is consulted
    /// at resolution time (see [`crate::Config::openai_surface_token`]). Redacted
    /// in diagnostics and the `Debug` rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl OpenAiConfig {
    /// `true` when the section carries no deviation from the disabled default.
    #[must_use]
    pub fn is_default(&self) -> bool {
        !self.enabled && self.token.is_none()
    }
}
