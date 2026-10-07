//! Shared helpers for the `team_memory_read` / `team_memory_write` tools.
//!
//! Both tools normalise a user-supplied `path` into a tag-safe bucket
//! identifier so teammates can partition notes into named buckets. The
//! normalisation previously lived as a copy in each tool (T-304); it now has a
//! single definition here.

/// Slugify an arbitrary string into a tag-safe fragment.
///
/// Lowercases the input, maps every non-alphanumeric character to `-`, trims
/// leading/trailing hyphens, and collapses runs of hyphens to one.
pub(crate) fn slugify(s: &str) -> String {
    let slug: String = s
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    let mut slug = slug.replace("--", "-");
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug
}

/// Normalise a user-supplied path into a tag-safe bucket identifier.
///
/// The result is `path-<slug>`; a path that slugs to the empty string uses the
/// `path-memory` default bucket.
pub(crate) fn path_tag(path: &str) -> String {
    let slug = slugify(path);
    if slug.is_empty() {
        "path-memory".to_string()
    } else {
        format!("path-{slug}")
    }
}
