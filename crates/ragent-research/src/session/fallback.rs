//! Default (mechanical) content generation — produces a summary, findings,
//! open questions, and cross-references when no LLM analysis engine is
//! available.
//!
//! These helpers were previously inline free functions in `session.rs`.
//! They are pure (no I/O, no async) and benefit from being isolated for
//! unit testing (Milestone F-004 adds small template helpers on top of
//! these builders).

use crate::document::CrossReference;
use crate::source::{LocalSourceKind, Source};

/// Build a mechanical summary string listing how many sources of each kind
/// were captured, plus the top-3 titles/paths/spec-ids.
pub(crate) fn default_summary(sources: &[Source], topic: &str) -> String {
    let web = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .collect::<Vec<_>>();
    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .collect::<Vec<_>>();
    let specs = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .collect::<Vec<_>>();
    let total = sources.len();

    if sources.is_empty() {
        return format!(
            "No sources were captured for '{topic}'. Re-run with a more specific topic or after enabling the relevant tools."
        );
    }

    let mut out = format!(
        "Gathered {total} source(s) for '{topic}' ({w} web, {l} local, {s} spec).",
        w = web.len(),
        l = local.len(),
        s = specs.len(),
        topic = topic,
        total = total,
    );

    // Web: name the top 3 by title so the reader knows what was actually pulled in.
    if !web.is_empty() {
        out.push_str("\n\n**Web sources:** ");
        let titles: Vec<String> = web
            .iter()
            .filter_map(|s| match s {
                Source::Web { title, url, .. } if !title.is_empty() => Some(title.clone()),
                Source::Web { url, .. } => Some(url.clone()),
                _ => None,
            })
            .take(3)
            .collect();
        out.push_str(&titles.join("; "));
        if web.len() > 3 {
            out.push_str(&format!(" (and {} more)", web.len() - 3));
        }
        out.push('.');
    }

    // Local: name the top 3 paths so the reader knows which files were pulled in.
    if !local.is_empty() {
        out.push_str("\n\n**Local files:** ");
        let paths: Vec<String> = local
            .iter()
            .filter_map(|s| match s {
                Source::Local { path, .. } => Some(path.clone()),
                _ => None,
            })
            .take(3)
            .collect();
        out.push_str(&paths.join("; "));
        if local.len() > 3 {
            out.push_str(&format!(" (and {} more)", local.len() - 3));
        }
        out.push('.');
    }

    // Specs: name each spec so the reader sees which prior specs informed this research.
    if !specs.is_empty() {
        out.push_str("\n\n**Prior specs cross-referenced:** ");
        let ids: Vec<String> = specs
            .iter()
            .filter_map(|s| match s {
                Source::Spec { spec_id, .. } => Some(spec_id.clone()),
                _ => None,
            })
            .collect();
        out.push_str(&ids.join(", "));
        out.push('.');
    }

    out.push_str(
        "\n\n_No LLM analysis was applied to these sources — the section above is a mechanical digest. Re-run with a configured model for a synthesized analysis._",
    );
    out
}

/// Assemble the five-paragraph finding template used by the mechanical
/// fallback. This is the single source of truth for the finding layout so
/// the format can be tested in isolation (Milestone F-004).
fn finding_template(
    headline: &str,
    observation: &str,
    analysis: &str,
    dependencies: &str,
    implication: &str,
) -> String {
    format!(
        "**Headline:** {headline}\n\n\
         **Observation:** {observation}\n\n\
         **Analysis:** {analysis}\n\n\
         **Cross-reference / Dependencies:** {dependencies}\n\n\
         **Implication:** {implication}"
    )
}

