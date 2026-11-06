//! Additive workspace-crate scaffolding for `--type crate` (FR-020).
//!
//! `--type crate` is the one app type that adds to a project instead of
//! creating one. Every other scaffold target is a fresh, empty directory
//! (the FR-002 guard refuses anything else); a crate targets the project the
//! command runs in, which is already full of a root manifest, sources, and
//! often sibling crates.
//!
//! The rules that follow from that:
//!
//! - **Additive only.** The planner writes `crates/<name>/` and nothing else.
//!   It never creates a `.ragent/`, `specs/`, `log/`, `AGENTS.md`, or
//!   `.gitignore` in the project root, because those already exist and a
//!   scaffold has no business rewriting an established project's files.
//! - **No silent overwrite.** `crates/<name>/` is planned through the same
//!   FR-016 emitter the rest of the engine uses, so an existing
//!   `Cargo.toml` or `src/lib.rs` is reported and left byte-identical rather
//!   than replaced.
//! - **Registration is a decision, not an edit.** A Cargo workspace that
//!   already globs `crates/*` (this project does) picks the new crate up with
//!   no manifest change at all. Only an explicit `members` list needs a new
//!   entry, and that entry is added to the list rather than rewriting the
//!   file. [`MemberStatus`] reports which of the three cases applied so the
//!   caller can say so instead of guessing.
//!
//! Pure planning plus the thin emitter call: the root-manifest inspection is
//! the only read, and it reads just enough to decide whether a member entry
//! is needed.

use std::path::Path;

use super::emit::{EmitReport, emit_files};
use super::flags::{AppType, Language, ScaffoldError};
use super::layout::PlannedFile;
use super::recipes::{LanguageRecipe, recipe_for};

/// Where a scaffolded crate's artifacts are written, relative to the project
/// root. The crate directory is `<CRATES_DIR>/<name>`.
pub const CRATES_DIR: &str = "crates";

/// Path of the project manifest that a workspace member is registered in.
pub const WORKSPACE_MANIFEST: &str = "Cargo.toml";

/// How the new crate relates to the project's workspace member list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberStatus {
    /// A `members` glob (e.g. `crates/*`) already covers the new directory, so
    /// the manifest needs no change.
    CoveredByGlob(String),
    /// An explicit `members` list gained the new entry; the manifest was
    /// rewritten with only that line added.
    Registered(String),
    /// The entry was already present, so the manifest was left alone.
    AlreadyListed(String),
}

impl MemberStatus {
    /// Human-readable one-line report for the scaffold summary.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::CoveredByGlob(pattern) => {
                format!("workspace member covered by existing pattern `{pattern}`")
            }
            Self::Registered(entry) => format!("registered workspace member `{entry}`"),
            Self::AlreadyListed(entry) => format!("workspace member `{entry}` already listed"),
        }
    }
}

/// Why a crate scaffold was refused.
///
/// Distinct from [`ScaffoldError`] because these are *crate-specific* refusals
/// that have no analogue in the standalone scaffold: a language that defines
/// no crate recipe, and a target that is not a Cargo workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrateWorkspaceError {
    /// The selected language has no crate recipe (only `rust` does).
    UnsupportedLanguage(String),
    /// The project root has no `Cargo.toml`.
    NotAWorkspace(String),
    /// The root `Cargo.toml` has a `[package]` section and therefore is not a
    /// workspace root; a crate cannot be added to it.
    NotAWorkspaceRoot(String),
    /// Reading the root manifest or emitting the crate failed.
    Scaffold(ScaffoldError),
}

impl std::fmt::Display for CrateWorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLanguage(lang) => write!(
                f,
                "--type crate is only supported for rust (got '{lang}'); \
                 the crate layout is a Cargo workspace member"
            ),
            Self::NotAWorkspace(root) => write!(
                f,
                "--type crate needs a Cargo workspace with a {WORKSPACE_MANIFEST} at \
                 '{root}', but none was found"
            ),
            Self::NotAWorkspaceRoot(root) => write!(
                f,
                "--type crate needs a workspace root, but '{root}/{WORKSPACE_MANIFEST}' \
                 declares a [package] and no [workspace]; add the crate to the \
                 workspace root instead"
            ),
            Self::Scaffold(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for CrateWorkspaceError {}

impl From<ScaffoldError> for CrateWorkspaceError {
    fn from(err: ScaffoldError) -> Self {
        Self::Scaffold(err)
    }
}

/// Result of a successful additive crate scaffold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateWorkspaceOutcome {
    /// Crate name as written into `Cargo.toml` (`package.name`).
    pub crate_name: String,
    /// Crate directory, relative to the project root (`crates/<name>`).
    pub crate_dir: String,
    /// What happened to the workspace member list.
    pub member: MemberStatus,
    /// Emitted-file report for the crate directory.
    pub report: EmitReport,
}

