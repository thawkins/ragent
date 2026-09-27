# Web source

- URL: https://qiita.com/Aqua-218/items/b953029e6f1095f53c17
- Title: tokio vs async-std、どっちを選ぶべきか本気で考えた - Qiita
- Author(s): @aqua_developer
- Language: Japanese
- Published (UTC): 2025-12-10T03:51:21+00:00
- Captured (UTC): 2026-09-07T01:07:03.287817815+00:00
- Relevance: High — title + snippet match query


```text
This Qiita article compares Rust's two major async runtimes, tokio and async-std. Tokio is the most popular (overwhelming crate.io downloads) and feature-rich (file I/O, timers, synchronization primitives, runtime customization), with a large ecosystem including axum, reqwest, tonic, sqlx, hyper, and tower; async-std offers a std-like API, simplicity, and low learning cost, with crates like tide, surf, and async-tungstenite. Code examples for file reading, sleep, task spawning, Mutex, and channels show the two APIs are nearly identical, and benchmarks indicate no significant performance difference overall (tokio is better at many short-lived tasks, async-std at startup time; tokio additionally supports io_uring on Linux 5.1+ and uses a work-stealing scheduler). However, tokio's ecosystem is overwhelmingly larger—new crates typically support tokio first—though runtime-agnostic crates (futures 0.3, async-trait) and async-compat 0.2 (which runs async-std code under tokio) exist, and sqlx 0.7 supports both runtimes. The article concludes that as of 2025, tokio should be the default choice unless you specifically prefer std-style APIs, tide/surf, or simplicity, and recommends checking which runtime your required crates depend on.
```
