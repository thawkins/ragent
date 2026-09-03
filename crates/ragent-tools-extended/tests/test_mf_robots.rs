#![allow(clippy::assert_is_empty)]
//! Integration tests for `masterfetch::robots` — robots.txt fetch + parse +
//! per-domain cache (T-010, FR-028, NFR-003).
//!
//! These tests exercise the pure parsing, path matching, cache TTL, and
//! `RobotsChecker` with injected cache (no network). Network-dependent tests
//! that fetch live `robots.txt` files are gated with `#[ignore]`.

use std::time::{Duration, Instant};

use ragent_tools_extended::masterfetch::robots::{
    DEFAULT_USER_AGENT, ROBOTS_CACHE_TTL, RobotsCache, RobotsChecker, RobotsRules, extract_domain,
    extract_path, parse_robots_txt, path_matches,
};

// ---------------------------------------------------------------------------
// parse_robots_txt — parsing
// ---------------------------------------------------------------------------

#[test]
fn test_parse_empty_file_allows_all() {
    let rules = parse_robots_txt("");
    assert!(rules.is_empty());
    assert!(rules.is_allowed("*", "/anything"));
    assert!(rules.is_allowed("MyBot", "/anything"));
}

#[test]
fn test_parse_whitespace_only_allows_all() {
    let rules = parse_robots_txt("   \n\n\t\n  ");
    assert!(rules.is_empty());
}

#[test]
fn test_parse_comments_only_allows_all() {
    let rules = parse_robots_txt("# just a comment\n# another\n");
    assert!(rules.is_empty());
}

#[test]
fn test_parse_simple_disallow_wildcard() {
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\n");
    assert!(!rules.is_allowed("*", "/private/secret.html"));
    assert!(rules.is_allowed("*", "/public/page.html"));
}

#[test]
fn test_parse_empty_disallow_value_allows_all() {
    let rules = parse_robots_txt("User-agent: *\nDisallow:\n");
    assert!(rules.is_allowed("*", "/anything"));
}

#[test]
fn test_parse_disallow_root_blocks_everything() {
    let rules = parse_robots_txt("User-agent: *\nDisallow: /\n");
    assert!(!rules.is_allowed("*", "/"));
    assert!(!rules.is_allowed("*", "/index.html"));
    assert!(!rules.is_allowed("*", "/deep/nested/path"));
}

#[test]
fn test_parse_allow_overrides_disallow() {
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\nAllow: /private/public/\n");
    assert!(!rules.is_allowed("*", "/private/secret.html"));
    assert!(rules.is_allowed("*", "/private/public/page.html"));
}

#[test]
fn test_parse_multiple_user_agent_groups() {
    let raw = "\
User-agent: BadBot
Disallow: /

User-agent: GoodBot
Disallow: /private/

User-agent: *
Disallow: /tmp/
";
    let rules = parse_robots_txt(raw);
    // BadBot blocked everywhere.
    assert!(!rules.is_allowed("BadBot", "/index.html"));
    // GoodBot only blocked on /private/.
    assert!(rules.is_allowed("GoodBot", "/index.html"));
    assert!(!rules.is_allowed("GoodBot", "/private/secret"));
    // Other bots use * group.
    assert!(rules.is_allowed("OtherBot", "/index.html"));
    assert!(!rules.is_allowed("OtherBot", "/tmp/file"));
    assert_eq!(rules.group_count(), 3);
}

#[test]
fn test_parse_consecutive_user_agents_share_rules() {
    let raw = "\
User-agent: BotA
User-agent: BotB
Disallow: /shared/
";
    let rules = parse_robots_txt(raw);
    assert!(!rules.is_allowed("BotA", "/shared/page"));
    assert!(!rules.is_allowed("BotB", "/shared/page"));
    assert!(rules.is_allowed("BotC", "/shared/page"));
    assert_eq!(rules.group_count(), 1);
}

#[test]
fn test_parse_user_agent_case_insensitive() {
    let rules = parse_robots_txt("User-agent: MyBot\nDisallow: /\n");
    assert!(!rules.is_allowed("mybot", "/page"));
    assert!(!rules.is_allowed("MYBOT", "/page"));
    assert!(!rules.is_allowed("MyBot", "/page"));
}

