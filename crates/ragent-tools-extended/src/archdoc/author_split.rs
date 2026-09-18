//! Marker-based splitting of a single authored LLM body into the three spec
//! files (SPEC.md / PLAN.md / TESTPLAN.md) that the `/spec govcreate`
//! authoring prompt demands (T-013, FR-013).
//!
//! The authoring prompt asks for markdown headed `1. …SPEC.md`,
//! `2. …PLAN.md`, `3. …TESTPLAN.md`. This module owns the splitting algorithm
//! shared by the TUI slash surface and the `ragent spec govcreate` CLI parity
//! path so both surfaces produce byte-identical sections.

/// Split an authored LLM body into `(spec_md, plan_md, testplan_md)`.
///
/// Assignment is driven by marker *identity* (the named file), not position,
/// so a reordered response (PLAN before SPEC) still lands each section in its
/// own file. The algorithm:
///
/// - find the first occurrence of each marker (case-insensitive), sort by
///   byte offset, and walk marker-to-marker windows so each section extends
///   to the next marker;
/// - text before the first marker is folded into SPEC.md (the preamble is, at
///   worst, model throat-clearing that belongs with the primary file);
/// - with no markers the whole body is treated as SPEC.md and minimal
///   placeholder bodies fill PLAN.md / TESTPLAN.md so the write stage always
///   has three files and the runner's FR-013 report names the failed stage
///   rather than panicking.
#[must_use]
pub fn split_authored_sections(body: &str) -> (String, String, String) {
    let markers: [(&str, usize); 3] = [("SPEC.md", 1), ("PLAN.md", 2), ("TESTPLAN.md", 3)];
    let mut cuts: Vec<(usize, usize)> = Vec::new(); // (byte_idx, which)
    let lowered = body.to_lowercase();
    for (name, which) in markers {
        if let Some(idx) = lowered.find(&name.to_lowercase()) {
            cuts.push((idx, which));
        }
    }
    cuts.sort_by_key(|(idx, _)| *idx);
    cuts.dedup_by_key(|(_, which)| *which);

    let plan_placeholder = "## Tasks\n\n(to be filled by /spec plan)\n".to_owned();
    let testplan_placeholder = "## Test Cases\n\n(to be filled by manual review)\n".to_owned();
    match cuts.as_slice() {
        [] => (body.to_owned(), plan_placeholder, testplan_placeholder),
        [(idx, which)] => {
            let (a, b) = body.split_at(*idx);
            match which {
                1 => (b.to_owned(), plan_placeholder, testplan_placeholder),
                2 => (a.to_owned(), b.to_owned(), testplan_placeholder),
                _ => (a.to_owned(), plan_placeholder, b.to_owned()),
            }
        }
        [first, ..] => {
            // Two or more markers: each cut's tail belongs to its marker's
            // file; the text before the first marker folds into SPEC.md and
            // the text after the last marker extends the last section. Marker
            // identity decides the slot, so a reordered response still
            // assigns the right content to the right file.
            let (prefix, _) = body.split_at(first.0);
            let mut spec_md = prefix.to_owned();
            let mut plan_md = plan_placeholder;
            let mut testplan_md = testplan_placeholder;
            let mut windows: Vec<((usize, usize), (usize, usize))> =
                cuts.windows(2).map(|w| (w[0], w[1])).collect();
            let last = cuts[cuts.len() - 1];
            windows.push((last, (body.len(), 0)));
            for ((start, which_a), (end, _)) in windows {
                let section = &body[start..end];
                match which_a {
                    1 => spec_md = section.to_owned(),
                    2 => plan_md = section.to_owned(),
                    _ => testplan_md = section.to_owned(),
                }
            }
            (spec_md, plan_md, testplan_md)
        }
    }
}
