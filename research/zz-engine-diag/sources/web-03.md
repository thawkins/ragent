# Web source

- URL: https://rustlang.com.br/artigos/tokio-vs-async-std
- Title: Tokio vs Async-std: Qual Runtime Rust Usar? | Rust Brasil
- Author(s): Equipe Rust Brasil
- Language: Portuguese
- Published (UTC): 2026-02-23T00:00:00+00:00
- Captured (UTC): 2026-09-07T01:07:07.005067848+00:00
- Relevance: High — title + snippet match query


```text
This February 23, 2026 article from Rust Brasil compares Rust's two main async runtimes, Tokio and async-std (Rust ships no async runtime in its standard library). Tokio, first released in 2018 and maintained by the Tokio Team (including Alice Ryhl), has ~50M+ monthly downloads and very active development, while async-std (2019, async-rs community, ~10M+ downloads) offers an API mirroring Rust's std library but has slowed maintenance. Both use work-stealing multi-threaded schedulers, making performance differences negligible for most apps—though TCP echo benchmarks at 10,000 concurrent connections show Tokio at ~950K msg/s vs. async-std's ~850K msg/s, and Tokio's scheduler is battle-tested at Discord, Cloudflare, and AWS. Tokio also provides richer tooling (mpsc, broadcast, watch, and oneshot channels; interval timers; select!) and dominates the ecosystem, powering Axum, Actix Web, Warp, Reqwest, Hyper, SQLx, SeaORM, Tonic, rdkafka, lapin, and tracing, whereas async-std's ecosystem is limited to Tide and Surf. The article's practical recommendation is to use Tokio for most projects, including new ones, noting async-std's ideas influenced Tokio; cross-runtime interop is possible via the async-compat crate (0.2) but adds complexity and subtle issues.
```
