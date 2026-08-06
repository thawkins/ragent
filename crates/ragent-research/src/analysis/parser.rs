//! Response parsing — extract structured `AnalysisResult` from LLM output,
//! validate citations and dates, detect malformed results, and provide
//! mechanical fallback findings.
//!
//! These helpers were previously inline in `analysis.rs`.

use super::{AnalysisOutcome, AnalysisResult, SourceBody};
use crate::document::CrossReference;
use crate::item::strip_control_chars;
use regex::Regex;

pub(crate) fn parse_analysis_response(text: &str) -> AnalysisResult {
    let mut result = AnalysisResult::default();
    let sections = split_sections(text);
    for (title, body) in sections {
        match title.to_lowercase().as_str() {
            "summary" => result.summary = body.trim().to_string(),
            "findings" => {
                let raw = parse_numbered_list(&body);
                result.findings = reorder_findings_by_dependency(&raw);
            }
            "in-project cross-references" | "cross-references" | "cross references" => {
                result.cross_references = parse_cross_reference_list(&body);
            }
            "open questions" => {
                result.open_questions = parse_bullet_list(&body);
            }
            _ => {}
        }
    }
    result
}

/// Parse the LLM response into an [`AnalysisResult`] paired with an
/// [`AnalysisOutcome`] (FR-005 / T-005).
///
/// Runs [`parse_analysis_response`] first. If the result is malformed
/// (see [`is_malformed_analysis_result`]), the mechanical fallback
/// ([`mechanical_fallback_findings`] + a placeholder summary) rescues the
/// raw text into structured findings and the outcome is
/// [`AnalysisOutcome::FallbackEmpty`]; otherwise the outcome is
/// [`AnalysisOutcome::Llm`]. Provider-level errors are surfaced by
/// [`LlmAnalysisEngine::analyze_with_outcome`] as `Err`, which `session.rs`
/// maps to [`crate::session::SynthesizeOutcome::FallbackError`].
pub(crate) fn parse_analysis_response_with_outcome(
    text: &str,
    sources: &[SourceBody],
) -> (AnalysisResult, AnalysisOutcome) {
    let parsed = parse_analysis_response(text);
    if is_malformed_analysis_result(&parsed) {
        // Sanitize the raw model text before mechanical extraction so control
        // characters (C0/C1) from model output don't corrupt the findings.
        let sanitized = strip_control_chars(text);
        let mut rescued = AnalysisResult::default();
        rescued.findings = mechanical_fallback_findings(&sanitized);
        // Preserve the model's own summary when it parsed successfully —
        // discarding a valid summary along with malformed findings loses
        // useful context. Only fall back to a diagnostic placeholder when
        // the summary is also empty.
        rescued.summary = if !parsed.summary.trim().is_empty() {
            parsed.summary
        } else if rescued.findings.is_empty() {
            "(the model response could not be parsed into structured findings; \
             see the raw response below)"
                .to_string()
        } else {
            "(the model response was malformed; the following findings were \
             extracted mechanically and may be incomplete)"
                .to_string()
        };
        (rescued, AnalysisOutcome::FallbackEmpty)
    } else {
        // FR-010 / T-009: validate citations and dates even on a "clean" parse.
        // Out-of-range `[#N]` citations and unsupported date claims are replaced
        // inline with warning placeholders so hallucinated evidence is visible
        // rather than silently propagated.
        let mut validated = parsed;
        let warnings = validate_citations_and_dates(&mut validated.findings, sources);
        if !warnings.is_empty() {
            for w in &warnings {
                tracing::warn!(warning = %w, "research: citation/date validation");
            }
        }
        // Sanitize control characters from the clean parse too — the model
        // output may contain C0/C1 control chars that would corrupt the
        // rendered RESEARCH.md if left in place.
        for finding in &mut validated.findings {
            *finding = strip_control_chars(finding);
        }
        validated.summary = strip_control_chars(&validated.summary);
        validated.open_questions = validated
            .open_questions
            .iter()
            .map(|q| strip_control_chars(q))
            .collect();
        validated.cross_references = validated
            .cross_references
            .iter()
            .map(|cr| CrossReference {
                path: strip_control_chars(&cr.path),
                relevance: strip_control_chars(&cr.relevance),
            })
            .collect();
        (validated, AnalysisOutcome::Llm)
    }
}

