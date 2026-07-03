//! Permission types used by the event system.
//!
//! The full permission checking system (`Permission`, `PermissionAction`,
//! `PermissionRule`, `PermissionRequest`, `PermissionChecker`) lives in
//! `ragent-config::permission` — see `REMPLAN.md` M1 / T1.2 for the
//! consolidation history.  Only [`PermissionDecision`] remains here because
//! the `Event::PermissionReplied` variant (defined in
//! [`crate::event::Event`]) references it, and `ragent-types` must not depend
//! on `ragent-config`.

use serde::{Deserialize, Serialize};

/// The user's response to a permission request.
///
/// Re-exported by `ragent-config::permission` and `ragent-agent::permission`
/// so all crates share a single definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Allow this single occurrence only.
    Once,
    /// Allow now and for all future matching requests.
    Always,
    /// Deny the request.
    Deny,
}