//! Shared pagination / limit constants for the VCS tool surface.
//!
//! GitHub and GitLab tools previously embedded bare `20` / `100` / `10` limits
//! across many modules with no single source of truth (ANTIPAT.md A-5, I-3).
//! They now share these named constants so the two providers keep the same
//! pagination policy.

/// Default number of items returned by a list tool when the caller omits
/// `limit`.
pub const DEFAULT_PAGE_LIMIT: u64 = 20;

/// Hard upper bound on `limit` for any list tool (one page for both the GitHub
/// and GitLab APIs).
pub const MAX_PAGE_LIMIT: u64 = 100;

/// Number of comments / notes / reviews rendered on a single "get" call.
///
/// Used both as the server-side `per_page` cap on GitHub/GitLab comment and
/// review fetches and as the display cap, so both providers fetch and render
/// the same number of items.
pub const NOTES_PER_PAGE: u64 = 10;
