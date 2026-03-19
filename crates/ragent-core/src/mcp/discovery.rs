//! MCP auto-discovery — scans `PATH`, npm global packages, and well-known
//! directories for MCP server executables.
//!
//! [`discover`] returns a list of [`DiscoveredMcpServer`] entries for any servers
//! found on the system. The caller decides whether to auto-connect or just
//! report them (e.g. via `/mcp discover`).

use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::McpServerConfig;

/// Origin of a discovered MCP server.
#[derive(Debug, Clone)]
pub enum McpDiscoverySource {
    /// Found on `PATH` via `which` / `where`.
    SystemPath,
    /// Found in npm global node_modules.
    NpmGlobal {
        /// Path to the npm prefix directory.
        prefix_dir: PathBuf,
    },
    /// Found in a Claude/Cline MCP directory.
    McpRegistry {
        /// Path to the registry directory.
        registry_dir: PathBuf,
    },
}

/// An MCP server executable discovered on the local system.
#[derive(Debug, Clone)]
pub struct DiscoveredMcpServer {
    /// Suggested `id` key for `ragent.json`.
    pub id: String,
    /// Human-readable name/description of the server.
    pub name: String,
    /// Full path to the executable.
    pub executable: PathBuf,
    /// Arguments needed to start the server.
    pub args: Vec<String>,
    /// Environment variables to set when starting.
    pub env: HashMap<String, String>,
    /// Where the executable was found.
    pub source: McpDiscoverySource,
}

impl DiscoveredMcpServer {
    /// Convert to a [`McpServerConfig`] ready to be inserted into `ragent.json`.
    #[must_use]
    pub fn to_config(&self) -> McpServerConfig {
        McpServerConfig {
            type_: crate::config::McpTransport::Stdio,
            command: Some(self.executable.to_string_lossy().into_owned()),
            args: self.args.clone(),
            env: self.env.clone(),
            url: None,
            disabled: true, // discovered servers are disabled until the user opts in
        }
    }
}

/// Well-known MCP server executables and their metadata.
struct KnownMcpServer {
    /// Unique identifier.
    id: &'static str,
    /// Human-readable name.
    name: &'static str,
    /// Candidate executable names (tried in order).
    executables: &'static [&'static str],
    /// Arguments required to start the server.
    args: &'static [&'static str],
}

const KNOWN_SERVERS: &[KnownMcpServer] = &[
    KnownMcpServer {
        id: "filesystem",
        name: "Filesystem MCP Server",
        executables: &["mcp-server-filesystem"],
        args: &[],
    },
    KnownMcpServer {
        id: "github",
        name: "GitHub MCP Server",
        executables: &["mcp-server-github", "gh-mcp"],
        args: &[],
    },
    KnownMcpServer {
        id: "git",
        name: "Git MCP Server",
        executables: &["mcp-server-git"],
        args: &[],
    },
    KnownMcpServer {
        id: "postgres",
        name: "PostgreSQL MCP Server",
        executables: &["mcp-server-postgres"],
        args: &[],
    },
    KnownMcpServer {
        id: "sqlite",
        name: "SQLite MCP Server",
        executables: &["mcp-server-sqlite"],
        args: &[],
    },
    KnownMcpServer {
        id: "memory",
        name: "Memory MCP Server",
        executables: &["mcp-server-memory"],
        args: &[],
    },
    KnownMcpServer {
        id: "brave-search",
        name: "Brave Search MCP Server",
        executables: &["mcp-server-brave-search"],
        args: &[],
    },
    KnownMcpServer {
        id: "fetch",
        name: "Fetch MCP Server",
        executables: &["mcp-server-fetch"],
        args: &[],
    },
    KnownMcpServer {
        id: "puppeteer",
        name: "Puppeteer MCP Server",
        executables: &["mcp-server-puppeteer"],
        args: &[],
    },
    KnownMcpServer {
        id: "slack",
        name: "Slack MCP Server",
        executables: &["mcp-server-slack"],
        args: &[],
    },
    KnownMcpServer {
        id: "google-drive",
        name: "Google Drive MCP Server",
        executables: &["mcp-server-gdrive"],
        args: &[],
    },
    KnownMcpServer {
        id: "google-maps",
        name: "Google Maps MCP Server",
        executables: &["mcp-server-google-maps"],
        args: &[],
    },
    KnownMcpServer {
        id: "sentry",
        name: "Sentry MCP Server",
        executables: &["mcp-server-sentry"],
        args: &[],
    },
    KnownMcpServer {
        id: "sequential-thinking",
        name: "Sequential Thinking MCP Server",
        executables: &["mcp-server-sequential-thinking"],
        args: &[],
    },
    KnownMcpServer {
        id: "everything",
        name: "Everything MCP Server",
        executables: &["mcp-server-everything"],
        args: &[],
    },
    KnownMcpServer {
        id: "time",
        name: "Time MCP Server",
        executables: &["mcp-server-time"],
        args: &[],
    },
    KnownMcpServer {
        id: "aws-kb-retrieval",
        name: "AWS Knowledge Base Retrieval MCP Server",
        executables: &["mcp-server-aws-kb-retrieval"],
        args: &[],
    },
    KnownMcpServer {
        id: "exa",
        name: "Exa Search MCP Server",
        executables: &["mcp-server-exa"],
        args: &[],
    },
];