/// Validate every `[#N]` citation and claimed publication date in `findings`
/// against the actual `sources` corpus (FR-010 / T-009).
///
/// Mutates `findings` in place:
/// - Out-of-range `[#N]` citations (N == 0 or N > `sources.len()`) are
///   rewritten to `[#N?] (out of range — not in source list)`.
/// - Explicit publication dates in a **Sources Cited / Date Spread** paragraph
///   that do not match any cited source's `published_at` are rewritten to
///   `(unsupported date)`.
///
/// Returns a list of human-readable warning strings (one per invalid claim)
/// so the caller can log them. Findings that pass validation are left
/// untouched.
pub(crate) fn validate_citations_and_dates(
    findings: &mut [String],
    sources: &[SourceBody],
) -> Vec<String> {
    let mut warnings = Vec::new();
    let citation_re = Regex::new(r"\[#(\d+)\]").expect("valid citation regex");
    // Match `published YYYY-MM-DD` or a bare `YYYY-MM-DD` inside a
    // **Sources Cited / Date Spread** paragraph. We keep this conservative
    // so we don't rewrite dates that appear in the Observation/Analysis
    // prose (which may legitimately reference unrelated dates).
    let date_re = Regex::new(r"(\d{4}-\d{2}-\d{2})").expect("valid date regex");
    let valid_dates: Vec<String> = sources
        .iter()
        .filter_map(|s| s.published_at.map(|dt| dt.format("%Y-%m-%d").to_string()))
        .collect();

    for finding in findings.iter_mut() {
        // ── Citation range validation ──────────────────────────────────────
        let mut new_finding = String::with_capacity(finding.len());
        let mut last_end = 0;
        for cap in citation_re.captures_iter(finding) {
            let m = cap.get(0).expect("full match");
            new_finding.push_str(&finding[last_end..m.start()]);
            let n: usize = cap[1].parse().unwrap_or(0);
            if n == 0 || n > sources.len() {
                let replacement = format!("[#{n}?] (out of range — not in source list)");
                new_finding.push_str(&replacement);
                warnings.push(format!(
                    "finding cites [#{n}] but only {} source(s) were captured",
                    sources.len()
                ));
            } else {
                new_finding.push_str(m.as_str());
            }
            last_end = m.end();
        }
        new_finding.push_str(&finding[last_end..]);
        *finding = new_finding;

        // ── Date claim validation (only inside the Sources Cited / Date
        // Spread paragraph, to avoid rewriting prose dates) ─────────────────
        if let Some(spread_start) = finding.find("**Sources Cited / Date Spread:**") {
            let spread = &finding[spread_start..];
            let mut validated_spread = String::with_capacity(spread.len());
            let mut last_end = 0;
            for cap in date_re.captures_iter(spread) {
                let m = cap.get(0).expect("full match");
                validated_spread.push_str(&spread[last_end..m.start()]);
                let claimed = &cap[1];
                if valid_dates.iter().any(|d| d == claimed) {
                    validated_spread.push_str(claimed);
                } else {
                    validated_spread.push_str("(unsupported date)");
                    warnings.push(format!(
                        "finding claims publication date {claimed} which is not among the captured sources' publication dates"
                    ));
                }
                last_end = m.end();
            }
            validated_spread.push_str(&spread[last_end..]);
            let prefix = &finding[..spread_start];
            *finding = format!("{prefix}{validated_spread}");
        }
    }
    warnings
}

