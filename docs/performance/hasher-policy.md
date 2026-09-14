# Hasher policy (PERF-080)

Status: active. Applies to every map/set in the workspace that is keyed by a
short or non-adversarial value.

## The rule

Choose the hasher by key type and contention, not by habit:

| Situation | Use | Why |
| --------- | --- | --- |
| Integer, enum, or short bounded non-adversarial keys (`usize`, `u64`, `(usize, usize)`, `PathBuf` on a hot walk) | `rustc_hash::FxHashMap` / `FxHashSet` | FxHash is ~2-5x faster than the default SipHash-1-3 for short keys and has no extra dependency (already a workspace dep, PERF-031). |
| Long string keys where collision resistance is *not* security-relevant but the key space is large | `FxHashMap` / `FxHashSet` | Same reasoning; FxHash is still fine for namespaced identifiers and paths that we generate. |
| Keyed by an attacker-controlled string on a public boundary (HTTP header, user-supplied name hashed directly) | `std::collections::HashMap` or `ahash` if already in the dependency graph | SipHash/ahash resist HashDoS. Do not swap one in unless the audit shows it there. |
| Contended concurrent map (read/write from many tasks) | `dashmap::DashMap` | Per-shard locking; `Mutex<HashMap>` serialises every access. |

The default `std::collections::HashMap` is not banned — it stays wherever the
key is genuinely adversarial or the map is not hot. The policy is "prefer FxHash
for short non-adversarial keys", not "ban the default".

`hashbrown` is deliberately **not** added as a direct dependency: it is already
transitively present (via ahash-backed crates) and `rustc_hash` covers the same
need with a smaller, already-vendored surface. Add `hashbrown` only if a future
consumer needs its raw API (e.g. `raw_entry`).

## Audited sites (Appendix C of PERFPLAN.md)

Converted in PERF-080:

| Crate | File | Site | Was | Now |
| ----- | ---- | ---- | --- | --- |
| `ragent-agent` | `trigger/runtime.rs` | `dedup_cache` (hash -> entry) | `HashMap` | `FxHashMap` |
| `ragent-agent` | `reference/fuzzy.rs` | project-file cache | `HashMap` | `FxHashMap` (PERF-051) |
| `ragent-research` | `search_budget.rs` | query cache (normalised query text) | `HashMap` | `FxHashMap` |
| `ragent-research` | `manager.rs` | item cache, `disk_names`, `cache_names` | `HashMap`/`HashSet` | `FxHashMap`/`FxHashSet` |
| `ragent-research` | `locus.rs` | term -> hits | `HashMap` | `FxHashMap` |
| `ragent-research` | `contradiction.rs` | dimension claims, seen pairs, best-by-pair | `HashMap` | `FxHashMap` |
| `ragent-research` | `corpus_critic.rs` | source-to-loci map | `HashMap` | `FxHashMap` |
| `ragent-research` | `session.rs` | `build_web_index_map` | `HashMap` | `FxHashMap` |
| `ragent-tools-extended` | `masterfetch/search/consensus.rs` | URL -> group entries | `HashMap` | `FxHashMap` |
| `ragent-tools-extended` | `masterfetch/search/engine.rs` | seen-URL set | `HashSet` | `FxHashSet` |

Sites intentionally left on the default hasher:

- `Manager::union_with_existing`/`disk_names` clone a caller-supplied set — the
  input type is `HashSet<String>` from the caller, so the signature keeps the
  default hasher.
- Any map keyed by a value that crosses a trust boundary is left on the default.

## Verification

Each converted site keeps its public signature byte-for-byte identical (same
key and value types); only the alias changes, so no test updates are required.
`cargo test -p ragent-research`, `cargo test -p ragent-tools-extended`, and
`cargo test -p ragent-agent` verify behaviour is unchanged.
