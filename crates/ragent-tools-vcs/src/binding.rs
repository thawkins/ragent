//! Binding a pull/merge request the agent just created to the session that
//! created it (spec SESSIONMENU FR-14).
//!
//! `github_create_pr` and `gitlab_create_mr` call [`bind_created`] after a
//! successful create. The binding goes to the top-level session (a request
//! opened by a sub-agent belongs to the session the user sees) through
//! [`crate::storage::StorageBackend::bind_session_request`]. Binding is a
//! convenience: a failure is logged and never fails the tool.

use ragent_storage::SessionRequest;

use crate::ToolContext;

/// The host of a request's browser URL: `https://host[:port]/...` yields
/// `host[:port]`, lower-cased. `None` when the URL has no host.
#[must_use]
pub fn web_url_host(web_url: &str) -> Option<String> {
    let rest = web_url.split_once("://").map(|(_, rest)| rest)?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Binds a newly created request to the calling session, if the context
/// has storage. `request.session_id` and `request.host` are filled in here
/// from `ctx` and `request.web_url`.
pub fn bind_created(ctx: &ToolContext, mut request: SessionRequest) {
    let Some(storage) = ctx.storage.as_ref() else {
        return;
    };
    if ctx.session_id.is_empty() || request.number == 0 {
        return;
    }
    let Some(host) = web_url_host(&request.web_url) else {
        tracing::debug!(url = %request.web_url, "created request has no web URL; not bound");
        return;
    };
    request.session_id.clone_from(&ctx.session_id);
    request.host = host;
    if let Err(e) = storage.bind_session_request(&request) {
        tracing::debug!(error = %e, number = request.number, "request not bound to session");
    }
}