/// Return `true` when `result` should be treated as a malformed LLM response
/// (FR-005): empty findings, any finding missing one of the four required
/// bold labels, or any finding that contains no `[#N]` citation.
pub(crate) fn is_malformed_analysis_result(result: &AnalysisResult) -> bool {
    if result.findings.is_empty() {
        return true;
    }
    let required = [
        "**Observation:**",
        "**Analysis:**",
        "**Cross-reference / Dependencies:**",
        "**Implication:**",
    ];
    let citation_re = Regex::new(r"\[#\d+\]").expect("valid citation regex");
    for finding in &result.findings {
        if !required.iter().all(|label| finding.contains(label)) {
            return true;
        }
        if !citation_re.is_match(finding) {
            return true;
        }
    }
    false
}

/// Deterministic mechanical extraction (FR-005) that turns a raw model
/// response into a list of findings, each carrying the four required bold
/// labels. Missing labels are inserted as placeholders; existing labels and
/// any `[#N]` citations are preserved verbatim.
///
/// **Non-empty guarantee (FR-011 / T-010):** this function ALWAYS returns at
/// least one finding. When the raw response has no extractable candidate
/// findings, a single placeholder finding is emitted whose **Observation**
/// paragraph reads "(findings could not be structured — see below)" and
/// includes the raw model output in a fenced code block so the research
/// item remains usable. Callers can rely on `findings.is_empty()` never
/// being true for the returned `Vec`.
///
/// Strategy:
/// 1. If the response contains a `## Findings` section, split its numbered
///    list items; each becomes a candidate finding.
/// 2. Otherwise, fall back to splitting the whole response on blank-line
///    paragraphs (or, if that yields a single blob, wrap the whole text as
///    one finding).
/// 3. For each candidate, ensure the four required labels are present,
///    inserting `**Label:** (missing)` placeholders for any that are absent.
/// 4. If no candidate text could be extracted, return a single placeholder
///    finding that quotes the raw response (truncated) so the research item
///    remains usable.
pub(crate) fn mechanical_fallback_findings(text: &str) -> Vec<String> {
    let candidates = extract_candidate_findings(text);
    let required = [
        "**Observation:**",
        "**Analysis:**",
        "**Cross-reference / Dependencies:**",
        "**Implication:**",
    ];
    let mut findings = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let mut normalized = candidate.trim().to_string();
        for label in required {
            if !normalized.contains(label) {
                let placeholder = format!("\n\n{label} (missing)");
                normalized.push_str(&placeholder);
            }
        }
        findings.push(normalized);
    }
    if findings.is_empty() {
        // FR-011 / T-010: the model returned fewer than one valid finding.
        // The fallback takes precedence — emit a single placeholder finding
        // so RESEARCH.md is never left with an empty Findings section. The
        // raw model output is preserved in a fenced code block for manual
        // review.
        let raw = text.trim();
        if raw.is_empty() {
            findings.push(
                "**Headline:** Findings could not be structured\n\n\
                 **Observation:** (findings could not be structured — see below)\n\n\
                 (no model response was returned)\n\n\
                 **Analysis:** (missing)\n\n\
                 **Cross-reference / Dependencies:** No direct dependencies.\n\n\
                 **Implication:** Re-run `/research create` with a configured \
                 model; the model returned no content to analyze."
                    .to_string(),
            );
        } else {
            let truncated = truncate_body(raw, 2000);
            findings.push(format!(
                "**Headline:** Model response could not be parsed\n\n\
                 **Observation:** (findings could not be structured — see below)\n\n\
                 The raw model response (truncated) is preserved for manual review:\n\n\
                 ```text\n{truncated}\n```\n\n\
                 **Analysis:** (extracted mechanically — the model output did not \
                 contain the four required labeled paragraphs)\n\n\
                 **Cross-reference / Dependencies:** No direct dependencies.\n\n\
                 **Implication:** Re-run `/research create` or refine the topic; \
                 the raw model output is preserved above for manual review.",
            ));
        }
    }
    findings
}

