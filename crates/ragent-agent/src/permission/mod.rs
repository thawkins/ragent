//! Permission checking and access-control primitives.
//!
//! This module is a thin re-export of the canonical implementation in
//! [`ragent_config::permission`]. The agent crate previously held its own
//! copy of `Permission` / `PermissionAction` / `PermissionRule` /
//! `PermissionRequest` / `PermissionChecker` (plus hand-written `From` impls
//! to bridge the two); those have been consolidated into `ragent-config` to
//! eliminate the triplication (see `REMPLAN.md` M1 / T1.2).
//!
//! `PermissionDecision` is re-exported from [`ragent_types::permission`],
//! where it lives because the `Event::PermissionReplied` variant (defined in
//! `ragent-types::event`) references it.
//!
//! All types below resolve to the single canonical definitions, so existing
//! `use crate::permission::*` sites continue to work unchanged.

pub use ragent_config::permission::{
    Permission, PermissionAction, PermissionChecker, PermissionRequest, PermissionRule,
    PermissionRuleset,
};
pub use ragent_types::permission::PermissionDecision;
