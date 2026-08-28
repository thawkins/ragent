//! robots.txt compliance for the masterfetch toolset.
//!
//! Implements **FR-028** and **NFR-003**.
//!
//! When the `mf_fetch` or `mf_crawl` tool receives `respect_robots = true`,
//! the system fetches and parses the target domain's `robots.txt`, caches the
//! result per-domain (TTL 3600 seconds), and refuses to fetch URLs disallowed
//! by the robots policy. When `respect_robots` is `false` (default), the check
//! is skipped entirely.
//!
//! This module is a native Rust port of Hound's `robots.py`, which itself wraps
//! Python's `urllib.robotparser`. The parsing logic follows [RFC 9309]
//! (Robots Exclusion Protocol) with the following simplifications appropriate
//! for an HTTP-only runtime:
//!
//! - **Allow-by-default**: if `robots.txt` is unreachable, returns `404`, or
//!   fails to parse, the URL is allowed. This matches Hound and the RFC's
//!   "no robots.txt → allow all" stance.
//! - **Path matching**: simple prefix matching with wildcard `*` and `$`
//!   end-of-path anchor support (the two special characters defined by
//!   RFC 9309 § 2.2.2). No regex engine is used.
//! - **Most-specific group wins**: rules are evaluated from the most specific
//!   matching user-agent group to the least specific (`*`).
//! - **Allow overrides Disallow**: when both an `Allow` and `Disallow` rule
//!   match a path, the longer (more specific) pattern wins, per RFC 9309
//!   § 2.2.2.
//!
//! # Design — pure vs. network
//!
//! To satisfy NFR-003 (testability without network), the module separates pure
//! parsing from network I/O:
//!
//! - [`RobotsRules`] — the parsed, in-memory representation of a `robots.txt`
//!   file. Fully testable without any network call.
//! - [`parse_robots_txt`] — a pure function that parses raw `robots.txt` text
//!   into [`RobotsRules`]. Has no side effects and no I/O.
//! - [`RobotsCache`] — a per-domain cache with TTL, holding parsed rules and
//!   their fetch timestamp. Pure in-memory, testable with injected rules.
//! - [`RobotsChecker`] — the orchestrator that fetches `robots.txt` over HTTP,
//!   parses it, caches it, and answers `is_allowed` queries. The network fetch
//!   is the only I/O; tests that exercise it are gated with `#[ignore]`.
//!
//! [RFC 9309]: https://www.rfc-editor.org/rfc/rfc9309.html
//!
//! # Examples
//!
//! Pure parsing (no network):
//!
//! ```
//! use ragent_tools_extended::masterfetch::robots::{RobotsRules, parse_robots_txt};
//!
//! let raw = "User-agent: *\nDisallow: /private/\nAllow: /private/public/\n";
//! let rules = parse_robots_txt(raw);
//! assert!(!rules.is_allowed("*", "/private/secret.html"));
//! assert!(rules.is_allowed("*", "/private/public/page.html"));
//! assert!(rules.is_allowed("*", "/index.html"));
//! ```
//!
//! Allow-by-default when no rules exist:
//!
//! ```
//! use ragent_tools_extended::masterfetch::robots::RobotsRules;
//!
//! let rules = RobotsRules::default();
//! assert!(rules.is_allowed("MyBot", "/anything"));
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use thiserror::Error;
use url::Url;

// ---------------------------------------------------------------------------
// Constants (FR-028)
// ---------------------------------------------------------------------------

/// Per-domain cache TTL in seconds (FR-028: 3600 = 1 hour).
pub const ROBOTS_CACHE_TTL: Duration = Duration::from_hours(1);