#[test]
fn test_parse_directive_case_insensitive() {
    let rules = parse_robots_txt("user-agent: *\ndisallow: /private/\n");
    assert!(!rules.is_allowed("*", "/private/secret"));
}

#[test]
fn test_parse_comments_stripped() {
    let raw = "\
User-agent: * # wildcard
Disallow: /private/ # no bots here
Allow: /private/public/ # this is ok
";
    let rules = parse_robots_txt(raw);
    assert!(!rules.is_allowed("*", "/private/secret"));
    assert!(rules.is_allowed("*", "/private/public/page"));
}

#[test]
fn test_parse_crawl_delay_wildcard() {
    let rules = parse_robots_txt("User-agent: *\nCrawl-delay: 5\n");
    assert_eq!(rules.crawl_delay("*"), Some(5.0));
    assert_eq!(rules.crawl_delay("MyBot"), Some(5.0));
}

#[test]
fn test_parse_crawl_delay_specific_ua() {
    let raw = "\
User-agent: MyBot
Crawl-delay: 10

User-agent: *
Crawl-delay: 1
";
    let rules = parse_robots_txt(raw);
    assert_eq!(rules.crawl_delay("MyBot"), Some(10.0));
    assert_eq!(rules.crawl_delay("*"), Some(1.0));
    assert_eq!(rules.crawl_delay("OtherBot"), Some(1.0));
}

#[test]
fn test_parse_crawl_delay_invalid_value_ignored() {
    let rules = parse_robots_txt("User-agent: *\nCrawl-delay: not-a-number\n");
    assert_eq!(rules.crawl_delay("*"), None);
}

#[test]
fn test_parse_crawl_delay_consecutive_uas() {
    let raw = "\
User-agent: BotA
User-agent: BotB
Crawl-delay: 7
";
    let rules = parse_robots_txt(raw);
    assert_eq!(rules.crawl_delay("BotA"), Some(7.0));
    assert_eq!(rules.crawl_delay("BotB"), Some(7.0));
}

#[test]
fn test_parse_unrecognised_directives_ignored() {
    let raw = "\
User-agent: *
Sitemap: https://example.com/sitemap.xml
Host: example.com
Disallow: /private/
";
    let rules = parse_robots_txt(raw);
    assert!(!rules.is_allowed("*", "/private/secret"));
}

#[test]
fn test_parse_no_colon_line_ignored() {
    let raw = "This line has no colon\nUser-agent: *\nDisallow: /private/\n";
    let rules = parse_robots_txt(raw);
    assert!(!rules.is_allowed("*", "/private/secret"));
}

#[test]
fn test_parse_extra_whitespace_around_values() {
    let rules = parse_robots_txt("User-agent:    *    \nDisallow:    /private/    \n");
    assert!(!rules.is_allowed("*", "/private/secret"));
}

#[test]
fn test_parse_only_user_agent_no_rules() {
    let rules = parse_robots_txt("User-agent: *\n");
    // Has a group but no rules → allow all.
    assert!(rules.is_allowed("*", "/anything"));
}

// ---------------------------------------------------------------------------
// path_matches — RFC 9309 § 2.2.2
// ---------------------------------------------------------------------------

#[test]
fn test_path_matches_simple_prefix() {
    assert!(path_matches("/private/", "/private/secret.html"));
    assert!(path_matches("/private/", "/private/"));
    assert!(!path_matches("/private/", "/public/page.html"));
}

#[test]
fn test_path_matches_exact_no_wildcard() {
    assert!(path_matches("/page", "/page"));
    // Without $, prefix match applies.
    assert!(path_matches("/page", "/page/sub"));
}

#[test]
fn test_path_matches_wildcard_star() {
    assert!(path_matches("/admin/*", "/admin/users/list"));
    assert!(path_matches("/admin/*", "/admin/"));
    assert!(!path_matches("/admin/*", "/public/page"));
}

