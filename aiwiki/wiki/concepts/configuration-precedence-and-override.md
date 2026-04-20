---
title: "Configuration Precedence and Override"
type: concept
generated: "2026-04-19T22:04:15.986288472+00:00"
---

# Configuration Precedence and Override

### From: mod

Configuration precedence and override is a fundamental concept in application configuration management that defines how settings from multiple sources are resolved when conflicts occur, establishing a hierarchy where certain sources take priority over others. This pattern is essential for creating flexible applications that can adapt to different deployment contexts—from developer workstations to production servers—without requiring code changes. The ragent GitLab module explicitly supports this concept through its documented support for credentials stored in encrypted SQLite, with the ability to override these via `ragent.json` configuration files or environment variables. This three-tier hierarchy typically follows the convention that environment variables (most volatile, deployment-specific) override file-based configuration (persistent user preferences), which in turn overrides database defaults (secure system-managed settings). This approach serves multiple use cases: developers can use persistent stored credentials for daily workflows, CI/CD pipelines can inject short-lived tokens through environment variables, and migration paths allow gradual adoption without disrupting existing setups. The explicit acknowledgment of legacy file migration in the API (`migrate_legacy_files`) further demonstrates awareness of configuration evolution, ensuring users can upgrade smoothly while maintaining access to historical settings. Proper implementation of configuration precedence requires careful documentation and predictable behavior, as users depend on understanding where their effective settings originate when debugging authentication or connectivity issues.

## Diagram

```mermaid
flowchart TB
    subgraph ConfigSources["Configuration Sources"]
        direction TB
        env["Environment Variables"]
        json["ragent.json File"]
        sqlite["Encrypted SQLite Database"]
    end
    
    subgraph Resolution["Precedence Resolution"]
        checkEnv{"Check Environment?"}
        checkJson{"Check JSON File?"}
        useSqlite["Use SQLite Database"]
    end
    
    subgraph Result["Effective Configuration"]
        finalConfig["Applied GitLabConfig"]
    end
    
    env --> checkEnv
    checkEnv -->|"set"| finalConfig
    checkEnv -->|"not set"| checkJson
    json --> checkJson
    checkJson -->|"exists"| finalConfig
    checkJson -->|"not exists"| useSqlite
    sqlite --> useSqlite --> finalConfig
    
    style env fill:#d4edda
    style json fill:#fff3cd
    style sqlite fill:#f8d7da
```

## External Resources

- [The Twelve-Factor App: Config principle](https://12factor.net/config) - The Twelve-Factor App: Config principle
- [Command Line Interface Guidelines - Configuration](https://clig.dev/#configuration) - Command Line Interface Guidelines - Configuration

## Related

- [Encrypted Credential Storage](encrypted-credential-storage.md)

## Sources

- [mod](../sources/mod.md)