/// Timeout for fetching `robots.txt` (seconds).
pub const ROBOTS_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Default user-agent string used when fetching and checking `robots.txt`.
///
/// Matches the masterfetch HTTP client's `User-Agent` (FR-025) so that
/// `robots.txt` rules targeting the fetcher's UA are evaluated correctly.
pub const DEFAULT_USER_AGENT: &str = "*";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error returned when a URL cannot be processed by the robots checker.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RobotsError {
    /// The URL failed to parse.
    #[error("URL parse error: {0}")]
    Parse(String),
    /// The URL has no host (e.g. a relative URL or `file:///` path).
    #[error("URL has no host")]
    NoHost,
    /// The URL uses a scheme other than `http` or `https`.
    #[error("Unsupported scheme: '{0}' — only http and https are allowed")]
    UnsupportedScheme(String),
}

// ---------------------------------------------------------------------------
// RobotsRules — parsed robots.txt (pure, no I/O)
// ---------------------------------------------------------------------------

/// A single path rule from a `robots.txt` `Allow` or `Disallow` directive.
///
/// The `pattern` is matched against the URL path (and query string if present
/// in the rule) using RFC 9309 § 2.2.2 wildcard matching: `*` matches any
/// sequence of characters and `$` anchors the end of the path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PathRule {
    /// `true` for `Allow`, `false` for `Disallow`.
    allow: bool,
    /// The path pattern (e.g. `/private/`, `/admin/*`).
    pattern: String,
}

/// A group of rules targeting a specific user-agent (or `*` for all).
///
/// Each group begins with one or more `User-agent:` lines and is followed by
/// `Allow:` / `Disallow:` lines. Multiple consecutive `User-agent:` lines
/// share the same rule block (RFC 9309 § 2.2.1).
#[derive(Debug, Clone, Default)]
struct RuleGroup {
    /// User-agent strings this group applies to (lowercased).
    /// Empty means the global `*` group.
    user_agents: Vec<String>,
    /// Path rules in the order they appear in the file.
    rules: Vec<PathRule>,
}

/// The parsed, in-memory representation of a `robots.txt` file.
///
/// Produced by [`parse_robots_txt`]. Holds zero or more [`RuleGroup`]s and a
/// flag indicating whether the file was empty / absent (which means
/// allow-all).
///
/// This struct is pure — it has no I/O and can be freely constructed and
/// tested in isolation (NFR-003).
#[derive(Debug, Clone, Default)]
pub struct RobotsRules {
    /// Rule groups in file order.
    groups: Vec<RuleGroup>,
    /// Crawl-delay hints per user-agent (seconds), if specified.
    /// Key is the lowercased user-agent or `*`.
    crawl_delays: HashMap<String, f64>,
    /// `true` when the source `robots.txt` was empty or whitespace-only.
    /// Empty files mean "allow all" per RFC 9309.
    is_empty: bool,
}

impl RobotsRules {
    /// Check whether `user_agent` is allowed to fetch `path` according to
    /// these rules.
    ///
    /// # Algorithm (RFC 9309 § 2.2.2)
    ///
    /// 1. Find the most specific group matching `user_agent`. Specificity:
    ///    an exact (case-insensitive) match beats `*`. If no exact match
    ///    exists, the `*` group is used. If neither exists, allow.
    /// 2. Within the matching group, evaluate all `Allow` and `Disallow`
    ///    rules. The most specific (longest) matching pattern wins. If an
    ///    `Allow` and `Disallow` pattern have the same length, `Allow` wins.
    /// 3. If no rule matches, the path is allowed.
    ///
    /// # Arguments
    ///
    /// - `user_agent` — the bot's user-agent token (e.g. `"MyBot"`). Use `"*"`
    ///   to check against the wildcard group.
    /// - `path` — the URL path (and optionally query) to check, e.g.
    ///   `/private/page.html`.
    ///
    /// # Returns
    ///
    /// `true` if the path is allowed (or no rules apply), `false` if
    /// explicitly disallowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_tools_extended::masterfetch::robots::parse_robots_txt;
    ///
    /// let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\n");
    /// assert!(!rules.is_allowed("*", "/private/secret.html"));
    /// assert!(rules.is_allowed("*", "/public/page.html"));
    /// ```
    #[must_use]
    pub fn is_allowed(&self, user_agent: &str, path: &str) -> bool {
        // Empty rules → allow all (RFC 9309: no robots.txt = allow).
        if self.is_empty {
            return true;
        }

        let ua_lower = user_agent.to_ascii_lowercase();
        let group = self.find_group(&ua_lower);

        let Some(group) = group else {
            // No matching group → allow.
            return true;
        };

        // Evaluate rules: find the most specific matching rule.
        // Specificity = pattern length (longer = more specific).
        // Ties broken by Allow winning over Disallow (RFC 9309 § 2.2.2).
        let mut best_match: Option<(&PathRule, usize)> = None;

        for rule in &group.rules {
            if path_matches(&rule.pattern, path) {
                let specificity = rule.pattern.len();
                match &best_match {
                    None => best_match = Some((rule, specificity)),
                    Some((_, best_len)) => {
                        if specificity > *best_len || (specificity == *best_len && rule.allow) {
                            best_match = Some((rule, specificity));
                        }
                    }
                }
            }
        }

        match best_match {
            Some((rule, _)) => rule.allow,
            None => true, // No rule matched → allow.
        }
    }

