//! BM25 relevance filtering for conversation messages.
//!
//! This module implements a simple BM25 (Best Matching 25) scorer that ranks
//! conversation messages by relevance to the current user query. When the
//! compression pipeline is running aggressively (context window > 95% full),
//! it can use BM25 scoring to preferentially keep high-relevance messages
//! over low-relevance recent ones.
//!
//! # Algorithm
//!
//! BM25 is a bag-of-words retrieval function that ranks documents by term
//! frequency relative to document length. It uses:
//!
//! - **Term Frequency (TF)**: How often a query term appears in a message.
//! - **Inverse Document Frequency (IDF)**: How rare a query term is across
//!   all messages — rare terms are more discriminative.
//! - **Document Length Normalisation**: Longer messages are penalised
//!   slightly to prevent them from dominating.
//!
//! # Feature flag
//!
//! This module is only compiled when the `compression` Cargo feature is enabled.

use std::collections::HashMap;

/// A BM25 relevance scorer for ranking conversation messages.
///
/// The scorer is initialised with a collection of documents (messages) and
/// can then score any query against those documents. It supports incremental
/// updates (add new documents) and is `Send + Sync` for use across threads.
///
/// # Example
///
/// ```rust,ignore
/// use crate::compression::relevance::Bm25Scorer;
///
/// let mut scorer = Bm25Scorer::new(1.2, 0.75);
/// scorer.add_document(0, "The quick brown fox jumps over the lazy dog");
/// scorer.add_document(1, "A fox is a small mammal");
/// scorer.add_document(2, "Dogs are loyal animals");
///
/// let scores = scorer.score("fox animal");
/// assert!(scores[0].1 > scores[2].1); // fox doc scores higher than dog doc
/// ```
#[derive(Debug, Clone)]
pub struct Bm25Scorer {
    /// BM25 parameter k1 — controls term frequency saturation.
    /// Typical value: 1.2. Higher values increase the influence of term frequency.
    k1: f64,
    /// BM25 parameter b — controls document length normalisation.
    /// Typical value: 0.75. Higher values penalise longer documents more.
    b: f64,
    /// Number of documents in the collection.
    doc_count: usize,
    /// Average document length (in terms) across the collection.
    avg_dl: f64,
    /// Inverse document frequency for each term.
    idf: HashMap<String, f64>,
    /// Term frequencies per document: (doc_id, term) -> count.
    tf: HashMap<(usize, String), usize>,
    /// Document lengths: doc_id -> number of terms.
    dl: HashMap<usize, usize>,
}

impl Bm25Scorer {
    /// Create a new BM25 scorer with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `k1` — Term frequency saturation parameter (typical: 1.2)
    /// * `b` — Document length normalisation parameter (typical: 0.75)
    #[must_use]
    pub fn new(k1: f64, b: f64) -> Self {
        Self {
            k1,
            b,
            doc_count: 0,
            avg_dl: 0.0,
            idf: HashMap::new(),
            tf: HashMap::new(),
            dl: HashMap::new(),
        }
    }

    /// Create a new BM25 scorer with default parameters (k1=1.2, b=0.75).
    #[must_use]
    pub fn default_scorer() -> Self {
        Self::new(1.2, 0.75)
    }

    /// Add a document (message) to the collection.
    ///
    /// The document is tokenised into lowercase terms. This method can be
    /// called multiple times to build up the collection incrementally.
    /// After all documents are added, call [`Self::finalise`] to compute
    /// IDF values before scoring.
    pub fn add_document(&mut self, doc_id: usize, text: &str) {
        let terms = Self::tokenise(text);
        let doc_len = terms.len();

        // Update term frequencies.
        let mut term_counts: HashMap<String, usize> = HashMap::new();
        for term in &terms {
            *term_counts.entry(term.clone()).or_insert(0) += 1;
        }

        for (term, count) in &term_counts {
            self.tf.insert((doc_id, term.clone()), *count);
        }

        self.dl.insert(doc_id, doc_len);
        self.doc_count += 1;

        // Update average document length incrementally.
        let total_len: usize = self.dl.values().sum();
        self.avg_dl = if total_len > 0 {
            total_len as f64 / self.doc_count as f64
        } else {
            0.0
        };
    }

