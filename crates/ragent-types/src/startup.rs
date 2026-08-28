//! Startup timing instrumentation.
//!
//! [`StartupTimings`] records the duration of each stage in the ragent startup
//! pipeline (CLI parse → config load → storage open → provider/tool registries
//! → TUI init → session create → code index, etc.).
//!
//! The collected data is displayed via the `/startup` TUI slash command so
//! users can identify which stages contribute most to the perceived startup
//! latency.

use std::time::{Duration, Instant};

/// A single named stage with its measured wall-clock duration.
#[derive(Debug, Clone)]
pub struct StartupStage {
    /// Human-readable stage label (e.g. "Config load").
    pub name: String,
    /// Duration of the stage in milliseconds.
    pub duration_ms: u128,
}

/// Collected timings for every instrumented startup stage.
#[derive(Debug, Clone)]
pub struct StartupTimings {
    /// Ordered list of completed stages.
    stages: Vec<StartupStage>,
    /// Wall-clock instant when timing started (set in [`StartupTimings::new`]).
    global_start: Instant,
    /// Frozen wall-clock total, set by [`StartupTimings::finish`]. When set,
    /// [`StartupTimings::total_elapsed_ms`] reports this value instead of the
    /// still-running clock, so `/startup` stays stable after startup completes.
    finished_total_ms: Option<u128>,
}

impl Default for StartupTimings {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupTimings {
    /// Create a new timings collector, recording the global start instant.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            global_start: Instant::now(),
            finished_total_ms: None,
        }
    }

    /// Record a completed stage.
    ///
    /// Call this after measuring a stage with `Instant::now()` / `elapsed()`:
    ///
    /// ```ignore
    /// let t0 = Instant::now();
    /// // ... do work ...
    /// timings.record("Config load", t0.elapsed());
    /// ```
    pub fn record(&mut self, name: impl Into<String>, duration: Duration) {
        self.stages.push(StartupStage {
            name: name.into(),
            duration_ms: duration.as_millis(),
        });
    }

    /// Freeze the wall-clock total at its current value.
    ///
    /// Call this once startup is complete (e.g. the primary agent goes ready).
    /// After this, [`Self::total_elapsed_ms`] and [`Self::untracked_ms`] report
    /// the value captured here rather than the still-advancing clock, so
    /// repeated `/startup` displays stay stable instead of growing the
    /// "Untracked" row.
    pub fn finish(&mut self) {
        if self.finished_total_ms.is_none() {
            self.finished_total_ms = Some(self.global_start.elapsed().as_millis());
        }
    }

    /// Total wall-clock elapsed time from [`StartupTimings::new`] to now.
    ///
    /// If [`Self::finish`] has been called, returns the frozen value captured
    /// at that point; otherwise returns the live elapsed time.
    #[must_use]
    pub fn total_elapsed_ms(&self) -> u128 {
        self.finished_total_ms
            .unwrap_or_else(|| self.global_start.elapsed().as_millis())
    }

    /// Sum of all recorded stage durations.
    ///
    /// Compared with [`Self::total_elapsed_ms`], the difference reveals time
    /// spent in uninstrumented sections of the startup pipeline.
    #[must_use]
    pub fn sum_stages_ms(&self) -> u128 {
        self.stages.iter().map(|s| s.duration_ms).sum()
    }

    /// Uninstrumented time: `total_elapsed_ms - sum_stages_ms`.
    #[must_use]
    pub fn untracked_ms(&self) -> u128 {
        self.total_elapsed_ms().saturating_sub(self.sum_stages_ms())
    }

    /// Return a reference to the recorded stages.
    #[must_use]
    pub fn stages(&self) -> &[StartupStage] {
        &self.stages
    }

    /// Drain all recorded stages, leaving `self` empty.
    ///
    /// This is used by `run_tui()` to extract sub-stages recorded inside
    /// `App::new()` and merge them into the main timings collector.
    pub fn drain_stages(&mut self) -> Vec<StartupStage> {
        std::mem::take(&mut self.stages)
    }

    /// Append stages from another `StartupTimings` into `self`.
    ///
    /// The stages are moved (not copied) from `other`, leaving `other` empty.
    /// The global start of `self` is preserved; `other`'s start is discarded.
    pub fn merge_stages(&mut self, other: &mut Self) {
        self.stages.append(&mut other.stages);
    }

    /// Format the timings as a human-readable report string.
    ///
    /// The output is a compact aligned table (no box-drawing borders) sized to
    /// fit within a 100-character-wide terminal display.
    #[must_use]
    pub fn format_report(&self) -> String {
        // Name column: width of the longest stage name, but at least 5 ("Stage").
        let name_w = self
            .stages
            .iter()
            .map(|s| s.name.len())
            .chain([5usize]) // "Stage" header
            .max()
            .unwrap_or(5)
            .max(5);

        // Time column: width of the longest formatted time ("NNN ms"), but at
        // least 4 ("Time"). The Sum and Untracked rows can exceed the total
        // (double-counted merged stages), so include their widths too.
        let time_w = self
            .stages
            .iter()
            .map(|s| format!("{} ms", s.duration_ms).len())
            .chain([4usize]) // "Time" header
            .chain([format!("{} ms", self.total_elapsed_ms()).len()])
            .chain([format!("{} ms", self.sum_stages_ms()).len()])
            .chain([format!("{} ms", self.untracked_ms()).len()])
            .max()
            .unwrap_or(4)
            .max(4);

        // Total visible width: name_w + 2 spaces + time_w.
        let rule = "-".repeat(name_w + 2 + time_w);

        let mut out = String::new();
        out.push_str("From: /startup\n\n");
        // Wrap the table in a fenced code block so the TUI markdown renderer
        // (`try_extract_research_code_block`) preserves the preformatted layout
        // verbatim instead of collapsing soft-wrapped lines into one paragraph.
        out.push_str("```\n");
        // Row formatter shared by the header, stage, Sum, Untracked, and Total
        // rows: right-align the ms value under the "Time" header.
        let row = |out: &mut String, name: &str, ms: u128| {
            out.push_str(&format!(
                "{:<name_w$}  {:>time_w$}\n",
                name,
                format!("{ms} ms"),
                name_w = name_w,
                time_w = time_w
            ));
        };
        // Header row: "Stage" in the name column, "Time" in the time column.
        out.push_str(&format!(
            "{:<name_w$}  {:>time_w$}\n",
            "Stage",
            "Time",
            name_w = name_w,
            time_w = time_w
        ));
        out.push_str(&rule);
        out.push('\n');
        for stage in &self.stages {
            row(&mut out, &stage.name, stage.duration_ms);
        }
        out.push_str(&rule);
        out.push('\n');
        // Show the sum of instrumented stages alongside the wall-clock total
        // so the uninstrumented gap is immediately visible.
        row(&mut out, "Sum", self.sum_stages_ms());
        row(&mut out, "Untracked", self.untracked_ms());
        row(&mut out, "Total", self.total_elapsed_ms());
        out.push_str("```\n");
        out
    }
}
