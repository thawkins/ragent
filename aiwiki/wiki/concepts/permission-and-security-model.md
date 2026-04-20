---
title: "Permission and Security Model"
type: concept
generated: "2026-04-19T20:07:04.612080228+00:00"
---

# Permission and Security Model

### From: mod

The permission and security model in ragent-core implements defense in depth for agent-executed operations, combining static capability declarations with dynamic runtime validation. Each tool declares a permission category (e.g., `"file:read"`, `"file:write"`, `"bash:execute"`, `"network:request"`) through the `permission_category` trait method, enabling coarse-grained access control policies. This categorical approach balances expressiveness with manageability compared to per-tool permissions or fully unrestricted execution.

Runtime security centers on path containment through `check_path_within_root`, which prevents directory traversal attacks by canonicalizing paths and verifying they remain within a designated root. This function handles complex edge cases including non-existent paths (for file creation), symbolic links, and relative path components. The implementation demonstrates careful attention to filesystem semantics: for paths where intermediate directories don't exist, it walks up the tree to find an existing ancestor for canonicalization, then reconstructs the target path.

The security model assumes a trusted computing base including the Rust standard library's canonicalize, the root directory configuration, and the permission manager that interprets categories. Threats addressed include: path traversal via `../` sequences, symlink-based escape attacks, and unauthorized access to system files. The model does not address: confused deputy problems where a legitimate tool is tricked into malicious actions, denial of service through resource exhaustion, or information leakage through side channels. The `EventBus` integration suggests audit logging and permission request flows, though the exact user consent mechanisms are not visible in this module.

## Diagram

```mermaid
flowchart TB
    subgraph Static["Static Declarations"]
        TOOL[Tool Implementation] -->|declares| CAT[permission_category\n"file:read", "bash:execute", etc.]
    end
    
    subgraph Runtime["Runtime Validation"]
        CALL[Tool Call] -->|checks| POLICY[Permission Policy]
        POLICY -->|granted?| EXEC[Execute Tool]
        POLICY -->|denied| REJECT[Reject / Request Consent]
    end
    
    subgraph PathSecurity["Path Security"]
        PATH["File Path"] --> CANON[check_path_within_root]
        CANON -->|canonicalize| CHECK{"starts_with root?"}
        CHECK -->|pass| SAFE[Proceed]
        CHECK -->|fail| BLOCK[Path Escape Error]
    end
    
    EXEC -->|file operations| PATH
    CAT -->|informs| POLICY
    
    style BLOCK fill:#ffcccc
    style SAFE fill:#ccffcc
    style REJECT fill:#ffeecc
```

## External Resources

- [OWASP Input Validation Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html) - OWASP Input Validation Cheat Sheet
- [Google security practices for agent systems (industry context)](https://security.googleblog.com/2023/09/google-acquired-mandiant-now-what.html) - Google security practices for agent systems (industry context)

## Sources

- [mod](../sources/mod.md)
