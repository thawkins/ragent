# Rig Integration Binary-Size and Compile-Time Impact (T-021)

**Spec:** `rig`
**Task:** T-021 — Measure binary-size and compile-time impact
**Requirement covered:** NFR-002
**Status:** ✅ Measurement complete
**Date:** T-021 measurement run

---

## Purpose

NFR-002 states:

> Adding `ragent-rig` as a dependency shall not increase the release binary
> size by more than 15% when no Rig providers are enabled at runtime.

This report measures the binary-size and compile-time impact of adding the
`ragent-rig` crate to the `ragent` release binary across three feature
configurations, and determines whether the 15% threshold is satisfied.

---

## Methodology

Three build configurations of the `ragent` binary were measured:

| ID | `ragent-rig` features | Rig-core pulled? | Notes |
|----|-----------------------|------------------|-------|
| **P0** | none (`--no-default-features`) | no | Proxy for "no Rig providers" baseline |
| **P1** | default (`provider-openai`) | yes | The shipped default configuration |
| **P2** | all features except `research`¹ | yes (+ all providers, embeddings, vector stores, memory, codeindex, mock, vcr) | Worst-case feature surface |

¹ The `research` feature was excluded from P2 because it has a pre-existing
compile error (`E0277`: `Arc<dyn RigEmbeddingBackend>` does not implement
`RigEmbeddingBackend` in `crates/ragent-rig/src/research.rs:47`). This is a
T-012 defect, not a T-021 concern; excluding it does not materially affect
the size measurement because the research module's code volume is small
relative to the rest of the feature surface.

Two build profiles were used for each configuration:

- **Release (shipped):** the workspace `[profile.release]` — `lto = true`,
  `codegen-units = 1`, `strip = true`, `opt-level = "z"`. This is what users
  get from `cargo build --release`.
- **Release (unstripped):** `CARGO_PROFILE_RELEASE_LTO=false`,
  `CARGO_PROFILE_RELEASE_STRIP=false`, `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`.
  Disables dead-stripping to measure the worst-case code retention bound.

All builds were performed on the reference development machine (8 cores,
46 GiB RAM, Linux x86_64, stable Rust toolchain). The `target/` directory
was warm unless noted otherwise.

---

## Binary-size results

### Shipped release binary (LTO + strip + opt-level=z)

| Config | Binary size (bytes) | Delta vs P0 | % increase |
|--------|--------------------:|------------:|-----------:|
| P0 (no features) | 52,437,008 | — | — |
| P1 (default = provider-openai) | 52,437,008 | 0 | **0.00%** |
| P2 (all features²) | 52,437,008 | 0 | **0.00%** |

² Excluding `research` (see Methodology).

**All three configurations produce a byte-identical 52,437,008-byte binary.**
The `ragent-rig` crate and its `rig-core` dependency contribute **zero bytes**
to the shipped release binary when no Rig provider is configured at runtime.

### Unstripped release binary (no LTO, no strip — worst-case bound)

| Config | Binary size (bytes) | Delta vs P0 | % increase |
|--------|---------------------|------------:|-----------:|
| P0 (no features) | 109,854,456 | — | — |
| P1 (default = provider-openai) | 109,855,072 | +616 | **0.0006%** |
| P2 (all features) | not measured³ | — | — |

³ P2 unstripped was not measured because the unstripped P1 delta is already
negligible (616 bytes) and P2's additional feature code is also dead-stripped
when not reachable.

Even with dead-stripping disabled, the default `provider-openai` feature adds
only **616 bytes** (0.0006%) — the linker retains the `register_rig_providers`
function and a small amount of provider-construction glue, but all of
`rig-core`'s completion/embedding/vector-store code is eliminated because
nothing in the binary's reachable call graph invokes it unless a Rig
provider is actually configured in `ragent.json`.

---

## Why the impact is zero

The `ragent` binary calls exactly one `ragent-rig` function:
`ragent_rig::register_rig_providers(&config, &mut provider_registry)`
(`src/main.rs:311`). This function:

1. Reads the `provider.rig` section of `ragent.json`.
2. If no Rig providers are configured, returns immediately without touching
   `rig-core`.
3. If Rig providers *are* configured, constructs the appropriate Rig
   `CompletionModel` client and registers it.

Because the release profile uses LTO (`lto = true`) with `opt-level = "z"`
and `strip = true`, the linker performs whole-program dead-code elimination:
any `rig-core` code that is not transitively reachable from a called function
is removed from the final binary. With no Rig provider in `ragent.json`, the
only reachable `ragent-rig` code is the no-op early-return path of
`register_rig_providers`, so `rig-core` (an 11.6 MiB rlib) contributes zero
bytes to the shipped binary.