/// A planned crate artifact pair: the manifest plus the library entry point.
///
/// Separate from [`super::layout::AppLayout`] because that type models "one
/// manifest + one app-type source", while a crate always carries both a
/// manifest and a `src/lib.rs` and its paths are rooted at `crates/<name>`
/// rather than at the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateLayout {
    /// Files to emit, in order: manifest first, then `src/lib.rs`.
    pub files: Vec<PlannedFile>,
    /// Crate directory relative to the project root (`crates/<name>`).
    pub crate_dir: String,
}

/// Plan the crate artifact pair for `name` under the project root.
///
/// Renders the crate manifest and `src/lib.rs` from the recipe tables so the
/// registry remains the single source of truth for the content. The crate
/// directory is always `crates/<name>`, matching the `crates/*` workspace glob
/// this project uses; a caller that wants a different parent should change
/// [`CRATES_DIR`] rather than duplicate the path here.
///
/// # Errors
///
/// [`CrateWorkspaceError::UnsupportedLanguage`] when `language` defines no
/// crate recipe (only Rust does today).
pub fn plan_crate_layout(
    language: Language,
    name: &str,
) -> Result<CrateLayout, CrateWorkspaceError> {
    let recipe = crate_recipe(language)
        .ok_or_else(|| CrateWorkspaceError::UnsupportedLanguage(language.as_str().to_owned()))?;
    let crate_dir = format!("{CRATES_DIR}/{name}");
    let manifest = recipe.source_for(AppType::Crate).map_or_else(
        || unreachable_manifest(name),
        |file| recipe.render_source_content(file, name),
    );
    let lib = recipe.render_crate_lib_content(name);
    Ok(CrateLayout {
        files: vec![
            PlannedFile {
                path: format!("{crate_dir}/{WORKSPACE_MANIFEST}"),
                content: manifest,
            },
            PlannedFile {
                path: format!("{crate_dir}/src/lib.rs"),
                content: lib,
            },
        ],
        crate_dir,
    })
}

/// Fallback manifest used only to keep [`plan_crate_layout`] total.
///
/// [`crate_recipe`] already proves the recipe has a `Crate` source, so this is
/// unreachable in practice; returning a minimal manifest is preferable to a
/// panic in a scaffold path.
fn unreachable_manifest(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

/// The recipe that defines a crate layout for `language`, if any.
///
/// A language qualifies when its registry entry carries an [`AppType::Crate`]
/// source; that keeps "which languages can make a crate" a property of the
/// recipe table rather than a second hard-coded list.
fn crate_recipe(language: Language) -> Option<&'static LanguageRecipe> {
    recipe_for(language).filter(|recipe| recipe.source_for(AppType::Crate).is_some())
}

/// Scaffold `crates/<name>/` into an existing project and report the
/// workspace-registration outcome.
///
/// Refuses before writing anything when the project root is not a Cargo
/// workspace root, so a mistyped target cannot leave a half-added crate in an
/// unrelated directory.
///
/// # Errors
///
/// [`CrateWorkspaceError`] for an unsupported language, a non-workspace target,
/// or an emission failure.
pub fn plan_crate_workspace(
    root: &Path,
    request: &super::flags::ScaffoldRequest,
    name: &str,
) -> Result<CrateWorkspaceOutcome, CrateWorkspaceError> {
    let layout = plan_crate_layout(request.language(), name)?;
    let membership = inspect_workspace_members(root, name)?;

    // The emitter refuses to replace an existing file, so a re-run over a
    // half-finished crate reports the files it could not write instead of
    // clobbering the user's edits.
    let report = emit_files(root, &layout.files)?;

    Ok(CrateWorkspaceOutcome {
        crate_name: name.to_owned(),
        crate_dir: layout.crate_dir,
        member: membership,
        report,
    })
}

/// Decide how `crates/<name>` joins the workspace member list, writing the
/// manifest only when an explicit entry is required.
///
/// Three cases, in order:
///
/// 1. a `members` glob already matches the new directory - nothing to do;
/// 2. the entry is already listed verbatim - nothing to do;
/// 3. otherwise append the entry to the `members` array.
///
/// A `[package]`-only manifest is refused: it is not a workspace root, and
/// silently writing `[workspace]` into it would restructure the project.
fn inspect_workspace_members(root: &Path, name: &str) -> Result<MemberStatus, CrateWorkspaceError> {
    let manifest_path = root.join(WORKSPACE_MANIFEST);
    let entries = super::guard::read_target_entries(root).map_err(CrateWorkspaceError::Scaffold)?;
    let has_manifest = entries
        .iter()
        .any(|entry| entry.name == WORKSPACE_MANIFEST && !entry.is_dir);
    if !has_manifest {
        return Err(CrateWorkspaceError::NotAWorkspace(
            root.display().to_string(),
        ));
    }
    let text = std::fs::read_to_string(&manifest_path).map_err(|err| {
        CrateWorkspaceError::Scaffold(ScaffoldError::TargetUnreadable(format!(
            "{}: {err}",
            manifest_path.display()
        )))
    })?;

    let entry = format!("{CRATES_DIR}/{name}");
    let (members, has_workspace) = parse_members(&text);
    if !has_workspace {
        return Err(CrateWorkspaceError::NotAWorkspaceRoot(
            root.display().to_string(),
        ));
    }
    if let Some(pattern) = members.iter().find(|m| glob_covers(m, &entry)) {
        return Ok(MemberStatus::CoveredByGlob(pattern.clone()));
    }
    if members.contains(&entry) {
        return Ok(MemberStatus::AlreadyListed(entry));
    }

    // Registration edits the manifest, which the scaffold's no-overwrite
    // policy deliberately protects. That is safe here because the edit is
    // *additive*: the only change is one appended array element, and the
    // write only happens after the read that produced `text`.
    let updated = append_member(&text, &entry);
    std::fs::write(&manifest_path, updated).map_err(|err| {
        CrateWorkspaceError::Scaffold(ScaffoldError::EmissionFailed {
            path: WORKSPACE_MANIFEST.to_owned(),
            source: err.to_string(),
        })
    })?;
    Ok(MemberStatus::Registered(entry))
}