/// Scan the system for installed MCP servers and return discovered entries.
///
/// Checks `PATH` for each known executable. Also scans common npm global
/// directories and MCP registry directories for installed servers.
///
/// # Errors
///
/// This function never fails — missing servers are silently skipped.
pub async fn discover() -> Vec<DiscoveredMcpServer> {
    let mut found = Vec::new();

    // Check PATH for known executables
    for server in KNOWN_SERVERS {
        for &exe in server.executables {
            if let Some(path) = which_async(exe).await {
                found.push(DiscoveredMcpServer {
                    id: server.id.to_string(),
                    name: server.name.to_string(),
                    executable: path,
                    args: server.args.iter().map(|s| s.to_string()).collect(),
                    env: HashMap::new(),
                    source: McpDiscoverySource::SystemPath,
                });
                break; // stop at first matching executable for this server
            }
        }
    }

    // Scan npm global directories for MCP packages
    found.extend(scan_npm_global().await);

    // Scan well-known MCP registry directories
    for registry_dir in mcp_registry_dirs() {
        found.extend(scan_mcp_registry(&registry_dir).await);
    }

    // Deduplicate by id (prefer earlier finds)
    let mut seen = std::collections::HashSet::new();
    found.retain(|s| seen.insert(s.id.clone()));

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
        // On Windows also try .exe, .cmd, .bat
        #[cfg(windows)]
        {
            for ext in &[".exe", ".cmd", ".bat"] {
                let with_ext = dir.join(format!("{}{}", exe, ext));
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}

/// Scan npm global node_modules for MCP server packages.
async fn scan_npm_global() -> Vec<DiscoveredMcpServer> {
    let mut found = Vec::new();

    // Try to get npm prefix via `npm prefix -g`
    let prefix = match get_npm_prefix().await {
        Some(p) => p,
        None => return found,
    };

    // Check for common MCP package patterns in node_modules
    let node_modules = if cfg!(windows) {
        prefix.join("node_modules")
    } else {
        prefix.join("lib").join("node_modules")
    };

    if !node_modules.exists() {
        return found;
    }

    // Look for @modelcontextprotocol scoped packages
    let mcp_scope = node_modules.join("@modelcontextprotocol");
    if mcp_scope.is_dir() {
        found.extend(scan_npm_mcp_scope(&mcp_scope, &prefix).await);
    }

    // Look for other mcp-server-* packages at top level
    if let Ok(mut entries) = tokio::fs::read_dir(&node_modules).await {
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(_) => break,
            };

            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("mcp-server-") || name_str.starts_with("mcp_server_") {
                if let Some(server) = try_npm_mcp_package(&entry.path(), &prefix).await {
                    found.push(server);
                }
            }
        }
    }

    found
}

/// Get the npm global prefix directory.
async fn get_npm_prefix() -> Option<PathBuf> {
    let output = tokio::process::Command::new("npm")
        .args(["prefix", "-g"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if prefix.is_empty() {
        return None;
    }

    Some(PathBuf::from(prefix))
}

/// Scan @modelcontextprotocol scope for server packages.
async fn scan_npm_mcp_scope(
    scope_dir: &std::path::Path,
    prefix: &PathBuf,
) -> Vec<DiscoveredMcpServer> {
    let mut found = Vec::new();

    let mut entries = match tokio::fs::read_dir(scope_dir).await {
        Ok(e) => e,
        Err(_) => return found,
    };

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => break,
        };

        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("server-") {
            if let Some(server) = try_npm_mcp_package(&entry.path(), prefix).await {
                found.push(server);
            }
        }
    }

    found
}

