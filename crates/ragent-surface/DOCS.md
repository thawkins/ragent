# ragent-surface

Shared surface helpers for the `/plugins` and `/connectors` command families:
usage attribution, store-directory resolution, harness steps, manifest
enrichment, marketplace ledger editing, and JSON-schema sample generation.
Each helper has one implementation here so the two command surfaces cannot
drift. Consumed by `ragent-plugins`, `ragent-connectors` and `ragent-agent`.

## Workspace Dependencies

- ragent-config
- ragent-types

## External Dependencies

- reqwest (blocking), serde_json, thiserror, tracing, url

Dev-dependencies: tempfile.

## Features

None declared.

## Public API (crate root)

### Modules

- **config_splice** — Atomic JSON-config splice: `write_atomic_json`.
- **harness** — `/test` harness step model: `StepOutcome`, `HarnessStep`, `step`, `truncate`, `sample_for_schema`, `schema_type`.
- **help** — Usage attribution and tokenising: `attribution`, `subcommand_of`.
- **manifest** — Shared manifest-enrichment client, cache and parser: `ManifestMeta`, `ManifestUpdate`, `NetworkManifestSource`, `fetch_and_parse`, `parse_manifest_meta`, `github_owner_repo`, `cache_get`, `cache_put`; limits `MAX_ENRICH_ENTRIES`, `ENRICH_CONCURRENCY`, `MANIFEST_TIMEOUT`, `MAX_MANIFEST_BYTES`.
- **marketplace** — Marketplace-source resolution: `MarketplaceSource`, `MarketplaceSourceError`, `resolve_source`.
- **marketplace_ledger** — Marketplace-ledger editing in the global config: `add_marketplace`, `remove_marketplace`, `global_config_path`, `read_or_empty_object`, `marketplaces_array_mut`, `matches_entry`.
- **net** — `https`-only redirect policy: `https_only_redirects`, `MAX_REDIRECT_HOPS`.
- **scan** — Store-scan failure row: `ScanFailure`.
- **store** — Store-directory resolution and entry checks: `StoreDirs`, `store_dirs`, `store_dirs_at`, `store_entry_is_dir`, `store_entry_dir`, `read_ledger_bytes`, `quarantine_corrupt_ledger`, `MAX_LEDGER_BYTES`.

_Maintained alongside `crates/ragent-surface/src/lib.rs` and `crates/ragent-surface/Cargo.toml`; update when those change._
