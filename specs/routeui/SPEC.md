---
status: implemented
audit:
  - { time: 1784025435, from: "none", to: "draft", actor: "system" }
---
# Specification: Router Provider TUI Configuration Interface

## Spec ID

routeui

## Summary

This specification defines a TUI interface for configuring the `router` virtual
provider. The router virtual provider selects one of four complexity tiers
(`SIMPLE`, `MEDIUM`, `COMPLEX`, `REASONING`) for each request and delegates the
actual LLM call to a concrete provider/model pair configured for that tier. The
feature adds an interactive setup flow reachable through the existing `/provider`
slash command, so users can select multiple configured providers, assign them to
the four router buckets, and persist the resulting `provider.router` cluster
configuration in `ragent.json`.

## Scope

### In Scope

- Register the `router` virtual provider as a first-class selectable entry in the
  `/provider` setup flow.
- Add a TUI panel for creating and editing a router cluster.
- Allow multi-selection of already-configured concrete providers that will feed
  the router buckets.
- Assign selected provider/model pairs to the four router tiers (`SIMPLE`,
  `MEDIUM`, `COMPLEX`, `REASONING`).
- Persist the tier mappings into `provider.router.tiers` in `ragent.json`.
- Visualise the active router configuration via `/provider show`.
- Keep the existing classifier, boundaries, and weights configuration accessible
  only through manual `ragent.json` editing in this phase.

### Out of Scope

- Re-implementing or replacing the router classification algorithm.
- Live router throughput graphs, cost analytics, or request history.
- Automatic tier recommendation based on observed prompts.
- Editing classifier weights or boundary thresholds in the TUI.

## Context

### Existing Router Implementation

- `crates/ragent-llm/src/providers/router.rs` defines `RouterProvider`, which
  implements the `Provider` trait with `id() == "router"` and `name() == "Model
  Router"`.
- `crates/ragent-llm/src/providers/router_config.rs` defines `RouterConfig`,
  `TierConfig`, `TierEntry`, and `Tier`.
- The router is disabled by default and currently configured entirely through
  `provider.router` in `ragent.json`.
- The `RouterProvider::create_client` method constructs a `RouterClient` that
  reads the current tier configuration at request time.

### Existing `/provider` Flow

- `/provider` opens `ProviderSetupStep::SelectProvider` and lets the user pick a
  concrete provider to configure.
- `/provider show` opens `ProviderSetupStep::ShowProviderConfig` to display
  already-configured providers.
- The setup steps live in `crates/ragent-tui/src/app/state.rs` and are rendered
  by the TUI layout code in `crates/ragent-tui/src/layout.rs`.

### Provider Registry

- `ProviderRegistry` contains concrete providers and exposes metadata such as
  `id`, `name`, and `default_models`.
- A configured provider is one whose credentials are available either from the
  environment or from the ragent database.

## Assumptions

1. The router is a virtual provider that never makes API calls itself; it only
   routes to already-configured concrete providers.
2. The user must configure at least one concrete provider before a router
   cluster can be created.
3. Provider credentials are not required for the router entry itself, but every
   concrete provider assigned to a bucket must already have working credentials.
4. The existing `RouterConfig::default()` provides sensible fallback tier
   models if a tier is left unconfigured in the TUI.
5. The router configuration is stored in `ragent.json` and reloaded on startup.

## Requirements

### Ubiquitous

- **FR-001** The system shall expose the `router` virtual provider in the
  provider registry as a model provider with `id` `"router"` and display name
  `"Model Router"`.
- **FR-002** The system shall include `"router"` as a selectable item in the
  `/provider` provider picker.
- **FR-003** The system shall provide a dedicated `ProviderSetupStep` variant
  for router cluster configuration.
- **FR-004** The system shall list all currently configured concrete providers
  when the user enters the router setup flow.
- **FR-005** The system shall allow the user to select multiple configured
  providers from the list and include them in the router cluster.
- **FR-006** The system shall provide a four-bucket assignment view, one bucket
  per router tier: `SIMPLE`, `MEDIUM`, `COMPLEX`, and `REASONING`.
