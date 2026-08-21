//! `/codeindex skillgen` — install the graphify skill into the global skills folder.
//!
//! This module embeds the graphify skill artifacts (rendered from the
//! `Graphify-Labs/graphify` repository, branch `v8`) and writes them to
//! `~/.ragent/skills/graphify/` on demand. If the skill already exists the
//! implementation files are overwritten so they always reflect the latest
//! version baked into this binary.

use std::fs;
use std::io;
use std::path::PathBuf;

/// The main skill file.
const SKILL_MD: &str = include_str!("graphify_skill_data/SKILL.md");

/// Reference files loaded on demand by the skill body.
const REF_ADD_WATCH: &str = include_str!("graphify_skill_data/references/add-watch.md");
const REF_EXPORTS: &str = include_str!("graphify_skill_data/references/exports.md");
const REF_EXTRACTION_SPEC: &str = include_str!("graphify_skill_data/references/extraction-spec.md");
const REF_GITHUB_MERGE: &str = include_str!("graphify_skill_data/references/github-and-merge.md");
const REF_HOOKS: &str = include_str!("graphify_skill_data/references/hooks.md");
const REF_QUERY: &str = include_str!("graphify_skill_data/references/query.md");
const REF_TRANSCRIBE: &str = include_str!("graphify_skill_data/references/transcribe.md");
const REF_UPDATE: &str = include_str!("graphify_skill_data/references/update.md");

/// All reference files, in the order they should be written.
const REFERENCE_FILES: &[(&str, &str)] = &[
    ("add-watch.md", REF_ADD_WATCH),
    ("exports.md", REF_EXPORTS),
    ("extraction-spec.md", REF_EXTRACTION_SPEC),
    ("github-and-merge.md", REF_GITHUB_MERGE),
    ("hooks.md", REF_HOOKS),
    ("query.md", REF_QUERY),
    ("transcribe.md", REF_TRANSCRIBE),
    ("update.md", REF_UPDATE),
];

/// Result of a skill generation run.
pub struct SkillgenResult {
    /// Number of files written (SKILL.md + references).
    pub files_written: usize,
    /// Destination directory.
    pub dest: PathBuf,
    /// Whether the skill directory already existed before this run.
    pub already_existed: bool,
}

/// Resolve the global personal skills directory: `~/.ragent/skills/`.
fn global_skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent").join("skills"))
}

/// Write a file, creating parent directories as needed.
fn write_file(path: &PathBuf, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

/// Generate the graphify skill in `~/.ragent/skills/graphify/`.
///
/// Overwrites any existing implementation files so the installed skill always
/// matches the version embedded in this binary. Returns the number of files
/// written and the destination directory.
pub fn generate_graphify_skill() -> io::Result<SkillgenResult> {
    let skills_dir = global_skills_dir().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "could not resolve home directory")
    })?;
    generate_graphify_skill_in(&skills_dir)
}

/// Generate the graphify skill into a caller-specified skills directory.
///
/// This is the testable inner: it does not touch the environment. The public
/// [`generate_graphify_skill`] resolves `~/.ragent/skills/` and delegates here.
pub fn generate_graphify_skill_in(skills_dir: &std::path::Path) -> io::Result<SkillgenResult> {
    let dest = skills_dir.join("graphify");
    let already_existed = dest.is_dir();

    // Write SKILL.md.
    let mut files_written = 0;
    write_file(&dest.join("SKILL.md"), SKILL_MD)?;
    files_written += 1;

    // Write reference files.
    for (name, content) in REFERENCE_FILES {
        write_file(&dest.join("references").join(name), content)?;
        files_written += 1;
    }

    Ok(SkillgenResult {
        files_written,
        dest,
        already_existed,
    })
}
