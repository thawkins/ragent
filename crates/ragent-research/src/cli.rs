//! CLI helpers for the `ragent research …` sub-commands (T-034, T-035).
//!
//! Provides parse, build-help, and JSON progress-emit helpers used by
//! `src/main.rs` to dispatch the `ragent research <subcommand>` family.

use crate::research_name::ResearchNameError;

/// Parsed `ragent research <subcommand>` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchCliCommand {
    /// `ragent research help` — show the help table.
    Help,
    /// `ragent research create <name> <topic>` — run a gathering session.
          Create {
              /// Validated research name (or raw string if validation hasn't run).
              name: String,
              /// Free-form topic description.
              topic: String,
              /// Optional FR-019 `--sources-dir <path>`.
              sources_dir: Option<String>,
              /// Optional FR-020 `--template <name>`.
              template: Option<String>,
              /// `--no-local` — skip the local-file scanning phase.
              no_local: bool,
              /// `--no-specs` — skip the prior-spec cross-reference phase.
              no_specs: bool,
          },    /// `ragent research list` — list every item.
    List {
        /// `--all` includes archived items.
        all: bool,
    },
    /// `ragent research open <name>` — print the absolute path of `RESEARCH.md`.
    Open {
        /// Research name.
        name: String,
    },
    /// `ragent research search <query>` — full-text search.
    Search {
        /// Search query string.
        query: String,
    },
    /// `ragent research show <name>` — print metadata.
    Show {
        /// Research name.
        name: String,
    },
    /// `ragent research delete <name>` — remove the item.
    Delete {
        /// Research name.
        name: String,
        /// `--yes` to skip confirmation.
        yes: bool,
    },
    /// `ragent research archive <name>` — mark as archived.
    Archive {
        /// Research name.
        name: String,
    },
    /// Unknown / malformed sub-command — preserves the raw input for an
    /// error message.
    Unknown(String),
}