    /// Find the most specific rule group matching `user_agent` (lowercased).
    ///
    /// Returns the first exact-match group, or the first `*` group, or `None`.
    fn find_group(&self, ua_lower: &str) -> Option<&RuleGroup> {
        // First pass: exact match (case-insensitive).
        let mut wildcard_group: Option<&RuleGroup> = None;
        for group in &self.groups {
            for agent in &group.user_agents {
                if agent == ua_lower {
                    return Some(group);
                }
                if agent == "*" && wildcard_group.is_none() {
                    wildcard_group = Some(group);
                }
            }
        }
        wildcard_group
    }

    /// Return the crawl-delay (in seconds) for `user_agent`, if specified.
    ///
    /// Falls back to the `*` group's delay if the exact UA has no delay.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_tools_extended::masterfetch::robots::parse_robots_txt;
    ///
    /// let rules = parse_robots_txt("User-agent: *\nCrawl-delay: 5\n");
    /// assert_eq!(rules.crawl_delay("MyBot"), Some(5.0));
    /// assert_eq!(rules.crawl_delay("*"), Some(5.0));
    /// ```
    #[must_use]
    pub fn crawl_delay(&self, user_agent: &str) -> Option<f64> {
        let ua_lower = user_agent.to_ascii_lowercase();
        if let Some(&delay) = self.crawl_delays.get(&ua_lower) {
            return Some(delay);
        }
        self.crawl_delays.get("*").copied()
    }

    /// Returns `true` if the parsed `robots.txt` was empty or whitespace-only.
    ///
    /// Empty rules mean "allow all" per RFC 9309.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.is_empty
    }

    /// Returns the number of rule groups parsed from the file.
    #[must_use]
    pub const fn group_count(&self) -> usize {
        self.groups.len()
    }
}

// ---------------------------------------------------------------------------
// parse_robots_txt — pure parser (no I/O, NFR-003)
// ---------------------------------------------------------------------------

