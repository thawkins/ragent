# Local source (in-project)

- Path: APERFPLAN.md
- Relevance: 500 match(es) on: agent, an, for, …(+19) — "# APERFPLAN — Agent & Team Performance Improvement Plan"
- Captured (UTC): 2026-06-25T05:53:23.378296753+00:00

```text
Excerpt — 500 keyword match(es)

▶    1:    1  # APERFPLAN — Agent & Team Performance Improvement Plan
     2:    2  
▶    3:    3  **Created:** 2026-06-22
▶    4:    4  **Source reviews:**
▶    5:    5  - `docs/reports/ragent-agent-performance-review.md` (swarm-s1, 2026-06-22)
▶    6:    6  - `docs/swarm-s2-ragent-team-performance-review.md` (swarm-s2, 2026-06-22)
▶    7:    7  **Related spec:** `specs/AgentPerf/` (implemented — AgentPerf v1)
     8:    8  
…
    12:   12  
▶   13:   13  Two independent performance reviews of the `ragent-agent` and `ragent-team` crates
▶   14:   14  identified **53 distinct issues** (10 high, 18 medium, 15 low in `ragent-agent`;
▶   15:   15  plus 18 issues across 10 themes in `ragent-team`). The findings converge on four
▶   16:   16  pervasive anti-patterns:
    17:   17  
▶   18:   18  1. **Blocking synchronous I/O on async executor threads.** Both crates perform
▶   19:   19     `std::fs` reads/writes, `serde_json` serialization, `std::process::Command`
▶   20:   20     spawns, and SQLite queries directly inside `async fn` bodies. Under concurrent
▶   21:   21     teammate activity or multi-step agent loops, this starves the tokio runtime,
▶   22:   22     stalls other futures, and inflates latency unpredictably. This is the
▶   23:   23     **single highest-leverage fix** — wrapping all file and DB I/O in
▶   24:   24     `tokio::task::spawn_blocking`.
    25:   25  
▶   26:   26  2. **Redundant loads / N+1 patterns.** `Config::load()` is called 3–4× per
▶   27:   27     `process_user_message`. `TeamStore::load()` is called 5× per `team_task_claim`.
▶   28:   28     `ToolRegistry::definitions()` re-sorts on every uncached call. `Mailbox::push`
▶   29:   29     re-reads and re-serializes the entire mailbox file for every single message.
▶   30:   30     These patterns multiply disk I/O and CPU work by a factor of 3–10× with no
▶   31:   31     functional benefit.
    32:   32  
▶   33:   33  3. **Excessive cloning of large data on the hot path.** `system_prompt`,

… (470 more match(es) elided)

```
