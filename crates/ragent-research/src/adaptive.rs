//! Adaptive stopping policy for the iterative research loop (T-007, FR-008).
//!
//! The stopper keeps a history of evaluation scores and terminates retrieval
//! early when progress stalls or when the configured iteration budget is
//! exhausted. It also supports an explicit "deeper run" override flag so the
//! user can keep iterating even when the score plateaus.

/// Decision returned by [`AdaptiveStopper::decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopDecision {
    /// Continue with another iteration.
    Continue,
    /// Stop because the iteration budget is exhausted.
    MaxIterations,
    /// Stop because the score did not improve over recent iterations.
    NoImprovement,
    /// Stop because the research state is complete.
    Complete,
    /// Stop because the user requested a halt.
    UserRequest,
}

impl StopDecision {
    /// `true` when the loop should terminate.
    pub fn should_stop(self) -> bool {
        !matches!(self, Self::Continue)
    }

    /// Short snake-case label for event rendering.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::MaxIterations => "max_iterations",
            Self::NoImprovement => "no_improvement",
            Self::Complete => "complete",
            Self::UserRequest => "user_request",
        }
    }
}

/// Adaptive stopping policy.
#[derive(Debug, Clone, Default)]
pub struct AdaptiveStopper {
    /// Maximum iterations allowed.
    max_iterations: u32,
    /// Minimum iterations before `NoImprovement` is allowed.
    min_iterations: u32,
    /// Score history in iteration order.
    history: Vec<u32>,
    /// Set to `true` to ignore the no-improvement rule.
    force_deeper: bool,
}

impl AdaptiveStopper {
    /// Build a stopper for the given budget. `min_iterations` defaults to 2.
    pub fn new(max_iterations: u32) -> Self {
        Self {
            max_iterations,
            min_iterations: 2,
            history: Vec::new(),
            force_deeper: false,
        }
    }

    /// Require at least this many iterations before a no-improvement stop.
    pub fn with_min_iterations(mut self, n: u32) -> Self {
        self.min_iterations = n;
        self
    }

    /// Ignore no-improvement stops (used for `--depth deep` / explicit deeper run).
    pub fn with_force_deeper(mut self, force: bool) -> Self {
        self.force_deeper = force;
        self
    }

    /// Decide whether to stop before starting iteration `iteration` (1-based).
    ///
    /// `score` is the evaluation score *after* the previous iteration. Pass
    /// `None` when no score is available.
    ///
    /// Rules, in order:
    ///
    /// 1. `Complete` if `is_complete` is `true`.
    /// 2. `MaxIterations` if `iteration > max_iterations`.
    /// 3. `NoImprovement` if `force_deeper` is `false`, `iteration > min_iterations + 1`,
    ///    and the latest score is not strictly greater than any score in the
    ///    previous two iterations.
    /// 4. Otherwise `Continue`.
    pub fn decide(
        &mut self,
        iteration: u32,
        score: Option<u32>,
        is_complete: bool,
    ) -> StopDecision {
        if is_complete {
            return StopDecision::Complete;
        }
        if iteration > self.max_iterations {
            return StopDecision::MaxIterations;
        }
        if let Some(s) = score {
            self.history.push(s);
        }
        if !self.force_deeper && iteration > self.min_iterations && self.history.len() >= 2 {
            let latest = self.history[self.history.len() - 1];
            let window = &self.history[self.history.len().saturating_sub(3)..self.history.len()];
            if window.iter().all(|prev| latest <= *prev) {
                return StopDecision::NoImprovement;
            }
        }
        StopDecision::Continue
    }

    /// Access the recorded score history.
    pub fn history(&self) -> &[u32] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopper_continues_within_budget() {
        let mut s = AdaptiveStopper::new(3);
        assert_eq!(s.decide(1, Some(50), false), StopDecision::Continue);
    }

    #[test]
    fn stopper_halts_at_max_iterations() {
        let mut s = AdaptiveStopper::new(2);
        s.decide(1, Some(50), false);
        s.decide(2, Some(60), false);
        assert_eq!(s.decide(3, Some(60), false), StopDecision::MaxIterations);
    }

    #[test]
    fn stopper_halts_on_no_improvement() {
        let mut s = AdaptiveStopper::new(5);
        s.decide(1, Some(50), false);
        s.decide(2, Some(60), false);
        s.decide(3, Some(60), false);
        assert_eq!(s.decide(4, Some(60), false), StopDecision::NoImprovement);
    }

    #[test]
    fn stopper_continues_when_improving() {
        let mut s = AdaptiveStopper::new(5);
        s.decide(1, Some(50), false);
        s.decide(2, Some(60), false);
        assert_eq!(s.decide(3, Some(70), false), StopDecision::Continue);
    }

    #[test]
    fn stopper_respects_force_deeper() {
        let mut s = AdaptiveStopper::new(5).with_force_deeper(true);
        s.decide(1, Some(50), false);
        s.decide(2, Some(60), false);
        assert_eq!(s.decide(3, Some(60), false), StopDecision::Continue);
    }

    #[test]
    fn stopper_stops_on_complete() {
        let mut s = AdaptiveStopper::new(10);
        assert_eq!(s.decide(1, None, true), StopDecision::Complete);
    }
}