/// Parse raw `robots.txt` text into [`RobotsRules`].
///
/// This is a pure function with no side effects and no I/O (NFR-003). It
/// handles:
///
/// - `User-agent:` lines (grouping consecutive UA lines, RFC 9309 § 2.2.1)
/// - `Disallow:` and `Allow:` path rules
/// - `Crawl-delay:` directives
/// - Comments (`#` to end of line)
/// - Case-insensitive directive names
/// - Empty / whitespace-only files (→ allow-all rules)
/// - Leading/trailing whitespace on values
///
/// Unrecognised directives are silently ignored (per RFC 9309 § 2.2.5).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::robots::parse_robots_txt;
///
/// let raw = "\
/// User-agent: BadBot
/// Disallow: /
///
/// User-agent: *
/// Disallow: /private/
/// Allow: /private/public/
/// Crawl-delay: 5
/// ";
/// let rules = parse_robots_txt(raw);
/// assert!(!rules.is_allowed("BadBot", "/index.html"));
/// assert!(!rules.is_allowed("*", "/private/secret.html"));
/// assert!(rules.is_allowed("*", "/private/public/page.html"));
/// assert_eq!(rules.crawl_delay("*"), Some(5.0));
/// ```
#[must_use]
pub fn parse_robots_txt(raw: &str) -> RobotsRules {
    let mut groups: Vec<RuleGroup> = Vec::new();
    let mut crawl_delays: HashMap<String, f64> = HashMap::new();
    let mut current_agents: Vec<String> = Vec::new();
    let mut current_rules: Vec<PathRule> = Vec::new();
    // Whether we've seen any directive (Allow/Disallow/Crawl-delay) for the
    // current group. Used to detect when a new User-agent line starts a new
    // group vs. shares rules with the previous consecutive User-agent lines.
    let mut current_group_started = false;
    let mut have_directives = false;

    // Flush the current group into `groups`.
    let flush = |agents: &mut Vec<String>,
                 rules: &mut Vec<PathRule>,
                 started: &mut bool,
                 groups: &mut Vec<RuleGroup>| {
        if !agents.is_empty() {
            groups.push(RuleGroup {
                user_agents: std::mem::take(agents),
                rules: std::mem::take(rules),
            });
        } else if !rules.is_empty() {
            // Rules with no preceding User-agent line → attach to `*`.
            groups.push(RuleGroup {
                user_agents: vec!["*".to_string()],
                rules: std::mem::take(rules),
            });
        }
        *started = false;
    };

    for line in raw.lines() {
        // Strip comments.
        let line = match line.find('#') {
            Some(idx) => &line[..idx],
            None => line,
        };
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        // Split into directive and value on the first `:`.
        let Some((directive, value)) = line.split_once(':') else {
            // No colon → ignore the line.
            continue;
        };

        let directive = directive.trim().to_ascii_lowercase();
        let value = value.trim();

        match directive.as_str() {
            "user-agent" => {
                // If the previous group already has directives (rules or
                // crawl-delay), a new User-agent line starts a new group.
                // Consecutive User-agent lines (before any directives) share
                // the same group (RFC 9309 § 2.2.1).
                if current_group_started {
                    flush(
                        &mut current_agents,
                        &mut current_rules,
                        &mut current_group_started,
                        &mut groups,
                    );
                }
                current_agents.push(value.to_ascii_lowercase());
                have_directives = true;
            }
            "disallow" => {
                // An empty Disallow value means "allow all" — we record it
                // as an empty pattern that matches nothing (per RFC 9309).
                if value.is_empty() {
                    // Empty Disallow = no restriction. Record as an Allow
                    // rule with empty pattern so it doesn't block anything.
                    current_rules.push(PathRule {
                        allow: true,
                        pattern: String::new(),
                    });
                } else {
                    current_rules.push(PathRule {
                        allow: false,
                        pattern: value.to_string(),
                    });
                }
                current_group_started = true;
                have_directives = true;
            }
            "allow" => {
                current_rules.push(PathRule {
                    allow: true,
                    pattern: value.to_string(),
                });
                current_group_started = true;
                have_directives = true;
            }
            "crawl-delay" => {
                if let Ok(delay) = value.parse::<f64>() {
                    // Crawl-delay applies to the most recently declared
                    // user-agent(s). If none, apply to `*`.
                    if current_agents.is_empty() {
                        crawl_delays.insert("*".to_string(), delay);
                    } else {
                        for agent in &current_agents {
                            crawl_delays.insert(agent.clone(), delay);
                        }
                    }
                    current_group_started = true;
                }
                have_directives = true;
            }
            // Unrecognised directives (Sitemap, Host, etc.) are ignored.
            _ => {}
        }
    }

    // Flush the final group.
    flush(
        &mut current_agents,
        &mut current_rules,
        &mut current_group_started,
        &mut groups,
    );

    let is_empty = !have_directives && groups.is_empty();

    RobotsRules {
        groups,
        crawl_delays,
        is_empty,
    }
}

// ---------------------------------------------------------------------------
// Path matching (RFC 9309 § 2.2.2)
// ---------------------------------------------------------------------------

