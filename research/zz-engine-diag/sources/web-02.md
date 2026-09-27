# Web source

- URL: https://async.rs/blog
- Title: async-std - Blog
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-07T01:07:18.984248958+00:00
- Relevance: Medium — partial query match


```text
The async.rs blog contains two posts about async-std, a stable port of Rust's standard library to the async/await world (first announced August 16). The first post describes a proposed new runtime/scheduler — adapting ideas from the Go runtime — that auto-detects blocking (eliminating the need for `spawn_blocking`), adapts between single- and multi-threaded execution on demand, removes unsafe code, and outperformed the old scheduler in benchmarks run on EC2 instances (m5a.8xlarge server, m5a.16xlarge wrk client); however, the note states this scheduler was ultimately not merged. The second post announces async-std 1.0, released alongside Rust 1.39 (which stabilized async/.await), built on five values — stability, ergonomics, accessibility, integration, and speed — with highlights including JoinHandle-based single-allocation tasks, a futures-aware sync module, full documentation, and an accompanying book. Microbenchmarks show `async_std::sync::Mutex` at least 2x faster under contention than futures-intrusive, tokio, and futures alternatives (893,650 ns/iter vs. 1,747,920–2,614,997), and task benchmarks up to ~2x faster than tokio (yield_many x1.95, spawn_many x1.69, ping_pong x1.39, chained_spawn x1.04). Upcoming features behind the `unstable` gate include fast bounded MPMC channels with backpressure, `spawn_blocking`, and `yield_now`; the project has 59 contributors and is funded by Ferrous Systems and Yoshua Wuyts, with an OpenCollective page for support.
```
