# Rust Toolchain Configuration Review

Review date: 2026-09-06
Scope: dev/debug rebuild times for the ragent workspace (963 crates in `Cargo.lock`,
16 workspace crates, target dir currently 482 GB).

## Current Setup (what is in place)

| Item              | Value                                                        | Source                                                     |
| ----------------- | ------------------------------------------------------------ | ---------------------------------------------------------- |
| Active toolchain  | stable, `rust-toolchain.toml` pins `channel = "stable"`      | `rust-toolchain.toml`                                       |
| Other toolchains  | 1.97.0                                                       | rustup                                                     |
| Linker            | `clang` + `mold` (via `-C link-arg=-fuse-ld=mold`)     | `~/.cargo/config.toml`                                   |
| Debug info        | `debug = "line-tables-only"`                               | `~/.cargo/config.toml` (verified applied to real builds) |
| Compile cache     | sccache 0.17.0,`RUSTC_WRAPPER=sccache`, 11 GB cache dir    | env var                                                    |
| Machine           | 8 cores, 46 GiB RAM, swap 6.4G/8G in use                     | --                                                         |

## Required cargo plugins

The ragent build, test, doc and release flows depend on these cargo plugins
(installed with `cargo install <name> --locked`). Clippy, fmt, add, toolchain
ship inside the rustup toolchain and are listed for completeness.

| Plugin | Purpose in ragent |
| --- | --- |
| `cargo-llvm-cov` | Coverage measurement (`/rust-hygiene` and the STATS.md coverage table) |
| `cargo-nextest` | Faster parallel test runner used by CI flows |
| `cargo-audit` | Security advisory audit (part of the standard hygiene check) |
| `cargo-deny` | License / advisory / duplicate-crate gate (`deny.toml`) |
| `cargo-outdated` | Dependency freshness checks |
| `cargo-udeps` | Detect unused dependencies |
| `cargo-machete` | Faster unused-dependency check |
| `cargo-semver-checks` | Public-API semver diff for release flow |
| `cargo-bloat` | Binary size breakdown |
| `cargo-cache` | Inspect / clean the cargo caches |
| `cargo-sweep` | Reap stale target-dir artifacts (target-dir housekeeping, item 4) |
| `cargo-deb` | Build Debian packages for ragent releases |
| `cargo-rpm` | Build RPM packages for ragent releases |
| `cargo-release` | Release/version flow automation |
| `cargo-set-version` | Bump workspace versions in lockstep |
| `cargo-install-update` | Keep installed plugins up to date |

One-shot install:

```bash
for p in llvm-cov nextest audit deny outdated udeps machete semver-checks \
         bloat cache sweep deb rpm release set-version install-update; do
  cargo install "cargo-$p" --locked
done
```

### Already good -- do not change

- **mold + clang linker**: correct choice on Linux; keep.
- **`debug = "line-tables-only"`**: verified it reaches the rustc invocation
  (`debuginfo=line-tables-only`). Big win for both clean builds and rebuilds.
- **Separate `target/flycheck0` dir**: the editor runs `cargo check` in its own
  target dir, so it does not contend for the lock with manual `cargo build`.
  Keep this separation.
- **`CARGO_BUILD_JOBS` unset**: defaults to nproc (8), which is right for this box.
- ** Vendored patches** (`pdf-extract`, `html2text`) have no build-time cost.

## Recommendations (in priority order)

### 1. Pin the toolchain and stop using `RUSTUP_TOOLCHAIN` (DONE)

Every nightly update changes the compiler hash, which invalidates the **entire
sccache cache** and forces a full rebuild of all 963 dependency crates. With
nightly updating daily, that is potentially a full rebuild storm every day.

- `rust-toolchain.toml` exists in the project root and now pins
  `channel = "stable"` (previously a pinned nightly). This makes the build
  reproducible for CI and other machines.
- **Important**: `RUSTUP_TOOLCHAIN` (if still exported in the shell
  environment) **overrides** `rust-toolchain.toml`. Remove the env var for the
  pin to take effect.
- Note: 18 workspace crates became 16 after the `ragent-prompt_opt` removal.

### 2. Move `[profile.dev]` from `~/.cargo/config.toml` into the workspace `Cargo.toml`

The user-level `debug = "line-tables-only"` works, but it is invisible to
other machines and to CI (which will silently build with full debuginfo and
link 500 MB+ binaries). Add to the root `Cargo.toml`:

```toml
[profile.dev]
debug = "line-tables-only"
```

Verified: this applies correctly in a workspace root manifest and produces no
cargo "unused manifest key" warning. Keep the user-level copy if other
projects benefit, or drop it to avoid divergence.

### 3. sccache: raise the cache ceiling and understand the misses

Current stats: 520 requests, 175 executed, 175 hits (100% of executed),
**345 non-cacheable** (176 "multiple input files", 142 "incremental").

- **Raise `SCCACHE_CACHE_SIZE`** (default cap is 10G; the dir is already 11G).
  With 963 deps plus nightly churn, a small cache thrashes:

  ```bash
  export SCCACHE_CACHE_SIZE=50G
  ```

  (Set it in the shell profile or systemd user environment -- wherever
  `RUSTUP_TOOLCHAIN` is currently exported.)
