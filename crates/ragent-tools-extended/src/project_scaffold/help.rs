//! `/new help` detailed help renderer (spec `newproj` T-016, FR-018,
//! NFR-001).
//!
//! Shared by the TUI slash surface and the `ragent new` CLI surface: both
//! print the same purpose statement, per-argument documentation, worked
//! examples, and registry-derived accepted-value lists. Only the usage
//! lines and the example invocations are surface-specific (slash form vs.
//! binary form) and are supplied by the caller.
//!
//! NFR-001: the `--language` / `--type` value lists come from
//! [`language_value_list`] / [`app_type_value_list`] and the known-stack
//! examples from [`STACK_RECIPES`] — the same registries `parse_flags`
//! validates against — so the help text cannot drift out of sync with
//! what the command actually accepts.

use super::flags::{Language, app_type_value_list, language_value_list};
use super::stack::STACK_RECIPES;

/// Known stacks grouped by language, rendered as indented help lines.
///
/// Derived from [`STACK_RECIPES`] (NFR-001): adding a stack recipe
/// extends the help page without touching this module.
fn stack_help_lines() -> Vec<String> {
    let mut groups: Vec<(Language, Vec<&str>)> = Vec::new();
    for recipe in STACK_RECIPES {
        match groups
            .iter_mut()
            .position(|group| group.0 == recipe.language)
        {
            Some(index) => groups[index].1.push(recipe.stack),
            None => groups.push((recipe.language, vec![recipe.stack])),
        }
    }
    groups
        .iter()
        .map(|(language, names)| format!("        {language}: {}", names.join(", ")))
        .collect()
}

/// Render the detailed `/new help` page (FR-018).
///
/// The page contains the command purpose, per-argument documentation
/// (argument name, one-line description, optionality, accepted values,
/// and the default behaviour when the argument is omitted), and two
/// worked example invocations.
///
/// # Arguments
///
/// * `usage_lines` — surface-specific usage block, pre-indented lines
///   without a trailing newline (e.g. the `/new …` slash spelling or the
///   `ragent new …` binary spelling).
/// * `example_minimal` — a minimal (no-hosting) example invocation line,
///   pre-indented.
/// * `example_hosted` — a hosting-flag example invocation line,
///   pre-indented.
///
/// The accepted-value lists are derived from the language and app-type
/// registries and [`STACK_RECIPES`] (NFR-001), so they always match what
/// [`parse_flags`](super::flags::parse_flags) accepts.
pub fn render_detailed_help(
    usage_lines: &str,
    example_minimal: &str,
    example_hosted: &str,
) -> String {
    let languages = language_value_list();
    let app_types = app_type_value_list();
    let stack_lines = stack_help_lines();
    let stacks_block = if stack_lines.is_empty() {
        "        (none registered)".to_owned()
    } else {
        stack_lines.join("\n")
    };
    format!(
        "Purpose:\n\
         \x20 Scaffold a brand-new project in the current directory: the\n\
         \x20 language layout with hello-world artifacts, the ragent workspace\n\
         \x20 (.ragent/, specs/, log/, .gitignore, AGENTS.md), starter\n\
         \x20 documentation (README.md, QUICKSTART.md, STATS.md, docs/), and a\n\
         \x20 local git repository with an initial commit. The target directory\n\
         \x20 must be empty (ragent artifacts excepted); existing scaffold files\n\
         \x20 are never overwritten.\n\
         \n\
         Usage:\n\
         {usage_lines}\n\
         \n\
         Arguments:\n\
         \x20 --language <lang>\n\
         \x20     Required. The computer language to scaffold. Every\n\
         \x20     language in the codeindex scanner's supported set has a\n\
         \x20     recipe: application languages (rust, python, go, ...)\n\
         \x20     scaffold runnable hello-world projects; data, markup, and\n\
         \x20     build formats (json, yaml, sql, cmake, maven, ...) scaffold\n\
         \x20     sample-document stubs.\n\
         \x20     Accepted values: {languages}\n\
         \x20     Values are matched case-insensitively; dialect aliases\n\
         \x20     from the codeindex id list are accepted (`ts`/`tsx`,\n\
         \x20     `js`/`jsx`, `c++`, `c_header`/`cpp_header`, `sh`/`bash`,\n\
         \x20     `yml`, `sv`, `vhd`, `tf`, `scad`, `kts`).\n\
         \x20     Omitted: validation error; nothing is created.\n\
         \n\
         \x20 --type <type>\n\
         \x20     Required. The kind of application to generate.\n\
         \x20     Accepted values: {app_types}\n\
         \x20       library  no binary entrypoint; exported module with a\n\
         \x20                public function\n\
         \x20       cmdline  console-entry layout\n\
         \x20       tui      terminal-UI starter\n\
         \x20       gui      GUI starter appropriate to the language\n\
         \x20     Omitted: validation error; nothing is created.\n\
         \n\
         \x20 --stack <name>\n\
         \x20     Optional. Layers a known framework starter on top of the\n\
         \x20     base layout: the framework dependency is added to the\n\
         \x20     language manifest and the hello-world source gains a minimal\n\
         \x20     starter snippet. Known stacks:\n\
         {stacks_block}\n\
         \x20     Unknown stacks warn and continue with the base layout; stack\n\
         \x20     values are matched case-insensitively and scoped to the\n\
         \x20     selected language.\n\
         \x20     Omitted: no stack layer is applied.\n\
         \n\
         \x20 --github\n\
         \x20     Optional flag. Creates a private GitHub repository, sets it\n\
         \x20     as `origin`, and pushes the initial commit.\n\
         \x20     Omitted: no remote is created and nothing is pushed (the\n\
         \x20     local git repository is still initialised with an initial\n\
         \x20     commit).\n\
         \n\
         \x20 --gitlab\n\
         \x20     Optional flag. Same as --github but on GitLab (server from\n\
         \x20     the GITLAB_URL variable or ~/.ragent/gitlab_config.json).\n\
         \x20     Omitted: no remote is created and nothing is pushed.\n\
         \x20     --github and --gitlab are mutually exclusive; supplying both\n\
         \x20     aborts with a conflict message and zero file mutations.\n\
         \n\
         Worked examples:\n\
         {example_minimal}\n\
         \x20     Minimal scaffold: language layout, ragent workspace, starter\n\
         \x20     docs, and a local git repository with an initial commit. No\n\
         \x20     remote is created and nothing is pushed.\n\
         \n\
         {example_hosted}\n\
         \x20     Same scaffold plus a private hosting repository created, set\n\
         \x20     as `origin`, and the initial commit pushed.\n"
    )
}
