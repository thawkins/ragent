//! Link and contradiction graph over the vault (FR-020).
//!
//! Edges are derived between notes — at least `cites`, `supports`,
//! `contradicts`, and `same-source` — and notes can be ranked by centrality.

pub mod centrality;
pub mod edge;
