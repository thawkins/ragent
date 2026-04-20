---
title: "Heredoc Handling in Security Scanning"
type: concept
generated: "2026-04-19T17:12:23.341424260+00:00"
---

# Heredoc Handling in Security Scanning

### From: bash

Heredoc handling represents a sophisticated security engineering challenge addressed in BashTool to prevent false positives in command analysis while maintaining detection accuracy. Here documents (heredocs) are shell syntax for multi-line string literals, written as `<<DELIMITER` followed by content lines until a matching delimiter line. The security challenge arises because heredoc content is arbitrary data that should not be analyzed as shell commands, yet naive string searching of the full command text could match banned patterns inside heredoc bodies. For example, a heredoc containing Rust documentation with the string `\nc
` (representing a newline, letter c, newline) might trigger a false positive match for `nc`, the netcat banned command. BashTool implements specialized heredoc parsing to strip these bodies before security analysis, ensuring that literal content never interferes with command safety decisions.

The implementation handles multiple heredoc variants through the `extract_heredoc_delimiter` function, which recognizes `<<EOF`, `<< EOF` (with space), `<<'EOF'` (single-quoted, no variable expansion), `<<"EOF"` (double-quoted), and `<<-EOF` (stripping leading tabs). The parsing extracts the bare delimiter string, handling quote stripping appropriately so that `'EOF'`, `"EOF"`, and `EOF` are all recognized as the same closing marker. The `strip_heredoc_bodies` function then uses this delimiter extraction to process the command line-by-line, preserving the opening marker line and closing delimiter line (so structural analysis remains valid) while omitting the body content between them. This preserves command structure for legitimate analysis—like detecting if a heredoc is used to construct a script that will be executed—while removing data that could confuse pattern matching.

This heredoc handling exemplifies careful input validation that considers context and data type, going beyond simple blacklist approaches. The security analysis in `contains_banned_command` operates on the stripped command, using byte-level scanning with word boundary detection to identify banned tools only when they appear as actual command tokens, not within string literals or data payloads. The combination of heredoc stripping and word boundary detection provides defense against both false positives (blocking legitimate commands) and false negatives (missing actual threats). The implementation also demonstrates Rust's string handling capabilities, using `as_bytes()` for precise byte-level matching that handles Unicode correctly while avoiding locale-dependent character classification. This attention to parsing correctness reflects the broader security principle that input validation must be as precise as the syntax being validated, approximations lead to bypasses or denial of service.

## Diagram

```mermaid
flowchart TB
    subgraph HeredocVariants["Heredoc Syntax Variants"]
        H1["<<EOF<br/>Standard"]
        H2["<<'EOF'<br/>No expansion"]
        H3["<<\"EOF\"<br/>No expansion"]
        H4["<<-EOF<br/>Strip leading tabs"]
        H5["<< EOF<br/>Space allowed"]
    end
    
    subgraph Processing["strip_heredoc_bodies"]
        direction TB
        Split["Split by newlines"]
        Iterate["Iterate lines"]
        Detect["extract_heredoc_delimiter"]
        Skip["Skip body lines until delimiter"]
        Preserve["Preserve: marker line + delimiter line"]
        Output["Reconstructed command"]
    end
    
    subgraph Security["Security Analysis"]
        Lowercase["to_lowercase"]
        Bytes["as_bytes for byte scan"]
        Boundaries["Word boundary detection"]
        Match["Check against BANNED_COMMANDS"]
    end
    
    Input[Command with heredoc] --> Processing
    Processing --> Security
    Security --> Result{Threat detected?}
    
    Input --> Split
    Split --> Iterate
    Iterate --> Detect
    Detect -->|found| Skip
    Skip --> Preserve
    Preserve --> Iterate
    Detect -->|not found| Accumulate
    Accumulate --> Iterate
    Iterate -->|complete| Output
    
    Output --> Lowercase --> Bytes --> Boundaries --> Match --> Result
```

## External Resources

- [Bash manual on Here Documents and redirections](https://www.gnu.org/software/bash/manual/bash.html#Redirections) - Bash manual on Here Documents and redirections
- [Wikipedia article on Here documents](https://en.wikipedia.org/wiki/Here_document) - Wikipedia article on Here documents

## Sources

- [bash](../sources/bash.md)