/// Build per-source findings (one per web/local/spec source) when no LLM
/// analysis engine is available.
pub(crate) fn default_findings(sources: &[Source], topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let web: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .collect();
    let local: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .collect();
    let specs: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .collect();

    // Per-web-source finding. The reader gets the title and a 200-char
    // excerpt so the finding stands on its own without opening the
    // supporting file.
    for (idx, src) in web.iter().enumerate() {
        if let Source::Web {
            published_at: None,
            title,
            url,
            body,
            ..
        } = src
        {
            let label = if title.is_empty() {
                url.as_str()
            } else {
                title.as_str()
            };
            let excerpt = body_excerpt(body, 200);
            let observation = if excerpt.is_empty() {
                format!(
                    "The web source **{label}** from <{url}> was captured, but no body text was returned by the fetch. [#{n}]",
                    n = idx + 1,
                )
            } else {
                format!(
                    "The web source **{label}** from <{url}> states: \"{excerpt}\" [#{n}]",
                    n = idx + 1,
                )
            };
            let previous = if idx > 0 {
                format!(
                    "This finding follows and reinforces the web-source thread begun in Finding {idx}."
                )
            } else {
                "No direct dependencies.".to_string()
            };
            let headline = crate::document::make_headline_from_observation(&observation);
            let finding = finding_template(
                &headline,
                &observation,
                &format!(
                    "This evidence relates directly to the topic '{topic}', providing public context that can be compared against project-local material."
                ),
                &previous,
                "The source should be treated as background unless it is corroborated by an in-project reference or a later finding; if no corroboration exists, flag it as an open question.",
            );
            out.push(finding);
        }
    }

    // Per-local-source findings.
    let local_offset = web.len();
    for (idx, src) in local.iter().enumerate() {
        if let Source::Local {
            path,
            relevance,
            body,
            ..
        } = src
        {
            let excerpt = body_excerpt(body, 200);
            let observation = if excerpt.is_empty() {
                format!(
                    "The in-project file `{path}` was matched as relevant (`{relevance}`), but no excerpt was captured. [#{n}]",
                    n = local_offset + idx + 1,
                )
            } else {
                format!(
                    "The in-project file `{path}` (relevance: `{relevance}`) contains the following excerpt: \"{excerpt}\" [#{n}]",
                    n = local_offset + idx + 1,
                )
            };
            let sibling_idx = if idx > 0 {
                Some(local_offset + idx)
            } else {
                None
            };
            let web_idx = if web.is_empty() { None } else { Some(1usize) };
            let dependencies = match (sibling_idx, web_idx) {
                (Some(s), Some(_)) => format!(
                    "This finding is related to Finding {s} (the previous local match) and builds on Finding 1 (the first web source) by grounding public information in project code.",
                ),
                (Some(s), None) => format!(
                    "This finding depends on Finding {s}, which established the first local match in this sequence.",
                ),
                (None, Some(_)) =>
                    "This finding is the first local match; it can be cross-checked against Finding 1 (the first web source)."
                        .to_string(),
                (None, None) => "No direct dependencies.".to_string(),
            };
            let headline = crate::document::make_headline_from_observation(&observation);
            let finding = finding_template(
                &headline,
                &observation,
                &format!(
                    "This in-project evidence shows how '{topic}' touches the current codebase and is the strongest signal of immediate relevance."
                ),
                &dependencies,
                "The referenced path is a concrete place to start implementation or further investigation; consider opening it as a cross-reference and verifying the excerpt against the latest source.",
            );
            out.push(finding);
        }
    }

    // Per-spec findings.
    let spec_offset = web.len() + local.len();
    for (idx, src) in specs.iter().enumerate() {
        if let Source::Spec {
            spec_id, relevance, ..
        } = src
        {
            let note = if relevance.is_empty() {
                format!("see specs/{spec_id}/SPEC.md")
            } else {
                relevance.clone()
            };
            let first_local = if local_offset > 0 {
                Some(local_offset + 1)
            } else {
                None
            };
            let first_web = if web.is_empty() { None } else { Some(1usize) };
            let dependencies = match (first_local, first_web) {
                (Some(l), Some(_)) => format!(
                    "This finding connects the prior specification to the in-project evidence in Finding {l} and the web background in Finding 1; treat it as the bridge between design intent and current code.",
                ),
                (Some(l), None) => format!(
                    "This finding depends on Finding {l}, which identified the in-project material that implements (or should implement) this spec.",
                ),
                (None, Some(_)) =>
                    "This finding is related to Finding 1 (web background) but no local implementation has been matched yet."
                        .to_string(),
                (None, None) => "No direct dependencies.".to_string(),
            };
            let headline = format!("Prior spec `{spec_id}` is relevant to '{topic}' ({note})");
            let observation = format!(
                "Prior spec `{spec_id}` is relevant to '{topic}' ({note}) [#{n}].",
                n = spec_offset + idx + 1,
            );
            let finding = finding_template(
                &headline,
                &observation,
                "This specification establishes requirements or decisions that pre-date the current research, and should constrain or guide any conclusions drawn from newer sources.",
                &dependencies,
                "Before acting on later findings, verify that the project still honours this spec; conflicts between this spec and newer evidence should be escalated as an open question.",
            );
            out.push(finding);
        }
    }

    if sources.is_empty() {
        out.push(finding_template(
            "No sources captured",
            &format!("No sources were captured for '{topic}'."),
            "Without captured web pages, local files, or prior specs, the research cannot yet support a substantive conclusion.",
            "No direct dependencies.",
            "Consider re-running with a more specific topic, or run inside a project with relevant files and specs so gathering has something to work with.",
        ));
    }
    out
}