#[test]
fn test_path_matches_end_anchor_dollar() {
    assert!(path_matches("/page$", "/page"));
    assert!(!path_matches("/page$", "/page/sub"));
    assert!(!path_matches("/page$", "/pages"));
}

#[test]
fn test_path_matches_wildcard_with_end_anchor() {
    assert!(path_matches("/*.php$", "/index.php"));
    assert!(path_matches("/*.php$", "/dir/index.php"));
    assert!(!path_matches("/*.php$", "/index.html"));
    assert!(!path_matches("/*.php$", "/index.php/sub"));
}

#[test]
fn test_path_matches_multiple_wildcards() {
    assert!(path_matches("/*/admin/*", "/x/admin/y"));
    assert!(path_matches("/*/admin/*", "/x/admin/"));
    assert!(!path_matches("/*/admin/*", "/x/user/y"));
}

#[test]
fn test_path_matches_empty_pattern_never_matches() {
    assert!(!path_matches("", "/anything"));
    assert!(!path_matches("", "/"));
}

#[test]
fn test_path_matches_root_pattern() {
    assert!(path_matches("/", "/"));
    assert!(path_matches("/", "/anything"));
    assert!(path_matches("/", "/deep/nested"));
}

#[test]
fn test_path_matches_exact_path_with_query() {
    assert!(path_matches("/search", "/search?q=1"));
    assert!(path_matches("/search?q=1", "/search?q=1"));
    assert!(!path_matches("/search?q=1", "/search?q=2"));
}

// ---------------------------------------------------------------------------
// RobotsRules.is_allowed — integration of parse + match
// ---------------------------------------------------------------------------

#[test]
fn test_is_allowed_default_rules_allow_all() {
    let rules = RobotsRules::default();
    assert!(rules.is_allowed("AnyBot", "/any/path"));
}

#[test]
fn test_is_allowed_empty_rules_allow_all() {
    let rules = parse_robots_txt("");
    assert!(rules.is_allowed("AnyBot", "/any/path"));
}

#[test]
fn test_is_allowed_specific_ua_group_over_wildcard() {
    let raw = "\
User-agent: *
Disallow: /private/

User-agent: GoodBot
Allow: /private/
";
    let rules = parse_robots_txt(raw);
    // GoodBot has its own group with Allow /private/ → allowed.
    assert!(rules.is_allowed("GoodBot", "/private/page"));
    // Other bots use * group → disallowed.
    assert!(!rules.is_allowed("OtherBot", "/private/page"));
}

#[test]
fn test_is_allowed_no_matching_group_allows() {
    let rules = parse_robots_txt("User-agent: SpecificBot\nDisallow: /\n");
    // No * group, no match for OtherBot → allow.
    assert!(rules.is_allowed("OtherBot", "/anything"));
}

#[test]
fn test_is_allowed_longer_allow_pattern_wins() {
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\nAllow: /private/public/\n");
    assert!(rules.is_allowed("*", "/private/public/page.html"));
    assert!(!rules.is_allowed("*", "/private/secret.html"));
}

#[test]
fn test_is_allowed_equal_length_allow_wins() {
    // Equal-length patterns: Allow wins (RFC 9309 § 2.2.2).
    let rules = parse_robots_txt("User-agent: *\nDisallow: /page\nAllow: /page\n");
    assert!(rules.is_allowed("*", "/page"));
}

#[test]
fn test_is_allowed_longer_disallow_wins() {
    let rules = parse_robots_txt("User-agent: *\nAllow: /a\nDisallow: /a/b\n");
    assert!(rules.is_allowed("*", "/a"));
    assert!(!rules.is_allowed("*", "/a/b"));
    assert!(rules.is_allowed("*", "/a/c"));
}

#[test]
fn test_is_allowed_crawl_delay_accessor() {
    let rules = parse_robots_txt("User-agent: *\nCrawl-delay: 3\n");
    assert_eq!(rules.crawl_delay("*"), Some(3.0));
    assert_eq!(rules.crawl_delay("AnyBot"), Some(3.0));
}

#[test]
fn test_is_allowed_crawl_delay_none_when_not_specified() {
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\n");
    assert_eq!(rules.crawl_delay("*"), None);
}