/// Extract candidate finding bodies from a raw model response.
///
/// Prefers numbered items found under a `## Findings` heading; falls back
/// to numbered items anywhere in the response; finally falls back to
/// blank-line-separated paragraphs.
fn extract_candidate_findings(text: &str) -> Vec<String> {
    // 1. Prefer items under a `## Findings` heading.
    let findings_body = split_sections(text)
        .into_iter()
        .find(|(title, _)| title.to_lowercase() == "findings")
        .map(|(_, body)| body);
    if let Some(body) = findings_body {
        let items = parse_numbered_list(&body);
        if !items.is_empty() {
            return items;
        }
        // `## Findings` present but no numbered items — fall through to
        // whole-response strategies.
    }
    // 2. Numbered items anywhere in the response.
    let anywhere = parse_numbered_list(text);
    if !anywhere.is_empty() {
        return anywhere;
    }
    // 3. Blank-line-separated paragraphs (skip headings and rule lines).
    let paragraphs: Vec<String> = text
        .trim()
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with('#') && !p.starts_with("---"))
        .map(str::to_string)
        .collect();
    if paragraphs.is_empty() {
        Vec::new()
    } else {
        paragraphs
    }
}

/// Split a markdown response into (heading, body) pairs based on `## ` H2
/// headings.
fn split_sections(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut current_title = String::new();
    let mut current_body = String::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            if !current_title.is_empty() {
                out.push((current_title.clone(), current_body.clone()));
            }
            current_title = title.trim().to_string();
            current_body.clear();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    if !current_title.is_empty() {
        out.push((current_title, current_body));
    }
    out
}

/// Parse a numbered markdown list (`1. ...`) into plain item strings.
///
/// Handles the common LLM output patterns:
/// * `1. First finding.` — number, dot, space, content on the same line
/// * `1.` followed by blank line and paragraphs — number on its own line,
///   content starts on subsequent lines
pub(crate) fn parse_numbered_list(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let mut is_item = false;
        let mut rest = "";
        if let Some((num_part, after_dot)) = trimmed.split_once(". ") {
            if num_part.parse::<usize>().is_ok() {
                is_item = true;
                rest = after_dot;
            }
        } else if let Some(num_part) = trimmed.strip_suffix('.')
            && !num_part.is_empty()
            && num_part.parse::<usize>().is_ok()
        {
            is_item = true;
            rest = "";
        }
        if is_item {
            if !current.is_empty() {
                items.push(current.trim().to_string());
            }
            current = rest.to_string();
            continue;
        }
        if !trimmed.is_empty() {
            current.push('\n');
            current.push_str(trimmed);
        }
    }
    if !current.is_empty() {
        items.push(current.trim().to_string());
    }
    items
}

