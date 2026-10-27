//! Unresolved permission verdicts, as the tool phase sees them.
//!
//! FR-013 of spec `uniformtools` states one rule: *while a tool call's
//! permission verdict is unresolved, the system shall not append a tool result
//! for it to history, so no turn can continue on an unapproved call's output.*
//!
//! The rule is state-driven rather than a property of the permission layer, and
//! that is what makes it a guard rather than a comment. The permission layer
//! holds a call at a prompt, but nothing in that layer knows when the call's
//! outcome is *recorded*: the tool phase records outcomes, and it used to do so
//! for every call it was told about. A call that is still at a prompt has no
//! outcome, so the only way a result could be appended for it is a call whose
//! verdict did not resolve at all - and that is exactly the failure this module
//! makes observable and refuses.
//!
//! ## What is tracked
//!
//! [`PendingVerdicts`] holds, per session, the call ids that are currently at a
//! permission prompt: opened by [`PendingVerdicts::begin`] immediately before the
//! prompt is published, closed by [`PendingVerdicts::settle`] when the verdict
//! resolves (allow, deny or timeout - a timed-out prompt is a denial, so it is
//! resolved). A call is therefore *unresolved* for exactly the interval in which
//! the user, the client or the timer has not answered yet.
//!
//! The tracker is shared by the two places that need the same answer:
//!
//! * the permission stage of the dispatch pipeline
//!   ([`crate::session::dispatch`]) marks its wait and settles it in every exit
//!   path, including a join failure after the call was dropped;
//! * the tool phase ([`crate::session::processor`]) refuses to append a result
//!   for a call the tracker still names.
//!
//! ## Why the guard can fire
//!
//! Two paths can reach the tool phase with a call that never got a verdict: a
//! tool task that is aborted (a cancellation, a panic in a task that was not
//! joined) and a pipeline stage that returns before the permission layer runs
//! after a prompt was already published. Neither is a normal outcome, and both
//! are exactly what FR-013 forbids from becoming a tool result. On that path the
//! phase records the call as `Pending` with no result instead of inventing one,
//! so the model can never read an unapproved call's output as a success - and
//! logs one warning naming the call, because a silent skip would hide a wedged
//! verdict.
//!
//! A tracker that is never consulted is inert: the common path marks, settles
//! and continues, and the guard reads one map and finds nothing.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The per-session set of tool calls whose permission verdict has not resolved.
///
/// Cloneable and cheap to share: the whole state sits behind one `Arc<Mutex<_>>`,
/// so the dispatch pipeline and the tool phase observe the same set rather than
/// two copies that could disagree.
#[derive(Clone, Default)]
pub struct PendingVerdicts {
    inner: Arc<parking_lot::Mutex<HashMap<String, HashSet<String>>>>,
}

impl std::fmt::Debug for PendingVerdicts {
    /// Renders the number of sessions holding an unresolved call, never the
    /// call ids themselves (a `Debug` render must not lock a map a caller may be
    /// holding).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingVerdicts").finish_non_exhaustive()
    }
}

impl PendingVerdicts {
    /// A tracker with no unresolved call in any session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `call_id` unresolved for `session_id`.
    ///
    /// Called immediately before a permission prompt is published, so the
    /// interval the tracker describes is the interval the user has not answered
    /// yet. Marking a call twice (two prompts for one call - the bash sub-command
    /// loop prompts once per sub-command) leaves it marked, and a single
    /// [`settle`](Self::settle) closes it: the call's verdict is unresolved until
    /// the last prompt it raised has an answer, and no answer is a denial.
    pub fn begin(&self, session_id: &str, call_id: &str) {
        let mut map = self.inner.lock();
        map.entry(session_id.to_string())
            .or_default()
            .insert(call_id.to_string());
    }

    /// Marks `call_id` resolved for `session_id`.
    ///
    /// Every exit path from a prompt calls this: an allow, a deny, a timeout and
    /// a dropped wait all end the unresolved interval, because none of them
    /// leaves the call awaiting an answer.
    pub fn settle(&self, session_id: &str, call_id: &str) {
        let mut map = self.inner.lock();
        if let Some(calls) = map.get_mut(session_id) {
            calls.remove(call_id);
            if calls.is_empty() {
                map.remove(session_id);
            }
        }
    }

    /// Returns `true` while `call_id`'s verdict has not resolved.
    #[must_use]
    pub fn is_unresolved(&self, session_id: &str, call_id: &str) -> bool {
        let map = self.inner.lock();
        map.get(session_id)
            .is_some_and(|calls| calls.contains(call_id))
    }
}

/// Test-only re-export of [`PendingVerdicts`] operations used by integration
/// tests, which live in a separate crate.
#[doc(hidden)]
#[must_use]
pub fn pending_verdicts_for_test() -> PendingVerdicts {
    PendingVerdicts::new()
}
