//! Catalogue-fetch, `/connectors stores` report, and endpoint-resolver glue
//! (spec `connectors` T-017/T-018; FR-035, FR-036, FR-037, NFR-002).
//!
//! This module is the single call site that connects the T-016 resolver
//! ([`CatalogueKind::effective_endpoint_with_source`]) to the two places the
//! connector system contacts - or reports - a catalogue endpoint:
//!
//! - **The catalogue fetch** ([`fetch_effective_catalogue`]): it resolves the
//!   effective endpoint for the catalogue, runs it through the resolver's
//!   absolute-`https`-with-host guard, and only then hands the validated
//!   [`CatalogueEndpoint`] to [`fetch_catalogue`]. A refused endpoint is a
//!   contained [`ConnectorError::CatalogueEndpoint`] returned *before* any
//!   network request, so a malformed or non-`https` value never silently falls
//!   back to an unvalidated one and never starts a fetch (FR-037).
//! - **The `/connectors stores` handler** ([`stores_report`]): it reports the
//!   endpoint each catalogue resolves to together with its provenance tag, so
//!   the row a user reads is the same value the fetch will contact (FR-035,
//!   FR-036).
//!
//! Resolution is a pure, offline function ([`CatalogueKind::effective_endpoint_with_source`],
//! NFR-003): this module performs no network access until [`fetch_catalogue`]
//! is reached, and every consumer resolves once per launch so editing or
//! clearing `connectors.stores.claude.url` takes effect on the next launch with
//! no rebuild (NFR-002).
//!
//! The `/connectors stores` report wording lives here too
//! ([`render_stores_report`], T-018; FR-036): each catalogue row prints the
//! resolved endpoint with its provenance tag read from the **same**
//! [`StoreEndpointRow`] the fetch resolves, so the endpoint and tag a user reads
//! can never disagree with the value that will be contacted. When a probe is
//! supplied ([`probe_stores`], the opt-in `--check` path) each row additionally
//! carries that catalogue's reachability, and nothing else performs network I/O.

use ragent_config::ConnectorsConfig;

use crate::error::ConnectorError;
use crate::fetch::{
    CatalogueCache, CatalogueFetcher, CatalogueLimits, FetchedCatalogue, fetch_catalogue,
};
use crate::store_index::{
    CatalogueCatalog, CatalogueEndpoint, CatalogueEndpointError, CatalogueKind, EndpointSource,
};

/// One resolved catalogue endpoint as the `/connectors stores` handler reads it.
///
/// Carries the validated endpoint and its provenance from the **same**
/// [`CatalogueKind::effective_endpoint_with_source`] call the fetch uses, so the
/// reported endpoint and tag can never disagree with the value actually
/// contacted (FR-035, FR-036). When resolution is refused the row holds the
/// contained [`CatalogueEndpointError`] instead of an endpoint, so a non-`https`
/// or malformed override is reported as a refusal rather than an unvalidated
/// value (FR-037).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreEndpointRow {
    /// The catalogue this row describes.
    pub kind: CatalogueKind,
    /// The validated endpoint and its provenance, when resolution succeeded.
    pub resolved: Result<(CatalogueEndpoint, EndpointSource), CatalogueEndpointError>,
}

impl StoreEndpointRow {
    /// The resolved endpoint, when resolution succeeded.
    #[must_use]
    pub fn endpoint(&self) -> Option<&CatalogueEndpoint> {
        self.resolved.as_ref().ok().map(|(endpoint, _)| endpoint)
    }

    /// The endpoint's provenance tag (`default`/`config`), when resolution
    /// succeeded (FR-036).
    #[must_use]
    pub fn source(&self) -> Option<EndpointSource> {
        self.resolved.as_ref().ok().map(|(_, source)| *source)
    }

    /// Whether the endpoint was refused by the `https` guard (FR-037).
    #[must_use]
    pub const fn is_refused(&self) -> bool {
        self.resolved.is_err()
    }
}

/// Resolve every catalogue's effective endpoint once per launch into a
/// [`CatalogueCatalog`] (FR-035, NFR-002).
///
/// A catalogue whose endpoint is refused (non-`https` or malformed, FR-037) is
/// omitted from the registry rather than recorded with an unvalidated value;
/// the caller reports the refusal from [`stores_report`]. The function performs
/// no network access (NFR-003).
#[must_use]
pub fn resolve_catalog(connectors: &ConnectorsConfig) -> CatalogueCatalog {
    let mut catalog = CatalogueCatalog::new();
    for kind in CatalogueKind::ALL {
        if let Ok((endpoint, _)) = kind.effective_endpoint_with_source(connectors) {
            catalog.insert(kind, endpoint);
        }
    }
    catalog
}