/// Check whether `path` matches a robots.txt `pattern`.
///
/// Supports the two special characters defined by RFC 9309:
///
/// - `*` — matches any sequence of zero or more characters.
/// - `$` — when it appears at the end of the pattern, anchors the match to
///   the end of the path. Without `$`, the pattern is a prefix match.
///
/// An empty pattern matches nothing (used for "empty Disallow = allow all").
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::robots::path_matches;
///
/// assert!(path_matches("/private/", "/private/secret.html"));
/// assert!(path_matches("/admin/*", "/admin/users/list"));
/// assert!(path_matches("/page$", "/page"));
/// assert!(!path_matches("/page$", "/page/sub"));
/// ```
#[must_use]
pub fn path_matches(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    // Optimisation: no wildcards → simple prefix match (common case).
    if !pattern.contains('*') && !pattern.contains('$') {
        return path.starts_with(pattern);
    }

    // Full wildcard matching with `*` and `$`.
    wildcard_match(pattern, path)
}

/// Wildcard match implementing `*` (any sequence) and `$` (end anchor).
///
/// Uses a standard two-pointer dynamic-programming approach for glob-style
/// matching with a single wildcard character.
fn wildcard_match(pattern: &str, path: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = path.chars().collect();
    let (m, n) = (pat.len(), s.len());

    // dp[i][j] = true if pattern[0..i] matches path[0..j].
    let mut dp = vec![vec![false; n + 1]; m + 1];
    dp[0][0] = true;

    // Pattern prefixes that are all `*` match empty string.
    for i in 1..=m {
        if pat[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        } else {
            break;
        }
    }

    for i in 1..=m {
        for j in 1..=n {
            match pat[i - 1] {
                '*' => {
                    // `*` matches zero chars (dp[i-1][j]) or more (dp[i][j-1]).
                    dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
                }
                '$' if i == m => {
                    // `$` only at end of pattern: matches only if we've
                    // consumed the entire path.
                    dp[i][j] = j == n && dp[i - 1][j];
                }
                c => {
                    dp[i][j] = dp[i - 1][j - 1] && c == s[j - 1];
                }
            }
        }
    }

    dp[m][n]
}

// ---------------------------------------------------------------------------
// RobotsCache — per-domain cache with TTL (FR-028)
// ---------------------------------------------------------------------------

/// A cached entry holding parsed rules and the time they were fetched.
#[derive(Debug, Clone)]
struct CacheEntry {
    rules: RobotsRules,
    fetched_at: Instant,
}

/// Per-domain cache of parsed `robots.txt` rules with TTL (FR-028).
///
/// The cache is keyed by domain (lowercased host). Each entry holds the parsed
/// [`RobotsRules`] and the [`Instant`] it was fetched. Entries older than
/// [`ROBOTS_CACHE_TTL`] are considered stale and must be re-fetched.
///
/// This struct is pure in-memory (no I/O) and can be tested by injecting
/// pre-parsed rules (NFR-003).
///
/// # Examples
///
/// ```
/// use std::time::{Duration, Instant};
/// use ragent_tools_extended::masterfetch::robots::{
///     RobotsCache, RobotsRules, ROBOTS_CACHE_TTL,
/// };
///
/// let mut cache = RobotsCache::new();
/// let rules = RobotsRules::default();
/// cache.insert("example.com", rules.clone());
///
/// // Fresh entry → returns the rules.
/// assert!(cache.get("example.com").is_some());
///
/// // Simulate expiry.
/// cache.insert_with_timestamp("example.com", rules, Instant::now() - ROBOTS_CACHE_TTL);
/// assert!(cache.get("example.com").is_none()); // expired
/// ```
#[derive(Debug, Clone, Default)]
pub struct RobotsCache {
    entries: HashMap<String, CacheEntry>,
}