#[test]
fn test_is_allowed_group_count() {
    let raw = "\
User-agent: A
Disallow: /a/

User-agent: B
Disallow: /b/

User-agent: *
Disallow: /c/
";
    let rules = parse_robots_txt(raw);
    assert_eq!(rules.group_count(), 3);
}

// ---------------------------------------------------------------------------
// RobotsCache — TTL and eviction (FR-028: TTL 3600s)
// ---------------------------------------------------------------------------

#[test]
fn test_cache_new_is_empty() {
    let cache = RobotsCache::new();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_cache_insert_and_get() {
    let mut cache = RobotsCache::new();
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\n");
    cache.insert("example.com", rules);
    assert_eq!(cache.len(), 1);
    assert!(cache.get("example.com").is_some());
}

#[test]
fn test_cache_get_missing_domain_returns_none() {
    let cache = RobotsCache::new();
    assert!(cache.get("unknown.com").is_none());
}

#[test]
fn test_cache_expired_entry_returns_none() {
    let mut cache = RobotsCache::new();
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\n");
    let expired = Instant::now()
        .checked_sub(ROBOTS_CACHE_TTL)
        .unwrap()
        .checked_sub(Duration::from_secs(1))
        .unwrap();
    cache.insert_with_timestamp("example.com", rules, expired);
    assert!(cache.get("example.com").is_none());
}

#[test]
fn test_cache_entry_just_before_ttl_is_fresh() {
    let mut cache = RobotsCache::new();
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\n");
    let near_expiry =
        Instant::now().checked_sub(ROBOTS_CACHE_TTL).unwrap() + Duration::from_secs(1);
    cache.insert_with_timestamp("example.com", rules, near_expiry);
    assert!(cache.get("example.com").is_some());
}

#[test]
fn test_cache_entry_at_exact_ttl_is_expired() {
    let mut cache = RobotsCache::new();
    let rules = parse_robots_txt("User-agent: *\nDisallow: /private/\n");
    // Exactly at TTL → elapsed >= TTL → expired.
    let exact = Instant::now().checked_sub(ROBOTS_CACHE_TTL).unwrap();
    cache.insert_with_timestamp("example.com", rules, exact);
    assert!(cache.get("example.com").is_none());
}

#[test]
fn test_cache_clear_removes_all() {
    let mut cache = RobotsCache::new();
    cache.insert("a.com", RobotsRules::default());
    cache.insert("b.com", RobotsRules::default());
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn test_cache_evict_single_domain() {
    let mut cache = RobotsCache::new();
    cache.insert("a.com", RobotsRules::default());
    cache.insert("b.com", RobotsRules::default());
    cache.evict("a.com");
    assert!(cache.get("a.com").is_none());
    assert!(cache.get("b.com").is_some());
}

#[test]
fn test_cache_clear_expired_removes_only_stale() {
    let mut cache = RobotsCache::new();
    let expired = Instant::now()
        .checked_sub(ROBOTS_CACHE_TTL)
        .unwrap()
        .checked_sub(Duration::from_secs(10))
        .unwrap();
    cache.insert_with_timestamp("old.com", RobotsRules::default(), expired);
    cache.insert("new.com", RobotsRules::default());
    let removed = cache.clear_expired();
    assert_eq!(removed, 1);
    assert!(cache.get("old.com").is_none());
    assert!(cache.get("new.com").is_some());
}

#[test]
fn test_cache_clear_expired_with_no_stale_removes_zero() {
    let mut cache = RobotsCache::new();
    cache.insert("fresh.com", RobotsRules::default());
    let removed = cache.clear_expired();
    assert_eq!(removed, 0);
    assert!(cache.get("fresh.com").is_some());
}

#[test]
fn test_cache_domain_keyed_case_insensitively() {
    let mut cache = RobotsCache::new();
    cache.insert("Example.COM", RobotsRules::default());
    assert!(cache.get("example.com").is_some());
    assert!(cache.get("EXAMPLE.com").is_some());
}

#[test]
fn test_cache_multiple_domains() {
    let mut cache = RobotsCache::new();
    cache.insert("a.com", parse_robots_txt("User-agent: *\nDisallow: /a/\n"));
    cache.insert("b.com", parse_robots_txt("User-agent: *\nDisallow: /b/\n"));
    assert_eq!(cache.len(), 2);
    assert!(!cache.get("a.com").unwrap().is_allowed("*", "/a/secret"));
    assert!(!cache.get("b.com").unwrap().is_allowed("*", "/b/secret"));
    assert!(cache.get("a.com").unwrap().is_allowed("*", "/b/secret"));
}

// ---------------------------------------------------------------------------
// extract_domain / extract_path
// ---------------------------------------------------------------------------

#[test]
fn test_extract_domain_basic() {
    assert_eq!(
        extract_domain("https://example.com/path").unwrap(),
        "example.com"
    );
}

#[test]
fn test_extract_domain_lowercases() {
    assert_eq!(
        extract_domain("https://Example.COM/Path").unwrap(),
        "example.com"
    );
}

#[test]
fn test_extract_domain_strips_port() {
    assert_eq!(
        extract_domain("http://example.com:8080/page").unwrap(),
        "example.com"
    );
}

#[test]
fn test_extract_domain_rejects_file_scheme() {
    assert!(extract_domain("file:///etc/passwd").is_err());
}

#[test]
fn test_extract_domain_rejects_ftp_scheme() {
    assert!(extract_domain("ftp://example.com").is_err());
}

#[test]
fn test_extract_domain_rejects_invalid_url() {
    assert!(extract_domain("not a url").is_err());
    assert!(extract_domain("").is_err());
}

#[test]
fn test_extract_path_basic() {
    assert_eq!(
        extract_path("https://example.com/private/page.html").unwrap(),
        "/private/page.html"
    );
}

#[test]
fn test_extract_path_root() {
    assert_eq!(extract_path("https://example.com").unwrap(), "/");
    assert_eq!(extract_path("https://example.com/").unwrap(), "/");
}

#[test]
fn test_extract_path_with_query() {
    assert_eq!(
        extract_path("https://example.com/search?q=1").unwrap(),
        "/search?q=1"
    );
}

#[test]
fn test_extract_path_rejects_non_http() {
    assert!(extract_path("file:///etc/passwd").is_err());
}

// ---------------------------------------------------------------------------
// RobotsChecker — with_cache (no network, NFR-003)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_checker_cached_domain_answered_from_cache() {
    let mut cache = RobotsCache::new();
    cache.insert(
        "example.com",
        parse_robots_txt("User-agent: *\nDisallow: /private/\n"),
    );
    let checker = RobotsChecker::with_cache(cache);

    assert!(
        !checker
            .is_allowed("https://example.com/private/secret", "*")
            .await
            .unwrap()
    );
    assert!(
        checker
            .is_allowed("https://example.com/public/page", "*")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_checker_uncached_domain_allows_by_default_no_client() {
    let checker = RobotsChecker::with_cache(RobotsCache::new());
    assert!(
        checker
            .is_allowed("https://uncached.com/anything", "*")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_checker_invalid_url_returns_error() {
    let checker = RobotsChecker::with_cache(RobotsCache::new());
    assert!(checker.is_allowed("not a url", "*").await.is_err());
}

#[tokio::test]
async fn test_checker_non_http_url_returns_error() {
    let checker = RobotsChecker::with_cache(RobotsCache::new());
    assert!(checker.is_allowed("file:///etc/passwd", "*").await.is_err());
}

#[tokio::test]
async fn test_checker_caches_uncached_domain_result() {
    let mut cache = RobotsCache::new();
    cache.insert("cached.com", RobotsRules::default());
    let checker = RobotsChecker::with_cache(cache);

    // Uncached domain → allowed + cached as empty rules.
    assert!(
        checker
            .is_allowed("https://new.com/page", "*")
            .await
            .unwrap()
    );
    assert_eq!(checker.cache_len(), 2);
}

#[tokio::test]
async fn test_checker_clear_cache() {
    let mut cache = RobotsCache::new();
    cache.insert("example.com", RobotsRules::default());
    let checker = RobotsChecker::with_cache(cache);
    assert_eq!(checker.cache_len(), 1);
    checker.clear_cache();
    assert_eq!(checker.cache_len(), 0);
}

#[tokio::test]
async fn test_checker_specific_user_agent_match() {
    let mut cache = RobotsCache::new();
    cache.insert(
        "example.com",
        parse_robots_txt("User-agent: BadBot\nDisallow: /\n\nUser-agent: *\nDisallow: /private/\n"),
    );
    let checker = RobotsChecker::with_cache(cache);

    assert!(
        !checker
            .is_allowed("https://example.com/index.html", "BadBot")
            .await
            .unwrap()
    );
    assert!(
        checker
            .is_allowed("https://example.com/index.html", "GoodBot")
            .await
            .unwrap()
    );
    assert!(
        !checker
            .is_allowed("https://example.com/private/secret", "GoodBot")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_checker_default_user_agent_constant() {
    // The DEFAULT_USER_AGENT constant should be "*" for wildcard matching.
    assert_eq!(DEFAULT_USER_AGENT, "*");
}

#[tokio::test]
async fn test_checker_path_with_query_checked() {
    let mut cache = RobotsCache::new();
    cache.insert(
        "example.com",
        parse_robots_txt("User-agent: *\nDisallow: /search?\n"),
    );
    let checker = RobotsChecker::with_cache(cache);

    assert!(
        !checker
            .is_allowed("https://example.com/search?q=test", "*")
            .await
            .unwrap()
    );
    assert!(
        checker
            .is_allowed("https://example.com/page", "*")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_checker_multiple_domains_independent() {
    let mut cache = RobotsCache::new();
    cache.insert("a.com", parse_robots_txt("User-agent: *\nDisallow: /\n"));
    cache.insert(
        "b.com",
        parse_robots_txt("User-agent: *\nDisallow: /private/\n"),
    );
    let checker = RobotsChecker::with_cache(cache);

    // a.com blocks everything.
    assert!(!checker.is_allowed("https://a.com/any", "*").await.unwrap());
    // b.com only blocks /private/.
    assert!(checker.is_allowed("https://b.com/any", "*").await.unwrap());
    assert!(
        !checker
            .is_allowed("https://b.com/private/secret", "*")
            .await
            .unwrap()
    );
}

// ---------------------------------------------------------------------------
// RobotsChecker — network tests (gated with #[ignore], NFR-003)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires network access — run with: cargo test -- --ignored"]
async fn test_checker_fetch_live_robots_txt() {
    // example.com has a robots.txt that allows all paths.
    let checker = RobotsChecker::new();
    let result = checker
        .is_allowed("https://example.com/anything", "*")
        .await;
    assert!(result.is_ok(), "is_allowed should not error for valid URLs");
    // example.com allows all → should be true.
    assert!(result.unwrap(), "example.com should allow all paths");
    // Verify it was cached.
    assert!(checker.cache_len() > 0);
}

#[tokio::test]
#[ignore = "requires network access — run with: cargo test -- --ignored"]
async fn test_checker_fetch_caches_result() {
    let checker = RobotsChecker::new();
    // First call fetches.
    let _ = checker
        .is_allowed("https://example.com/page1", "*")
        .await
        .unwrap();
    assert_eq!(checker.cache_len(), 1);
    // Second call uses cache (no new entry).
    let _ = checker
        .is_allowed("https://example.com/page2", "*")
        .await
        .unwrap();
    assert_eq!(checker.cache_len(), 1);
}

#[tokio::test]
#[ignore = "requires network access — run with: cargo test -- --ignored"]
async fn test_checker_nonexistent_domain_allows_by_default() {
    let checker = RobotsChecker::new();
    // A domain that likely doesn't resolve → fetch fails → allow by default.
    let result = checker
        .is_allowed(
            "https://this-domain-definitely-does-not-exist-xyz.invalid/page",
            "*",
        )
        .await;
    assert!(result.is_ok());
    assert!(
        result.unwrap(),
        "unreachable robots.txt should allow by default"
    );
}
