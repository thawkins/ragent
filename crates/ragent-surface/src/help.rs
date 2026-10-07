//! Shared usage-attribution and subcommand-token helpers for the `/plugins`
//! and `/connectors` command families (FR-006, FR-014, FR-017).
//!
//! Two items were byte-identical across the crates apart from the trigger
//! string:
//!
//! * [`attribution`] - the `From: <trigger> <subcommand>` header every report
//!   renders;
//! * [`subcommand_of`] - the first whitespace-delimited token of an argument
//!   list, used to dispatch a slash invocation.
//!
//! The trigger is a parameter so each family keeps its own wording while the
//! format defined here cannot drift between them.

/// The `From: <trigger> <subcommand>` attribution line a report renders.
///
/// `trigger` is the surface trigger (`/plugins` or `/connectors`); `sub` is the
/// invoked subcommand, or empty for a bare invocation, in which case the
/// trailing token is omitted.
#[must_use]
pub fn attribution(trigger: &str, sub: &str) -> String {
    let sub = sub.trim();
    if sub.is_empty() {
        format!("From: {trigger}")
    } else {
        format!("From: {trigger} {sub}")
    }
}

/// The subcommand token from a slash invocation: the first whitespace-delimited
/// word of `args`, or `""` when none is present.
///
/// A bare invocation is intercepted before this is called (the caller treats
/// empty args as usage); the dispatcher renders usage when the token is `help`
/// or anything it does not recognise.
#[must_use]
pub fn subcommand_of(args: &str) -> &str {
    args.split_whitespace().next().unwrap_or("")
}