/// Build a per-source bullet title + short excerpt suitable for embedding
/// in the Findings section when no LLM analysis is available. Returns an
/// empty string when the body is empty / unavailable.
pub(crate) fn body_excerpt(body: &str, max_chars: usize) -> String {
    // Strip the "Excerpt — N keyword match(es)" header that the local
    // gatherer prepends so we don't double-print it in the Findings section.
    let stripped = body.strip_prefix("Excerpt —").map_or(body, |rest| {
        rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '\n')
    });
    // Strip markdown code fences (e.g. ```text) that can appear at the start
    // of supporting-file bodies so the excerpt begins with real content.
    let stripped = stripped
        .trim_start()
        .trim_start_matches("```text")
        .trim_start_matches("```")
        .trim_start();
    // Collapse whitespace so the excerpt fits on one logical line.
    let collapsed: String = stripped
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let collapsed = collapsed.split_whitespace().collect::<Vec<_>>().join(" ");
    // Drop a trailing markdown fence that survived the collapse.
    let collapsed = collapsed
        .strip_suffix("```")
        .map_or(collapsed.clone(), |s| s.trim_end().to_string());
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        // Reserve one character for the ellipsis so the total output never
        // exceeds the requested limit.
        let budget = max_chars.saturating_sub(1);
        let mut out: String = collapsed.chars().take(budget).collect();
        out.push('…');
        out
    }
}

/// Build default open-questions bullets when no LLM analysis is available.
pub(crate) fn default_open_questions(sources: &[Source], topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let web = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .count();
    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    let spec = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    if sources.is_empty() {
        out.push(format!(
            "Why was nothing captured for '{topic}' — was a tool unavailable, the topic too narrow, or the search query off?"
        ));
    } else {
        if web == 0 {
            out.push("No web sources were captured — was `websearch` unavailable, or does the topic lack good public references?".into());
        }
        if local == 0 {
            out.push(
                "No in-project files matched — is there a code path or doc the topic should touch that grep did not surface?"
                    .into(),
            );
        }
        if spec == 0 {
            out.push(
                "No prior specs were cross-referenced — is the topic genuinely new, or were existing specs filtered out by the keyword match?"
                    .into(),
            );
        }
        out.push(format!(
            "Re-run `/research {topic}` with a configured LLM to produce an LLM-synthesized analysis instead of this mechanical digest."
        ));
    }
    out
}

/// Extract `CrossReference` entries from local sources.
#[allow(dead_code)]
pub(crate) fn cross_references_from(sources: &[Source]) -> Vec<CrossReference> {
    sources
        .iter()
        .filter_map(|s| match s {
            Source::Local {
                path,
                relevance,
                kind,
                ..
            } => Some(CrossReference {
                path: path.clone(),
                relevance: format_with_kind(relevance, *kind),
            }),
            _ => None,
        })
        .collect()
}

#[allow(dead_code)]
fn format_with_kind(relevance: &str, kind: LocalSourceKind) -> String {
    match kind {
        LocalSourceKind::InProject => relevance.to_string(),
        LocalSourceKind::Extra => format!("{relevance} (from --sources-dir)"),
    }
}