/// Reorder findings so any finding that depends on another appears after its
/// dependency, then renumber all internal `Finding N` references consistently.
///
/// The parser receives the raw numbered list in the order the LLM produced it.
/// Often the model lists a child finding before its prerequisite, which makes
/// the final document harder to read. This helper builds a directed graph from
/// the **Cross-reference / Dependencies** paragraph of each finding, topologically
/// sorts it, and rewrites dependency references so they point to the new
/// positions.
///
/// Cycles (e.g. Finding 2 depends on Finding 3 and Finding 3 depends on
/// Finding 2) are broken by falling back to the original order for the involved
/// items.
pub(crate) fn reorder_findings_by_dependency(findings: &[String]) -> Vec<String> {
    if findings.len() <= 1 {
        return findings.to_vec();
    }

    // Build an adjacency list: edge i -> j means finding i depends on finding j,
    // so j must come before i.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); findings.len()];
    let finding_re = Regex::new(r"(?i)\bfinding\s+(\d+)\b").expect("valid regex");
    for (idx, finding) in findings.iter().enumerate() {
        for cap in finding_re.captures_iter(finding) {
            let dep_num: usize = cap[1].parse().unwrap_or(0);
            if dep_num == 0 || dep_num > findings.len() {
                continue;
            }
            let dep_idx = dep_num - 1;
            if dep_idx != idx && !adj[idx].contains(&dep_idx) {
                adj[idx].push(dep_idx);
            }
        }
    }

    // Kahn's algorithm. `in_degree[i]` is the number of dependencies finding i
    // has (the count of edges leaving node i toward its prerequisites).
    let mut in_degree: Vec<usize> = adj.iter().map(std::vec::Vec::len).collect();

    // Roots (no dependencies) keep their original relative order via a FIFO
    // queue. Each queue item is placed before its dependants are released.
    let mut queue: Vec<usize> = (0..findings.len()).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(findings.len());
    let mut processed = vec![false; findings.len()];
    let mut front = 0usize;

    // We built edges as dependant -> dependency. To apply Kahn's we need the
    // reverse graph: dependency -> dependant, so we can decrement in-degrees of
    // dependants once a dependency is placed.
    let mut reverse: Vec<Vec<usize>> = vec![Vec::new(); findings.len()];
    for (idx, deps) in adj.iter().enumerate() {
        for &d in deps {
            reverse[d].push(idx);
        }
    }

    while front < queue.len() {
        let node = queue[front];
        front += 1;
        if processed[node] {
            continue;
        }
        processed[node] = true;
        order.push(node);
        for &dependant in &reverse[node] {
            if processed[dependant] {
                continue;
            }
            in_degree[dependant] -= 1;
            if in_degree[dependant] == 0 {
                queue.push(dependant);
            }
        }
    }

    // If we couldn't place everything, there is a cycle. Append the remaining
    // nodes in original order so we still emit all findings.
    for (i, was_processed) in processed.iter().enumerate() {
        if !was_processed {
            order.push(i);
        }
    }

    // Remap old numbers (1-based, index+1) to new numbers.
    let mut old_to_new = vec![0usize; findings.len()];
    for (new_pos, &old_idx) in order.iter().enumerate() {
        old_to_new[old_idx] = new_pos + 1;
    }

    // Rewrite each finding's Finding N references.
    order
        .into_iter()
        .map(|old_idx| {
            let text = &findings[old_idx];
            let mut out = String::with_capacity(text.len());
            let mut last_end = 0;
            for cap in finding_re.captures_iter(text) {
                let m = cap.get(0).expect("full match");
                out.push_str(&text[last_end..m.start()]);
                let old_num: usize = cap[1].parse().unwrap_or(0);
                if old_num > 0 && old_num <= findings.len() {
                    out.push_str(&format!("Finding {}", old_to_new[old_num - 1]));
                } else {
                    out.push_str(m.as_str());
                }
                last_end = m.end();
            }
            out.push_str(&text[last_end..]);
            out
        })
        .collect()
}

/// Parse a bullet list (`* ...` or `- ...`) into plain item strings.
pub(crate) fn parse_bullet_list(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("* ") || trimmed.starts_with("- ") {
            if !current.is_empty() {
                items.push(current.trim().to_string());
            }
            current = trimmed[2..].trim().to_string();
        } else if !trimmed.is_empty() {
            current.push('\n');
            current.push_str(trimmed);
        }
    }
    if !current.is_empty() {
        items.push(current.trim().to_string());
    }
    items
}

/// Parse cross-reference bullets into [`CrossReference`] structs. Expected
/// format: `* `path` — note` or `* path — note`.
pub(crate) fn parse_cross_reference_list(body: &str) -> Vec<CrossReference> {
    let mut out = Vec::new();
    for item in parse_bullet_list(body) {
        let (path, relevance) = if let Some(idx) = item.find(" — ") {
            let split_at = idx + " — ".len();
            (
                item[..idx].trim().to_string(),
                item[split_at..].trim().to_string(),
            )
        } else {
            (item.clone(), String::new())
        };
        let path = path.trim_matches('`').to_string();
        out.push(CrossReference { path, relevance });
    }
    out
}

/// Truncate a source body to a character budget so the prompt fits in common
/// context windows. The limit is approximate and errs on the side of inclusion.
pub(crate) fn truncate_body(body: &str, max_chars: usize) -> String {
    if body.chars().count() <= max_chars {
        return body.to_string();
    }
    let mut out = String::with_capacity(max_chars);
    for (count, ch) in body.chars().enumerate() {
        if count >= max_chars {
            out.push_str("\n\n… (truncated for prompt size)");
            break;
        }
        out.push(ch);
    }
    out
}
