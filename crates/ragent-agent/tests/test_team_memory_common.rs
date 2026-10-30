//! The shared `team_memory_common` helpers (spec `teamssetup`, T-304).
//!
//! `slugify` / `path_tag` normalise a user-supplied `path` into a tag-safe
//! bucket identifier for `team_memory_read` / `team_memory_write`. The
//! normalisation used to be copied into both tools; it now has one definition,
//! and these tests pin the rules that copy established:
//!
//! 1. Lowercasing, so `Notes` and `notes` are one bucket.
//! 2. Every non-alphanumeric character becomes a hyphen.
//! 3. Leading and trailing hyphens are trimmed, and interior runs collapse to
//!    one - so two spellings of the same path that differ only in punctuation
//!    hit the same bucket rather than splitting the team's notes in two.
//! 4. A path that slugs to nothing falls back to the `path-memory` bucket
//!    instead of producing the bare prefix `path-`.
//! 5. The normalised identifier carries the `path-` prefix, which is what makes
//!    a memory tag distinguishable from a free-form tag.
//!
//! The module is compiled into this crate's module tree through its `#[path]`
//! include in `src/tool/mod.rs`, so the private `slugify` / `path_tag` helpers
//! are reachable from a child module here.

#[path = "../src/tool/team_memory_common.rs"]
mod team_memory_common;

use team_memory_common::{path_tag, slugify};

#[test]
fn test_slugify_lowercases() {
    assert_eq!(slugify("Notes"), "notes");
    assert_eq!(slugify("MiXeD-Case"), "mixed-case");
}

#[test]
fn test_slugify_replaces_non_alphanumerics() {
    assert_eq!(slugify("api/design"), "api-design");
    assert_eq!(slugify("a b\tc"), "a-b-c");
    assert_eq!(slugify("q1.2026"), "q1-2026");
}

#[test]
fn test_slugify_trims_outer_hyphens_and_collapses_runs() {
    assert_eq!(
        slugify("--api//design--"),
        "api-design",
        "leading/trailing hyphens are trimmed and the interior run collapses"
    );
    assert_eq!(
        slugify("a   b"),
        "a-b",
        "two spellings that differ only in punctuation must agree"
    );
}

#[test]
fn test_slugify_of_punctuation_only_is_empty() {
    assert_eq!(slugify("///"), "");
    assert_eq!(slugify(""), "");
}

#[test]
fn test_path_tag_prefixes_the_slug() {
    assert_eq!(path_tag("api/design"), "path-api-design");
}

#[test]
fn test_path_tag_falls_back_to_memory_bucket() {
    // The regression this pins: `format!("path-{}", slugify(path))` produced the
    // bare prefix `path-` for an empty slug, which is not a usable bucket name.
    assert_eq!(path_tag(""), "path-memory");
    assert_eq!(path_tag("///"), "path-memory");
}
