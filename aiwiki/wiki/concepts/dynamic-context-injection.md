---
title: "Dynamic Context Injection"
type: concept
generated: "2026-04-19T20:22:02.785193291+00:00"
---

# Dynamic Context Injection

### From: invoke

Dynamic context injection represents a powerful but security-sensitive capability allowing skill bodies to execute shell commands and incorporate their output into processed content. The mechanism recognizes the `` !`command` `` pattern within skill bodies, executes the command in the specified working directory, and substitutes the command's stdout output in place of the pattern. This enables skills to incorporate runtime environment state—current git branch, file system contents, process status, or custom tooling output—without requiring the invoking agent to manually gather and format this information.

The security architecture surrounding dynamic context injection implements defense-in-depth through explicit opt-in semantics. The SkillInfo.allow_dynamic_context boolean field defaults to false, meaning skills must explicitly declare their intent to execute commands. This design prevents accidental execution of untrusted skill definitions and creates clear audit boundaries for security review. The implementation in invoke_skill checks this flag before calling inject_dynamic_context, and the test suite validates that disabled skills preserve command patterns as literal text rather than executing them. This pattern reflects lessons from template injection vulnerabilities in web frameworks where automatic evaluation of template expressions creates exploitation vectors.

Practical applications of dynamic context injection span development workflow automation. Skills might use ``!`git branch --show-current` `` to incorporate branch context into deployment instructions, ``!`find . -name '*.rs' -mtime -1` `` to identify recently modified files for review, or ``!`cargo test --no-run 2>&1 | head -20` `` to include compilation status in bug reports. The test cases demonstrate date command substitution for temporal context and echo commands for deterministic testing. The working_dir parameter ensures commands execute in appropriate filesystem contexts, supporting both project-scoped skills (executing in repository roots) and global skills (with configurable execution directories).

## External Resources

- [OWASP Command Injection vulnerability guidance](https://owasp.org/www-community/vulnerabilities/Command_Injection) - OWASP Command Injection vulnerability guidance
- [Shell escaping patterns for command safety](https://docs.rs/shell-escape/latest/shell_escape/) - Shell escaping patterns for command safety

## Related

- [Argument Substitution](argument-substitution.md)

## Sources

- [invoke](../sources/invoke.md)
