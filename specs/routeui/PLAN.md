# Implementation Plan: Router Provider TUI Configuration Interface

## Spec ID

routeui

## Objective

Add a TUI-driven configuration flow for the `router` virtual provider so users
can create a router cluster from already-configured providers, assign
provider/model pairs to the four router buckets, and persist the result to
`ragent.json` without hand-editing JSON.

## Approach

1. Extend the existing `ProviderSetupStep` state machine with a dedicated
   router setup variant and sub-steps.
2. Reuse the `ConfiguredProvider` list and `ProviderRegistry` metadata for model
   discovery.
3. Build a two-pane panel: provider multi-selection on the left, four tier
   buckets on the right.
4. Keep classifier weights and boundaries read-only in the TUI; preserve them
   on save by round-tripping the existing `RouterConfig`.
5. Wire keyboard navigation into the existing input handler rather than adding a
   new global input mode.
6. Add unit tests for state transitions and persistence, plus one integration
   test for the `/provider` slash-command entry point.

## Assumptions and Risks

- The `RouterProvider` is already registered in the provider registry and can be
  selected; this work only adds the interactive setup flow.
- Some providers (e.g. Azure Resource) expose models only after extra setup; the
  initial implementation may show only `default_models()` and skip dynamic
  discovery for the router flow.
- Recursive router-to-router assignments must be explicitly filtered in the UI.
- Saving `ragent.json` currently happens through `ragent_config`; the panel will
  call the same path to avoid introducing a second persistence mechanism.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add router setup enum variants to `ProviderSetupStep` | FR-003, NFR-004 | S | Critical | completed | — |
| T-002 | Include `router` in the `/provider` provider picker list | FR-001, FR-002 | S | Critical | completed | — |
| T-003 | Build configured-provider query helper for the router flow | FR-004 | S | Critical | completed | — |
| T-004 | Implement router cluster draft state in `AppState` | FR-006, FR-013 | M | Critical | completed | T-001, T-003 |
| T-005 | Render provider multi-selection list in the router panel | FR-005, FR-007 | M | High | completed | T-004 |
| T-006 | Render the four tier bucket columns | FR-006, FR-019 | M | High | completed | T-004 |
| T-007 | Implement provider/model selection and bucket assignment input handling | FR-007, FR-012 | M | Critical | completed | T-005, T-006 |
| T-008 | Implement model picker for a selected provider in the router flow | FR-018 | M | High | completed | T-005 |
| T-009 | Implement save/cancel logic and `ragent.json` persistence | FR-008, FR-009, FR-014 | M | Critical | completed | T-004 |
| T-010 | Preserve existing classifier weights and boundaries on save | FR-026 | S | High | completed | T-009 |
| T-011 | Add validation: reject recursive router assignments | FR-024 | S | High | completed | T-007 |
| T-012 | Add validation: require at least one non-empty tier | FR-025 | S | High | completed | T-009 |
| T-013 | Update `/provider show` to render router cluster | FR-010 | M | Medium | completed | T-009 |
| T-014 | Update status bar provider label when router is active | FR-020 | S | Medium | completed | T-002 |
| T-015 | Add empty-state help when no concrete providers are configured | FR-016 | S | Medium | completed | T-003 |
| T-016 | Optional: support re-ordering models within a bucket | FR-021 | S | Low | completed | T-007 |
| T-017 | Optional: show per-bucket cost estimates | FR-022 | S | Low | completed | T-006 |
| T-018 | Write unit tests for router setup state machine | NFR-002 | M | High | completed | T-004 |
| T-019 | Write integration test for `/provider` → router setup flow | NFR-002 | M | High | completed | T-002 |
| T-020 | Update slash-command help text for `/provider` | FR-002 | S | Low | completed | T-002 |
| T-021 | Update CHANGELOG.md and routeui spec status | — | S | Low | completed | T-013 |
## Milestones

### Milestone 1 — State machine and picker (T-001, T-002, T-003, T-020)

The `router` entry appears in the `/provider` provider picker, selecting it
enters a new router setup state, and the configured-provider helper is in
place.

### Milestone 2 — Draft cluster UI (T-004, T-005, T-006, T-008)

The two-pane router panel renders: provider multi-selection on the left, four
buckets on the right, and a model picker for the selected provider.

### Milestone 3 — Assignment and persistence (T-007, T-009, T-010, T-011, T-012, T-015)

Users can assign models to buckets, validate the cluster, save to
`ragent.json`, and the system preserves existing classifier settings.

### Milestone 4 — Visualisation and polish (T-013, T-014, T-016, T-017)

`/provider show` displays the router cluster, the status bar reflects the
active router, and optional reordering/cost features are added.

### Milestone 5 — Tests and docs (T-018, T-019, T-021)

Unit and integration tests pass, the spec moves from `draft` to
`implemented`, and the changelog is updated.

## Definition of Done

- `cargo check -p ragent-tui` and `cargo test -p ragent-tui` pass.
- All acceptance criteria 1–7 are demonstrated by manual or automated test
  evidence.
- The spec status is moved from `draft` to `implemented` with audit metadata.
- No existing `/provider` flow for concrete providers is broken.
- Recursive router assignments and empty clusters are rejected in the UI.
- `CHANGELOG.md` documents the new router TUI setup flow.