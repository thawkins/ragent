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
use regex::Regex;

/// Append a `\n\n<header> <top-3 items joined by '; '> (and N more).` block
/// to `out`. Items after the third are summarised as a count. Does nothing
/// when `items` is empty.
fn append_top_three_list(out: &mut String, header: &str, items: &[impl AsRef<str>]) {
    if items.is_empty() {
        return;
    }
    out.push_str("\n\n");
    out.push_str(header);
    out.push(' ');
    for (i, item) in items.iter().take(3).enumerate() {
        if i > 0 {
            out.push_str("; ");
        }
        out.push_str(item.as_ref());
    }
    if items.len() > 3 {
        out.push_str(&format!(" (and {} more)", items.len() - 3));
    }
    out.push('.');
}

/// Build a mechanical summary string listing how many sources of each kind
/// were captured, plus the top-3 titles/paths/spec-ids.
pub fn default_summary(sources: &[Source], topic: &str) -> String {
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
    let web_titles: Vec<String> = web
        .iter()
        .filter_map(|s| match s {
            Source::Web { title, url, .. } if !title.is_empty() => Some(title.clone()),
            Source::Web { url, .. } => Some(url.clone()),
            _ => None,
        })
        .collect();
    append_top_three_list(&mut out, "**Web sources:**", &web_titles);

    // Local: name the top 3 paths so the reader knows which files were pulled in.
    let local_paths: Vec<String> = local
        .iter()
        .filter_map(|s| match s {
            Source::Local { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    append_top_three_list(&mut out, "**Local files:**", &local_paths);

    // Specs: name each spec so the reader sees which prior specs informed this research.
    let spec_ids: Vec<String> = specs
        .iter()
        .filter_map(|s| match s {
            Source::Spec { spec_id, .. } => Some(spec_id.clone()),
            _ => None,
        })
        .collect();
    append_top_three_list(&mut out, "**Prior specs cross-referenced:**", &spec_ids);

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
pub fn default_findings(sources: &[Source], topic: &str) -> Vec<String> {
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
            title, url, body, ..
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
            let analysis = format!(
                "This evidence relates directly to the topic '{topic}', providing public context that can be compared against project-local material. \
                 The web source **{label}** from <{url}> contributes an external perspective that may confirm, contradict, or extend what the local codebase reveals. \
                 Because this is a publicly available source, its authority and recency should be weighed against any in-project evidence: a web source that \
                 predates a recent code change may describe stale behavior, while a freshly published source may capture the current state more accurately. \
                 The excerpt captured from the source — \"{excerpt}\" — is a snapshot at fetch time and may not reflect subsequent edits; treat it as a point-in-time \
                 observation rather than a permanent truth. When {web_count} web source(s) were gathered, this finding should be read alongside the others \
                 to identify areas of agreement and disagreement, and to triangulate the most reliable account. If no in-project file corroborates this \
                 web source, the finding should be treated as background context only and flagged as an open question for follow-up verification.",
                label = label,
                url = url,
                excerpt = excerpt,
                web_count = web.len(),
            );
            let finding = finding_template(
                &headline,
                &observation,
                &analysis,
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
            let analysis = format!(
                "This in-project evidence shows how '{topic}' touches the current codebase and is the strongest signal of immediate relevance. \
                 The file `{path}` was matched with relevance note `{relevance}`, meaning the local gatherer identified it as directly connected to the research topic. \
                 Unlike web sources, which provide external context, this local file is part of the project under investigation and its contents reflect the \
                 actual implementation, configuration, or documentation as it exists right now. The excerpt — \"{excerpt}\" — is a verbatim snapshot from \
                 the file, so it can be trusted as a primary source for the current state of the codebase. However, the excerpt is limited to the matching \
                 lines and their immediate context; the full file may contain additional relevant material outside the captured region. When {local_count} \
                 local file(s) were gathered, this finding should be cross-referenced with the others to build a complete picture of how the topic \
                 manifests across the project. If web sources were also captured, compare the local implementation against the external descriptions to \
                 identify gaps, drift, or contradictions — these are the most actionable findings because they point to concrete changes that may be needed.",
                path = path,
                relevance = relevance,
                excerpt = excerpt,
                local_count = local.len(),
            );
            let finding = finding_template(
                &headline,
                &observation,
                &analysis,
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
            let analysis = format!(
                "This specification establishes requirements or decisions that pre-date the current research, and should constrain or guide any conclusions \
                 drawn from newer sources. The spec `{spec_id}` (noted as: {note}) represents a deliberate design decision made by the project team, \
                 and its requirements carry more authority than a single web source because it reflects the project's intended direction. When evaluating \
                 newer evidence from web or local sources, any conclusion that conflicts with this spec should be treated as a potential deviation that \
                 needs explicit resolution — either the spec should be updated to reflect the new reality, or the code should be brought back into \
                 compliance. The spec may also define acceptance criteria, constraints, or non-functional requirements that are not visible in the \
                 code excerpt alone, so it should be consulted alongside any implementation changes. When {spec_count} spec(s) were cross-referenced, \
                 each one should be checked for overlap or conflict with the others; specs that address the same concern from different angles may \
                 contain complementary or contradictory requirements that need to be reconciled before acting on the research findings.",
                spec_id = spec_id,
                note = note,
                spec_count = specs.len(),
            );
            let finding = finding_template(
                &headline,
                &observation,
                &analysis,
                &dependencies,
                "Before acting on later findings, verify that the project still honours this spec; conflicts between this spec and newer evidence should be escalated as an open question.",
            );
            out.push(finding);
        }
    }

    if sources.is_empty() {
        let analysis = format!(
            "Without captured web pages, local files, or prior specs, the research cannot yet support a substantive conclusion. \
             The gathering phase attempted to find relevant material for the topic '{topic}' but returned empty results from all source types: \
             no web pages were fetched, no in-project files matched the search keywords, and no prior specifications were cross-referenced. \
             This means the research is in an evidence vacuum — any conclusion drawn without sources would be speculation rather than analysis. \
             There are several possible reasons for the empty result: the topic may be too narrow or use terminology that does not appear in the \
             codebase or on the web; the web search tools may have failed to return results due to network issues or rate limits; the local file \
             gatherer may not have found keyword matches because the relevant code uses different naming conventions; or there may genuinely be \
             no prior work on this topic in the project or the public web. The recommended next step is to re-run the research with a broader \
             or rephrased topic, ensure the web search tools are operational, and verify that the project directory contains files relevant to \
             the research question. If the topic is genuinely novel with no existing material, consider whether the research question itself \
             should be reframed or whether this is a greenfield area where the findings will be entirely forward-looking."
        );
        out.push(finding_template(
            "No sources captured",
            &format!("No sources were captured for '{topic}'."),
            &analysis,
            "No direct dependencies.",
            "Consider re-running with a more specific topic, or run inside a project with relevant files and specs so gathering has something to work with.",
        ));
    }
    out
}

pub fn default_top_implications(findings: &[String], topic: &str) -> Vec<String> {
    // Try to extract the first sentence from each finding's **Implication:** paragraph.
    let re = Regex::new(r"(?i)\*\*Implication:\*\*\s*([^\n]+)").expect("valid implication regex");
    let mut extracted: Vec<String> = findings
        .iter()
        .filter_map(|f| {
            re.captures(f).and_then(|cap| {
                let sentence = cap[1].trim().trim_end_matches('.').to_string();
                if sentence.is_empty() {
                    None
                } else {
                    Some(sentence)
                }
            })
        })
        .collect();

    // Pad with generic implications if fewer than 5 were extracted.
    let generics = [
        format!(
            "Further reading on '{topic}' is recommended before making architectural decisions."
        ),
        format!("Compare the gathered sources against current project constraints for '{topic}'."),
        format!("Identify which findings about '{topic}' are supported by in-project evidence."),
        format!("Re-run research on '{topic}' after collecting more targeted sources."),
        format!("Validate any actionable conclusions about '{topic}' with domain experts."),
    ];
    while extracted.len() < 5 {
        let next = generics[extracted.len() % generics.len()].clone();
        if !extracted.contains(&next) {
            extracted.push(next);
        } else {
            break;
        }
    }

    extracted.into_iter().take(5).collect()
}

/// Build a per-source bullet title + short excerpt suitable for embedding
/// in the Findings section when no LLM analysis is available. Returns an
/// empty string when the body is empty / unavailable.
pub fn body_excerpt(body: &str, max_chars: usize) -> String {
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
pub fn default_open_questions(sources: &[Source], topic: &str) -> Vec<String> {
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
pub fn cross_references_from(sources: &[Source]) -> Vec<CrossReference> {
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

// reason: only exercised when a cross-referenced local source carries
// the `Extra` kind; the simpler `InProject` path is taken in practice.
#[allow(dead_code)]
fn format_with_kind(relevance: &str, kind: LocalSourceKind) -> String {
    match kind {
        LocalSourceKind::InProject => relevance.to_string(),
        LocalSourceKind::Extra => format!("{relevance} (from --sources-dir)"),
    }
}