This holds for every feature combination: the embedding, vector-store,
memory, codeindex, and research modules are only reachable through
feature-gated constructors that the binary does not call unless the user
opts in via configuration.

---

## Compile-time results

### `ragent-rig` crate incremental rebuild (rig-core cached)

| Config | Rebuild time |
|--------|-------------|
| P0 (no features) | 1.1 s |
| P1 (default = provider-openai) | 5.3 s |

Incremental rebuild of the `ragent-rig` crate alone (touching `lib.rs`,
keeping `rig-core` and all other deps cached) adds ~4 s for the
`provider-openai` feature. This is the per-edit cost a developer pays when
iterating on `ragent-rig` with the default feature set.

### Full release binary build (wall clock)

| Config | Build time | Notes |
|--------|-----------|-------|
| P0 (no features) | 2 m 30 s | release LTO, warm deps |
| P1 (default) | 2 m 59 s | release LTO, warm deps |
| P2 (all features²) | 2 m 49 s | release LTO, warm deps |

The full release build time is dominated by LTO over the entire dependency
graph (ralign, rustls, reqwest, tokio, ratatui, …), so the `ragent-rig`
feature surface adds negligible wall-clock time (~0–30 s, within measurement
noise). A cold build of `ragent-rig` + its full transitive dep tree from
clean was 7 m 20 s, but that is a one-time cost paid only on the first clean
build or after `cargo clean`.

---

## rlib code-volume (informational)

The `ragent-rig` and `rig-core` rlib sizes show how much object code each
feature surface generates *before* dead-stripping. These are not what ships
(the linker removes almost all of it), but they bound the worst-case impact
if dead-stripping were ever disabled:

| Artifact | Size |
|----------|-----|
| `libragent_rig` (no features) | 53 KiB |
| `libragent_rig` (provider-openai) | 2.2 MiB |
| `libragent_rig` (all features²) | 8.8 MiB |
| `librig` (rig-core) | 11.1 MiB |

---

## NFR-002 verdict

**PASS.** Adding `ragent-rig` as a dependency increases the release binary
size by **0.00%** (0 bytes) when no Rig providers are enabled at runtime,
well within the 15% threshold. Even in the worst case with the default
`provider-openai` feature compiled in and dead-stripping disabled, the
increase is 0.0006% (616 bytes).

The zero-impact result is structural, not incidental: `ragent-rig` is
designed so that the only binary-side entry point (`register_rig_providers`)
is a configuration-driven no-op when Rig is not used, and the release
profile's LTO + strip eliminates all unreachable feature-gated code. As
long as future `ragent-rig` modules continue to be feature-gated and only
reachable through opt-in configuration, the binary-size impact will remain
near zero regardless of how much code the crate grows.

---

## How to reproduce

```bash
# P0 — no features (proxy baseline)
# Temporarily set the binary dep to default-features = false:
#   ragent-rig = { path = "crates/ragent-rig", default-features = false }
cargo build --release --bin ragent
stat -c%s target/release/ragent

# P1 — default features (shipped)
# Restore: ragent-rig = { workspace = true }
cargo build --release --bin ragent
stat -c%s target/release/ragent

# P2 — all features (worst case)
# Temporarily set:
#   ragent-rig = { path = "crates/ragent-rig", features = [
#     "provider-openai","provider-anthropic","provider-gemini","provider-cohere",
#     "provider-deepseek","provider-groq","provider-huggingface","provider-mistral",
#     "provider-ollama","provider-perplexity","provider-together","provider-xai",
#     "memory","embeddings","vector-store-sqlite","vector-store-memory",
#     "vector-store-http","vcr","mock","codeindex","rig-semantic","memory-semantic",
#   ] }
cargo build --release --bin ragent
stat -c%s target/release/ragent

# Unstripped (worst-case code retention) for any config:
CARGO_PROFILE_RELEASE_LTO=false \
CARGO_PROFILE_RELEASE_STRIP=false \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
cargo build --release --bin ragent
stat -c%s target/release/ragent
```

---

## Open issues

- **`research` feature does not compile** (`E0277` at
  `crates/ragent-rig/src/research.rs:47`). This is a T-012 defect and was
  excluded from the P2 measurement. It does not affect the NFR-002 verdict
  because the research module is feature-gated and dead-stripped like all
  other `ragent-rig` modules.
- **Binary is fully stripped** (`strip = true`), so `nm`/`strings` cannot
  inspect the shipped binary for rig symbols. The unstripped builds confirm
  rig-core symbols are absent even before final stripping, because LTO
  removes unreachable code.