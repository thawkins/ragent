//! Open background sessions (spec MULTISESSION, milestone M3).
//!
//! The focused session lives in `App::session`; every other open session is
//! parked here. Views are kept in most-recently-focused order: parking the
//! focused session inserts it at the front, so index 0 is always the session
//! the user was looking at last. That order is what `/session close` falls
//! back to and what the sidebar (M5) will list.

use super::session_view::{Attention, SessionView};

/// Background sessions, most recently focused first.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    views: Vec<SessionView>,
}

impl SessionRegistry {
    /// Number of background sessions.
    pub fn len(&self) -> usize {
        self.views.len()
    }

    /// Whether no session is parked in the background.
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// Iterates over background sessions, most recently focused first.
    pub fn iter(&self) -> impl Iterator<Item = &SessionView> {
        self.views.iter()
    }

    /// Index of the background session with the given id.
    ///
    /// # Arguments
    ///
    /// * `session_id` - full session id.
    ///
    /// # Returns
    ///
    /// The index, or `None` when no background session has that id.
    pub fn position(&self, session_id: &str) -> Option<usize> {
        self.views
            .iter()
            .position(|v| v.session_id.as_deref() == Some(session_id))
    }

    /// Parks a session as the most recently focused one.
    ///
    /// # Arguments
    ///
    /// * `view` - the session being moved out of focus.
    pub fn insert_front(&mut self, view: SessionView) {
        self.views.insert(0, view);
    }

    /// Removes and returns the background session at `index`.
    ///
    /// # Arguments
    ///
    /// * `index` - position from [`SessionRegistry::position`] or iteration.
    ///
    /// # Returns
    ///
    /// The removed view, or `None` when `index` is out of range.
    pub fn remove(&mut self, index: usize) -> Option<SessionView> {
        (index < self.views.len()).then(|| self.views.remove(index))
    }

    /// Number of background sessions waiting on the user (FR-06).
    pub fn waiting_count(&self) -> usize {
        self.views
            .iter()
            .filter(|v| v.attention() == Attention::Waiting)
            .count()
    }

    /// Mutable access to the view at `index`, for swapping it into focus.
    pub(crate) fn view_mut(&mut self, index: usize) -> Option<&mut SessionView> {
        self.views.get_mut(index)
    }

    /// Mutable access to the view at `index`, for tests that seed a
    /// background session's state.
    ///
    /// # Panics
    ///
    /// Panics when `index` is out of range.
    #[doc(hidden)]
    pub fn view_mut_for_test(&mut self, index: usize) -> &mut SessionView {
        &mut self.views[index]
    }
}
