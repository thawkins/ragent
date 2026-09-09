//! Project scaffolding engine for the `/new` command (spec `specs/newproj`).
//!
//! Module family layout:
//!
//! - [`flags`] — scaffold flag types, argument parsing, validation, and the
//!   error enum. Pure logic, no filesystem access (T-001).
//! - [`recipes`] — per-language scaffold recipes (manifests, hello-world
//!   sources, run/test commands, gitignore bodies). Pure data (T-002).
//! - [`workspace`] — ragent workspace initialiser planner (`.ragent/`,
//!   `specs/`, `log/`, `.gitignore`, `AGENTS.md`). Pure logic (T-005).
//! - [`guard`] — FR-002 empty-directory guard I/O shell: gathers real
//!   directory entries and runs the pure emptiness decision (T-006).
//! - [`layout`] — FR-006 app-type layout mapping: composes recipe + app type
//!   + slug into the concrete file plan. Pure logic (T-003).
//! - [`emit`] — FR-016 no-silent-overwrite emitter: writes planned files,
//!   skips and reports existing ones (T-007).
//! - [`gitinit`] — local git init + initial commit (FR-008 local half,
//!   FR-015 tolerance) (T-008).
//! - [`remote`] — GitHub + GitLab remote-init flows: hosting repo create,
//!   `origin` registration, initial push, FR-010 failure containment,
//!   FR-015 retry tolerance (T-009, T-010).
//! - [`summary`] — FR-011 scaffold summary report: created files, chosen
//!   language/type/stack, git + remote status (T-008).
//! - [`stack`] — FR-007 stack overlay planner: known-stack dependency +
//!   starter snippets layered onto the base layout, unknown stacks warn and
//!   continue (T-004).
//! - [`help`] — FR-018 `/new help` detailed help renderer: purpose,
//!   per-argument docs, registry-derived value lists, and worked examples
//!   (T-016).
//! - [`docs`] — FR-019 documentation scaffold: `README.md`, `QUICKSTART.md`,
//!   `STATS.md`, and the `docs/` folder, all rendered from the language
//!   recipe data (NFR-002) (T-017).

pub mod docs;
pub mod emit;
pub mod flags;
pub mod gitinit;
pub mod guard;
pub mod help;
pub mod layout;
pub mod recipes;
pub mod remote;
pub mod stack;
pub mod summary;
pub mod workspace;

pub use docs::{DocFile, doc_artifacts};
pub use emit::{
    EmitReport, FileStatus, PlannedArtifact, emit_file, emit_files, ensure_workspace_dirs,
};
pub use gitinit::{GitFailure, GitInitReport, GitStep, init_and_commit};
pub use remote::{
    RemoteFailure, RemoteReport, RemoteStep, init_github_remote, init_github_remote_with_token,
    init_gitlab_remote, init_gitlab_remote_with_token, load_github_token, load_gitlab_base_url,
    load_gitlab_token,
};
pub use summary::{RemoteStatus, ScaffoldSummary};

pub use flags::{
    AppType, FilePathEntry, HostingTarget, Language, ScaffoldError, ScaffoldRequest,
    app_type_value_list, is_help, language_value_list, parse_flags, validate_target_directory,
};
pub use guard::{enforce_empty_directory_guard, read_target_entries};
pub use help::render_detailed_help;
pub use layout::{AppLayout, PlannedFile, plan_app_layout};
pub use recipes::{LanguageRecipe, REGISTRY, SourceFile, recipe_for};
pub use stack::{
    STACK_RECIPES, StackOverlay, StackRecipe, apply_stack_overlay, resolve_stack_overlay,
};
pub use workspace::{WorkspaceArtifact, workspace_artifacts, workspace_dirs};

/// Plan and emit the full scaffold file set for a validated
/// [`ScaffoldRequest`] (FR-004 + FR-005 + FR-006 + FR-007 + FR-016 + FR-019).
///
/// The single shared pipeline behind the TUI `/new` command and the
/// `ragent new` CLI: app-layout planning, stack-overlay resolution
/// (unknown stacks produce the `[warn]` note and continue with the base
/// layout), FR-019 documentation planning, and the FR-016 no-silent-overwrite
/// emission of all three layers. The caller supplies the UTC timestamp
/// (the engine stays clock-free for testability) and the resolved recipe
/// ([`recipe_for`]).
///
/// Local git init ([`init_and_commit`]) and the remote-init flows are
/// caller-owned so the TUI can stream progress around them.
///
/// Returns the emitted-file summary plus the stack warning note (empty when
/// no unknown stack was supplied); the caller attaches the git/remote
/// outcomes to the summary and renders it.
///
/// # Errors
///
/// [`ScaffoldError`] from directory creation or the first failing file
/// write; earlier creations remain on disk (reported, not rolled back).
pub fn plan_and_emit(
    root: &std::path::Path,
    request: &ScaffoldRequest,
    slug: &str,
    recipe: &'static LanguageRecipe,
    generated_at_utc: &str,
) -> Result<(ScaffoldSummary, String), ScaffoldError> {
    let mut layout = plan_app_layout(recipe, request.app_type(), slug);
    let workspace_files = workspace_artifacts(recipe, slug);

    // FR-007 stack overlay: known stacks layer the framework dependency +
    // starter snippet on the base layout; unknown stacks warn and continue.
    let mut applied_stack: Option<&str> = None;
    let stack_note = match resolve_stack_overlay(request.stack(), request.language()) {
        StackOverlay::None => String::new(),
        StackOverlay::Applied(stack) => {
            apply_stack_overlay(&mut layout, recipe, stack);
            applied_stack = Some(stack.stack);
            String::new()
        }
        StackOverlay::Unknown(name) => format!(
            "\n[warn] unknown stack '{name}' for {}; continuing with the base layout\n",
            request.language()
        ),
    };

    // FR-019 documentation layer, rendered from the same recipe data as the
    // code artifacts (NFR-002); docs are excluded from the STATS file count.
    // The recorded stack prefers the applied canonical name and falls back
    // to the raw user value (kept verbatim for unknown stacks).
    let doc_files = doc_artifacts(
        recipe,
        request.app_type(),
        slug,
        applied_stack.or(request.stack()),
        layout.files.len() + workspace_files.len(),
        generated_at_utc,
        env!("CARGO_PKG_VERSION"),
    );

    ensure_workspace_dirs(root)?;
    let mut report = emit_files(root, &layout.files)?;
    report.merge(emit_files(root, &workspace_files)?);
    report.merge(emit_files(root, &doc_files)?);

    Ok((
        ScaffoldSummary::from_reports(
            slug.to_owned(),
            root.to_path_buf(),
            request.language(),
            request.app_type(),
            applied_stack
                .map(str::to_owned)
                .or_else(|| request.stack().map(str::to_owned)),
            report,
            None,
        ),
        stack_note,
    ))
}