impl RobotsCache {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get cached rules for `domain` if the entry is still fresh.
    ///
    /// Returns `None` if the domain is not cached or the entry has exceeded
    /// [`ROBOTS_CACHE_TTL`]. The lookup is case-insensitive (the domain is
    /// lowercased before lookup).
    ///
    /// # Arguments
    ///
    /// - `domain` — the domain (host) to look up. Case is normalised
    ///   internally.
    #[must_use]
    pub fn get(&self, domain: &str) -> Option<&RobotsRules> {
        let key = domain.to_ascii_lowercase();
        let entry = self.entries.get(&key)?;
        if entry.fetched_at.elapsed() >= ROBOTS_CACHE_TTL {
            return None;
        }
        Some(&entry.rules)
    }

    /// Insert parsed rules for `domain` with the current timestamp.
    pub fn insert(&mut self, domain: &str, rules: RobotsRules) {
        self.entries.insert(
            domain.to_ascii_lowercase(),
            CacheEntry {
                rules,
                fetched_at: Instant::now(),
            },
        );
    }

    /// Insert parsed rules for `domain` with a specific fetch timestamp.
    ///
    /// This is primarily for testing TTL expiry without waiting (NFR-003).
    pub fn insert_with_timestamp(&mut self, domain: &str, rules: RobotsRules, fetched_at: Instant) {
        self.entries.insert(
            domain.to_ascii_lowercase(),
            CacheEntry { rules, fetched_at },
        );
    }

    /// Remove a single domain's entry from the cache.
    ///
    /// The domain is lowercased before lookup, so the call is case-insensitive.
    pub fn evict(&mut self, domain: &str) {
        self.entries.remove(&domain.to_ascii_lowercase());
    }

    /// Remove all expired entries from the cache.
    ///
    /// Returns the number of entries removed.
    pub fn clear_expired(&mut self) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| entry.fetched_at.elapsed() < ROBOTS_CACHE_TTL);
        before - self.entries.len()
    }

    /// Clear all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the number of entries in the cache (including expired ones).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Extract the URL scheme ("http" or "https") from a URL string.
///
/// Returns `None` if the URL is invalid or uses another scheme.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::robots::extract_scheme;
///
/// assert_eq!(extract_scheme("https://example.com/path"), Some("https".to_string()));
/// assert_eq!(extract_scheme("http://example.com"), Some("http".to_string()));
/// assert!(extract_scheme("not a url").is_none());
/// ```
#[must_use]
pub fn extract_scheme(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let scheme = parsed.scheme();
    if scheme == "http" || scheme == "https" {
        Some(scheme.to_string())
    } else {
        None
    }
}

/// Build the path (and optional query) string used for robots matching.
fn build_path(url: &Url) -> String {
    let path = url.path();
    let query = url.query();
    match query {
        Some(q) if !q.is_empty() => format!("{path}?{q}"),
        _ => {
            if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            }
        }
    }
}

/// Extract the domain (lowercased host) from a URL string.
///
/// Returns `None` if the URL is invalid, has no host, or uses a non-HTTP
/// scheme.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::robots::extract_domain;
///
/// assert_eq!(extract_domain("https://Example.com/path").unwrap(), "example.com");
/// assert_eq!(extract_domain("http://example.com:8080/page").unwrap(), "example.com");
/// assert!(extract_domain("file:///etc/passwd").is_err());
/// assert!(extract_domain("not a url").is_err());
/// ```
pub fn extract_domain(url: &str) -> Result<String, RobotsError> {
    let parsed = Url::parse(url).map_err(|e| RobotsError::Parse(e.to_string()))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(RobotsError::UnsupportedScheme(scheme.to_string()));
    }

    let host = parsed
        .host_str()
        .ok_or(RobotsError::NoHost)?
        .to_ascii_lowercase();

    Ok(host)
}