impl ResearchCliCommand {
    /// Parse `ragent research …` arguments. The first positional argument is
    /// the subcommand; remaining arguments are subcommand-specific.
    pub fn parse(args: &str) -> Self {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return Self::Help;
        }
        let mut parts = trimmed.split_whitespace();
        let sub = parts.next().unwrap_or("");
        let rest: Vec<&str> = parts.collect();
        match sub {
            "help" | "-h" | "--help" => Self::Help,
            "create" => Self::parse_create(&rest),
            "list" | "ls" => {
                let all = rest.contains(&"--all");
                Self::List { all }
            }
            "open" => Self::parse_open(&rest),
            "search" => {
                let query = rest.join(" ");
                Self::Search { query }
            }
            "show" => Self::parse_show(&rest),
            "delete" | "rm" => Self::parse_delete(&rest),
            "archive" => Self::Archive {
                name: rest.join(" "),
            },
            other => {
                              // Treat as `create <name> <topic…>` if it looks like a name.
                              let name = other.to_string();
                              let topic = rest.join(" ");
                              if topic.is_empty() {
                                  Self::Unknown(name)
                              } else {
                                  Self::Create {
                                      name,
                                      topic,
                                      sources_dir: None,
                                      template: None,
                                      no_local: false,
                                      no_specs: false,
                                  }
                              }
                          }        }
    }

    fn parse_create(rest: &[&str]) -> Self {
              // Parse: ragent research create <name> <topic> [--sources-dir <path>]
              //        [--template <name>] [--no-local] [--no-specs]
            let mut i = 0;
            let mut name: Option<String> = None;
            let mut topic_words: Vec<&str> = Vec::new();
            let mut sources_dir: Option<String> = None;
            let mut template: Option<String> = None;
            let mut no_local = false;
            let mut no_specs = false;
            while i < rest.len() {
                let arg = rest[i];
                match arg {
                    "--sources-dir" => {
                        if let Some(v) = rest.get(i + 1) {
                            sources_dir = Some((*v).to_string());
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    "--template" => {
                        if let Some(v) = rest.get(i + 1) {
                            template = Some((*v).to_string());
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    "--no-local" => {
                        no_local = true;
                        i += 1;
                    }
                    "--no-specs" => {
                        no_specs = true;
                        i += 1;
                    }
                    _ => {
                        if name.is_none() {
                            name = Some(arg.to_string());
                        } else {
                            topic_words.push(arg);
                        }
                        i += 1;
                    }
                }
            }
            let Some(name) = name else {
                return Self::Unknown("create".to_string());
            };
            let topic = topic_words.join(" ");
            Self::Create {
                name,
                topic,
                sources_dir,
                template,
                no_local,
                no_specs,
            }
        }
    fn parse_open(rest: &[&str]) -> Self {
        let name = rest
            .iter()
            .find(|a| !a.starts_with("--"))
            .map(|s| s.to_string())
            .unwrap_or_default();
        if name.is_empty() {
            Self::Unknown("open".to_string())
        } else {
            Self::Open { name }
        }
    }

    fn parse_show(rest: &[&str]) -> Self {
        let name = rest
            .iter()
            .find(|a| !a.starts_with("--"))
            .map(|s| s.to_string())
            .unwrap_or_default();
        if name.is_empty() {
            Self::Unknown("show".to_string())
        } else {
            Self::Show { name }
        }
    }

    fn parse_delete(rest: &[&str]) -> Self {
        let yes = rest.iter().any(|a| *a == "--yes" || *a == "-y");
        let name = rest
            .iter()
            .find(|a| !a.starts_with("--") && !a.starts_with('-'))
            .map(|s| s.to_string())
            .unwrap_or_default();
        if name.is_empty() {
            Self::Unknown("delete".to_string())
        } else {
            Self::Delete { name, yes }
        }
    }

    /// Build the static help message shown by `ragent research help`.
    pub fn build_help_message() -> &'static str {
        "ragent research — manage research items under research/\n\
         \n\
         USAGE:\n\
           ragent research <SUBCOMMAND> [ARGS]\n\
         \n\
         SUBCOMMANDS:\n\
                      create <name> <topic> [--sources-dir <path>] [--template <name>]\n\
                            [--no-local] [--no-specs]\n\
                            Run an information-gathering session and write RESEARCH.md.\n\
                            --no-local  Skip local-file scanning (in-project + extras).\n\
                            --no-specs  Skip prior-spec cross-referencing.\n\
             list [--all]                  List every research item.\n\
           open <name>                   Print the absolute path of RESEARCH.md.\n\
           search <query>                Full-text search across all RESEARCH.md.\n\
           show <name>                   Print metadata for a single item.\n\
           delete <name> [--yes]         Remove a research item (prompts unless --yes).\n\
           archive <name>                Mark a research item as archived.\n\
           help                          Show this message.\n\
         \n\
         Output: by default `ragent research` emits machine-readable JSON lines\n\
         prefixed with `ragent-research:` so callers can pipe the output\n\
         through `jq` or other tools."
    }

    /// `true` if this is a usage-error variant (i.e. parse succeeded but the
    /// caller passed wrong args).
    pub fn is_usage_error(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

/// Render a [`SessionEvent`] as a single machine-readable JSON line on stdout
/// (T-035). The prefix `ragent-research:` makes the line easy to grep in a
/// mixed transcript.
pub fn render_session_event_json(event: &crate::session::SessionEvent) -> String {
    use crate::session::SessionEvent;
    let (kind, payload) = match event {
        SessionEvent::Phase { phase } => ("phase", serde_json::json!({ "phase": phase.as_str() })),
        SessionEvent::WebCaptured { url, title } => {
            ("web", serde_json::json!({ "url": url, "title": title }))
        }
        SessionEvent::LocalCaptured { path, score } => {
            ("local", serde_json::json!({ "path": path, "score": score }))
        }
        SessionEvent::SpecCaptured { spec_id } => {
            ("spec", serde_json::json!({ "spec_id": spec_id }))
        }
        SessionEvent::SynthesizeResult { outcome, detail } => (
            "synthesize",
            serde_json::json!({
                "outcome": outcome.as_str(),
                "detail": detail,
            }),
        ),
        SessionEvent::Done { total_sources } => (
            "done",
            serde_json::json!({ "total_sources": total_sources }),
        ),
    };
    format!(
        "ragent-research: {}",
        serde_json::to_string(&serde_json::json!({
            "kind": kind,
            "payload": payload,
        }))
        .unwrap_or_else(|_| "{}".to_string())
    )
}

/// Pretty-print a research item (T-034: `ragent research show`).
pub fn render_show_output(
    name: &str,
    title: &str,
    topic: &str,
    status: &str,
    created: &str,
    modified: &str,
    sources: &[(String, String, String, String)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Research item: {name}\n"));
    out.push_str(&format!("Title:         {title}\n"));
    out.push_str(&format!("Topic:         {topic}\n"));
    out.push_str(&format!("Status:        {status}\n"));
    out.push_str(&format!("Created (UTC): {created}\n"));
    out.push_str(&format!("Modified (UTC):{modified}\n"));
    out.push_str(&format!("\nReferences ({}):\n", sources.len()));
    for (i, (kind, path, title, captured)) in sources.iter().enumerate() {
        out.push_str(&format!(
            "  #{i:>2}  [{kind:<11}] {path:<32}  {title}  ({captured})\n",
        ));
    }
    out
}

/// Pretty-print a research list (T-034: `ragent research list`).
pub fn render_list_output(rows: &[(String, String, String, String, String)]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<22} {:<32} {:<11} {:<24} {:<24}\n",
        "NAME", "TITLE", "STATUS", "CREATED", "MODIFIED"
    ));
    out.push_str(&"-".repeat(120));
    out.push('\n');
    for (name, title, status, created, modified) in rows {
        out.push_str(&format!(
            "{:<22} {:<32} {:<11} {:<24} {:<24}\n",
            truncate(name, 22),
            truncate(title, 32),
            truncate(status, 11),
            truncate(created, 24),
            truncate(modified, 24),
        ));
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Pretty-print a search result list (T-030).
pub fn render_search_output(hits: &[(String, String, String)]) -> String {
    let mut out = String::new();
    for (name, title, snippet) in hits {
        out.push_str(&format!("• {name} — {title}\n    {snippet}\n"));
    }
    if out.is_empty() {
        out.push_str("(no matches)\n");
    }
    out
}

/// Convenience: map a [`ResearchNameError`] to a user-facing message.
pub fn explain_name_error(err: &ResearchNameError) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_help() {
        assert_eq!(ResearchCliCommand::parse("help"), ResearchCliCommand::Help);
        assert_eq!(ResearchCliCommand::parse(""), ResearchCliCommand::Help);
    }

    #[test]
          fn parse_create_with_topic() {
              let cmd = ResearchCliCommand::parse("create rust-async async/await idioms in stable Rust");
              match cmd {
                  ResearchCliCommand::Create {
                      name,
                      topic,
                      sources_dir,
                      template,
                      no_local,
                      no_specs,
                  } => {
                      assert_eq!(name, "rust-async");
                      assert_eq!(topic, "async/await idioms in stable Rust");
                      assert!(sources_dir.is_none());
                      assert!(template.is_none());
                      assert!(!no_local);
                      assert!(!no_specs);
                  }
                  other => panic!("unexpected variant: {other:?}"),
              }
          }

          #[test]
          fn parse_create_with_sources_dir_and_template() {
              let cmd = ResearchCliCommand::parse(
                  "create foo topic words --sources-dir /tmp/notes --template deepdive",
              );
              match cmd {
                  ResearchCliCommand::Create {
                      name,
                      topic,
                      sources_dir,
                      template,
                      no_local,
                      no_specs,
                  } => {
                      assert_eq!(name, "foo");
                      assert_eq!(topic, "topic words");
                      assert_eq!(sources_dir.as_deref(), Some("/tmp/notes"));
                      assert_eq!(template.as_deref(), Some("deepdive"));
                      assert!(!no_local);
                      assert!(!no_specs);
                  }
                  other => panic!("unexpected variant: {other:?}"),
              }
          }

          #[test]
          fn parse_create_with_no_local_flag() {
              let cmd = ResearchCliCommand::parse("create foo a topic --no-local");
              match cmd {
                  ResearchCliCommand::Create {
                      name,
                      topic,
                      no_local,
                      no_specs,
                      ..
                  } => {
                      assert_eq!(name, "foo");
                      assert_eq!(topic, "a topic");
                      assert!(no_local);
                      assert!(!no_specs);
                  }
                  other => panic!("unexpected variant: {other:?}"),
              }
          }

          #[test]
          fn parse_create_with_no_specs_flag() {
              let cmd = ResearchCliCommand::parse("create foo a topic --no-specs");
              match cmd {
                  ResearchCliCommand::Create {
                      name,
                      topic,
                      no_local,
                      no_specs,
                      ..
                  } => {
                      assert_eq!(name, "foo");
                      assert_eq!(topic, "a topic");
                      assert!(!no_local);
                      assert!(no_specs);
                  }
                  other => panic!("unexpected variant: {other:?}"),
              }
          }

          #[test]
          fn parse_create_with_both_no_flags() {
              let cmd = ResearchCliCommand::parse(
                  "create foo a topic --no-local --no-specs --sources-dir /tmp/x",
              );
            match cmd {
                ResearchCliCommand::Create {
                    name,
                    topic,
                    sources_dir,
                    no_local,
                    no_specs,
                    ..
                } => {
                    assert_eq!(name, "foo");
                    assert_eq!(topic, "a topic");
                    assert_eq!(sources_dir.as_deref(), Some("/tmp/x"));
                    assert!(no_local);
                    assert!(no_specs);
                }
                other => panic!("unexpected variant: {other:?}"),
            }
        }
    #[test]
    fn parse_list_all() {
        let cmd = ResearchCliCommand::parse("list --all");
        assert!(matches!(cmd, ResearchCliCommand::List { all: true }));
    }

    #[test]
    fn parse_list_default() {
        let cmd = ResearchCliCommand::parse("list");
        assert!(matches!(cmd, ResearchCliCommand::List { all: false }));
    }

    #[test]
    fn parse_open() {
        let cmd = ResearchCliCommand::parse("open rust-async");
        assert!(matches!(cmd, ResearchCliCommand::Open { ref name } if name == "rust-async"));
    }

    #[test]
    fn parse_search() {
        let cmd = ResearchCliCommand::parse("search async patterns");
        assert!(
            matches!(cmd, ResearchCliCommand::Search { ref query } if query == "async patterns")
        );
    }

    #[test]
    fn parse_show() {
        let cmd = ResearchCliCommand::parse("show foo");
        assert!(matches!(cmd, ResearchCliCommand::Show { ref name } if name == "foo"));
    }

    #[test]
    fn parse_delete_with_yes() {
        let cmd = ResearchCliCommand::parse("delete foo --yes");
        assert!(matches!(cmd, ResearchCliCommand::Delete { ref name, yes: true } if name == "foo"));
    }

    #[test]
    fn parse_archive() {
        let cmd = ResearchCliCommand::parse("archive foo");
        assert!(matches!(cmd, ResearchCliCommand::Archive { ref name } if name == "foo"));
    }

    #[test]
          fn parse_implicit_create_when_unknown_subcommand() {
              let cmd = ResearchCliCommand::parse("foo bar baz");
              match cmd {
                  ResearchCliCommand::Create {
                      name,
                      topic,
                      no_local,
                      no_specs,
                      ..
                  } => {
                      assert_eq!(name, "foo");
                      assert_eq!(topic, "bar baz");
                      assert!(!no_local);
                      assert!(!no_specs);
                  }
                  other => panic!("unexpected variant: {other:?}"),
              }
          }
    #[test]
    fn help_message_contains_documented_subcommands() {
        let h = ResearchCliCommand::build_help_message();
        for sub in [
            "create", "list", "open", "search", "show", "delete", "archive",
        ] {
            assert!(h.contains(sub), "help missing `{sub}`");
        }
    }

    #[test]
    fn render_session_event_json_for_phase() {
        let event = crate::session::SessionEvent::Phase {
            phase: crate::session::SessionPhase::Web,
        };
        let line = render_session_event_json(&event);
        assert!(line.starts_with("ragent-research:"));
        assert!(line.contains("\"web\""));
    }

    #[test]
    fn render_session_event_json_for_done() {
        let event = crate::session::SessionEvent::Done { total_sources: 7 };
        let line = render_session_event_json(&event);
        assert!(line.contains("\"done\""));
        assert!(line.contains("\"total_sources\":7"));
    }

    #[test]
    fn render_show_output_includes_metadata() {
        let out = render_show_output(
            "rust-async",
            "Rust Async",
            "async/await idioms",
            "complete",
            "2024-01-15T10:30:00Z",
            "2024-01-15T10:31:00Z",
            &[(
                "web".into(),
                "https://example.com".into(),
                "Example".into(),
                "2024-01-15T10:31:00Z".into(),
            )],
        );
        assert!(out.contains("rust-async"));
        assert!(out.contains("Rust Async"));
        assert!(out.contains("https://example.com"));
    }

    #[test]
    fn render_list_output_handles_empty() {
        let out = render_list_output(&[]);
        assert!(out.contains("NAME"));
    }

    #[test]
    fn truncate_short_string_passes_through() {
        assert_eq!(truncate("abc", 5), "abc");
    }

    #[test]
    fn truncate_long_string_ellipsises() {
        let s = "a".repeat(20);
        let t = truncate(&s, 5);
        assert_eq!(t.chars().count(), 5);
        assert!(t.ends_with('…'));
    }
}

// ── Filesystem-backed LocalTool for the CLI ───────────────────────────────

use crate::local_gatherer::{GrepMatch, LocalTool};
use crate::research_name::ResearchName;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Filesystem-backed implementation of [`LocalTool`] for use by the
/// `ragent research` CLI when no agent tool registry is available (T-034).
///
/// Behaviour:
/// - [`FsLocalTool::glob`] runs a synchronous glob over `project_root` and
///   returns project-relative paths. The implementation is intentionally
///   simple — it does the standard `walkdir`-style recursion itself so the
///   CLI doesn't need to depend on the `glob` crate.
/// - [`FsLocalTool::grep`] is a line-by-line case-insensitive substring
///   match over any of `terms`.
/// - [`FsLocalTool::read`] reads the file as UTF-8.
/// - [`FsLocalTool::list_specs`] lists the directories directly under
///   `specs/` and returns their base names.
/// - [`FsLocalTool::spec_title`] reads the first `#` heading of
///   `specs/<id>/SPEC.md` if present.
pub struct FsLocalTool;

impl FsLocalTool {
    /// Build a new filesystem-backed local tool.
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl Default for FsLocalTool {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl LocalTool for FsLocalTool {
    async fn glob(&self, project_root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
        // Translate a `**/*.ext` style pattern into a walkdir-style scan.
        // We only support two pattern shapes:
        //   1. `**/*.<ext>`     → match every file with that extension
        //   2. `*.<ext>`        → match every file with that extension (1-deep)
        // Anything else returns an empty vec — the gatherer treats that as
        // "no candidates" and moves on.
        let ext = pattern.rsplit('.').next().unwrap_or("");
        let ext = if ext == pattern { "" } else { ext };
        if ext.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<PathBuf> = Vec::new();
        walk(project_root, project_root, ext, &mut out).await?;
        out.sort();
        Ok(out)
    }

    async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
        let body = match tokio::fs::read_to_string(path).await {
            Ok(b) => b,
            Err(_) => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let lower = line.to_lowercase();
            if terms.iter().any(|t| lower.contains(&t.to_lowercase())) {
                out.push(GrepMatch {
                    line: i + 1,
                    text: line.to_string(),
                });
            }
        }
        Ok(out)
    }

    async fn read(&self, path: &Path) -> anyhow::Result<String> {
        Ok(tokio::fs::read_to_string(path).await?)
    }

    async fn list_specs(&self, project_root: &Path) -> anyhow::Result<Vec<String>> {
        let specs_dir = project_root.join("specs");
        let mut entries = match tokio::fs::read_dir(&specs_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }
            if ResearchName::try_new(&name).is_ok() {
                out.push(name);
            }
        }
        out.sort();
        Ok(out)
    }

    async fn spec_title(&self, project_root: &Path, spec_id: &str) -> anyhow::Result<String> {
        let path = project_root.join("specs").join(spec_id).join("SPEC.md");
        let body = match tokio::fs::read_to_string(&path).await {
            Ok(b) => b,
            Err(_) => return Ok(String::new()),
        };
        for raw in body.lines() {
            let line = raw.trim();
            if let Some(rest) = line.strip_prefix("# ") {
                return Ok(rest.trim().to_string());
            }
            if let Some(rest) = line.strip_prefix("## ") {
                return Ok(rest.trim().to_string());
            }
        }
        Ok(String::new())
    }
}

async fn walk(root: &Path, dir: &Path, ext: &str, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            // Skip the research/ output directory and obvious junk so the
            // gatherer doesn't index its own previous outputs.
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "research"
                || name == "target"
                || name == ".git"
                || name == "node_modules"
                || name.starts_with('.')
            {
                continue;
            }
            Box::pin(walk(root, &path, ext, out)).await?;
        } else if file_type.is_file() {
            let matches_ext = entry
                .file_name()
                .to_string_lossy()
                .rsplit('.')
                .next()
                .map(|e| e == ext)
                .unwrap_or(false);
            if matches_ext {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_path_buf());
                }
            }
        }
    }
    Ok(())
}
