//! Shared marketplace-source resolution for the `/plugins` and `/connectors`
//! `marketplace add` command families.
//!
//! Both families accept the same four source forms and derive the same display
//! name and `https` marketplace-index URL. They differed only in the
//! store-specific marketplace-document path, so the resolver takes that path as
//! a parameter and both crates delegate. The two copies had drifted into
//! byte-identical private functions (`resolve_source`, `owner_repo_slug`,
//! `name_from_url`, `local_marketplace_name`); the single implementation lives
//! here.
//!
//! No network access happens here: a source is resolved syntactically and the
//! URL is recorded for the browser to fetch later.

use std::path::Path;

use serde_json::Value;

/// A resolved marketplace source: a display name and, when it is fetchable over
/// `https`, the marketplace-index URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceSource {
    /// The marketplace display name (the repo slug, or a local directory name).
    pub name: String,
    /// The `https` marketplace-index URL, or `None` for a local (non-fetchable)
    /// marketplace.
    pub url: Option<String>,
}

/// Why a marketplace source could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarketplaceSourceError {
    /// The source string was empty or whitespace-only.
    #[error("no marketplace source given")]
    Empty,
    /// The source carried no recognisable owner/repo or URL.
    #[error(
        "unrecognised marketplace source `{0}`: expected owner/repo, an https:// \
             URL, or a local marketplace directory"
    )]
    Unsupported(String),
}

/// Resolve a user-typed marketplace `source`.
///
/// `doc_path` is the store-specific marketplace document path (relative to a
/// repository root) used when an `owner/repo` (or SSH) source is resolved to a
/// raw GitHub URL. Accepts `owner/repo`, `git@github.com:owner/repo.git`, an
/// `https://` URL, or a local directory path. Returns a
/// [`MarketplaceSourceError`] for an empty or unrecognisable source. Performs no
/// network access; the URL is derived syntactically and anchored to branch
/// `main`.
pub fn resolve_source(
    source: &str,
    cwd: &Path,
    doc_path: &str,
) -> Result<MarketplaceSource, MarketplaceSourceError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(MarketplaceSourceError::Empty);
    }

    // SSH git remote: git@github.com:owner/repo.git
    if let Some(rest) = source.strip_prefix("git@github.com:") {
        let slug = rest.trim_end_matches(".git").trim_end_matches('/');
        return owner_repo_slug(slug, doc_path)
            .ok_or_else(|| MarketplaceSourceError::Unsupported(source.to_string()));
    }

    // GitHub repo URL: https://github.com/owner/repo (optionally .git).
    if let Some(rest) = source
        .strip_prefix("https://github.com/")
        .or_else(|| source.strip_prefix("http://github.com/"))
    {
        let slug = rest.trim_end_matches(".git").trim_end_matches('/');
        // Keep only the first two path segments (owner/repo) so a URL with a
        // trailing `/tree/main` still resolves; `owner_repo_slug` then validates
        // the pair and rejects anything that is not exactly two segments.
        let slug = match slug
            .split_once('/')
            .and_then(|(owner, tail)| tail.split_once('/').map(|(repo, _)| (owner, repo)))
        {
            Some((owner, repo)) => format!("{owner}/{repo}"),
            None => slug.to_string(),
        };
        return owner_repo_slug(&slug, doc_path)
            .ok_or_else(|| MarketplaceSourceError::Unsupported(source.to_string()));
    }

    // Any other https URL: a marketplace index document (or a raw repo file).
    if source.starts_with("https://") {
        let name = name_from_url(source);
        return Ok(MarketplaceSource {
            name,
            url: Some(source.to_string()),
        });
    }

    // Explicit local directory: ./path or /abs/path or a bare path that exists.
    let looks_local = source.starts_with("./")
        || source.starts_with("../")
        || source.starts_with('/')
        || cwd.join(source).is_dir();
    if looks_local {
        let path = if source.starts_with('/') {
            std::path::PathBuf::from(source)
        } else {
            cwd.join(source)
        };
        let name = local_marketplace_name(&path, doc_path)
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "local".to_string());
        return Ok(MarketplaceSource { name, url: None });
    }

    // Bare `owner/repo` GitHub shorthand.
    if let Some(resolved) = owner_repo_slug(source, doc_path) {
        return Ok(resolved);
    }

    Err(MarketplaceSourceError::Unsupported(source.to_string()))
}

/// Build the resolved source for an `owner/repo` slug, or `None` when the slug
/// is not a two-segment pair.
fn owner_repo_slug(slug: &str, doc_path: &str) -> Option<MarketplaceSource> {
    let slug = slug.trim_matches('/');
    let (owner, repo) = slug.split_once('/')?;
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    Some(MarketplaceSource {
        name: repo.to_string(),
        url: Some(format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/main/{doc_path}"
        )),
    })
}

/// Derive a display name from a marketplace-index URL.
///
/// A `raw.githubusercontent.com/<owner>/<repo>/...` URL names the repo; any
/// other URL falls back to its final path segment before the file name, then
/// the host.
fn name_from_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://raw.githubusercontent.com/") {
        let mut segments = rest.split('/');
        if segments.next().is_some()
            && let Some(repo) = segments.next()
            && !repo.is_empty()
        {
            return repo.to_string();
        }
    }
    let trimmed = url.trim_end_matches('/');
    let without_file = trimmed
        .rsplit_once('/')
        .map(|(head, _)| head)
        .unwrap_or(trimmed);
    if let Some(last) = without_file.rsplit('/').next()
        && !last.is_empty()
        && !last.starts_with("https:")
    {
        return last.to_string();
    }
    "marketplace".to_string()
}

/// Read the `name` field from a local marketplace manifest, if it is present
/// and readable.
fn local_marketplace_name(path: &Path, doc_path: &str) -> Option<String> {
    let candidate = path.join(doc_path);
    let root = if candidate.is_file() {
        candidate
    } else {
        path.join("marketplace.json")
    };
    let bytes = std::fs::read(root).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}
