//! LSP auto-discovery — scans `PATH` and VS Code extension directories for
//! known language server executables.
//!
//! [`discover`] returns a list of [`DiscoveredServer`] entries for any servers
//! found on the system. The caller decides whether to auto-connect or just
//! report them (e.g. via `/lsp discover`).

use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::LspServerConfig;

/// Origin of a discovered LSP server.
#[derive(Debug, Clone)]
pub enum DiscoverySource {
    /// Found on `PATH` via `which` / `where`.
    SystemPath,
    /// Found in a VS Code or VS Code Server extension directory.
    VsCodeExtension {
        /// Path to the extension directory.
        ext_dir: PathBuf,
    },
}

/// A language server executable discovered on the local system.
#[derive(Debug, Clone)]
pub struct DiscoveredServer {
    /// Language / ecosystem identifier (e.g. `"rust"`, `"typescript"`).
    pub language: String,
    /// Suggested `id` key for `ragent.json`.
    pub id: String,
    /// Full path to the executable.
    pub executable: PathBuf,
    /// Arguments needed to start the server (e.g. `["--stdio"]`).
    pub args: Vec<String>,
    /// File extensions this server handles.
    pub extensions: Vec<String>,
    /// Where the executable was found.
    pub source: DiscoverySource,
    /// Version string, if parseable from the executable path or extension dir name.
    pub version: Option<String>,
}

impl DiscoveredServer {
    /// Convert to a [`LspServerConfig`] ready to be inserted into `ragent.json`.
    #[must_use]
    pub fn to_config(&self) -> LspServerConfig {
        LspServerConfig {
            command: Some(self.executable.to_string_lossy().into_owned()),
            args: self.args.clone(),
            env: HashMap::new(),
            extensions: self.extensions.clone(),
            disabled: true, // discovered servers are disabled until the user opts in
            timeout_ms: LspServerConfig::default_timeout_ms(),
        }
    }
}

/// Well-known LSP server executables and their metadata.
struct KnownServer {
    /// Language identifier.
    language: &'static str,
    /// Candidate executable names (tried in order).
    executables: &'static [&'static str],
    /// Arguments required to start in stdio mode.
    args: &'static [&'static str],
    /// File extensions handled.
    extensions: &'static [&'static str],
}

const KNOWN_SERVERS: &[KnownServer] = &[
    KnownServer {
        language: "rust",
        executables: &["rust-analyzer"],
        args: &[],
        extensions: &["rs"],
    },
    KnownServer {
        language: "typescript",
        executables: &["typescript-language-server", "tsserver"],
        args: &["--stdio"],
        extensions: &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
    },
    KnownServer {
        language: "python",
        executables: &["pyright-langserver", "pylsp", "jedi-language-server"],
        args: &["--stdio"],
        extensions: &["py", "pyi"],
    },
    KnownServer {
        language: "go",
        executables: &["gopls"],
        args: &[],
        extensions: &["go"],
    },
    KnownServer {
        language: "c",
        executables: &["clangd"],
        args: &[],
        extensions: &["c", "h", "cpp", "hpp", "cc", "cxx"],
    },
    KnownServer {
        language: "java",
        executables: &["jdtls", "java-language-server"],
        args: &[],
        extensions: &["java"],
    },
    KnownServer {
        language: "lua",
        executables: &["lua-language-server"],
        args: &[],
        extensions: &["lua"],
    },
    KnownServer {
        language: "ruby",
        executables: &["solargraph"],
        args: &["stdio"],
        extensions: &["rb", "gemspec"],
    },
    KnownServer {
        language: "csharp",
        executables: &["OmniSharp", "csharp-ls"],
        args: &["--languageserver"],
        extensions: &["cs"],
    },
    KnownServer {
        language: "html",
        executables: &["vscode-html-language-server"],
        args: &["--stdio"],
        extensions: &["html", "htm"],
    },
    KnownServer {
        language: "css",
        executables: &["vscode-css-language-server"],
        args: &["--stdio"],
        extensions: &["css", "scss", "less"],
    },
    KnownServer {
        language: "json",
        executables: &["vscode-json-language-server"],
        args: &["--stdio"],
        extensions: &["json", "jsonc"],
    },
];

/// Scan the system for installed LSP servers and return discovered entries.
///
/// Checks `PATH` for each known executable. Also scans common VS Code extension
/// directories for bundled language servers. VS Code entries are deduplicated to
/// the highest-versioned install of each server.
///
/// This function never fails — missing servers are silently skipped.
pub async fn discover() -> Vec<DiscoveredServer> {
    let mut found = Vec::new();

    for server in KNOWN_SERVERS {
        for &exe in server.executables {
            if let Some(path) = which_async(exe).await {
                found.push(DiscoveredServer {
                    language: server.language.to_string(),
                    id: server.language.to_string(),
                    executable: path,
                    args: server
                        .args
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
                    extensions: server
                        .extensions
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
                    source: DiscoverySource::SystemPath,
                    version: None,
                });
                break; // stop at first matching executable for this language
            }
        }
    }

    // Scan VS Code extension directories for additional bundled servers.
    // Collect all candidates from all vscode dirs, then deduplicate by language
    // keeping the highest version (handles .vscode vs .vscode-server duplicates).
    let mut vscode_candidates: Vec<DiscoveredServer> = Vec::new();
    for ext_dir in vscode_extension_dirs() {
        vscode_candidates.extend(scan_vscode_extensions(&ext_dir).await);
    }

    // For each language, keep only the highest-versioned vscode entry.
    let mut best_vscode: HashMap<String, DiscoveredServer> = HashMap::new();
    for srv in vscode_candidates {
        let version_tuple = srv
            .version
            .as_deref()
            .and_then(parse_version_tuple)
            .unwrap_or((0, 0, 0));
        let entry = best_vscode.entry(srv.language.clone()).or_insert_with(|| srv.clone());
        let existing_tuple = entry
            .version
            .as_deref()
            .and_then(parse_version_tuple)
            .unwrap_or((0, 0, 0));
        if version_tuple > existing_tuple {
            *entry = srv;
        }
    }
    found.extend(best_vscode.into_values());

    found
}

