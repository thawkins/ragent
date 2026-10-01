//! Catalogue normalisation providers: dialect-specific transforms from a vendor
//! catalogue document into the internal connector-entry model (spec `connectors`
//! T-005; FR-025).
//!
//! Each catalogue serves its document in its own shape. A [`ConnectorProvider`]
//! is the strategy that knows one shape and normalises it into the same
//! [`ParsedCatalogue`] every consumer renders, so the fetch, the search report,
//! and the browser are catalogue-agnostic:
//!
//! - [`NativeConnectorProvider`] parses ragent's own catalogue shape: an object
//!   with a `connectors` (or `servers`) array, or a bare top-level array, whose
//!   entries carry `id`/`name` and a `servers` array (or an `mcpServers` object).
//! - [`ClaudeConnectorProvider`] parses Anthropic's connector-directory feed: an
//!   object with a `servers` array whose entries carry a `slug`/`name`, a
//!   `remote` object (`{"url":...,"transport":"streamable-http"}`), `categories`,
//!   `works_with`, and a `remote` auth posture.
//!
//! Both providers share the same tolerant, per-entry-skip discipline
//! (`ragent_plugins::store_provider`; FR-025): an entry that cannot be
//! normalised is counted in [`ParsedCatalogue::skipped`] rather than failing the
//! whole document, and an entry whose servers are all unexpressible is likewise
//! skipped (its recorded label is reported by `/connectors list`).
//!
//! The provider's own [`CatalogueError`] is the pre-conversion failure; the
//! caller maps it onto the reportable [`ConnectorError`] variants so a malformed
//! document is refused with a named reason (FR-031).

use serde_json::Value;
use url::Url;

use crate::descriptor::{ConnectorDescriptor, ConnectorProvenance};
use crate::entry::{ConnectorEntry, ConnectorEntryError};
use crate::error::ConnectorError;
use crate::store_index::CatalogueKind;

/// The strategy that knows one catalogue's document shape (FR-025).
///
/// Implementers are stateless unit values selected by [`provider_for`]. The trait
/// is the seam that makes the catalogue domain catalogue-agnostic: the fetch path
/// and the fixture seam both parse through a provider rather than assuming a
/// single catalogue format.
pub trait ConnectorProvider: Send + Sync {
    /// The catalogue this provider parses.
    fn kind(&self) -> CatalogueKind;

