//! GitHub API client for ragent.
//!
//! Provides [`GitHubClient`] for authenticated GitHub API calls.
//! Token resolution order: `GITHUB_TOKEN` env var → `~/.ragent/github_token`.

pub mod auth;
pub mod client;

pub use auth::{delete_token, device_flow_login, load_token, save_token};
pub use client::GitHubClient;