/// Extract the `members` array values and whether `[workspace]` is present.
///
/// Deliberately minimal: the scaffold only needs the string values of the
/// first `members` key, and a full TOML parser is not a dependency of this
/// crate. Quoted values and bare `crates/*`-style globs are both handled,
/// which is the full shape Cargo accepts in practice.
fn parse_members(text: &str) -> (Vec<String>, bool) {
    let mut members = Vec::new();
    let mut in_workspace = false;
    let mut collecting = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace = trimmed == "[workspace]";
            collecting = false;
            continue;
        }
        if !in_workspace {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("members") {
            let rest = rest.trim_start();
            // `members = [...]` starts the array on this line; the values may
            // span several lines, so switch to collection mode and fall
            // through to the same value scan below.
            if rest.starts_with('=') {
                collecting = true;
                let values = rest.trim_start_matches('=');
                collect_array_values(values, &mut members, &mut collecting);
            }
            continue;
        }
        if collecting {
            collect_array_values(trimmed, &mut members, &mut collecting);
        }
    }
    (members, in_workspace)
}

/// Pull quoted or bare values out of one line of a TOML array literal,
/// clearing the `collecting` flag when the closing `]` is seen.
fn collect_array_values(line: &str, members: &mut Vec<String>, collecting: &mut bool) {
    for raw in line.split(',') {
        let value = raw.trim().trim_start_matches('[').trim();
        if value.contains(']') {
            *collecting = false;
        }
        let value = value.trim_end_matches(']').trim();
        if value.is_empty() {
            continue;
        }
        // Comments end the logical values on their line.
        if value.starts_with('#') {
            continue;
        }
        let cleaned = value.trim_matches('"').trim_matches('\'').to_owned();
        if !cleaned.is_empty() {
            members.push(cleaned);
        }
    }
}

/// Whether a `members` pattern covers `entry`.
///
/// Only the trailing-wildcard form Cargo documents (`crates/*`) is treated as
/// a glob; anything else must match literally, so an unrecognised pattern is
/// reported as "not covered" (and the crate is registered explicitly) rather
/// than assumed to match.
fn glob_covers(pattern: &str, entry: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        // A `crates/*` pattern covers exactly the direct children of
        // `crates/`, so the prefix keeps its trailing separator and the
        // remainder must be a single path segment. `"crates/"` + `"demo"`
        // matches; `"crates/"` + `"a/b"` does not.
        return entry.strip_prefix(prefix).is_some_and(
            |rest| !rest.is_empty() /* INTENTIONAL: any child */ && !rest.contains('/'),
        );
    }
    pattern == entry
}

/// Append `entry` to the manifest's `members` array, preserving everything else.
///
/// A one-line array (`members = ["crates/*"]`) gains the element inline; a
/// multi-line array gains a new element after its last entry. If the array
/// cannot be located the text is returned unchanged, because a manifest that
/// does not match either shape is one the scaffold should not be rewriting.
fn append_member(text: &str, entry: &str) -> String {
    let mut out = String::with_capacity(text.len() + entry.len() + 8);
    let mut in_workspace = false;
    let mut done = false;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if !done && trimmed.starts_with('[') {
            in_workspace = trimmed.trim_end_matches('\n') == "[workspace]";
        }
        if !done && in_workspace && trimmed.starts_with("members") && trimmed.contains('=') {
            if let Some((head, tail)) = line.split_once(']') {
                // Inline array: add the entry before the closing bracket.
                let separator = if head.trim_end().ends_with('[') {
                    ""
                } else {
                    ", "
                };
                out.push_str(head);
                out.push_str(separator);
                out.push('"');
                out.push_str(entry);
                out.push('"');
                out.push(']');
                out.push_str(tail);
                done = true;
                continue;
            }
            // Multi-line array: remember we are inside it; the entry goes
            // after the line that closes it.
            out.push_str(line);
            continue;
        }
        if !done && in_workspace && trimmed.starts_with(']') {
            // Close a multi-line members array with the new element appended.
            let indent = "    ";
            out.push_str(&format!("{indent}\"{entry}\",\n"));
            out.push_str(line);
            done = true;
            continue;
        }
        out.push_str(line);
    }

    if done { out } else { text.to_owned() }
}