/// Extract the URL path (and query string if present) for robots matching.
///
/// Returns the path component; if a query string is present it is appended
/// (some `robots.txt` rules include query parameters). For a root URL the
/// path is `/`.
///
/// # Errors
///
/// Returns [`RobotsError`] if the URL is invalid.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::robots::extract_path;
///
/// assert_eq!(extract_path("https://example.com/private/page.html").unwrap(), "/private/page.html");
/// assert_eq!(extract_path("https://example.com").unwrap(), "/");
/// assert_eq!(extract_path("https://example.com/search?q=1").unwrap(), "/search?q=1");
/// ```
pub fn extract_path(url: &str) -> Result<String, RobotsError> {
    let parsed = Url::parse(url).map_err(|e| RobotsError::Parse(e.to_string()))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(RobotsError::UnsupportedScheme(scheme.to_string()));
    }

    let _ = parsed.host_str().ok_or(RobotsError::NoHost)?;

    let path = parsed.path();
    let query = parsed.query();

    let full_path = match query {
        Some(q) if !q.is_empty() => format!("{path}?{q}"),
        _ => {
            if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            }
        }
    };

    Ok(full_path)
}

// ---------------------------------------------------------------------------
// RobotsChecker — fetch + parse + cache orchestrator
// ---------------------------------------------------------------------------

/// Orchestrates `robots.txt` fetching, parsing, caching, and `is_allowed`
/// checks (FR-028).
///
/// Holds a [`RobotsCache`] and an optional [`reqwest::Client`]. When
/// [`is_allowed`] is called for a domain not in the cache (or whose entry has
/// expired), the checker fetches `https://{domain}/robots.txt`, parses it,
/// and caches the result.
///
/// **Allow-by-default**: if the fetch fails (network error, non-200 status,
/// parse error), the URL is allowed. This matches Hound's behaviour and
/// RFC 9309's "no robots.txt → allow all" stance.
///
/// # Examples
///
/// ```no_run
/// # async fn demo() -> anyhow::Result<()> {
/// use ragent_tools_extended::masterfetch::robots::RobotsChecker;
///
/// let checker = RobotsChecker::new();
/// // This would fetch example.com/robots.txt over the network:
/// // let allowed = checker.is_allowed("https://example.com/private/page", "*").await?;
/// # Ok(()) }
/// ```
///
/// # Testability (NFR-003)
///
/// For unit testing without network, use [`RobotsChecker::with_cache`] to
/// inject pre-populated rules, or test [`RobotsRules`] / [`parse_robots_txt`]
/// / [`RobotsCache`] directly. Network-dependent tests use `#[ignore]`.
pub struct RobotsChecker {
    cache: Arc<Mutex<RobotsCache>>,
    client: Option<reqwest::Client>,
}

impl RobotsChecker {
    /// Create a new checker with an empty cache and a default HTTP client.
    ///
    /// The HTTP client is lazily built from [`crate::masterfetch::http::build_default_client`].
    /// If the client cannot be built, the checker operates in allow-by-default
    /// mode (all fetches fail → all URLs allowed).
    #[must_use]
    pub fn new() -> Self {
        let client = crate::masterfetch::http::build_default_client().ok();
        Self {
            cache: Arc::new(Mutex::new(RobotsCache::new())),
            client,
        }
    }