- **The 142 "incremental" misses**: sccache cannot cache any crate compiled
  with `-C incremental` (on by default in the dev profile). Two coherent
  policies -- pick one:

  | Policy              | Command                             | Best for                                          |
  | ------------------- | ----------------------------------- | ------------------------------------------------- |
  | A. Cache everything | `CARGO_INCREMENTAL=0 cargo build` | Full/clean rebuilds, CI warmups, branch switching |
  | B. Keep incremental | default                             | Tight edit-compile loop on workspace crates       |

  A hybrid works well in practice: leave incremental on for day-to-day work,
  and run policy A occasionally (or on CI) to repopulate the sccache cache.
  Do not assume policy A is a pure win -- it removes incremental recompiles
  for the crate you are actively editing.
- The 176 "multiple input files" calls are cargo's internal compiler probes;
  they are cheap and not worth chasing.

### 4. Reclaim the target directory (482 GB)

`target/` is 482 GB, of which `target/debug/incremental` alone is 70 GB.
The incremental cache grows monotonically and is rarely reaped.

- One-off: `cargo clean` then one policy-A build to repopulate sccache.
- Ongoing: install `cargo-sweep` and run `cargo sweep --time 14` weekly, or
  script a `cargo clean` when free space drops below a threshold.
- Deleting `target/debug/incremental` is safe and recovers 70 GB instantly
  (next build recreates it).

### 5. Find the real hot spots with `--timings`

963 dependency crates is the structural cost. To see which ones dominate:

```bash
cargo build --timings
# opens target/cargo-timings/cargo-timing.html
```

Expect `tree-sitter` + its ~17 grammars, `tantivy`, `tokio`, `rustls`/`ring`,
and `zstd-sys` to dominate. Feature-trimming the workspace dependency table
(e.g. tokio features, `default-features = false` where possible) is the only
lever that shrinks the dependency floor.

### 6. Decide deliberately: nightly vs stable

Nightly was previously needed only for `-Z threads=8` (parallel rustc frontend).
Costs: daily cache invalidation (item 1), occasional nightly breakage,
extra memory from the parallel frontend. Gains: typically 5-15% on large
single crates; modest on a build dominated by many small crates.

- **Resolution (2026-09-12)**: the workspace moved to **stable** via
  `rust-toolchain.toml` (`channel = "stable"`). `-Z threads=8` was removed
  from `~/.cargo/config.toml` (nightly-only). The sccache cache now stays
  valid across stable point releases, and full rebuilds after toolchain
  updates become rare.
- Keep this section as the record of the decision. If a future feature
  requires nightly, pin `channel = "nightly-YYYY-MM-DD"` in
  `rust-toolchain.toml` and re-add `-Z threads=8` to `~/.cargo/config.toml`.
  Measure before re-switching: `time cargo build` on a clean target with each
  setup; only nightly-worthy if the win exceeds ~10%.

### 7. Release profile (context only)

`[profile.release]` uses `lto = true`, `codegen-units = 1`, `opt-level = "z"`
-- deliberately slow, size-optimised builds for distribution. If you ever need
a quick-but-optimised build, add:

```toml
[profile.reldev]
inherits = "release"
lto = "thin"
codegen-units = 16
opt-level = 2
```

...and build with `cargo build --profile reldev`. No change to the shipped
release profile.

### 8. Minor: memory pressure

Swap is 6.4G/8G used with 34G in page cache -- mostly harmless. If you see
OOM-ish stalls during big dependency builds, cap jobs with
`CARGO_BUILD_JOBS=6` rather than swapping.

## Verification performed

- `debug = "line-tables-only"` confirmed in the actual rustc command line for
  this workspace (verbose build probe).
- `[profile.dev]` in a workspace-root manifest confirmed to apply
  (`debuginfo=line-tables-only`) with no cargo warnings.
- Stable toolchain confirmed to reject `-Z threads` (nightly-only) -- flag
  removed from `~/.cargo/config.toml` when the stable move landed.
- sccache stats read via `sccache --show-stats`; cache dir measured at 11 GB.
- `target/` measured at 482 GB, incremental cache at 70 GB.
- Stable move verified: `rust-toolchain.toml` pins `channel = "stable"`,
  `rustup component list --installed` shows `llvm-tools` (needed by
  `cargo-llvm-cov`), and all required cargo plugins are present in
  `~/.cargo/bin` (see table above).

## Suggested order of operations

1. Locate and remove `RUSTUP_TOOLCHAIN=nightly` from wherever it is exported.
2. Add `rust-toolchain.toml` with `channel = "stable"` (done 2026-09-12).
3. Add `[profile.dev] debug = "line-tables-only"` to the root `Cargo.toml`.
4. `export SCCACHE_CACHE_SIZE=50G`.
5. Install the required cargo plugins (table above) and `rustup component add
   llvm-tools-preview`.
6. `cargo clean`, then one full build to repopulate sccache, and measure with
   `cargo build --timings` from there.
