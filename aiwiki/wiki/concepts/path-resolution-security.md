---
title: "Path Resolution Security"
type: concept
generated: "2026-04-19T16:42:06.845978291+00:00"
---

# Path Resolution Security

### From: create

Path Resolution Security encompasses techniques for safely handling filesystem paths in multi-tenant or sandboxed environments, preventing directory traversal attacks where malicious input attempts to access files outside intended boundaries. The CreateTool implementation demonstrates a defense-in-depth approach through its `resolve_path` helper function combined with the `check_path_within_root` validation. This two-layer security ensures that even if path manipulation succeeds at the string level, the final resolved path is verified to remain within the agent's designated working directory.

The attack vector being mitigated is path traversal (also known as directory climbing), where inputs like `"../../../etc/passwd"` attempt to escape a chroot-like restriction. The `resolve_path` function handles the first layer by joining relative paths to a working directory, but critically preserves absolute paths as-is—which is why the second layer is essential. The `check_path_within_root` call (defined in the parent module) performs canonicalization and prefix checking to ensure the resolved path is truly contained within the root. This pattern recognizes that path security is subtle: Rust's `Path::join` behavior, symlink following, and platform-specific path formats all create potential escape vectors.

Beyond direct security, this approach enables practical system designs where agents can be given intuitive path interfaces (relative paths from their working directory) while maintaining hard boundaries. The working directory acts as a capability—agents without access to paths outside their root cannot exfiltrate data or corrupt system files even if compromised. This pattern appears in container runtimes, web server configurations, and build systems, all of which need to process user-provided paths without trusting the user. The async nature of the actual file operations (via Tokio) adds complexity, as security checks must complete before any I/O begins to prevent race conditions between check and use.

## Diagram

```mermaid
sequenceDiagram
    participant Agent
    participant CreateTool
    participant resolve_path
    participant check_path_within_root
    participant Filesystem
    Agent->>CreateTool: execute({"path": "../../../etc/passwd", ...})
    CreateTool->>resolve_path: resolve_path(working_dir, "../../../etc/passwd")
    resolve_path->>CreateTool: PathBuf("/project/../../../etc/passwd")
    CreateTool->>check_path_within_root: check_path_within_root(path, working_dir)
    check_path_within_root->>check_path_within_root: canonicalize(path)
    check_path_within_root->>CreateTool: Err(PathOutsideRoot)
    CreateTool->>Agent: Error: path outside root directory
    note over Agent,Filesystem: Attack prevented before filesystem access
```

## External Resources

- [OWASP Path Traversal attack documentation](https://owasp.org/www-community/attacks/Path_Traversal) - OWASP Path Traversal attack documentation
- [Rust standard library Path documentation](https://doc.rust-lang.org/std/path/struct.Path.html) - Rust standard library Path documentation

## Sources

- [create](../sources/create.md)