    /// Create a new checker with a pre-populated cache and no HTTP client.
    ///
    /// This is the primary constructor for testing (NFR-003): inject
    /// pre-parsed rules and never touch the network. All `is_allowed` calls
    /// will be answered from the cache; uncached domains return `true`
    /// (allow-by-default, since the fetch "fails" with no client).
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_tools_extended::masterfetch::robots::{
    ///     RobotsCache, RobotsChecker, parse_robots_txt,
    /// };
    ///
    /// let mut cache = RobotsCache::new();
    /// cache.insert("example.com", parse_robots_txt("User-agent: *\nDisallow: /private/\n"));
    /// let checker = RobotsChecker::with_cache(cache);
    /// // No network call — answered from cache.
    /// ```
    #[must_use]
    pub fn with_cache(cache: RobotsCache) -> Self {
        Self {
            cache: Arc::new(Mutex::new(cache)),
            client: None,
        }
    }

    /// Check whether `url` is allowed for `user_agent` per `robots.txt`.
    ///
    /// This is the primary entry point (FR-028). It:
    ///
    /// 1. Extracts the domain and path from `url`.
    /// 2. Checks the cache for fresh rules for that domain.
    /// 3. If no fresh rules are cached, fetches and parses `robots.txt`.
    /// 4. Evaluates the parsed rules against the path.
    ///
    /// **Allow-by-default**: returns `true` if:
    /// - The URL is invalid (malformed, no host, non-HTTP scheme).
    /// - The `robots.txt` fetch fails (network error, non-200, timeout).
    /// - The `robots.txt` fails to parse.
    /// - No matching rule group or path rule applies.
    ///
    /// Returns `false` only if `robots.txt` explicitly disallows the URL.
    ///
    /// # Errors
    ///
    /// Returns [`RobotsError`] only for invalid URLs (parse failure, no host,
    /// unsupported scheme). Network and parse failures of `robots.txt` itself
    /// do **not** produce errors — they result in `true` (allow).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo() -> anyhow::Result<()> {
    /// use ragent_tools_extended::masterfetch::robots::RobotsChecker;
    ///
    /// let checker = RobotsChecker::new();
    /// let allowed = checker.is_allowed("https://example.com/page", "*").await?;
    /// # Ok(()) }
    /// ```
    pub async fn is_allowed(&self, url: &str, user_agent: &str) -> Result<bool, RobotsError> {
        let parsed = Url::parse(url).map_err(|e| RobotsError::Parse(e.to_string()))?;
        let domain = parsed
            .host_str()
            .ok_or(RobotsError::NoHost)?
            .to_ascii_lowercase();
        let path = build_path(&parsed);

        // Check cache first.
        {
            let cache = self.cache.lock().expect("robots cache lock poisoned");
            if let Some(rules) = cache.get(&domain) {
                return Ok(rules.is_allowed(user_agent, &path));
            }
        }

        // Cache miss — fetch robots.txt, preserving scheme, host and port.
        let mut robots_url = parsed;
        robots_url.set_path("/robots.txt");
        robots_url.set_query(None);
        let rules = self.fetch_robots_txt(&domain, robots_url.as_str()).await;

        // Cache the result (even if fetch failed → cache empty rules to
        // avoid refetching within TTL).
        {
            let mut cache = self.cache.lock().expect("robots cache lock poisoned");
            cache.insert(&domain, rules.clone());
        }

        Ok(rules.is_allowed(user_agent, &path))
    }

    /// Fetch and parse `robots.txt` from the given URL.
    ///
    /// Returns empty (allow-all) rules on any failure (allow-by-default).
    async fn fetch_robots_txt(&self, domain: &str, robots_url: &str) -> RobotsRules {
        let Some(client) = &self.client else {
            // No HTTP client → allow by default.
            tracing::debug!(
                domain,
                "no HTTP client available for robots.txt fetch — allowing by default"
            );
            return RobotsRules::default();
        };

        tracing::debug!(domain, url = %robots_url, "fetching robots.txt");

        let response = client
            .get(robots_url)
            .timeout(ROBOTS_FETCH_TIMEOUT)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(
                    domain,
                    error = %e,
                    "robots.txt fetch failed — allowing by default"
                );
                return RobotsRules::default();
            }
        };

        let status = response.status();
        if !status.is_success() {
            tracing::debug!(
                domain,
                status = status.as_u16(),
                "robots.txt returned non-200 — allowing by default"
            );
            // 404 or other error → no robots.txt → allow all.
            return RobotsRules::default();
        }

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                tracing::debug!(
                    domain,
                    error = %e,
                    "failed to read robots.txt body — allowing by default"
                );
                return RobotsRules::default();
            }
        };

        let rules = parse_robots_txt(&body);
        tracing::debug!(domain, groups = rules.group_count(), "parsed robots.txt");
        rules
    }

    /// Clear all cached `robots.txt` rules.
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock().expect("robots cache lock poisoned");
        cache.clear();
    }

    /// Return the number of cached domains (including expired entries).
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.lock().expect("robots cache lock poisoned").len()
    }
}

impl Default for RobotsChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for RobotsChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RobotsChecker")
            .field("cache", &self.cache)
            .field("client", &self.client.as_ref().map(|_| "reqwest::Client"))
            .finish()
    }
}