/// Resolve an executable on `PATH` asynchronously.
async fn which_async(exe: &str) -> Option<PathBuf> {
    let exe = exe.to_string();
    tokio::task::spawn_blocking(move || which_sync(&exe))
        .await
        .ok()
        .flatten()
}

/// Synchronous PATH lookup.
fn which_sync(exe: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe);
        if candidate.is_file() {
            return Some(candidate);
        }
        // On Windows also try .exe — but ragent targets Linux/macOS.
    }
    None
}

/// Parse a dotted version string (e.g. `"0.3.2845"`) into a comparable tuple.
fn parse_version_tuple(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.splitn(3, '.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next().unwrap_or("0").parse().ok()?;
    let patch: u64 = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// Common locations of VS Code extension directories.
fn vscode_extension_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".vscode").join("extensions"));
        dirs.push(home.join(".vscode-server").join("extensions"));
        dirs.push(home.join(".vscode-insiders").join("extensions"));
    }
    dirs
}

/// Parse a semver-like version string from a VS Code extension directory name.
///
/// Extension directories are named `<publisher>.<name>-<version>[-<platform>]`,
/// e.g. `rust-lang.rust-analyzer-0.3.2234-linux-x64`.
/// Returns the version component as a comparable tuple of (major, minor, patch).
fn parse_ext_version(dir_name: &str, prefix: &str) -> Option<(u64, u64, u64)> {
    // Strip the prefix, leaving e.g. "-0.3.2234-linux-x64"
    let after = dir_name.strip_prefix(prefix)?;
    // Must start with '-' followed by the version number
    let version_str = after.strip_prefix('-')?;
    // Take only the leading version digits (stop at the next '-' which is the platform)
    let version_part = version_str.split('-').next()?;
    let mut parts = version_part.splitn(3, '.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next().unwrap_or("0").parse().ok()?;
    let patch: u64 = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// Format a version tuple back to a display string.
fn format_version(v: (u64, u64, u64)) -> String {
    if v.2 == 0 {
        format!("{}.{}", v.0, v.1)
    } else {
        format!("{}.{}.{}", v.0, v.1, v.2)
    }
}

/// Scan a VS Code extensions directory for bundled language server executables.
///
/// For each known extension prefix, collects all installed versions and returns
/// only the highest-versioned one, with the version displayed in the entry.
async fn scan_vscode_extensions(ext_dir: &std::path::Path) -> Vec<DiscoveredServer> {
    let mut found = Vec::new();

    let read_dir = match tokio::fs::read_dir(ext_dir).await {
        Ok(rd) => rd,
        Err(_) => return found,
    };

    // Well-known paths within VS Code extensions for common language servers.
    // (extension_name_prefix, relative_exe_path, language, extensions)
    const EXT_SERVER_HINTS: &[(&str, &str, &str, &[&str])] = &[
        (
            "rust-lang.rust-analyzer",
            "server/rust-analyzer",
            "rust",
            &["rs"],
        ),
        ("ms-python.python", "bundled/stubs", "python", &["py"]),
        (
            "ms-vscode.cpptools",
            "bin/cpptools",
            "c",
            &["c", "h", "cpp", "hpp"],
        ),
    ];

    // Collect all entries from the extensions directory.
    let mut all_entries: Vec<(std::ffi::OsString, PathBuf)> = Vec::new();
    let mut rd = read_dir;
    loop {
        let entry = match rd.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) | Err(_) => break,
        };
        all_entries.push((entry.file_name(), entry.path()));
    }

    // For each extension hint, find the highest-versioned matching directory.
    for &(prefix, rel_path, language, extensions) in EXT_SERVER_HINTS {
        // Collect all (version_tuple, dir_path) candidates for this prefix.
        let mut candidates: Vec<((u64, u64, u64), PathBuf)> = Vec::new();
        for (dir_name, dir_path) in &all_entries {
            let dir_str = dir_name.to_string_lossy();
            if !dir_str.starts_with(prefix) {
                continue;
            }
            let candidate_exe = dir_path.join(rel_path);
            if !candidate_exe.is_file() {
                continue;
            }
            let version = parse_ext_version(&dir_str, prefix).unwrap_or((0, 0, 0));
            candidates.push((version, dir_path.clone()));
        }

        if candidates.is_empty() {
            continue;
        }

        // Pick the highest version.
        candidates.sort_by_key(|(v, _)| *v);
        let (best_version, best_dir) = candidates.into_iter().last().unwrap();
        let candidate_exe = best_dir.join(rel_path);

        found.push(DiscoveredServer {
            language: language.to_string(),
            id: format!("{language}-vscode"),
            executable: candidate_exe,
            args: vec!["--stdio".to_string()],
            extensions: extensions
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            source: DiscoverySource::VsCodeExtension {
                ext_dir: ext_dir.to_path_buf(),
            },
            version: Some(format_version(best_version)),
        });
    }

    found
}