    /// Normalise a catalogue document into the [`ConnectorEntry`] list plus a
    /// count of the entries that could not be normalised.
    ///
    /// `origin` is the endpoint URL the bytes were fetched from; it anchors the
    /// default `source`. Malformed JSON and an unrecognised shape are reported as
    /// [`CatalogueError`]; an individual entry that cannot be normalised is **not**
    /// an error here - it is dropped from the returned list and its count carried
    /// in [`NormalisedEntries::skipped`] for the caller to add to the descriptor
    /// skips (FR-025).
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueError::MalformedJson`] when the document is not valid
    /// JSON and [`CatalogueError::MalformedShape`] when it is not this provider's
    /// shape.
    fn parse_entries(
        &self,
        bytes: &[u8],
        origin: &Url,
    ) -> Result<NormalisedEntries, CatalogueError>;
}

/// The entries a [`ConnectorProvider`] could normalise from a document, plus the
/// count it had to skip (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NormalisedEntries {
    /// The entries that normalised successfully, in document order.
    pub entries: Vec<ConnectorEntry>,
    /// How many entries were dropped because they could not be normalised.
    pub skipped: usize,
}

/// The provider for `kind` (FR-025).
#[must_use]
pub fn provider_for(kind: CatalogueKind) -> Box<dyn ConnectorProvider> {
    match kind {
        CatalogueKind::Claude => Box::new(ClaudeConnectorProvider),
    }
}

/// A raw parse failure reported by a [`ConnectorProvider`] (FR-031).
///
/// This is the pre-conversion failure: it distinguishes a malformed document
/// from an unrecognised shape so the fetch can map it onto the reportable
/// [`ConnectorError`] variants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogueError {
    /// The document was not valid JSON.
    #[error("catalogue is not valid JSON: {detail}")]
    MalformedJson {
        /// The serde JSON failure detail, including the error position.
        detail: String,
    },

    /// The document parsed as JSON but was not the provider's shape.
    #[error("catalogue has an unexpected shape: {detail}")]
    MalformedShape {
        /// What was expected instead.
        detail: String,
    },
}

impl From<CatalogueError> for ConnectorError {
    fn from(error: CatalogueError) -> Self {
        match error {
            CatalogueError::MalformedJson { detail } => Self::CatalogueParse { detail },
            CatalogueError::MalformedShape { detail } => Self::CatalogueShape { detail },
        }
    }
}

/// The outcome of normalising a catalogue document (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedCatalogue {
    /// The entries that survived normalisation and descriptor validation, in
    /// document order.
    pub connectors: Vec<ConnectorDescriptor>,
    /// How many entries were skipped as malformed or unexpressible (FR-025).
    pub skipped: usize,
}

/// Parse `bytes` through `provider`, converting each entry into a validated
/// [`ConnectorDescriptor`] (FR-025).
///
/// An entry that cannot be normalised or whose servers are all unexpressible is
/// counted in [`ParsedCatalogue::skipped`] and never fails the document; only a
/// malformed document is an error (FR-031).
///
/// # Errors
///
/// Returns [`ConnectorError::CatalogueParse`] for malformed JSON and
/// [`ConnectorError::CatalogueShape`] for an unrecognised shape.
pub fn parse_catalogue(
    provider: &dyn ConnectorProvider,
    bytes: &[u8],
    origin: &Url,
) -> Result<ParsedCatalogue, ConnectorError> {
    let normalised = provider.parse_entries(bytes, origin)?;
    Ok(convert_entries(
        normalised.entries,
        normalised.skipped,
        ConnectorProvenance::Catalogue,
    ))
}

/// Convert normalised entries into validated descriptors, adding the entries that
/// could not be normalised to the skip count (FR-025).
#[must_use]
pub fn convert_entries(
    entries: Vec<ConnectorEntry>,
    pre_skipped: usize,
    provenance: ConnectorProvenance,
) -> ParsedCatalogue {
    let mut connectors = Vec::new();
    let mut skipped = pre_skipped;
    for entry in entries {
        match entry.to_descriptor(provenance) {
            Ok(descriptor) => connectors.push(descriptor),
            Err(error) => {
                skipped += 1;
                tracing::debug!(error = %error, "connector catalogue entry skipped");
            }
        }
    }
    ParsedCatalogue {
        connectors,
        skipped,
    }
}

/// The provider that parses ragent's own connector catalogue shape (FR-025).
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeConnectorProvider;

impl ConnectorProvider for NativeConnectorProvider {
    fn kind(&self) -> CatalogueKind {
        CatalogueKind::Claude
    }

    fn parse_entries(
        &self,
        bytes: &[u8],
        origin: &Url,
    ) -> Result<NormalisedEntries, CatalogueError> {
        let root: Value =
            serde_json::from_slice(bytes).map_err(|e| CatalogueError::MalformedJson {
                detail: e.to_string(),
            })?;
        let items = connectors_array(&root, &["connectors", "servers"])?;
        Ok(collect_entries(items, origin))
    }
}

/// The provider that parses Anthropic's connector-directory feed (FR-025).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeConnectorProvider;

impl ConnectorProvider for ClaudeConnectorProvider {
    fn kind(&self) -> CatalogueKind {
        CatalogueKind::Claude
    }

    fn parse_entries(
        &self,
        bytes: &[u8],
        origin: &Url,
    ) -> Result<NormalisedEntries, CatalogueError> {
        let root: Value =
            serde_json::from_slice(bytes).map_err(|e| CatalogueError::MalformedJson {
                detail: e.to_string(),
            })?;
        let items = connectors_array(&root, &["servers", "connectors", "plugins"])?;
        Ok(collect_entries(items, origin))
    }
}

/// Normalise each catalog item, counting the ones that refuse (FR-025).
fn collect_entries(items: &[Value], origin: &Url) -> NormalisedEntries {
    let mut entries = Vec::with_capacity(items.len());
    let mut skipped = 0usize;
    for item in items {
        match entry_from_value(item, origin) {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                skipped += 1;
                tracing::debug!(error = %error, "catalogue entry skipped");
            }
        }
    }
    NormalisedEntries { entries, skipped }
}

/// The catalogue array from a document: the first present candidate key, or a
/// bare top-level JSON array (FR-025).
///
/// Anything else is a [`CatalogueError::MalformedShape`].
fn connectors_array<'a>(root: &'a Value, keys: &[&str]) -> Result<&'a [Value], CatalogueError> {
    match root {
        Value::Array(items) => Ok(items),
        Value::Object(map) => {
            for key in keys {
                if let Some(Value::Array(items)) = map.get(*key) {
                    return Ok(items);
                }
            }
            Err(CatalogueError::MalformedShape {
                detail: format!(
                    "object is missing a {} array",
                    keys.iter()
                        .map(|k| format!("\"{k}\""))
                        .collect::<Vec<_>>()
                        .join(" or ")
                ),
            })
        }
        _ => Err(CatalogueError::MalformedShape {
            detail: "expected an object with a catalogue array or a bare array".to_string(),
        }),
    }
}

/// Normalise one item, unwrapping a nested `connector` object before delegating
/// to [`ConnectorEntry::from_value`] (FR-025).
fn entry_from_value(item: &Value, origin: &Url) -> Result<ConnectorEntry, ConnectorEntryError> {
    let candidate = item
        .as_object()
        .and_then(|map| map.get("connector"))
        .filter(|inner| inner.is_object())
        .unwrap_or(item);
    ConnectorEntry::from_value(candidate, origin)
}