/// The resolved-endpoint rows the `/connectors stores` handler prints, one per
/// catalogue (FR-035, FR-036, FR-037).
///
/// Each row is produced by the same resolver the catalogue fetch uses, so the
/// endpoint a user reads is the endpoint that will be contacted, and a refused
/// endpoint is carried as a refusal. Pure and offline (NFR-003).
#[must_use]
pub fn stores_report(connectors: &ConnectorsConfig) -> Vec<StoreEndpointRow> {
    CatalogueKind::ALL
        .into_iter()
        .map(|kind| StoreEndpointRow {
            kind,
            resolved: kind.effective_endpoint_with_source(connectors),
        })
        .collect()
}

/// The effective endpoint for one catalogue, guarded (FR-035, FR-037).
///
/// The single call site that turns configuration into a validated
/// [`CatalogueEndpoint`] for the fetch path, so no caller reaches the network
/// with an unresolved endpoint.
///
/// # Errors
///
/// Returns [`ConnectorError::CatalogueEndpoint`] when the compiled default or
/// the configured override is not an absolute `https` URL carrying a host
/// (FR-037).
pub fn effective_endpoint(
    kind: CatalogueKind,
    connectors: &ConnectorsConfig,
) -> Result<CatalogueEndpoint, ConnectorError> {
    let (endpoint, _) = kind.effective_endpoint_with_source(connectors)?;
    Ok(endpoint)
}

/// Resolve the effective catalogue endpoint and fetch its catalogue, guarded
/// (FR-035, FR-037, NFR-002).
///
/// Resolution happens first: a refused endpoint is a contained
/// [`ConnectorError::CatalogueEndpoint`] and no fetch is attempted, so a
/// non-`https` or malformed value can never be contacted and never falls back to
/// an unvalidated default (FR-037). On success the validated endpoint and the
/// caller's `limits` and `cache` drive [`fetch_catalogue`] exactly as before.
///
/// `now` is the current Unix time in seconds for the cache TTL, threaded in so
/// the call stays pure and testable.
///
/// # Errors
///
/// Returns [`ConnectorError::CatalogueEndpoint`] when the endpoint is refused
/// (FR-037), or the contained [`ConnectorError`] the `fetcher` reports on any
/// fetch failure.
pub fn fetch_effective_catalogue(
    fetcher: &dyn CatalogueFetcher,
    kind: CatalogueKind,
    connectors: &ConnectorsConfig,
    limits: &CatalogueLimits,
    cache: Option<&CatalogueCache>,
    now: u64,
) -> Result<FetchedCatalogue, ConnectorError> {
    let endpoint = effective_endpoint(kind, connectors)?;
    fetch_catalogue(fetcher, kind, &endpoint, limits, cache, now)
}

/// The attribution prefix every `/connectors` report carries (FR-006).
///
/// T-012 (`/connectors help`) owns the full usage block; this module needs only
/// the `stores` subcommand header. Rendered here so the TUI family and the
/// `ragent connectors` CLI (T-014) print one wording.
#[must_use]
pub fn stores_attribution() -> String {
    crate::help::attribution("stores")
}

/// The result of contacting one catalogue endpoint (`/connectors stores --check`).
///
/// `Available { count }` means the endpoint answered with a fetchable catalogue
/// advertising `count` connectors; `Unavailable { detail }` means it could not be
/// resolved, reached, or parsed and carries a short ASCII cause to print on the
/// row. It never holds a raw [`ConnectorError`] so the report stays a plain
/// string and never panics (SPEC error policy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreProbe {
    /// The catalogue answered and advertised this many connectors.
    Available {
        /// Number of accepted catalogue connectors.
        count: usize,
    },
    /// The catalogue could not be resolved, reached, or parsed; `detail` names
    /// the cause.
    Unavailable {
        /// Short cause, e.g. `catalogue endpoint must use https, not http`.
        detail: String,
    },
}

impl StoreProbe {
    /// Render the probe suffix appended to a catalogue's report line.
    #[must_use]
    fn render(&self) -> String {
        match self {
            Self::Available { count } => format!(" [ok] available, {count} connectors"),
            Self::Unavailable { detail } => format!(" [err] unavailable: {detail}"),
        }
    }
}

