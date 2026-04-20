---
title: "JSON Frontmatter Pattern"
type: concept
generated: "2026-04-19T15:00:25.377628106+00:00"
---

# JSON Frontmatter Pattern

### From: custom

JSON frontmatter is a document structure pattern that embeds structured metadata within human-readable prose documents, popularized by static site generators like Jekyll and Hugo. Unlike YAML frontmatter which uses `---` delimiters with YAML syntax, this implementation uses JSON within the same delimiter structure, trading YAML's terseness for JSON's universal parser availability in Rust ecosystems. The format requires opening and closing `---` lines with valid JSON between them, followed by free-form markdown content that becomes the system prompt. This design enables single-file agent definitions where behavioral instructions (the prose) coexist with operational parameters (the JSON). The parsing implementation handles edge cases including Windows-style line endings (`
`), optional trailing dashes on closing delimiters, and content before the opening delimiter. The pattern supports literate programming approaches—agents can be documented, versioned, and shared as readable markdown while remaining machine-parseable. This bridges the gap between technical users who prefer editing JSON directly and less technical users who benefit from prose explanations and examples embedded in the configuration.

## External Resources

- [Jekyll front matter documentation, popularizer of the pattern](https://jekyllrb.com/docs/front-matter/) - Jekyll front matter documentation, popularizer of the pattern
- [Pandoc document converter with extensive markdown flavor support](https://pandoc.org/) - Pandoc document converter with extensive markdown flavor support

## Sources

- [custom](../sources/custom.md)