/// Try to create a discovered server from an npm package directory.
async fn try_npm_mcp_package(
    pkg_dir: &std::path::Path,
    prefix: &PathBuf,
) -> Option<DiscoveredMcpServer> {
    // Read package.json to find the bin entry
    let pkg_json_path = pkg_dir.join("package.json");
    let content = tokio::fs::read_to_string(&pkg_json_path).await.ok()?;
    let pkg_json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let pkg_name = pkg_json.get("name")?.as_str()?;
    let description = pkg_json
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or(pkg_name);

    // Find the bin entry
    let bin = pkg_json.get("bin")?;
    let bin_path = match bin {
        serde_json::Value::String(s) => pkg_dir.join(s),
        serde_json::Value::Object(map) => {
            // Take the first bin entry
            let (_, path) = map.iter().next()?;
            pkg_dir.join(path.as_str()?)
        }
        _ => return None,
    };

    // Derive a clean ID from the package name
    let id = pkg_name
        .trim_start_matches("@modelcontextprotocol/")
        .trim_start_matches("server-")
        .trim_start_matches("mcp-server-")
        .trim_start_matches("mcp_server_")
        .to_string();

    Some(DiscoveredMcpServer {
        id,
        name: description.to_string(),
        executable: bin_path,
        args: Vec::new(),
        env: HashMap::new(),
        source: McpDiscoverySource::NpmGlobal {
            prefix_dir: prefix.clone(),
        },
    })
}

/// Common locations of MCP registry directories.
fn mcp_registry_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        // Claude Desktop MCP servers
        dirs.push(home.join(".claude").join("mcp-servers"));
        // Cline MCP servers
        dirs.push(home.join(".cline").join("mcp-servers"));
        // Generic MCP directory
        dirs.push(home.join(".mcp").join("servers"));
        // XDG config
        if let Some(config) = dirs::config_dir() {
            dirs.push(config.join("mcp").join("servers"));
            dirs.push(config.join("claude").join("mcp-servers"));
        }
    }
    dirs
}

/// Scan an MCP registry directory for server configurations.
async fn scan_mcp_registry(registry_dir: &std::path::Path) -> Vec<DiscoveredMcpServer> {
    let mut found = Vec::new();

    let mut entries = match tokio::fs::read_dir(registry_dir).await {
        Ok(e) => e,
        Err(_) => return found,
    };

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => break,
        };

        let path = entry.path();

        // Check for server.json config files
        if path.is_dir() {
            let server_json = path.join("server.json");
            if server_json.exists() {
                if let Some(server) =
                    try_mcp_registry_entry(&path, &server_json, registry_dir).await
                {
                    found.push(server);
                }
            }
        } else if path.extension().map(|e| e == "json").unwrap_or(false) {
            // Direct JSON config file
            if let Some(server) = try_mcp_json_config(&path, registry_dir).await {
                found.push(server);
            }
        }
    }

    found
}

/// Try to create a discovered server from an MCP registry entry directory.
async fn try_mcp_registry_entry(
    dir: &std::path::Path,
    config_path: &std::path::Path,
    registry_dir: &std::path::Path,
) -> Option<DiscoveredMcpServer> {
    let content = tokio::fs::read_to_string(config_path).await.ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;

    let command = config.get("command")?.as_str()?;
    let id = dir.file_name()?.to_string_lossy().to_string();
    let name = config
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(&id)
        .to_string();

    let args: Vec<String> = config
        .get("args")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let env: HashMap<String, String> = config
        .get("env")
        .and_then(|e| e.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    Some(DiscoveredMcpServer {
        id,
        name,
        executable: PathBuf::from(command),
        args,
        env,
        source: McpDiscoverySource::McpRegistry {
            registry_dir: registry_dir.to_path_buf(),
        },
    })
}

/// Try to create a discovered server from a standalone MCP JSON config.
async fn try_mcp_json_config(
    config_path: &std::path::Path,
    registry_dir: &std::path::Path,
) -> Option<DiscoveredMcpServer> {
    let content = tokio::fs::read_to_string(config_path).await.ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;

    let command = config.get("command")?.as_str()?;
    let id = config_path.file_stem()?.to_string_lossy().to_string();
    let name = config
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(&id)
        .to_string();

    let args: Vec<String> = config
        .get("args")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let env: HashMap<String, String> = config
        .get("env")
        .and_then(|e| e.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    Some(DiscoveredMcpServer {
        id,
        name,
        executable: PathBuf::from(command),
        args,
        env,
        source: McpDiscoverySource::McpRegistry {
            registry_dir: registry_dir.to_path_buf(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_server_to_config() {
        let server = DiscoveredMcpServer {
            id: "test".to_string(),
            name: "Test Server".to_string(),
            executable: PathBuf::from("/usr/bin/mcp-test"),
            args: vec!["--arg1".to_string()],
            env: HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
            source: McpDiscoverySource::SystemPath,
        };

        let config = server.to_config();
        assert_eq!(config.command, Some("/usr/bin/mcp-test".to_string()));
        assert_eq!(config.args, vec!["--arg1"]);
        assert!(config.disabled);
        assert_eq!(config.env.get("KEY"), Some(&"VALUE".to_string()));
    }

    #[test]
    fn test_which_sync_not_found() {
        // Test with a definitely non-existent command
        let result = which_sync("definitely_not_a_real_command_12345");
        assert!(result.is_none());
    }
}
