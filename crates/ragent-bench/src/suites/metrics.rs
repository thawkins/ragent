//! Shared native benchmark metric helpers.

use crate::model::BenchGenerationResult;
use crate::suites::BenchMetricEvaluation;

/// Count exact-match samples against a normalized reference string.
#[must_use]
pub(crate) fn exact_match_count(generation: &BenchGenerationResult, reference: &str) -> usize {
    generation
        .samples
        .iter()
        .filter(|sample| normalized_code(&sample.text) == normalized_code(reference))
        .count()
}

/// Return whether the first sample matches exactly after normalization.
#[must_use]
pub(crate) fn first_sample_exact_match(
    generation: &BenchGenerationResult,
    reference: &str,
) -> bool {
    generation
        .samples
        .first()
        .is_some_and(|sample| normalized_code(&sample.text) == normalized_code(reference))
}

/// Pick the best sample by exact match first, then edit similarity.
#[must_use]
pub(crate) fn best_exact_or_similarity_sample(
    generation: &BenchGenerationResult,
    reference: &str,
) -> (String, f64) {
    generation
        .samples
        .iter()
        .map(|sample| {
            let similarity = edit_similarity(&sample.text, reference);
            let exact = normalized_code(&sample.text) == normalized_code(reference);
            (sample.text.clone(), if exact { 1.0 } else { similarity })
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .unwrap_or_else(|| (String::new(), 0.0))
}

/// Normalize code-like content for text-based comparisons.
#[must_use]
pub(crate) fn normalized_code(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Compute normalized edit similarity in the range `[0, 1]`.
#[must_use]
pub(crate) fn edit_similarity(actual: &str, expected: &str) -> f64 {
    let left = normalized_code(actual);
    let right = normalized_code(expected);
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let left_chars = left.chars().collect::<Vec<_>>();
    let right_chars = right.chars().collect::<Vec<_>>();
    let left_len = left_chars.len();
    let right_len = right_chars.len();
    if left_len == 0 || right_len == 0 {
        return 0.0;
    }

    let mut prev = (0..=right_len).collect::<Vec<_>>();
    let mut curr = vec![0usize; right_len + 1];
    for (i, left_char) in left_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + substitution_cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    let distance = prev[right_len] as f64;
    let longest = left_len.max(right_len) as f64;
    ((longest - distance) / longest).clamp(0.0, 1.0)
}

/// Compute a lightweight CodeBLEU-style token overlap score in the range `[0, 1]`.
#[must_use]
pub(crate) fn codebleu_score(actual: &str, expected: &str) -> f64 {
    if normalized_code(actual) == normalized_code(expected) {
        return 1.0;
    }
    let actual_tokens = tokenize_code(actual);
    let expected_tokens = tokenize_code(expected);
    if actual_tokens.is_empty() && expected_tokens.is_empty() {
        return 1.0;
    }
    if actual_tokens.is_empty() || expected_tokens.is_empty() {
        return 0.0;
    }

    let unigram_precision = overlap_precision(&actual_tokens, &expected_tokens);
    let bigram_precision =
        overlap_precision(&ngrams(&actual_tokens, 2), &ngrams(&expected_tokens, 2));
    let syntax_overlap = syntax_keyword_overlap(&actual_tokens, &expected_tokens);
    let textual_similarity = edit_similarity(actual, expected);
    (unigram_precision * 0.3
        + bigram_precision * 0.2
        + syntax_overlap * 0.2
        + textual_similarity * 0.3)
        .clamp(0.0, 1.0)
}

/// Compute pass@k from the number of generated samples and successes.
#[must_use]
pub(crate) fn pass_at_k(sample_count: usize, successful_samples: usize, k: usize) -> f64 {
    if sample_count == 0 || successful_samples == 0 || k == 0 {
        return 0.0;
    }
    let k = k.min(sample_count);
    if sample_count.saturating_sub(successful_samples) < k {
        return 1.0;
    }

    let total = (sample_count - successful_samples + 1..=sample_count)
        .map(|n| 1.0 - (k as f64 / n as f64))
        .product::<f64>();
    1.0 - total
}

/// Compute a resolution-rate style metric.
#[must_use]
pub(crate) fn resolution_rate(resolved_count: usize, attempted_count: usize) -> f64 {
    if attempted_count == 0 {
        0.0
    } else {
        resolved_count as f64 / attempted_count as f64
    }
}

/// Build a skipped ratio metric row.
#[must_use]
pub(crate) fn skipped_metric(
    metric_name: &str,
    skipped_count: usize,
    notes: &str,
) -> BenchMetricEvaluation {
    BenchMetricEvaluation {
        metric_name: metric_name.to_string(),
        metric_value: 0.0,
        metric_unit: "ratio".to_string(),
        passed_count: Some(0),
        failed_count: Some(0),
        skipped_count: Some(skipped_count),
        notes: notes.to_string(),
    }
}

/// Build an accuracy-style ratio metric row.
#[must_use]
pub(crate) fn accuracy_metric(
    metric_name: &str,
    passed_count: usize,
    failed_count: usize,
    skipped_count: usize,
    notes: &str,
) -> BenchMetricEvaluation {
    let attempted = passed_count + failed_count;
    BenchMetricEvaluation {
        metric_name: metric_name.to_string(),
        metric_value: resolution_rate(passed_count, attempted),
        metric_unit: "ratio".to_string(),
        passed_count: Some(passed_count),
        failed_count: Some(failed_count),
        skipped_count: Some(skipped_count),
        notes: notes.to_string(),
    }
}

/// Build an average-valued metric row.
#[must_use]
pub(crate) fn average_metric(
    metric_name: &str,
    values: &[f64],
    passed_count: usize,
    failed_count: usize,
    skipped_count: usize,
    notes: &str,
) -> BenchMetricEvaluation {
    BenchMetricEvaluation {
        metric_name: metric_name.to_string(),
        metric_value: if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        },
        metric_unit: "ratio".to_string(),
        passed_count: Some(passed_count),
        failed_count: Some(failed_count),
        skipped_count: Some(skipped_count),
        notes: notes.to_string(),
    }
}

fn tokenize_code(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn ngrams(tokens: &[String], n: usize) -> Vec<String> {
    if tokens.len() < n || n == 0 {
        return Vec::new();
    }
    tokens.windows(n).map(|window| window.join(" ")).collect()
}

fn overlap_precision(actual: &[String], expected: &[String]) -> f64 {
    if actual.is_empty() || expected.is_empty() {
        return 0.0;
    }
    let expected_counts =
        expected
            .iter()
            .fold(std::collections::HashMap::new(), |mut counts, token| {
                *counts.entry(token).or_insert(0usize) += 1;
                counts
            });
    let matched = actual
        .iter()
        .fold((0usize, expected_counts), |(matched, mut counts), token| {
            if let Some(count) = counts.get_mut(token)
                && *count > 0
            {
                *count -= 1;
                return (matched + 1, counts);
            }
            (matched, counts)
        });
    matched.0 as f64 / actual.len() as f64
}

fn syntax_keyword_overlap(actual: &[String], expected: &[String]) -> f64 {
    const KEYWORDS: &[&str] = &[
        "def", "fn", "class", "return", "if", "else", "for", "while", "match", "let", "import",
        "from", "where", "print", "raise",
    ];
    let actual_keywords = actual
        .iter()
        .filter(|token| KEYWORDS.contains(&token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let expected_keywords = expected
        .iter()
        .filter(|token| KEYWORDS.contains(&token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if actual_keywords.is_empty() && expected_keywords.is_empty() {
        return 1.0;
    }
    overlap_precision(&actual_keywords, &expected_keywords)
}

#[cfg(test)]
mod tests {
    use super::{codebleu_score, edit_similarity, pass_at_k, resolution_rate};

    #[test]
    fn test_codebleu_score_prefers_similar_code() {
        let exact = codebleu_score("def add(a, b): return a + b", "def add(a, b): return a + b");
        let near = codebleu_score("def add(a, b): return a - b", "def add(a, b): return a + b");
        let far = codebleu_score("print('hello')", "def add(a, b): return a + b");
        assert!(exact > near);
        assert!(near > far);
    }

    #[test]
    fn test_pass_at_k_clamps_k_to_sample_count() {
        assert_eq!(pass_at_k(1, 0, 5), 0.0);
        assert_eq!(pass_at_k(1, 1, 5), 1.0);
    }

    #[test]
    fn test_resolution_rate_handles_empty_attempts() {
        assert_eq!(resolution_rate(0, 0), 0.0);
        assert_eq!(resolution_rate(3, 4), 0.75);
    }

    #[test]
    fn test_edit_similarity_returns_ratio() {
        let exact = edit_similarity("return helper(value)", "return helper(value)");
        let partial = edit_similarity("return helper()", "return helper(value)");
        assert_eq!(exact, 1.0);
        assert!(partial < 1.0);
        assert!(partial > 0.0);
    }
}
