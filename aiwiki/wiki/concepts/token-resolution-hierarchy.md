---
title: "Token Resolution Hierarchy"
type: concept
generated: "2026-04-19T21:22:28.591072780+00:00"
---

# Token Resolution Hierarchy

### From: mod

Token resolution hierarchy is a design pattern for credential management that establishes a prioritized search order across multiple storage locations. In the ragent GitHub module, this hierarchy follows: environment variables first, then persistent file storage, with device flow authentication as the ultimate fallback. This ordering reflects practical deployment considerations: environment variables are preferred for CI/CD pipelines and containerized environments where files may be ephemeral or shared; file storage supports developer workstations with persistent authentication; and device flow handles initial setup or credential expiration scenarios.

The implementation of token resolution requires careful consideration of security trade-offs. Environment variables are transient and typically excluded from process dumps and shell history (when properly set), but may be visible to other processes on the same system. File-based storage enables credential persistence across sessions but requires appropriate filesystem permissions—hence the dedicated `~/.ragent/` directory which can be created with restricted access. The resolution logic must also handle partial failures gracefully, such as malformed tokens or permission-denied errors on the token file, potentially with informative error messages guiding users toward resolution.

This pattern extends beyond GitHub authentication to represent a general approach to configuration and secret management in modern applications. Similar hierarchies appear in cloud SDKs (AWS, GCP, Azure), container orchestration tools (Kubernetes), and development frameworks (Twelve-Factor App methodology). The specific paths and variable names become part of the application's contract with its users, requiring documentation and stability commitments. Advanced implementations may add additional resolution sources like credential helpers, system keychains (macOS Keychain, Windows Credential Manager, Linux Secret Service), or cloud secret managers.

## External Resources

- [Twelve-Factor App configuration methodology](https://12factor.net/config) - Twelve-Factor App configuration methodology
- [GitHub personal access token management](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens) - GitHub personal access token management

## Sources

- [mod](../sources/mod.md)