- **FR-007** The system shall allow the user to drag or move a selected
  provider/model pair into any of the four router buckets.
- **FR-008** The system shall persist the resulting tier mappings to
  `provider.router.tiers` in `ragent.json` in the same serialised form used by
  `RouterConfig`.
- **FR-009** The system shall enable the router (`provider.router.enabled:
  true`) when a non-empty cluster is saved.
- **FR-010** The system shall display the active router cluster configuration in
  `/provider show` output, including each tier and its assigned models.

### Event-Driven

- **FR-011** When the user selects the `router` item from the `/provider`
  provider picker, the system shall transition into the router cluster setup
  step.
- **FR-012** When the user toggles a concrete provider in the multi-selection
  list, the system shall add or remove that provider from the cluster palette.
- **FR-013** When the user assigns a provider/model pair to a tier bucket, the
  system shall immediately update the in-memory draft `RouterConfig`.
- **FR-014** When the user confirms the router cluster, the system shall write
  the updated `ragent.json` and emit a status message confirming the router is
  enabled.
- **FR-015** When the user cancels the router setup flow, the system shall
  discard the draft configuration and return to the normal chat input state.
- **FR-016** When no concrete providers are configured and the user attempts to
  set up the router, the system shall display a help message directing the user
  to configure a concrete provider first.

### State-Driven

- **FR-017** While the router setup panel is open, the TUI shall render the
  provider multi-selection list on the left and the four bucket columns on the
  right.
- **FR-018** While a provider is selected in the multi-selection list, the
  system shall show the provider's available models and allow the user to pick
  one before assigning it to a bucket.
- **FR-019** While a bucket has at least one assigned model, the system shall
  display the entries in priority order (primary first, fallback below).
- **FR-020** While the router is the active provider, the status bar shall
  display `"Model Router"` as the provider name.

### Optional

- **FR-021** The router setup panel may allow the user to re-order models within
  a bucket using `Ctrl+Up`/`Ctrl+Down`.
- **FR-022** The router setup panel may display a cost estimate per bucket based
  on the configured model's `Cost` metadata.
- **FR-023** The router setup panel may pre-fill the `MEDIUM` bucket with the
  user's current active provider/model pair when the flow is first opened.

### Unwanted

- **FR-024** The system shall not allow the router to route to itself or to
  another router cluster, preventing recursive routing.
- **FR-025** The system shall not permit saving a router cluster where every
  tier is empty; at least one tier must contain a valid provider/model pair.
- **FR-026** The system shall not overwrite existing `provider.router` classifier
  weights or boundary thresholds when saving the cluster through the TUI.

## Non-Functional Requirements

- **NFR-001** The router setup panel shall be navigable entirely with the
  keyboard (arrow keys, Tab, Space, Enter, Esc).
- **NFR-002** The panel state shall be unit-testable without launching a full
  terminal backend.
- **NFR-003** Saving the router cluster shall not block the TUI event loop for
  more than 100 ms.
- **NFR-004** The router TUI code shall live in `ragent-tui` and reuse the
  existing `ProviderSetupStep` state machine rather than introducing a separate
  modal system.

## Acceptance Criteria

1. Running `/provider`, selecting `Model Router`, and completing the setup flow
   writes a valid `provider.router` block to `ragent.json`.
2. The saved `provider.router.enabled` field is `true` after the flow.
3. `/provider show` displays the router and its tier mappings.
4. Selecting the router as the active provider makes subsequent chat requests
   route through the configured tiers.
5. A router cluster cannot be saved with all four tiers empty.
6. Recursive router assignments are rejected in the UI.
7. Existing `provider.router.weights` and `provider.router.boundaries` values
   are preserved when the cluster is saved through the TUI.

## Related Work

- Existing router implementation: `crates/ragent-llm/src/providers/router.rs`,
  `router_config.rs`, `router_client.rs`, `router_classifier.rs`.
- Existing `/provider` flow: `crates/ragent-tui/src/app/state.rs` and
  `crates/ragent-tui/src/app/slash.rs`.