/// Whether a `/connectors stores` invocation carries the `--check` flag, which
/// additionally contacts each catalogue endpoint (`/connectors stores --check`).
///
/// The flag is accepted anywhere in the argument text, mirroring
/// `ragent_plugins::commands::stores_check_requested`; an unrecognised token is
/// ignored so a stray argument still renders the plain config report.
#[must_use]
pub fn stores_check_requested(args: &str) -> bool {
    args.split_whitespace().any(|token| token == "--check")
}

/// Contact each catalogue's effective endpoint and summarise availability and
/// catalogue size (`/connectors stores --check`).
///
/// Each catalogue is resolved through the same [`effective_endpoint`] the fetch
/// uses (a non-empty override wins, otherwise the compiled default; FR-035), then
/// fetched once through `fetcher` under `connectors.stores`' byte/time budget. A
/// catalogue whose endpoint is refused by the `https` guard, or whose fetch
/// fails, yields a contained [`StoreProbe::Unavailable`]; no path panics. The
/// returned slice is aligned with [`CatalogueKind::ALL`].
///
/// This is the only function in this module that may perform network I/O, and it
/// is a no-op unless the caller invoked `--check` (NFR-003). Blocking: off-loop
/// callers should run it on a task (the CLI is a one-shot process).
#[must_use]
pub fn probe_stores(
    connectors: &ConnectorsConfig,
    fetcher: &dyn CatalogueFetcher,
) -> Vec<StoreProbe> {
    let limits = CatalogueLimits::from(&connectors.stores_or_default());
    CatalogueKind::ALL
        .into_iter()
        .map(|kind| match effective_endpoint(kind, connectors) {
            Ok(endpoint) => match fetcher.fetch(kind, &endpoint, &limits) {
                Ok(parsed) => StoreProbe::Available {
                    count: parsed.connectors.len(),
                },
                Err(err) => StoreProbe::Unavailable {
                    detail: err.to_string(),
                },
            },
            Err(err) => StoreProbe::Unavailable {
                detail: err.to_string(),
            },
        })
        .collect()
}

/// Render the `/connectors stores` report: each catalogue's effective endpoint
/// and its provenance tag (FR-036).
///
/// Every catalogue is always listed, even when none is overridden, so the user
/// can confirm the out-of-the-box compiled default (FR-034). The row is built
/// from the same [`StoreEndpointRow`] the fetch resolves (FR-035): a successful
/// resolution prints `- <token>: [<tag>] <endpoint>` with the tag
/// `default`/`config` from [`EndpointSource::tag`] (FR-036), and a refused
/// endpoint prints `- <token>: [err] <reason>` naming the offending scheme or
/// malformed input rather than silently substituting an unvalidated value
/// (FR-037). Pure string rendering: no network access, no state change
/// (NFR-003). ASCII only and prefixed `From: /connectors stores` (FR-006).
#[must_use]
pub fn render_stores_report(connectors: &ConnectorsConfig) -> String {
    render_stores_report_with_probes(connectors, None)
}

/// [`render_stores_report`] with an optional per-catalogue availability probe
/// appended to each row (`/connectors stores --check`).
///
/// `probes`, when present, is aligned with [`CatalogueKind::ALL`]; a missing or
/// short slice leaves the remaining rows as the plain config report, and rows are
/// matched by their [`CatalogueKind`] so a probe can never land on the wrong
/// catalogue.
#[must_use]
pub fn render_stores_report_with_probes(
    connectors: &ConnectorsConfig,
    probes: Option<&[StoreProbe]>,
) -> String {
    let mut lines = vec![stores_attribution(), String::new()];
    for (i, row) in stores_report(connectors).into_iter().enumerate() {
        let probe = probes
            .and_then(|p| p.get(i))
            .map(StoreProbe::render)
            .unwrap_or_default();
        match row.resolved {
            Ok((endpoint, source)) => lines.push(format!(
                "- {token}: [{tag}] {endpoint}{probe}",
                token = row.kind.token(),
                tag = source.tag(),
                endpoint = endpoint.as_str(),
            )),
            Err(err) => lines.push(format!(
                "- {token}: [err] {err}{probe}",
                token = row.kind.token()
            )),
        }
    }
    lines.join("\n")
}

/// Render the `/connectors stores` report for `connectors`, contacting each
/// endpoint only when `args` carries `--check` (FR-036, NFR-003).
#[must_use]
pub fn stores_report_with_check(
    connectors: &ConnectorsConfig,
    fetcher: &dyn CatalogueFetcher,
    args: &str,
) -> String {
    if stores_check_requested(args) {
        let probes = probe_stores(connectors, fetcher);
        render_stores_report_with_probes(connectors, Some(&probes))
    } else {
        render_stores_report(connectors)
    }
}
