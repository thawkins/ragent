//! `/plugins add` and `/plugins remove` command glue (spec `plugins` T-011;
//! FR-007, FR-010).
//!
//! This module is the single entry point the command surfaces (the TUI
//! `/plugins` slash family and, later, `ragent plugins` CLI parity in T-017)
//! call for the two store-mutating subcommands. It parses the raw argument
//! text, invokes the store operation in [`crate::add`](mod@crate::add) /
//! [`crate::remove`](mod@crate::remove), and
//! returns the report string to print via [`crate::report`]. Keeping the
//! parse-and-run here means the two surfaces share one wording and one set of
//! guards.

use std::path::Path;

use crate::add::add;
use crate::help::attribution;
use crate::remove::remove;
use crate::report::{add_error_report, add_report, remove_error_report, remove_report};
use crate::store::StoreDirs;

/// A parsed store subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreCommand {
    /// `add <source> [--force]`.
    Add {
        /// The source path or `https://` URL to install from.
        source: String,
        /// Overwrite an existing plugin with the same id.
        force: bool,
    },
    /// `remove <pluginid>`.
    Remove {
        /// The plugin id to uninstall.
        plugin_id: String,
    },
}

/// Why a store subcommand could not be parsed (malformed arguments — reported
/// as an `[err]` row that changes no state, per the SPEC error policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreArgError {
    /// `add` was given no source.
    MissingAddSource,
    /// `remove` was given no plugin id.
    MissingRemoveId,
}

impl StoreArgError {
    /// Render the usage error for the offending subcommand.
    #[must_use]
    pub fn report(self, sub: &str) -> String {
        match self {
            Self::MissingAddSource => format!(
                "{}\n\n[err] Missing <source>.\n\n\
                 Usage: `/plugins add <source> [--force]`\n\
                 Source forms: a local directory, a local `.zip`/`.tar.gz` file, or an \
                 `https://` URL ending in `.zip`/`.tar.gz`.",
                attribution("add")
            ),
            Self::MissingRemoveId => format!(
                "{}\n\n[err] Missing <pluginid>.\n\n\
                 Usage: `/plugins {sub} <pluginid>`",
                attribution("remove")
            ),
        }
    }
}

/// Parse `/plugins <sub> <args>` for the two store subcommands.
///
/// Returns `None` when `sub` is neither `add` nor `remove` (the caller falls
/// through to the other subcommands), `Some(Err(_))` for malformed arguments,
/// and `Some(Ok(_))` for a valid command. `--force` is accepted anywhere in the
/// `add` argument list; the remaining tokens joined by a single space form the
/// source (so a quoted path with spaces still resolves).
#[must_use]
pub fn parse_store_command(sub: &str, args: &str) -> Option<Result<StoreCommand, StoreArgError>> {
    match sub {
        "add" => {
            let mut force = false;
            let mut source_tokens: Vec<&str> = Vec::new();
            for token in args.split_whitespace() {
                if token == "--force" {
                    force = true;
                } else {
                    source_tokens.push(token);
                }
            }
            if source_tokens.is_empty() {
                return Some(Err(StoreArgError::MissingAddSource));
            }
            Some(Ok(StoreCommand::Add {
                source: source_tokens.join(" "),
                force,
            }))
        }
        "remove" => {
            let plugin_id = args.split_whitespace().next();
            match plugin_id {
                Some(id) => Some(Ok(StoreCommand::Remove {
                    plugin_id: id.to_string(),
                })),
                None => Some(Err(StoreArgError::MissingRemoveId)),
            }
        }
        _ => None,
    }
}

/// Run the `add`/`remove` store subcommand and return the report string to
/// print, or `None` when `sub` is not a store subcommand.
///
/// `dirs` are the resolved plugin store legs and `workdir` the working
/// directory used to resolve a relative `add` source. When the master switch
/// `plugins.enabled` is false the install/uninstall is refused and the disabled
/// subsystem is reported instead (SPEC configuration schema).
#[must_use]
pub fn run_store_command(
    config: &ragent_config::PluginsConfig,
    dirs: &StoreDirs,
    workdir: &Path,
    sub: &str,
    args: &str,
) -> Option<String> {
    let parsed = parse_store_command(sub, args)?;
    if !config.is_enabled() {
        return Some(crate::control::disabled_subsystem_report(sub));
    }
    let report = match parsed {
        Ok(StoreCommand::Add { source, force }) => match add(dirs, workdir, &source, force) {
            Ok(outcome) => add_report(&outcome),
            Err(err) => add_error_report(&err),
        },
        Ok(StoreCommand::Remove { plugin_id }) => match remove(dirs, &plugin_id) {
            Ok(outcome) => remove_report(&outcome),
            Err(err) => remove_error_report(&err),
        },
        Err(arg_err) => arg_err.report(sub),
    };
    Some(report)
}