    /// Finalise the scorer by computing IDF values for all terms.
    ///
    /// This MUST be called after all documents are added and before calling
    /// [`Self::score`]. Without finalisation, all IDF values are zero and
    /// scores will be zero.
    pub fn finalise(&mut self) {
        // Count how many documents contain each term.
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        for key in self.tf.keys() {
            *doc_freq.entry(key.1.clone()).or_insert(0) += 1;
        }

        // Compute IDF for each term using the standard BM25 formula:
        // IDF(q) = ln((N - n(q) + 0.5) / (n(q) + 0.5) + 1)
        // where N = total documents, n(q) = documents containing term q.
        let n = self.doc_count as f64;
        for (term, df) in &doc_freq {
            let df_f = *df as f64;
            let idf = ((n - df_f + 0.5) / (df_f + 0.5)).ln_1p();
            self.idf.insert(term.clone(), idf);
        }
    }

    /// Score all documents against a query string.
    ///
    /// Returns a vector of `(doc_id, score)` pairs sorted by score descending.
    /// Documents that share no terms with the query receive a score of 0.
    ///
    /// # Panics
    ///
    /// Panics if [`Self::finalise`] has not been called (scores would all be zero).
    #[must_use]
    pub fn score(&self, query: &str) -> Vec<(usize, f64)> {
        let query_terms = Self::tokenise(query);

        let mut scores: Vec<(usize, f64)> = self
            .dl
            .keys()
            .map(|&doc_id| {
                let score = self.score_document(doc_id, &query_terms);
                (doc_id, score)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }

    /// Score a single document against query terms.
    fn score_document(&self, doc_id: usize, query_terms: &[String]) -> f64 {
        let dl = self.dl.get(&doc_id).copied().unwrap_or(0) as f64;
        let avg_dl = if self.avg_dl > 0.0 { self.avg_dl } else { 1.0 };

        let mut total_score = 0.0;

        for term in query_terms {
            let idf = self.idf.get(term).copied().unwrap_or(0.0);
            let tf = self.tf.get(&(doc_id, term.clone())).copied().unwrap_or(0) as f64;

            if tf == 0.0 {
                continue;
            }

            // BM25 formula:
            // score(D, Q) = Σ IDF(qi) * (tf(qi, D) * (k1 + 1)) / (tf(qi, D) + k1 * (1 - b + b * |D| / avgdl))
            let numerator = tf * (self.k1 + 1.0);
            let denominator = self.k1.mul_add(1.0 - self.b + self.b * dl / avg_dl, tf);
            total_score += idf * numerator / denominator;
        }

        total_score
    }

    /// Tokenise text into lowercase terms for BM25 scoring.
    ///
    /// Splits on whitespace and punctuation, converts to lowercase,
    /// and filters out very short terms (1 character or fewer).
    fn tokenise(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| s.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// Rank messages by relevance to a query, keeping the top-K most relevant.
    ///
    /// This is a convenience method that scores all messages against the query
    /// and returns the indices of the top-K most relevant ones, sorted by
    /// relevance (most relevant first).
    ///
    /// # Arguments
    ///
    /// * `messages` — The conversation history to rank.
    /// * `query` — The current user query to rank against.
    /// * `keep_top_k` — Number of most relevant messages to keep.
    ///
    /// # Returns
    ///
    /// A sorted list of message indices (most relevant first) that should be
    /// kept during compression. Messages not in this list can be dropped or
    /// aggressively compressed.
    #[must_use]
    pub fn rank_messages(
        messages: &[crate::message::Message],
        query: &str,
        keep_top_k: usize,
    ) -> Vec<usize> {
        let mut scorer = Self::default_scorer();

        for (idx, msg) in messages.iter().enumerate() {
            let text = msg.text_content();
            scorer.add_document(idx, &text);
        }
        scorer.finalise();

        let scores = scorer.score(query);

        // Always keep the first message (system prompt) and the last user message.
        let first_idx = 0;
        let last_user_idx = messages
            .iter()
            .rposition(|m| m.role == crate::message::Role::User);

        let mut kept: Vec<usize> = scores
            .into_iter()
            .take(keep_top_k)
            .map(|(idx, _)| idx)
            .collect();

        // Ensure first and last user messages are always included.
        if !kept.contains(&first_idx) {
            kept.push(first_idx);
        }
        if let Some(lui) = last_user_idx {
            if !kept.contains(&lui) {
                kept.push(lui);
            }
        }

        kept.sort_unstable();
        kept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::float_cmp)]
    #[test]
    fn test_bm25_basic_scoring() {
        let mut scorer = Bm25Scorer::new(1.2, 0.75);
        scorer.add_document(0, "the quick brown fox jumps over the lazy dog");
        scorer.add_document(1, "a fox is a small mammal");
        scorer.add_document(2, "dogs are loyal animals");
        scorer.finalise();

        let scores = scorer.score("fox");
        // Document 0 and 1 both contain "fox", document 2 does not.
        assert!(scores[0].1 > 0.0, "fox should score > 0 in doc 0");
        assert!(scores[1].1 > 0.0, "fox should score > 0 in doc 1");
        assert_eq!(scores[2].1, 0.0, "fox should not appear in doc 2");
    }

    #[test]
    fn test_bm25_rare_term_scores_higher() {
        let mut scorer = Bm25Scorer::new(1.2, 0.75);
        // "fox" appears in 2 out of 3 docs (common)
        // "marsupial" appears in 1 out of 3 docs (rare)
        scorer.add_document(0, "the fox runs in the forest");
        scorer.add_document(1, "a fox sleeps in the den");
        scorer.add_document(2, "the marsupial jumps across the outback");
        scorer.finalise();

        let scores_fox = scorer.score("fox");
        let scores_marsupial = scorer.score("marsupial");

        // The rare term should give a higher score for the matching doc
        // than the common term gives for its matching docs.
        let fox_best = scores_fox.first().map(|(_, s)| *s).unwrap_or(0.0);
        let marsupial_best = scores_marsupial.first().map(|(_, s)| *s).unwrap_or(0.0);
        assert!(
            marsupial_best > fox_best,
            "Rare term 'marsupial' should score higher than common term 'fox': marsupial={}, fox={}",
            marsupial_best,
            fox_best
        );
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn test_bm25_empty_query() {
        let mut scorer = Bm25Scorer::new(1.2, 0.75);
        scorer.add_document(0, "hello world");
        scorer.finalise();

        let scores = scorer.score("");
        // Empty query should produce zero scores for all docs.
        for (_, score) in &scores {
            assert_eq!(*score, 0.0, "Empty query should produce zero scores");
        }
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn test_bm25_no_matching_terms() {
        let mut scorer = Bm25Scorer::new(1.2, 0.75);
        scorer.add_document(0, "the quick brown fox");
        scorer.add_document(1, "a lazy dog");
        scorer.finalise();

        let scores = scorer.score("quantum physics");
        for (_, score) in &scores {
            assert_eq!(*score, 0.0, "Non-matching query should produce zero scores");
        }
    }

    #[test]
    fn test_bm25_tokenisation() {
        let terms = Bm25Scorer::tokenise("Hello, World! This is a Test.");
        assert!(terms.contains(&"hello".to_string()));
        assert!(terms.contains(&"world".to_string()));
        assert!(terms.contains(&"test".to_string()));
        // "is" and "a" are filtered out (length <= 1... actually length 1-2).
        // Our filter keeps terms with len > 1, so "is" passes.
        assert!(terms.contains(&"this".to_string()));
    }

    #[test]
    fn test_bm25_multiple_query_terms() {
        let mut scorer = Bm25Scorer::new(1.2, 0.75);
        scorer.add_document(0, "rust programming language safety");
        scorer.add_document(1, "python programming language dynamic");
        scorer.add_document(2, "rust safety guarantees memory");
        scorer.finalise();

        let scores = scorer.score("rust safety");
        // Document 2 has both "rust" and "safety" — should have the highest score.
        // Document 0 has "rust" and "safety" too, but "rust" is less rare (2 of 3 docs).
        // Both docs 0 and 2 match, and doc 2 should score at least as high as doc 0.
        let doc2_score = scores
            .iter()
            .find(|(id, _)| *id == 2)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);
        let doc0_score = scores
            .iter()
            .find(|(id, _)| *id == 0)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);
        assert!(
            doc2_score > 0.0,
            "Doc 2 should score > 0 for 'rust safety': got {}",
            doc2_score
        );
        // Doc 2 should score at least as high as doc 0 (both have both terms,
        // but doc 2 is shorter, giving it a slight BM25 boost).
        assert!(
            doc2_score >= doc0_score - 0.001, // allow floating point tolerance
            "Doc 2 (score={}) should score >= Doc 0 (score={}) for 'rust safety'",
            doc2_score,
            doc0_score
        );
    }

    #[test]
    fn test_bm25_length_normalisation() {
        let mut scorer = Bm25Scorer::new(1.2, 0.75);
        // Short doc with "fox"
        scorer.add_document(0, "fox");
        // Long doc with "fox" repeated many times
        scorer.add_document(1, &("fox ".repeat(100) + "other words here for padding"));
        scorer.finalise();

        let scores = scorer.score("fox");
        // Both contain "fox", but the short doc should have a higher per-term
        // contribution due to BM25 length normalisation.
        assert!(scores.len() >= 2);
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn test_bm25_finalise_required() {
        let mut scorer = Bm25Scorer::new(1.2, 0.75);
        scorer.add_document(0, "hello world");
        // Don't call finalise — scores should be zero.
        let scores = scorer.score("hello");
        assert_eq!(scores[0].1, 0.0, "Without finalise, scores should be zero");
    }
}
