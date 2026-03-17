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
/// directories for bundled language servers.
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
                    args: server.args.iter().map(|s| s.to_string()).collect(),
                    extensions: server.extensions.iter().map(|s| s.to_string()).collect(),
                    source: DiscoverySource::SystemPath,
                });
                break; // stop at first matching executable for this language
            }
        }
    }

    // Scan VS Code extension directories for additional bundled servers.
    for ext_dir in vscode_extension_dirs() {
        found.extend(scan_vscode_extensions(&ext_dir).await);
    }

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

/// Scan a VS Code extensions directory for bundled language server executables.
///
/// Returns any discovered servers not already found via PATH.
async fn scan_vscode_extensions(ext_dir: &std::path::Path) -> Vec<DiscoveredServer> {
    let mut found = Vec::new();

    let read_dir = match tokio::fs::read_dir(ext_dir).await {
        Ok(rd) => rd,
        Err(_) => return found,
    };

    // Well-known paths within VS Code extensions for common language servers.
    const EXT_SERVER_HINTS: &[(&str, &str, &str, &[&str])] = &[
        // (extension_name_prefix, relative_exe_path, language, extensions)
        (
            "rust-lang.rust-analyzer",
            "server/rust-analyzer",
            "rust",
            &["rs"],
        ),
        (
            "ms-python.python",
            "bundled/stubs",
            "python",
            &["py"],
        ),
        (
            "ms-vscode.cpptools",
            "bin/cpptools",
            "c",
            &["c", "h", "cpp", "hpp"],
        ),
    ];

    let mut rd = read_dir;
    loop {
        let entry = match rd.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => break,
        };

        let dir_name = entry.file_name();
        let dir_str = dir_name.to_string_lossy();

        for &(prefix, rel_path, language, extensions) in EXT_SERVER_HINTS {
            if dir_str.starts_with(prefix) {
                let candidate = entry.path().join(rel_path);
                if candidate.is_file() {
                    found.push(DiscoveredServer {
                        language: language.to_string(),
                        id: format!("{language}-vscode"),
                        executable: candidate,
                        args: vec!["--stdio".to_string()],
                        extensions: extensions.iter().map(|s| s.to_string()).collect(),
                        source: DiscoverySource::VsCodeExtension {
                            ext_dir: ext_dir.to_path_buf(),
                        },
                    });
                }
            }
        }
    }

    found
}
