---
title: "Pattern Matching Heuristics"
type: concept
generated: "2026-04-19T21:52:48.056352674+00:00"
---

# Pattern Matching Heuristics

### From: knowledge_graph

The pattern matching subsystem implements lightweight linguistic analysis for extracting compound concept expressions through proximity-based keyword detection. The extract_pattern_entities function processes tokenized content to identify sequences where specific pattern-indicating keywords follow meaningful content words, capturing expressions like "TDD pattern", "microservices architecture", or "agile methodology". This approach leverages regularity in how developers describe conventions—typically as "[Adjective/Noun] [Keyword]" constructions where the keyword disambiguates the preceding term's semantic category.

The implementation applies careful preprocessing to handle punctuation and special characters that would otherwise fragment tokenization, filtering each token to retain only alphanumeric characters before analysis. This normalization ensures robust matching against varied text formatting while potentially over-normalizing in cases where special characters carry semantic significance. The keyword list encompasses five pattern-indicating terms—pattern, convention, approach, methodology, paradigm—providing broad coverage of convention-description vocabulary with manageable complexity.

The positional requirement that keywords must follow a preceding token of at least three characters eliminates false positives from sentence-initial keywords and single-letter artifacts, though this heuristic may exclude valid short-form patterns like "Go convention" or "C pattern". The extracted patterns preserve original casing and spacing from source text, maintaining natural readability while associating the compound expression with EntityType::Pattern for knowledge graph integration. This extraction mechanism complements dictionary-based approaches by capturing domain-specific and emergent conventions without requiring exhaustive enumeration.

## Diagram

```mermaid
sequenceDiagram
    participant Input as Memory Content
    participant Tokenize as Tokenization
    participant Filter as Alphanumeric Filter
    participant Match as Pattern Matching
    participant Output as ExtractedEntity
    
    Input->>Tokenize: "We use TDD pattern & CI/CD"
    Tokenize->>Filter: ["We", "use", "TDD", "pattern", "&", "CI/CD"]
    
    loop Each Token
        Filter->>Filter: Filter to alphanumeric
        Note right of Filter: "TDD"→"TDD", "pattern"→"pattern",<br/>"&"→"", "CI/CD"→"CICD"
    end
    
    Filter->>Match: ["We", "use", "TDD", "pattern", "", "CICD"]
    
    Note over Match: Check if token matches keywords<br/>and has valid predecessor
    
    Match->>Match: i=3: "pattern" in keywords, i>0, len("TDD")=3 ✓
    Match->>Output: ExtractedEntity{name: "TDD pattern", type: Pattern}
    
    Match->>Match: i=5: "CICD" not in keywords
    Match->>Match: No match
```

## External Resources

- [Heuristic methods in computer science](https://en.wikipedia.org/wiki/Heuristic) - Heuristic methods in computer science
- [Tokenization in lexical analysis](https://en.wikipedia.org/wiki/Tokenization_(lexical_analysis)) - Tokenization in lexical analysis

## Related

- [Entity Extraction](entity-extraction.md)

## Sources

- [knowledge_graph](../sources/knowledge-graph.md)
