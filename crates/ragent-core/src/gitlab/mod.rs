//! GitLab API client for ragent.
//!
//! Provides [`GitLabClient`] for authenticated GitLab API calls.
//! Credentials are stored encrypted in the ragent SQLite database and
//! can be overridden via `ragent.json` or environment variables.

pub mod auth;
pub mod client;

pub use auth::{
    GitLabConfig, delete_config, delete_token, load_config, load_token, migrate_legacy_files,
    save_config, save_token,
};
pub use client::GitLabClient;
